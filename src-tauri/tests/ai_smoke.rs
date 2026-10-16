//! Tests réels avec les CLI d'IA installées, ignorés par défaut (plan § 11) : ils consomment
//! l'abonnement de l'utilisateur.
//!
//! ```text
//! cargo test --manifest-path src-tauri/Cargo.toml --test ai_smoke -- --ignored
//! ```
//!
//! Claude Code appelle `fake__echo` à travers la passerelle, configurée comme le fera
//! l'adaptateur (§ 4.2) : `--mcp-config` avec le jeton lu dans `CADENCE_MCP_TOKEN`
//! (jamais en argument ni sur disque), `--strict-mcp-config`.

use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use cadence_lib::{
    gateway::{
        PluginSummary,
        scope::SessionScope,
        server::{Gateway, GatewayContext},
    },
    plugins::{
        env::{LaunchSpec, server_env},
        host::{HostConfig, PluginHost},
        process::ProcessState,
    },
};
use secrecy::ExposeSecret;
use serde_json::{Value, json};

fn fake_plugin() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent()
        .and_then(|deps| deps.parent())
        .unwrap()
        .join("examples")
        .join(format!("fake-plugin{}", std::env::consts::EXE_SUFFIX))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "lance Claude Code (abonnement de l'utilisateur)"]
async fn ai_smoke_claude_calls_a_plugin_tool_through_the_gateway() {
    let claude = which::which("claude").expect("Claude Code n'est pas installé");
    let dir = std::env::temp_dir().join(format!("cadence-ai-smoke-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

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
        vec![PluginSummary { id: "fake".into(), name: "Factice".into(), description: String::new(), views: vec![], has_server: true }]
    });
    let context = GatewayContext::new("0.3.0", host.clone(), plugins, Arc::new(|_| {}));
    let gateway = Gateway::start(context, 0).await.unwrap();
    let token = gateway.issue_token(SessionScope { session: "smoke".into(), run: None });

    // `${CADENCE_MCP_TOKEN}` est développé par Claude Code depuis son environnement.
    let config = json!({
        "mcpServers": { "cadence": {
            "type": "http",
            "url": gateway.url(),
            "headers": { "Authorization": "Bearer ${CADENCE_MCP_TOKEN}" },
            "timeout": 600000
        } }
    });
    let config_path = dir.join("mcp.json");
    std::fs::write(&config_path, config.to_string()).unwrap();
    assert!(!std::fs::read_to_string(&config_path).unwrap().contains(token.expose_secret()));

    let output = tokio::time::timeout(
        Duration::from_secs(240),
        tokio::process::Command::new(claude)
            .args([
                "-p",
                "Appelle l'outil MCP fake__echo du serveur cadence avec text=\"cadence-ok\", puis réponds uniquement par la valeur de echo.text renvoyée.",
                "--mcp-config",
            ])
            .arg(&config_path)
            .args(["--strict-mcp-config", "--allowedTools", "mcp__cadence__fake__echo", "--output-format", "json"])
            .current_dir(&dir)
            .env("CADENCE_MCP_TOKEN", token.expose_secret())
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .expect("Claude Code n'a pas répondu en 4 min")
    .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "claude a échoué : {stdout}\n{}", String::from_utf8_lossy(&output.stderr));
    let result: Value = serde_json::from_str(stdout.trim()).unwrap_or(Value::Null);
    let text = result["result"].as_str().unwrap_or(&stdout);
    assert!(text.contains("cadence-ok"), "réponse inattendue : {stdout}");
    assert_eq!(host.status("fake").state, ProcessState::Ready, "le serveur du plugin a été lancé par la passerelle");

    host.stop_all().await;
    gateway.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}
