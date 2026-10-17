//! `fake-plugin` : serveur MCP stdio de test pour l'hôte des plugins (plan § 11).
//!
//! Construit par `cargo test` (les exemples sont compilés avec les tests) et lancé par
//! `tests/plugin_host.rs`. Outils :
//! - `echo` : renvoie ses arguments (`structuredContent`) ;
//! - `slow` : attend `ms` millisecondes ;
//! - `crash` : quitte le processus (code 3) ;
//! - `ui_only` : marqué `_meta["cadence/visibility"] = "ui"` ;
//! - `env` : variables `CADENCE_*` reçues, dossier courant, fuite éventuelle de jetons d'IA ;
//! - `grow` : ajoute l'outil `extra` et envoie `notifications/tools/list_changed` ;
//! - `fail` : répond par une erreur MCP ;
//! - `emit` : envoie `notifications/cadence/event` (`event`, `data` repris des arguments) ;
//! - `received` : notifications `notifications/cadence/*` reçues de l'hôte (méthode, paramètres) ;
//! - `slow_progress` : attend `ms` millisecondes en signalant sa progression toutes les `every` ms.
//!
//! Arguments : `--crash-on-start` (quitte avant `initialize`), `--hang-on-start` (ne répond
//! jamais). Écrit « fake-plugin prêt » sur stderr au démarrage.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use rmcp::{
    ErrorData, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, CustomNotification, JsonObject,
        ListToolsResult, MetaObject, PaginatedRequestParams, ProgressNotificationParam, ServerCapabilities, ServerConfig,
        ServerNotification, Tool,
    },
    service::{NotificationContext, RequestContext},
};
use serde_json::{Value, json};

#[derive(Clone, Default)]
struct Fake {
    grown: Arc<AtomicBool>,
    received: Arc<Mutex<Vec<Value>>>,
}

fn schema(properties: Value) -> Arc<JsonObject> {
    let Value::Object(map) = json!({ "type": "object", "properties": properties }) else { unreachable!() };
    Arc::new(map)
}

fn tool(name: &'static str, description: &'static str, properties: Value) -> Tool {
    Tool::new(name, description, schema(properties))
}

fn ok(value: Value) -> CallToolResponse {
    let mut result = CallToolResult::success(vec![ContentBlock::text(value.to_string())]);
    result.structured_content = Some(value);
    result.into()
}

impl ServerHandler for Fake {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_tool_list_changed().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut ui_only = tool("ui_only", "Outil réservé à l'interface du plugin.", json!({}));
        let Value::Object(meta) = json!({ "cadence/visibility": "ui" }) else { unreachable!() };
        ui_only.meta = Some(MetaObject(meta));
        let mut tools = vec![
            tool("echo", "Renvoie ses arguments.", json!({ "text": { "type": "string" } })),
            tool("slow", "Attend `ms` millisecondes.", json!({ "ms": { "type": "integer" } })),
            tool("crash", "Arrête le serveur.", json!({})),
            ui_only,
            tool("env", "Variables d'environnement reçues.", json!({})),
            tool("grow", "Ajoute l'outil `extra`.", json!({})),
            tool("fail", "Répond par une erreur.", json!({})),
            tool("emit", "Envoie un événement aux iframes du plugin.", json!({ "event": { "type": "string" } })),
            tool("received", "Notifications reçues de l'hôte.", json!({})),
            tool("slow_progress", "Attend `ms` ms en signalant sa progression.", json!({ "ms": { "type": "integer" }, "every": { "type": "integer" } })),
        ];
        if self.grown.load(Ordering::SeqCst) {
            tools.push(tool("extra", "Ajouté par `grow`.", json!({})));
        }
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let args = request.arguments.clone().unwrap_or_default();
        match request.name.as_ref() {
            "echo" => Ok(ok(json!({ "echo": args }))),
            "slow" => {
                let ms = args.get("ms").and_then(Value::as_u64).unwrap_or(100);
                tokio::time::sleep(Duration::from_millis(ms)).await;
                Ok(ok(json!({ "slept": ms })))
            }
            "crash" => {
                eprintln!("fake-plugin : arrêt demandé");
                std::process::exit(3);
            }
            "ui_only" | "extra" => Ok(ok(json!({ "ok": true }))),
            "env" => {
                let var = |name: &str| std::env::var(name).ok();
                Ok(ok(json!({
                    "CADENCE_API": var("CADENCE_API"),
                    "CADENCE_VERSION": var("CADENCE_VERSION"),
                    "CADENCE_DATA_DIR": var("CADENCE_DATA_DIR"),
                    "CADENCE_SETTINGS": var("CADENCE_SETTINGS"),
                    "CADENCE_LOG": var("CADENCE_LOG"),
                    "CADENCE_SECRET_TOKEN": var("CADENCE_SECRET_TOKEN"),
                    "FAKE_PLUGIN_LEAK": var("FAKE_PLUGIN_LEAK"),
                    "cwd": std::env::current_dir().ok().map(|d| d.to_string_lossy().into_owned()),
                })))
            }
            "grow" => {
                self.grown.store(true, Ordering::SeqCst);
                let _ = context.peer.notify_tool_list_changed().await;
                Ok(ok(json!({ "grown": true })))
            }
            "fail" => Err(ErrorData::invalid_params("échec demandé", None)),
            "emit" => {
                let params = json!({ "event": args.get("event").cloned().unwrap_or(json!("sync.updated")), "data": args.get("data") });
                let notification = CustomNotification::new("notifications/cadence/event", Some(params));
                let _ = context.peer.send_notification(ServerNotification::CustomNotification(notification)).await;
                Ok(ok(json!({ "emitted": true })))
            }
            "received" => Ok(ok(json!({ "received": *self.received.lock().unwrap() }))),
            "slow_progress" => {
                let total = args.get("ms").and_then(Value::as_u64).unwrap_or(500);
                let every = args.get("every").and_then(Value::as_u64).unwrap_or(100).max(10);
                let token = context.meta.get_progress_token();
                let mut done = 0;
                while done < total {
                    tokio::time::sleep(Duration::from_millis(every)).await;
                    done += every;
                    if let Some(token) = &token {
                        let _ = context.peer.notify_progress(ProgressNotificationParam::new(token.clone(), done as f64)).await;
                    }
                }
                Ok(ok(json!({ "slept": total, "progress": token.is_some() })))
            }
            other => Err(ErrorData::invalid_params(format!("outil inconnu : {other}"), None)),
        }
    }

    async fn on_custom_notification(&self, notification: CustomNotification, _context: NotificationContext<RoleServer>) {
        if notification.method.starts_with("notifications/cadence/") {
            self.received.lock().unwrap().push(json!({ "method": notification.method, "params": notification.params }));
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    eprintln!("fake-plugin prêt");
    if args.iter().any(|a| a == "--crash-on-start") {
        eprintln!("fake-plugin : plantage au démarrage");
        std::process::exit(2);
    }
    if args.iter().any(|a| a == "--hang-on-start") {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }
    let service = match Fake::default().serve(rmcp::transport::stdio()).await {
        Ok(service) => service,
        Err(err) => {
            eprintln!("fake-plugin : initialize impossible ({err})");
            std::process::exit(1);
        }
    };
    let _ = service.waiting().await;
}
