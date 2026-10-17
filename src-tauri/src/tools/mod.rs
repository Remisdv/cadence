//! Registre d'outils (plan, révision 1.2) : Cadence détecte, installe et connecte les programmes
//! dont les IA, les services et les plugins ont besoin (Git, GitHub CLI, Claude Code, Codex,
//! OpenCode, Node.js), toujours sur un clic de l'utilisateur et en montrant la commande exacte.
//! Cadence ne voit ni ne garde les identifiants : chaque outil a son propre stockage.

pub mod detect;
pub mod registry;
pub mod run;

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tokio::sync::{Notify, RwLock};
use ts_rs::TS;

pub use registry::ToolId;
use registry::{Manager, Os};

use crate::{
    app_settings::AppSettings,
    domain::auth::CliTest,
    error::{AppError, AppResult},
    plugins::manifest::{Manifest, PluginService},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ToolStatus {
    /// Installé, à jour et connecté (ou rien à connecter).
    Ready,
    Missing,
    /// Version inférieure au minimum.
    Outdated,
    /// Installé, mais sans compte connecté.
    NotConnected,
    /// Binaire trouvé qui ne répond pas comme prévu.
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ToolRequesterKind {
    /// Service de l'app (`repos`).
    Service,
    /// Étape « IA de code ».
    Ai,
    Plugin,
    /// Prérequis d'un autre outil (`npm` ⇒ Node.js).
    Tool,
}

/// Qui demande l'outil, et à quel titre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolRequester {
    pub kind: ToolRequesterKind,
    pub id: String,
    pub name: String,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolRecipe {
    /// `winget`, `brew`, `npm`, `script`…
    pub id: String,
    /// « recette winget », « installeur officiel »…
    pub label: String,
    /// Commande exacte, telle que Cadence la lancera.
    pub command: String,
    /// Gestionnaire présent sur la machine.
    pub available: bool,
    pub needs: Option<ToolId>,
    /// Le système demandera une élévation (UAC, mot de passe).
    pub elevated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolInfo {
    pub id: ToolId,
    pub name: String,
    pub description: String,
    pub status: ToolStatus,
    pub path: Option<String>,
    pub version: Option<String>,
    pub min_version: String,
    /// Compte connecté, s'il est connu (« @rlhuillier », « Abonnement Max · … »).
    pub account: Option<String>,
    /// Dernière session de test (IA seulement).
    pub test: Option<CliTest>,
    /// Commande de connexion, `None` s'il n'y a rien à connecter.
    pub login: Option<String>,
    /// La connexion s'ouvre dans un terminal du système (choix au clavier).
    pub login_interactive: bool,
    pub homepage: String,
    /// Recettes pour ce système, dans l'ordre de préférence.
    pub recipes: Vec<ToolRecipe>,
    pub requested_by: Vec<ToolRequester>,
    /// Au moins un demandeur en a besoin : « Continuer » attend qu'il soit prêt.
    pub required: bool,
    /// Écarté par l'utilisateur (« Ignorer », IA désactivée).
    pub ignored: bool,
    /// Erreur de détection.
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolsReport {
    /// « Windows 11 », « macOS 15.1 »…
    pub platform: String,
    /// Gestionnaires présents (« winget 1.29.380 », « npm 11.6.2 »).
    pub managers: Vec<String>,
    pub tools: Vec<ToolInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ToolRunKind {
    Install,
    Login,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolRunResult {
    pub ok: bool,
    pub exit_code: Option<i32>,
    pub message: Option<String>,
    pub duration_ms: u32,
}

/// Événement `tool://install` : une ligne du journal, ou la fin de la commande.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolRunEvent {
    pub tool: ToolId,
    pub kind: ToolRunKind,
    pub line: Option<String>,
    /// La ligne remplace la précédente (barre de progression).
    pub replace: bool,
    pub done: Option<ToolRunResult>,
}

pub type RunSink = Arc<dyn Fn(ToolRunEvent) + Send + Sync>;

/// Résultat brut de la détection d'un outil.
#[derive(Debug, Clone, Default)]
struct Probe {
    path: Option<PathBuf>,
    version: Option<String>,
    login: Option<detect::Login>,
    error: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct Detected {
    platform: String,
    managers: Vec<(Manager, String)>,
    probes: BTreeMap<ToolId, Probe>,
}

/// Chemin choisi à la main dans les réglages (« Localiser le binaire… »).
pub fn configured_path(settings: &AppSettings, tool: ToolId) -> Option<String> {
    match tool.ai() {
        Some(ai) => settings.ai.get(ai).path.clone(),
        None => settings.tool_paths.get(&tool).cloned(),
    }
}

fn is_ignored(settings: &AppSettings, tool: ToolId) -> bool {
    match tool.ai() {
        Some(ai) => !settings.ai.get(ai).enabled,
        None => settings.ignored_tools.contains(&tool),
    }
}

async fn probe(tool: ToolId, configured: Option<&str>) -> Probe {
    let Some(path) = detect::locate(tool, configured) else {
        return Probe::default();
    };
    match detect::version(&path).await {
        Err(error) => Probe {
            path: Some(path),
            error: Some(error),
            ..Probe::default()
        },
        Ok(version) => {
            let login = match tool.login() {
                Some(_) => Some(detect::login_state(tool, &path).await),
                None => None,
            };
            Probe {
                path: Some(path),
                version: Some(version),
                login,
                error: None,
            }
        }
    }
}

fn status_of(tool: ToolId, probe: &Probe) -> ToolStatus {
    match (&probe.path, &probe.version) {
        (None, _) => ToolStatus::Missing,
        (Some(_), None) => ToolStatus::Error,
        (Some(_), Some(v)) if !detect::meets(v, tool.min_version()) => ToolStatus::Outdated,
        _ if probe.login == Some(detect::Login::NotConnected) => ToolStatus::NotConnected,
        _ => ToolStatus::Ready,
    }
}

/// `CADENCE_TOOLS` : chemin de chaque outil trouvé, chemins personnalisés compris (recherche
/// synchrone dans le `PATH` et les emplacements usuels, sans lancer les binaires).
pub fn located_paths(settings: &AppSettings) -> BTreeMap<String, String> {
    ToolId::ALL
        .into_iter()
        .filter_map(|tool| {
            let path = detect::locate(tool, configured_path(settings, tool).as_deref())?;
            Some((tool.as_str().to_string(), path.display().to_string()))
        })
        .collect()
}

/// Demandeurs de chaque outil : service `repos`, plugins activés, IA choisies, prérequis.
fn requesters(
    settings: &AppSettings,
    plugins: &[Manifest],
    probes: &BTreeMap<ToolId, Probe>,
    managers: &[(Manager, String)],
    os: Os,
) -> BTreeMap<ToolId, Vec<ToolRequester>> {
    fn add(map: &mut BTreeMap<ToolId, Vec<ToolRequester>>, tool: ToolId, r: ToolRequester) {
        let list = map.entry(tool).or_default();
        if !list.iter().any(|x| x.kind == r.kind && x.id == r.id) {
            list.push(r);
        }
    }
    let mut map: BTreeMap<ToolId, Vec<ToolRequester>> = BTreeMap::new();

    if plugins
        .iter()
        .any(|p| p.services.contains(&PluginService::Repos))
    {
        let repos = |optional| ToolRequester {
            kind: ToolRequesterKind::Service,
            id: "repos".into(),
            name: "Dépôts GitHub".into(),
            optional,
        };
        add(&mut map, ToolId::Git, repos(false));
        add(&mut map, ToolId::Gh, repos(true));
    }
    for plugin in plugins {
        for req in &plugin.tools {
            add(
                &mut map,
                req.id,
                ToolRequester {
                    kind: ToolRequesterKind::Plugin,
                    id: plugin.id.clone(),
                    name: plugin.name.clone(),
                    optional: req.optional,
                },
            );
        }
    }
    for tool in ToolId::ALL {
        let Some(ai) = tool.ai() else { continue };
        let is_default = settings.default_ai == Some(ai);
        let found = probes.get(&tool).is_some_and(|p| p.path.is_some());
        if is_default || (found && settings.ai.get(ai).enabled) {
            add(
                &mut map,
                tool,
                ToolRequester {
                    kind: ToolRequesterKind::Ai,
                    id: "ai".into(),
                    name: "IA de code".into(),
                    optional: !is_default,
                },
            );
        }
    }

    // Node.js : prérequis d'un outil demandé, manquant, qui ne s'installe que par npm ici.
    let has = |m: Manager| {
        managers.iter().any(|(x, _)| *x == m) || (m != Manager::Npm && detect::manager_available(m))
    };
    let node_for: Vec<(ToolId, bool)> = map
        .iter()
        .filter(|(tool, _)| **tool != ToolId::Node)
        .filter(|(tool, _)| probes.get(tool).is_none_or(|p| p.path.is_none()))
        .filter(|(tool, _)| {
            let recipes = registry::recipes(**tool, os);
            recipes.iter().any(|r| r.needs == Some(ToolId::Node))
                && !recipes.iter().any(|r| r.needs.is_none() && has(r.manager))
        })
        .map(|(tool, reqs)| (*tool, reqs.iter().all(|r| r.optional)))
        .collect();
    for (tool, optional) in node_for {
        add(
            &mut map,
            ToolId::Node,
            ToolRequester {
                kind: ToolRequesterKind::Tool,
                id: tool.as_str().into(),
                name: tool.name().into(),
                optional,
            },
        );
    }
    map
}

pub struct ToolService {
    detected: RwLock<Option<Detected>>,
    tests: Mutex<HashMap<ToolId, CliTest>>,
    running: Mutex<HashMap<ToolId, Arc<Notify>>>,
}

impl Default for ToolService {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolService {
    pub fn new() -> Self {
        Self {
            detected: RwLock::new(None),
            tests: Mutex::new(HashMap::new()),
            running: Mutex::new(HashMap::new()),
        }
    }

    async fn detect_all(settings: &AppSettings) -> Detected {
        let os = Os::current();
        let probes = ToolId::ALL.map(|tool| {
            let configured = configured_path(settings, tool);
            async move { (tool, probe(tool, configured.as_deref()).await) }
        });
        let [a, b, c, d, e, f] = probes;
        let (platform, managers, a, b, c, d, e, f) = tokio::join!(
            detect::platform_label(),
            detect::managers(os),
            a,
            b,
            c,
            d,
            e,
            f
        );
        Detected {
            platform,
            managers,
            probes: [a, b, c, d, e, f].into_iter().collect(),
        }
    }

    /// Détection au premier appel, puis cache jusqu'à `refresh` (ou une installation).
    pub async fn report(
        &self,
        settings: &AppSettings,
        plugins: &[Manifest],
        refresh: bool,
    ) -> ToolsReport {
        if refresh || self.detected.read().await.is_none() {
            let detected = Self::detect_all(settings).await;
            *self.detected.write().await = Some(detected);
        }
        let detected = self.detected.read().await.clone().unwrap_or_default();
        self.build(settings, plugins, &detected)
    }

    fn build(
        &self,
        settings: &AppSettings,
        plugins: &[Manifest],
        detected: &Detected,
    ) -> ToolsReport {
        let os = Os::current();
        let requested = requesters(settings, plugins, &detected.probes, &detected.managers, os);
        let tests = self.tests.lock().unwrap().clone();
        let has_manager = |m: Manager| {
            detected.managers.iter().any(|(x, _)| *x == m)
                || (m != Manager::Npm && detect::manager_available(m))
        };
        let tools = ToolId::ALL
            .into_iter()
            .map(|tool| {
                let probe = detected.probes.get(&tool).cloned().unwrap_or_default();
                let requested_by = requested.get(&tool).cloned().unwrap_or_default();
                let login = tool.login();
                ToolInfo {
                    id: tool,
                    name: tool.name().into(),
                    description: tool.description().into(),
                    status: status_of(tool, &probe),
                    path: probe.path.as_ref().map(|p| p.display().to_string()),
                    version: probe.version.clone(),
                    min_version: tool.min_version().into(),
                    account: match &probe.login {
                        Some(detect::Login::Connected(account)) => account.clone(),
                        _ => None,
                    },
                    test: tests.get(&tool).cloned(),
                    login: login
                        .as_ref()
                        .map(|l| format!("{} {}", tool.binary(), l.args.join(" "))),
                    login_interactive: login.as_ref().is_some_and(|l| l.interactive),
                    homepage: tool.homepage().into(),
                    recipes: registry::recipes(tool, os)
                        .into_iter()
                        .map(|r| ToolRecipe {
                            id: r.manager.id().into(),
                            label: r.manager.label().into(),
                            command: r.display(),
                            available: has_manager(r.manager),
                            needs: r.needs,
                            elevated: r.elevated,
                        })
                        .collect(),
                    required: requested_by.iter().any(|r| !r.optional),
                    requested_by,
                    ignored: is_ignored(settings, tool),
                    message: probe.error.clone(),
                }
            })
            .collect();
        ToolsReport {
            platform: detected.platform.clone(),
            managers: detected
                .managers
                .iter()
                .map(|(_, label)| label.clone())
                .collect(),
            tools,
        }
    }

    /// Redétecte un seul outil (après une installation, une connexion, un chemin choisi).
    pub async fn refresh_one(&self, settings: &AppSettings, tool: ToolId) {
        let fresh = probe(tool, configured_path(settings, tool).as_deref()).await;
        let mut guard = self.detected.write().await;
        if let Some(detected) = guard.as_mut() {
            detected.probes.insert(tool, fresh);
            if tool == ToolId::Node {
                detected.managers = detect::managers(Os::current()).await;
            }
        }
    }

    /// Chemin de l'exécutable détecté (`CADENCE_TOOLS`, lancement des IA).
    pub async fn path(&self, tool: ToolId) -> Option<PathBuf> {
        self.detected
            .read()
            .await
            .as_ref()?
            .probes
            .get(&tool)?
            .path
            .clone()
    }

    fn start(&self, tool: ToolId) -> AppResult<Arc<Notify>> {
        let mut running = self.running.lock().unwrap();
        if running.contains_key(&tool) {
            return Err(AppError::Validation(format!(
                "{} : une commande est déjà en cours.",
                tool.name()
            )));
        }
        let cancel = Arc::new(Notify::new());
        running.insert(tool, cancel.clone());
        Ok(cancel)
    }

    fn finish(&self, tool: ToolId) {
        self.running.lock().unwrap().remove(&tool);
    }

    /// Arrête la commande en cours pour cet outil ; `false` s'il n'y en avait pas.
    pub fn cancel(&self, tool: ToolId) -> bool {
        match self.running.lock().unwrap().get(&tool) {
            Some(cancel) => {
                cancel.notify_one();
                true
            }
            None => false,
        }
    }

    async fn stream(
        &self,
        tool: ToolId,
        kind: ToolRunKind,
        program: &Path,
        args: Vec<String>,
        press_enter: bool,
        sink: RunSink,
    ) -> AppResult<ToolRunResult> {
        let cancel = self.start(tool)?;
        let line_sink = sink.clone();
        let on_line: Arc<dyn Fn(String, bool) + Send + Sync> = Arc::new(move |line, replace| {
            line_sink(ToolRunEvent {
                tool,
                kind,
                line: Some(line),
                replace,
                done: None,
            })
        });
        let result = run::run_streaming(program, &args, press_enter, cancel, on_line).await;
        self.finish(tool);
        sink(ToolRunEvent {
            tool,
            kind,
            line: None,
            replace: false,
            done: Some(result.clone()),
        });
        Ok(result)
    }

    /// Lance la recette choisie (sinon la première utilisable), puis redétecte l'outil.
    pub async fn install(
        &self,
        settings: &AppSettings,
        tool: ToolId,
        recipe: Option<&str>,
        sink: RunSink,
    ) -> AppResult<ToolRunResult> {
        let os = Os::current();
        let recipes = registry::recipes(tool, os);
        let spec = match recipe {
            Some(id) => recipes.iter().find(|r| r.manager.id() == id),
            None => recipes
                .iter()
                .find(|r| detect::manager_available(r.manager))
                .or(recipes.first()),
        }
        .ok_or_else(|| AppError::Validation(format!("Recette inconnue pour {}.", tool.name())))?
        .clone();

        let program = self.resolve_program(&spec).await?;
        let args = spec.args.iter().map(|a| a.to_string()).collect();
        let result = self
            .stream(tool, ToolRunKind::Install, &program, args, false, sink)
            .await?;
        self.refresh_one(settings, tool).await;
        Ok(result)
    }

    /// Programme d'une recette : `npm` juste installé n'est pas encore dans le `PATH`.
    async fn resolve_program(&self, spec: &registry::RecipeSpec) -> AppResult<PathBuf> {
        if let Ok(path) = which::which(spec.program) {
            return Ok(path);
        }
        if spec.needs == Some(ToolId::Node) {
            let node = self
                .path(ToolId::Node)
                .await
                .or_else(|| detect::locate(ToolId::Node, None));
            if let Some(dir) = node.as_deref().and_then(Path::parent) {
                let name = if cfg!(windows) { "npm.cmd" } else { "npm" };
                if dir.join(name).is_file() {
                    return Ok(dir.join(name));
                }
            }
            return Err(AppError::Validation(
                "Installez d'abord Node.js : cette recette passe par npm.".into(),
            ));
        }
        Err(AppError::Validation(format!(
            "{} est introuvable sur cette machine.",
            spec.program
        )))
    }

    /// Connexion guidée : la commande de l'outil, dans le journal ou un terminal du système.
    pub async fn login(
        &self,
        settings: &AppSettings,
        tool: ToolId,
        sink: RunSink,
    ) -> AppResult<ToolRunResult> {
        let spec = tool.login().ok_or_else(|| {
            AppError::Validation(format!("{} n'a pas de compte à connecter.", tool.name()))
        })?;
        let path = match self.path(tool).await {
            Some(path) => path,
            None => detect::locate(tool, configured_path(settings, tool).as_deref()).ok_or_else(
                || AppError::Validation(format!("{} n'est pas installé.", tool.name())),
            )?,
        };
        if spec.interactive {
            run::open_in_terminal(&path, spec.args)?;
            return Ok(ToolRunResult {
                ok: true,
                exit_code: None,
                message: Some("Terminal ouvert : terminez la connexion, puis revérifiez.".into()),
                duration_ms: 0,
            });
        }
        let args = spec.args.iter().map(|a| a.to_string()).collect();
        let result = self
            .stream(
                tool,
                ToolRunKind::Login,
                &path,
                args,
                tool == ToolId::Gh,
                sink,
            )
            .await?;
        self.refresh_one(settings, tool).await;
        Ok(result)
    }

    /// Session de test d'une IA ; le résultat est gardé pour `ToolInfo::test`.
    pub async fn test(&self, settings: &AppSettings, tool: ToolId) -> AppResult<CliTest> {
        if tool.ai().is_none() {
            return Err(AppError::Validation(format!(
                "{} n'est pas une IA de code.",
                tool.name()
            )));
        }
        self.refresh_one(settings, tool).await;
        let path = match self.path(tool).await {
            Some(path) => path,
            None => detect::locate(tool, configured_path(settings, tool).as_deref()).ok_or_else(
                || AppError::Validation("Binaire introuvable : impossible de le tester.".into()),
            )?,
        };
        let test = detect::ai_test(tool, &path).await;
        self.tests.lock().unwrap().insert(tool, test.clone());
        Ok(test)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_settings::AiId, plugins::manifest};

    fn plugin(text: &str) -> Manifest {
        manifest::parse(text, "0.3.0").unwrap()
    }

    const BASE: &str = "api = 1\nid = \"gp\"\nname = \"Gestion de projet\"\nversion = \"1.0.0\"\nauthors = [\"a\"]\nmin_cadence = \"0.3.0\"\n";

    fn found(tools: &[ToolId]) -> BTreeMap<ToolId, Probe> {
        tools
            .iter()
            .map(|t| {
                (
                    *t,
                    Probe {
                        path: Some(PathBuf::from(t.binary())),
                        version: Some("99.0.0".into()),
                        ..Probe::default()
                    },
                )
            })
            .collect()
    }

    #[test]
    fn repos_service_requests_git_and_gh() {
        let p = plugin(&format!("{BASE}services = [\"repos\"]\n"));
        let map = requesters(
            &AppSettings::default(),
            &[p],
            &BTreeMap::new(),
            &[],
            Os::Linux,
        );
        assert!(!map[&ToolId::Git][0].optional);
        assert!(map[&ToolId::Gh][0].optional);
        assert!(!map.contains_key(&ToolId::Claude));
    }

    #[test]
    fn default_ai_is_required_and_others_found_are_optional() {
        let settings = AppSettings {
            default_ai: Some(AiId::ClaudeCode),
            ..AppSettings::default()
        };
        let map = requesters(&settings, &[], &found(&[ToolId::Codex]), &[], Os::Linux);
        assert!(!map[&ToolId::Claude][0].optional);
        assert!(map[&ToolId::Codex][0].optional);
        assert!(!map.contains_key(&ToolId::Opencode));
    }

    #[test]
    fn node_is_requested_for_npm_only_tools() {
        let settings = AppSettings {
            default_ai: Some(AiId::Codex),
            ..AppSettings::default()
        };
        let map = requesters(&settings, &[], &BTreeMap::new(), &[], Os::Linux);
        let node = &map[&ToolId::Node];
        assert_eq!(node[0].kind, ToolRequesterKind::Tool);
        assert_eq!(node[0].id, "codex");
        assert!(!node[0].optional);

        // Codex déjà là : Node.js n'est plus demandé.
        let map = requesters(&settings, &[], &found(&[ToolId::Codex]), &[], Os::Linux);
        assert!(!map.contains_key(&ToolId::Node));
    }

    #[test]
    fn plugin_tools_are_listed_with_the_plugin() {
        let p = plugin(&format!("{BASE}\n[[requires]]\ntool = \"node\"\noptional = true\n"));
        let map = requesters(
            &AppSettings::default(),
            &[p],
            &BTreeMap::new(),
            &[],
            Os::Linux,
        );
        assert_eq!(map[&ToolId::Node][0].name, "Gestion de projet");
        assert!(map[&ToolId::Node][0].optional);
    }

    #[test]
    fn computes_status() {
        let mut p = found(&[ToolId::Gh]).remove(&ToolId::Gh).unwrap();
        assert_eq!(status_of(ToolId::Gh, &p), ToolStatus::Ready);
        p.login = Some(detect::Login::NotConnected);
        assert_eq!(status_of(ToolId::Gh, &p), ToolStatus::NotConnected);
        p.version = Some("1.0.0".into());
        assert_eq!(status_of(ToolId::Gh, &p), ToolStatus::Outdated);
        assert_eq!(
            status_of(ToolId::Gh, &Probe::default()),
            ToolStatus::Missing
        );
    }

    /// Détection réelle : `cargo test --lib -- --ignored detects_local_tools --nocapture`
    #[test]
    #[ignore]
    fn detects_local_tools() {
        tauri::async_runtime::block_on(async {
            let service = ToolService::new();
            let report = service.report(&AppSettings::default(), &[], true).await;
            println!("{} · {:?}", report.platform, report.managers);
            for t in report.tools {
                println!(
                    "{:?} {:?} {:?} {:?} {:?}",
                    t.id, t.status, t.version, t.account, t.path
                );
            }
        });
    }
}
