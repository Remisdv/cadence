//! Passerelle MCP interrogée par un client rmcp HTTP, devant le vrai hôte et `fake-plugin`
//! (plan § 11) : jeton invalide → 401, `Host`/`Origin` non locaux refusés, outil `ui`
//! invisible et non appelable, outils du cœur.
//!
//! `cargo test` construit `examples/fake-plugin.rs` avant de lancer les tests.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use cadence_lib::{
    gateway::{
        PluginSummary,
        core_tools::GatewayEvent,
        scope::SessionScope,
        server::{Gateway, GatewayContext},
    },
    plugins::{
        env::{LaunchSpec, server_env},
        host::{HostConfig, PluginHost},
        manifest::PluginViewInfo,
    },
};
use rmcp::{
    ServiceError, ServiceExt,
    model::CallToolRequestParams,
    transport::{StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig},
};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};

fn fake_plugin() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let path = exe
        .parent()
        .and_then(|deps| deps.parent())
        .unwrap()
        .join("examples")
        .join(format!("fake-plugin{}", std::env::consts::EXE_SUFFIX));
    assert!(path.is_file(), "`cargo build --example fake-plugin` d'abord ({} absent)", path.display());
    path
}

fn view(id: &str) -> PluginViewInfo {
    PluginViewInfo { id: id.into(), title: id.into(), icon: "file".into(), section: None, hidden: false, home: false }
}

struct Fixture {
    gateway: Gateway,
    host: Arc<PluginHost>,
    events: Arc<Mutex<Vec<GatewayEvent>>>,
    dir: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn fixture(name: &str) -> Fixture {
    let dir = std::env::temp_dir().join(format!("cadence-gateway-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (program, data_root) = (fake_plugin(), dir.join("plugins-data"));
    let resolve = Arc::new(move |plugin: &str| {
        let data_dir = data_root.join(plugin);
        Ok(LaunchSpec {
            plugin: plugin.to_string(),
            program: program.clone(),
            args: vec![],
            env: server_env(std::env::vars(), "0.3.0", &data_dir, &json!({})),
            data_dir,
            secrets: vec![],
            idle_shutdown: None,
            log_file: None,
        })
    });
    let host = Arc::new(PluginHost::new(HostConfig::new("0.3.0"), resolve, Arc::new(|_| {})));
    let plugins = Arc::new(|| {
        vec![
            PluginSummary { id: "fake".into(), name: "Factice".into(), description: String::new(), views: vec![view("accueil")], has_server: true },
            PluginSummary { id: "docs".into(), name: "Documents".into(), description: String::new(), views: vec![view("document")], has_server: false },
        ]
    });
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let context = GatewayContext::new("0.3.0", host.clone(), plugins, Arc::new(move |e| sink.lock().unwrap().push(e)));
    let gateway = Gateway::start(context, 0).await.unwrap();
    Fixture { gateway, host, events, dir }
}

type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;

async fn connect(url: &str, token: &SecretString) -> Result<Client, String> {
    let config = StreamableHttpClientTransportConfig::with_uri(url.to_string()).auth_header(token.expose_secret().to_string());
    ().serve(StreamableHttpClientTransport::from_config(config)).await.map_err(|e| e.to_string())
}

async fn call(client: &Client, tool: &str, args: Value) -> Result<rmcp::model::CallToolResult, ServiceError> {
    let params = CallToolRequestParams::new(tool.to_string()).with_arguments(args.as_object().unwrap().clone());
    client.peer().call_tool(params).await
}

#[tokio::test(flavor = "multi_thread")]
async fn exposes_plugin_and_core_tools_to_an_authorized_session() {
    let f = fixture("tools").await;
    assert!(f.gateway.url().starts_with("http://127.0.0.1:"));
    let token = f.gateway.issue_token(SessionScope { session: "chat-1".into(), run: None });
    let client = connect(&f.gateway.url(), &token).await.unwrap();

    let mut names: Vec<String> = client.peer().list_all_tools().await.unwrap().into_iter().map(|t| t.name.into_owned()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "cadence__open_view",
            "cadence__plugins_info",
            "cadence__run_report",
            "fake__crash",
            "fake__echo",
            "fake__emit",
            "fake__env",
            "fake__fail",
            "fake__grow",
            "fake__slow",
        ],
        "`ui_only` n'est jamais exposé"
    );

    let echo = call(&client, "fake__echo", json!({ "text": "bonjour" })).await.unwrap();
    assert_eq!(echo.structured_content, Some(json!({ "echo": { "text": "bonjour" } })));
    // `resultType` (exigé en 2026-07-28, que Claude Code négocie) : ce client négocie une
    // révision plus ancienne et ne le reçoit pas ; c'est `ai_smoke` qui le vérifie.

    // Ni outil `ui` deviné, ni plugin sans serveur, ni outil inventé.
    for name in ["fake__ui_only", "docs__lire", "fake__absent", "sans_prefixe", "cadence__inconnu"] {
        let err = call(&client, name, json!({})).await.unwrap_err();
        assert!(matches!(err, ServiceError::McpError(_)), "{name} : {err:?}");
    }
    // Erreur du plugin : résultat d'erreur lisible par l'IA, pas une panne.
    let failed = call(&client, "fake__fail", json!({})).await.unwrap();
    assert_eq!(failed.is_error, Some(true));

    let info = call(&client, "cadence__plugins_info", json!({})).await.unwrap();
    assert_eq!(info.structured_content.unwrap()["plugins"][1]["views"][0]["id"], "document");

    let opened = call(&client, "cadence__open_view", json!({ "plugin": "docs", "view": "document", "params": { "page": "12" } })).await.unwrap();
    assert_eq!(opened.is_error, Some(false));
    let refused = call(&client, "cadence__open_view", json!({ "plugin": "docs", "view": "absente" })).await.unwrap();
    assert_eq!(refused.is_error, Some(true));
    match f.events.lock().unwrap().as_slice() {
        [GatewayEvent::OpenView(request)] => {
            assert_eq!((request.session.as_str(), request.plugin.as_str(), request.view.as_str()), ("chat-1", "docs", "document"));
            assert_eq!(request.params.get("page").map(String::as_str), Some("12"));
        }
        other => panic!("une demande d'ouverture attendue : {other:?}"),
    }

    // Hors exécution de fond, pas de rapport.
    let no_run = call(&client, "cadence__run_report", json!({ "text": "fini" })).await.unwrap();
    assert_eq!(no_run.is_error, Some(true));

    let _ = client.cancel().await;
    f.host.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn records_background_run_reports() {
    let f = fixture("runs").await;
    let token = f.gateway.issue_token(SessionScope { session: "run-7".into(), run: Some("run-7".into()) });
    let client = connect(&f.gateway.url(), &token).await.unwrap();

    call(&client, "cadence__run_report", json!({ "text": "Lecture des issues", "progress": 40 })).await.unwrap();
    let last = f.gateway.context().last_report("run-7").unwrap();
    assert_eq!((last.progress, last.is_final), (Some(40), false));
    call(&client, "cadence__run_report", json!({ "text": "3 issues préparées", "final": true, "data": { "issues": [1, 2, 3] } }))
        .await
        .unwrap();
    let last = f.gateway.context().last_report("run-7").unwrap();
    assert!(last.is_final);
    assert_eq!(last.data, Some(json!({ "issues": [1, 2, 3] })));
    let reports = f.events.lock().unwrap().iter().filter(|e| matches!(e, GatewayEvent::RunReport(_))).count();
    assert_eq!(reports, 2);
    let _ = client.cancel().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn refuses_unknown_tokens_and_foreign_hosts() {
    let f = fixture("auth").await;
    let url = f.gateway.url();
    let http = reqwest::Client::new();
    let initialize = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } }
    });
    let post = |token: Option<&str>| {
        let mut request = http
            .post(&url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .body(initialize.to_string());
        if let Some(token) = token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
        request
    };

    assert_eq!(post(None).send().await.unwrap().status(), 401);
    assert_eq!(post(Some("inconnu")).send().await.unwrap().status(), 401);

    let token = f.gateway.issue_token(SessionScope { session: "chat-2".into(), run: None });
    let ok = post(Some(token.expose_secret())).send().await.unwrap();
    assert!(ok.status().is_success(), "jeton valide : {}", ok.status());

    let host = post(Some(token.expose_secret())).header("Host", "evil.example").send().await.unwrap();
    assert!(host.status().is_client_error() && host.status() != 401, "Host étranger : {}", host.status());
    let origin = post(Some(token.expose_secret())).header("Origin", "http://evil.example").send().await.unwrap();
    assert!(origin.status().is_client_error(), "Origin présent : {}", origin.status());

    // Session terminée : son jeton ne vaut plus rien.
    f.gateway.revoke("chat-2");
    assert_eq!(post(Some(token.expose_secret())).send().await.unwrap().status(), 401);
    assert!(connect(&url, &token).await.is_err());
}
