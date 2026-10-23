//! Serveur MCP HTTP local (streamable HTTP, `http://127.0.0.1:<port>/mcp`) que les CLI d'IA
//! appellent avec leur jeton de session.
//!
//! - jeton porteur absent ou inconnu → 401 ;
//! - `Host` non local refusé (protection contre le DNS rebinding, faite par rmcp) ; toute
//!   requête portant un `Origin` refusée (un navigateur n'a rien à faire ici) ;
//! - `tools/list` : outils `all` des plugins activés qui ont un serveur (démarré au besoin),
//!   préfixés, puis outils du cœur ;
//! - `tools/call` : route vers le serveur du plugin (600 s), ou vers les outils du cœur. Un
//!   plugin indisponible répond par un résultat d'erreur lisible par l'IA, pas par une panne ;
//! - `notifications/tools/list_changed` aux sessions connectées quand les outils d'un plugin
//!   changent, ou qu'un plugin est activé, désactivé, installé ou retiré.

use std::{
    collections::{BTreeSet, HashMap},
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::{
    ErrorData, Peer, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation, ListToolsResult,
        PaginatedRequestParams, ResultType, ServerCapabilities, ServerConfig,
    },
    service::{NotificationContext, RequestContext},
    transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager},
};
use tokio::sync::oneshot;

use super::{
    PluginSummary,
    core_tools::{self, GatewayEvent, RunReport},
    scope::{self, CORE, SessionScope, Tokens},
};
use crate::{
    error::{AppError, AppResult},
    plugins::host::PluginHost,
    repos::RepoService,
};

/// Plugins activés, lus à chaque requête (activation et désinstallation prises en compte).
pub type PluginLister = Arc<dyn Fn() -> Vec<PluginSummary> + Send + Sync>;
/// Reçoit les demandes des outils du cœur (ouvrir une vue, rapport d'exécution).
pub type GatewayEventSink = Arc<dyn Fn(GatewayEvent) + Send + Sync>;

/// Délai d'un appel d'outil venant d'une IA (plan § 3.2).
pub const AI_CALL_TIMEOUT: Duration = Duration::from_secs(600);

pub struct GatewayContext {
    pub app_version: String,
    pub host: Arc<PluginHost>,
    pub plugins: PluginLister,
    pub events: GatewayEventSink,
    pub tokens: Tokens,
    pub call_timeout: Duration,
    /// Dernier rapport de chaque exécution de fond.
    reports: Mutex<HashMap<String, RunReport>>,
    /// Service Dépôts (`cadence__repo_checkout`), branché au démarrage de l'app.
    repos: OnceLock<Arc<RepoService>>,
    /// Dossiers ouverts à chaque session par `cadence__repo_checkout`, en mémoire seulement :
    /// le chat les passe à la CLI au prochain lancement de la session.
    grants: Mutex<HashMap<String, BTreeSet<PathBuf>>>,
    /// Sessions MCP connectées, prévenues quand la liste des outils change.
    peers: Mutex<Vec<Peer<RoleServer>>>,
}

impl GatewayContext {
    pub fn new(app_version: &str, host: Arc<PluginHost>, plugins: PluginLister, events: GatewayEventSink) -> Arc<Self> {
        Arc::new(Self {
            app_version: app_version.to_string(),
            host,
            plugins,
            events,
            tokens: Tokens::default(),
            call_timeout: AI_CALL_TIMEOUT,
            reports: Mutex::default(),
            repos: OnceLock::new(),
            grants: Mutex::default(),
            peers: Mutex::default(),
        })
    }

    /// Prévient les sessions connectées que la liste des outils a changé ; oublie celles qui
    /// sont fermées.
    pub async fn notify_tools_changed(&self) {
        self.peers.lock().unwrap().retain(|p| !p.is_transport_closed());
        let peers: Vec<Peer<RoleServer>> = self.peers.lock().unwrap().clone();
        for peer in peers {
            if let Err(err) = peer.notify_tool_list_changed().await {
                tracing::debug!(%err, "session MCP non prévenue du changement d'outils");
            }
        }
    }

    /// Branche le service Dépôts (une seule fois).
    pub fn set_repos(&self, repos: Arc<RepoService>) {
        let _ = self.repos.set(repos);
    }

    pub fn repos(&self) -> Option<&Arc<RepoService>> {
        self.repos.get()
    }

    /// Ouvre `dir` à la session (worktree rendu par `cadence__repo_checkout`).
    pub fn grant(&self, session: &str, dir: PathBuf) {
        self.grants.lock().unwrap().entry(session.to_string()).or_default().insert(dir);
    }

    /// Dossiers ouverts à la session, triés.
    pub fn granted(&self, session: &str) -> Vec<PathBuf> {
        self.grants.lock().unwrap().get(session).map(|dirs| dirs.iter().cloned().collect()).unwrap_or_default()
    }

    /// La conversation disparaît : ses accès aussi.
    pub fn forget_grants(&self, session: &str) {
        self.grants.lock().unwrap().remove(session);
    }

    pub(crate) fn record_report(&self, report: RunReport) {
        self.reports.lock().unwrap().insert(report.run.clone(), report);
    }

    /// Dernier rapport d'une exécution (le résultat si `final`).
    pub fn last_report(&self, run: &str) -> Option<RunReport> {
        self.reports.lock().unwrap().get(run).cloned()
    }
}

/// Ce qu'un outil de la passerelle demande en mode « Demander » (recette du 21/10, N12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolAccess {
    /// Outil du cœur (`cadence__*`) : jamais demandé.
    Core,
    /// Outil de plugin en lecture seule (`readOnlyHint`, sans `destructiveHint`).
    ReadOnly,
    /// Outil de plugin qui écrit : l'utilisateur donne son accord. `plugin` est son nom affiché.
    Writes { plugin: String, tool: String },
    /// Plugin ou outil introuvable (serveur arrêté…) : demandé, par prudence.
    Unknown,
}

impl GatewayContext {
    /// Accès d'un outil exposé (`<plugin>__<outil>`), d'après les annotations de son serveur.
    pub async fn tool_access(&self, exposed: &str) -> ToolAccess {
        let Some((plugin, tool)) = scope::split(exposed) else { return ToolAccess::Unknown };
        if plugin == CORE {
            return ToolAccess::Core;
        }
        let Some(summary) = (self.plugins)().into_iter().find(|p| p.id == plugin && p.has_server) else {
            return ToolAccess::Unknown;
        };
        match self.host.tools(plugin).await {
            Ok(tools) => match tools.iter().find(|t| t.name == tool && !scope::is_ui_only(t)) {
                Some(found) if !scope::writes(found) => ToolAccess::ReadOnly,
                Some(_) => ToolAccess::Writes { plugin: summary.name, tool: tool.to_string() },
                None => ToolAccess::Unknown,
            },
            Err(_) => ToolAccess::Unknown,
        }
    }
}

/// Passerelle démarrée.
pub struct Gateway {
    addr: SocketAddr,
    context: Arc<GatewayContext>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
}

impl Gateway {
    /// Écoute sur `127.0.0.1:<port>` (`0` = port libre choisi par le système).
    pub async fn start(context: Arc<GatewayContext>, port: u16) -> AppResult<Self> {
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
        let addr = listener.local_addr()?;

        let config = StreamableHttpServerConfig::default().enforce_origin_validation();
        let cancel = config.cancellation_token.clone();
        let factory = {
            let context = context.clone();
            move || Ok(GatewayHandler { context: context.clone() })
        };
        let service = StreamableHttpService::new(factory, Arc::new(LocalSessionManager::default()), config);
        let router = Router::new()
            .route_service("/mcp", service)
            .layer(middleware::from_fn_with_state(context.clone(), authorize));

        // Outils d'un plugin modifiés : les sessions connectées relisent `tools/list`.
        {
            let context = context.clone();
            let mut changes = context.host.subscribe_tools_changed();
            tokio::spawn(async move {
                // Fermé : l'hôte a disparu ; en retard : une seule notification suffit.
                while let Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) = changes.recv().await {
                    context.notify_tools_changed().await;
                }
            });
        }

        let (tx, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let stop = async move {
                let _ = rx.await;
                cancel.cancel();
            };
            if let Err(err) = axum::serve(listener, router).with_graceful_shutdown(stop).await {
                tracing::error!(%err, "passerelle MCP arrêtée");
            }
        });
        tracing::info!(%addr, "passerelle MCP prête");
        Ok(Self { addr, context, shutdown: Mutex::new(Some(tx)) })
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// URL à donner aux CLI (`mcp.json`, `-c mcp_servers.cadence.url=…`).
    pub fn url(&self) -> String {
        format!("http://{}/mcp", self.addr)
    }

    pub fn context(&self) -> &Arc<GatewayContext> {
        &self.context
    }

    /// Jeton d'une session d'IA, à révoquer à la fin de la session (`revoke`).
    pub fn issue_token(&self, scope: SessionScope) -> secrecy::SecretString {
        self.context.tokens.issue(scope)
    }

    pub fn revoke(&self, session: &str) {
        self.context.tokens.revoke(session);
    }

    pub fn shutdown(&self) {
        if let Some(tx) = self.shutdown.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for Gateway {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Jeton porteur obligatoire ; la session trouvée suit la requête jusqu'au handler MCP.
async fn authorize(State(context): State<Arc<GatewayContext>>, mut request: Request, next: Next) -> Response {
    let scope = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| context.tokens.resolve(value));
    match scope {
        Some(scope) => {
            request.extensions_mut().insert(scope);
            next.run(request).await
        }
        None => (StatusCode::UNAUTHORIZED, [(header::WWW_AUTHENTICATE, "Bearer")], "Jeton de passerelle absent ou invalide.")
            .into_response(),
    }
}

#[derive(Clone)]
struct GatewayHandler {
    context: Arc<GatewayContext>,
}

/// Session de la requête en cours (déposée par `authorize`).
fn scope_of(context: &RequestContext<RoleServer>) -> Result<SessionScope, ErrorData> {
    context
        .extensions
        .get::<http::request::Parts>()
        .and_then(|parts| parts.extensions.get::<SessionScope>())
        .cloned()
        .ok_or_else(|| ErrorData::invalid_request("Session de passerelle inconnue.", None))
}

fn tool_error(err: &AppError) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(err.to_string())])
}

impl ServerHandler for GatewayHandler {
    fn get_info(&self) -> ServerConfig {
        let mut config = ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_tool_list_changed().build());
        config.server_info = Implementation::new("cadence", &self.context.app_version);
        config.instructions = Some(
            "Outils des plugins installés dans Cadence, nommés `<plugin>__<outil>`, et outils du cœur `cadence__*`."
                .into(),
        );
        config
    }

    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        self.context.peers.lock().unwrap().push(context.peer);
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        scope_of(&context)?;
        let mut pending = tokio::task::JoinSet::new();
        for plugin in (self.context.plugins)().into_iter().filter(|p| p.has_server) {
            let host = self.context.host.clone();
            pending.spawn(async move {
                let tools = host.tools(&plugin.id).await;
                (plugin.id, tools)
            });
        }
        let mut by_plugin = Vec::new();
        while let Some(joined) = pending.join_next().await {
            match joined {
                Ok((plugin, Ok(tools))) => by_plugin.push((plugin, tools)),
                Ok((plugin, Err(err))) => tracing::warn!(plugin, %err, "outils du plugin indisponibles"),
                Err(err) => tracing::warn!(%err, "lecture des outils interrompue"),
            }
        }
        // Ordre stable d'une requête à l'autre : par plugin.
        by_plugin.sort_by(|a, b| a.0.cmp(&b.0));
        let mut tools: Vec<_> = by_plugin.into_iter().flat_map(|(plugin, tools)| scope::expose_plugin_tools(&plugin, tools)).collect();
        tools.extend(core_tools::tools());
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let session = scope_of(&context)?;
        let unknown = || ErrorData::invalid_params(format!("Outil inconnu : {}.", request.name), None);
        let (plugin, tool) = scope::split(&request.name).ok_or_else(unknown)?;
        let args = request.arguments.clone().unwrap_or_default();

        if plugin == CORE && tool == core_tools::REPO_CHECKOUT {
            return Ok(core_tools::repo_checkout(&self.context, &session, &args).await.into());
        }
        if plugin == CORE {
            return core_tools::call(&self.context, &session, tool, &args).map(Into::into).ok_or_else(unknown);
        }
        if !(self.context.plugins)().iter().any(|p| p.id == plugin && p.has_server) {
            return Err(unknown());
        }
        // Un outil `ui` n'est pas appelable par l'IA, même si elle en devine le nom.
        let tools = match self.context.host.tools(plugin).await {
            Ok(tools) => tools,
            Err(err) => return Ok(tool_error(&err).into()),
        };
        if !tools.iter().any(|t| t.name == tool && !scope::is_ui_only(t)) {
            return Err(unknown());
        }
        let result = self.context.host.call_with_timeout(plugin, tool, Some(args), self.context.call_timeout).await;
        let mut result = result.unwrap_or_else(|err| tool_error(&err));
        // Le serveur du plugin a pu négocier une révision plus ancienne (sans `resultType`) ;
        // la révision 2026-07-28 l'exige, et rmcp le retire pour les clients plus anciens.
        result.result_type.get_or_insert(ResultType::COMPLETE);
        Ok(result.into())
    }
}
