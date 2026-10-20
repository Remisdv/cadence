//! Tests réels avec les CLI d'IA installées, ignorés par défaut (plan § 11) : ils consomment
//! l'abonnement de l'utilisateur.
//!
//! ```text
//! cargo test --manifest-path src-tauri/Cargo.toml --test ai_smoke -- --ignored
//! ```
//!
//! Claude Code appelle `fake__echo` à travers la passerelle, configurée comme le fera
//! l'adaptateur (§ 4.2) : `--mcp-config` avec le jeton lu dans `CADENCE_MCP_TOKEN`
//! (jamais en argument ni sur disque), `--strict-mcp-config`. Puis le chat du cœur complet :
//! un tour qui appelle l'outil, redémarrage du service, un tour qui reprend la session.

use std::{
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use cadence_lib::{
    ai::events::AiEvent,
    chat::{
        ChatConfig, ChatSendInput, ChatService, GatewayAccess,
        store::{ChatState, ChatStore},
    },
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

fn fake_host(dir: &std::path::Path) -> Arc<PluginHost> {
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
    Arc::new(PluginHost::new(HostConfig::new("0.3.0"), resolve, Arc::new(|_| {})))
}

async fn chat_service(dir: &std::path::Path, gateway: &Gateway) -> Arc<ChatService> {
    let claude = which::which("claude").expect("Claude Code n'est pas installé");
    let store = ChatStore::open(&dir.join("chat.sqlite3")).await.unwrap();
    let mut config = ChatConfig::new(dir.join("sessions"), dir.join("staging"));
    config.model = Some("haiku".into());
    let access = GatewayAccess { url: gateway.url(), context: gateway.context().clone() };
    ChatService::new(store, config, Arc::new(move |_| Ok(claude.clone())), Arc::new(Vec::new), Some(access), Arc::new(|_| {}))
}

async fn wait_idle(chat: &ChatService, id: &str) -> ChatState {
    let start = Instant::now();
    loop {
        let state = chat.detail(id).await.unwrap().session.state;
        if !matches!(state, ChatState::Running | ChatState::WaitingApproval) {
            return state;
        }
        assert!(start.elapsed() < Duration::from_secs(240), "tour trop long");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "lance Claude Code (abonnement de l'utilisateur)"]
async fn ai_smoke_chat_calls_a_tool_then_resumes_after_a_restart() {
    let dir = std::env::temp_dir().join(format!("cadence-ai-chat-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let host = fake_host(&dir);
    let plugins = Arc::new(|| {
        vec![PluginSummary { id: "fake".into(), name: "Factice".into(), description: String::new(), views: vec![], has_server: true }]
    });
    let reports = Arc::new(Mutex::new(Vec::new()));
    let sink = reports.clone();
    let context = GatewayContext::new("0.3.0", host.clone(), plugins, Arc::new(move |e| sink.lock().unwrap().push(e)));
    let gateway = Gateway::start(context, 0).await.unwrap();

    let chat = chat_service(&dir, &gateway).await;
    let input = ChatSendInput {
        session: None,
        text: "Appelle l'outil fake__echo avec text=\"marmotte\", puis réponds uniquement par la valeur renvoyée.".into(),
        ai: None,
        attachments: None,
        permissions: None,
    };
    let id = chat.send(input).await.unwrap().session;
    assert_eq!(wait_idle(&chat, &id).await, ChatState::Idle, "{:?}", chat.detail(&id).await.unwrap().events);
    let events: Vec<AiEvent> = chat.detail(&id).await.unwrap().events.into_iter().map(|e| e.event).collect();
    assert!(
        events.iter().any(|e| matches!(e, AiEvent::ToolStarted { name, .. } if name == "mcp__cadence__fake__echo")),
        "outil du plugin appelé : {events:?}"
    );
    assert!(events.iter().any(|e| matches!(e, AiEvent::MessageCompleted { text, .. } if text.contains("marmotte"))));
    chat.shutdown().await;

    // Nouveau service (redémarrage de l'app) : la conversation reprend la session native.
    let chat = chat_service(&dir, &gateway).await;
    let follow = ChatSendInput { session: Some(id.clone()), text: "Quel mot as-tu envoyé à l'outil ? Réponds par ce mot seulement.".into(), ai: None, attachments: None, permissions: None };
    chat.send(follow).await.unwrap();
    assert_eq!(wait_idle(&chat, &id).await, ChatState::Idle);
    let last = chat.detail(&id).await.unwrap().events.into_iter().rev().find_map(|e| match e.event {
        AiEvent::MessageCompleted { text, .. } => Some(text),
        _ => None,
    });
    assert!(last.unwrap_or_default().to_lowercase().contains("marmotte"), "contexte repris après redémarrage");

    chat.shutdown().await;
    host.stop_all().await;
    gateway.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

/// CLI d'une IA pour les tests de fumée : variable `CADENCE_SMOKE_<IA>` (chemin), sinon le `PATH`.
fn smoke_cli(var: &str, name: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).or_else(|| which::which(name).ok()).unwrap_or_else(|| panic!("{name} introuvable ({var})"))
}

async fn chat_for(dir: &std::path::Path, gateway: &Gateway, cli: PathBuf) -> Arc<ChatService> {
    let store = ChatStore::open(&dir.join("chat.sqlite3")).await.unwrap();
    let config = ChatConfig::new(dir.join("sessions"), dir.join("staging"));
    let access = GatewayAccess { url: gateway.url(), context: gateway.context().clone() };
    ChatService::new(store, config, Arc::new(move |_| Ok(cli.clone())), Arc::new(Vec::new), Some(access), Arc::new(|_| {}))
}

async fn fake_gateway(dir: &std::path::Path) -> (Arc<PluginHost>, Gateway) {
    let host = fake_host(dir);
    let plugins = Arc::new(|| {
        vec![PluginSummary { id: "fake".into(), name: "Factice".into(), description: String::new(), views: vec![], has_server: true }]
    });
    let context = GatewayContext::new("0.3.0", host.clone(), plugins, Arc::new(|_| {}));
    (host, Gateway::start(context, 0).await.unwrap())
}

fn texts(events: &[AiEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            AiEvent::MessageCompleted { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// OpenCode isolé de la configuration de l'utilisateur (dossiers XDG temporaires) : il répond
/// avec le modèle gratuit d'OpenCode Zen, sans compte.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "lance OpenCode (modèle gratuit d'OpenCode Zen, réseau)"]
async fn ai_smoke_opencode_chat_calls_a_tool_then_resumes() {
    let cli = smoke_cli("CADENCE_SMOKE_OPENCODE", "opencode");
    let dir = std::env::temp_dir().join(format!("cadence-ai-opencode-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for var in ["XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME"] {
        // SAFETY : test isolé, variables posées avant tout lancement de processus.
        unsafe { std::env::set_var(var, dir.join("xdg").join(var.to_lowercase())) };
    }
    let (host, gateway) = fake_gateway(&dir).await;
    let chat = chat_for(&dir, &gateway, cli.clone()).await;
    let input = ChatSendInput {
        session: None,
        text: "Appelle l'outil cadence_fake__echo avec text=\"marmotte\", puis réponds uniquement par la valeur renvoyée.".into(),
        ai: Some(cadence_lib::app_settings::AiId::Opencode),
        attachments: None,
        permissions: None,
    };
    let id = chat.send(input).await.unwrap().session;
    assert_eq!(wait_idle(&chat, &id).await, ChatState::Idle, "{:?}", chat.detail(&id).await.unwrap().events);
    let events: Vec<AiEvent> = chat.detail(&id).await.unwrap().events.into_iter().map(|e| e.event).collect();
    assert!(
        events.iter().any(|e| matches!(e, AiEvent::ToolStarted { source: cadence_lib::ai::events::ToolSource::Cadence { plugin, tool }, .. }
            if plugin == "fake" && tool == "echo")),
        "outil du plugin appelé : {events:?}"
    );
    assert!(texts(&events).to_lowercase().contains("marmotte"), "{events:?}");
    chat.shutdown().await;

    let chat = chat_for(&dir, &gateway, cli).await;
    let follow = ChatSendInput { session: Some(id.clone()), text: "Quel mot as-tu envoyé à l'outil ? Réponds par ce mot seulement.".into(), ai: None, attachments: None, permissions: None };
    chat.send(follow).await.unwrap();
    assert_eq!(wait_idle(&chat, &id).await, ChatState::Idle);
    let events: Vec<AiEvent> = chat.detail(&id).await.unwrap().events.into_iter().map(|e| e.event).collect();
    let last = events.iter().rev().find_map(|e| match e {
        AiEvent::MessageCompleted { text, .. } => Some(text.clone()),
        _ => None,
    });
    assert!(last.unwrap_or_default().to_lowercase().contains("marmotte"), "contexte repris après redémarrage : {events:?}");

    chat.shutdown().await;
    host.stop_all().await;
    gateway.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

fn git(cwd: &std::path::Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(["-c", "user.name=Cadence", "-c", "user.email=cadence@example.invalid", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git introuvable");
    assert!(out.status.success(), "git {args:?} : {}", String::from_utf8_lossy(&out.stderr));
}

/// Service Dépôts sur un dépôt « GitHub » local `me/app` dont le README porte `marqueur`.
fn local_repos(dir: &std::path::Path, marqueur: &str) -> Arc<cadence_lib::repos::RepoService> {
    let bare = dir.join("remotes").join("me").join("app.git");
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "--bare", "-q", "-b", "main"]);
    let work = dir.join("work");
    std::fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "main"]);
    std::fs::write(work.join("README.md"), format!("{marqueur}\n")).unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "init"]);
    git(&work, &["push", "-q", &bare.to_string_lossy(), "main"]);
    let settings = Arc::new(cadence_lib::app_settings::AppSettingsStore::load(&dir.join("config")));
    settings
        .update(|s| {
            s.tracked_repos.insert("gestion-projet".into(), vec!["me/app".into()]);
        })
        .unwrap();
    let base = format!("{}{}", dir.join("remotes").display(), std::path::MAIN_SEPARATOR);
    let secrets = Arc::new(cadence_lib::secrets::MemorySecrets::default());
    Arc::new(cadence_lib::repos::RepoService::new(dir.join("repos"), settings, secrets).with_remote_base(base))
}

/// Un tour : l'IA demande le worktree (`cadence__repo_checkout`) puis lit un fichier dedans,
/// dans le même tour, sans carte d'approbation (accès accordé d'office).
async fn reads_a_granted_worktree(ai: cadence_lib::app_settings::AiId, cli: PathBuf, checkout_tool: &str, dir: &std::path::Path) {
    let (host, gateway) = fake_gateway(dir).await;
    gateway.context().set_repos(local_repos(dir, "marqueur-tamanoir"));
    let chat = chat_for(dir, &gateway, cli).await;
    let input = ChatSendInput {
        session: None,
        text: format!(
            "Appelle l'outil {checkout_tool} avec repo=\"me/app\" et task=\"smoke\". Puis lis le fichier README.md \
             du dossier `path` renvoyé avec ton outil de lecture de fichiers (pas de shell), et réponds uniquement par \
             son contenu."
        ),
        ai: Some(ai),
        attachments: None,
        permissions: None,
    };
    let id = chat.send(input).await.unwrap().session;
    assert_eq!(wait_idle(&chat, &id).await, ChatState::Idle, "{:?}", chat.detail(&id).await.unwrap().events);
    let detail = chat.detail(&id).await.unwrap();
    let events: Vec<AiEvent> = detail.events.into_iter().map(|e| e.event).collect();
    assert!(!events.iter().any(|e| matches!(e, AiEvent::ApprovalRequested { .. })), "aucune carte d'approbation : {events:?}");
    assert!(texts(&events).contains("marqueur-tamanoir"), "fichier du worktree lu : {events:?}");
    assert_eq!(detail.worktrees.len(), 1);
    chat.shutdown().await;
    host.stop_all().await;
    gateway.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "lance OpenCode (modèle gratuit d'OpenCode Zen, réseau)"]
async fn ai_smoke_opencode_reads_a_granted_worktree_in_the_same_turn() {
    let cli = smoke_cli("CADENCE_SMOKE_OPENCODE", "opencode");
    let dir = std::env::temp_dir().join(format!("cadence-ai-opencode-wt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for var in ["XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME"] {
        // SAFETY : test isolé, variables posées avant tout lancement de processus.
        unsafe { std::env::set_var(var, dir.join("xdg").join(var.to_lowercase())) };
    }
    reads_a_granted_worktree(cadence_lib::app_settings::AiId::Opencode, cli, "cadence_cadence__repo_checkout", &dir).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "lance Claude Code (abonnement de l'utilisateur)"]
async fn ai_smoke_claude_reads_a_granted_worktree_in_the_same_turn() {
    let cli = smoke_cli("CADENCE_SMOKE_CLAUDE", "claude");
    let dir = std::env::temp_dir().join(format!("cadence-ai-claude-wt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    reads_a_granted_worktree(cadence_lib::app_settings::AiId::ClaudeCode, cli, "mcp__cadence__cadence__repo_checkout", &dir).await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// Codex connecté (compte ChatGPT ou clé) : un tour qui appelle l'outil du plugin.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "lance Codex CLI (abonnement de l'utilisateur)"]
async fn ai_smoke_codex_chat_calls_a_tool() {
    let cli = smoke_cli("CADENCE_SMOKE_CODEX", "codex");
    let dir = std::env::temp_dir().join(format!("cadence-ai-codex-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (host, gateway) = fake_gateway(&dir).await;
    let chat = chat_for(&dir, &gateway, cli).await;
    let input = ChatSendInput {
        session: None,
        text: "Appelle l'outil MCP fake__echo du serveur cadence avec text=\"marmotte\", puis réponds uniquement par la valeur renvoyée.".into(),
        ai: Some(cadence_lib::app_settings::AiId::Codex),
        attachments: None,
        permissions: None,
    };
    let id = chat.send(input).await.unwrap().session;
    assert_eq!(wait_idle(&chat, &id).await, ChatState::Idle, "{:?}", chat.detail(&id).await.unwrap().events);
    let events: Vec<AiEvent> = chat.detail(&id).await.unwrap().events.into_iter().map(|e| e.event).collect();
    assert!(events.iter().any(|e| matches!(e, AiEvent::ToolStarted { name, .. } if name == "mcp__cadence__fake__echo")), "{events:?}");
    assert!(texts(&events).to_lowercase().contains("marmotte"), "{events:?}");
    chat.shutdown().await;
    host.stop_all().await;
    gateway.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Codex sans compte (`CODEX_HOME` vide) : la poignée de main et le fil marchent, le tour
/// échoue proprement avec une erreur de connexion.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "lance Codex CLI (réseau)"]
async fn ai_smoke_codex_reports_a_missing_login() {
    let cli = smoke_cli("CADENCE_SMOKE_CODEX", "codex");
    let dir = std::env::temp_dir().join(format!("cadence-ai-codex-login-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("codex-home")).unwrap();
    // SAFETY : test isolé, variable posée avant tout lancement de processus.
    unsafe { std::env::set_var("CODEX_HOME", dir.join("codex-home")) };
    let (host, gateway) = fake_gateway(&dir).await;
    let chat = chat_for(&dir, &gateway, cli).await;
    let input = ChatSendInput { session: None, text: "Réponds OK".into(), ai: Some(cadence_lib::app_settings::AiId::Codex), attachments: None, permissions: None };
    let id = chat.send(input).await.unwrap().session;
    assert_eq!(wait_idle(&chat, &id).await, ChatState::Failed);
    let events: Vec<AiEvent> = chat.detail(&id).await.unwrap().events.into_iter().map(|e| e.event).collect();
    assert!(
        events.iter().any(|e| matches!(e, AiEvent::Error { kind: cadence_lib::ai::events::AiErrorKind::Auth, .. })),
        "{events:?}"
    );
    chat.shutdown().await;
    host.stop_all().await;
    gateway.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Exécution de fond avec OpenCode : le jeton de passerelle porte l'exécution, l'IA rend son
/// résultat par `cadence__run_report`, le processus se ferme à la fin du tour.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "lance OpenCode (modèle gratuit d'OpenCode Zen, réseau)"]
async fn ai_smoke_opencode_background_action_reports_its_result() {
    use cadence_lib::{
        ai::prompt::action_prompt,
        app_settings::AiId,
        chat::{ActionStart, store::ChatMode},
        gateway::core_tools::GatewayEvent,
    };
    let cli = smoke_cli("CADENCE_SMOKE_OPENCODE", "opencode");
    let dir = std::env::temp_dir().join(format!("cadence-ai-opencode-bg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for var in ["XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME"] {
        // SAFETY : test isolé, variables posées avant tout lancement de processus.
        unsafe { std::env::set_var(var, dir.join("xdg").join(var.to_lowercase())) };
    }
    let host = fake_host(&dir);
    let plugins = Arc::new(|| {
        vec![PluginSummary { id: "fake".into(), name: "Factice".into(), description: String::new(), views: vec![], has_server: true }]
    });
    let reports = Arc::new(Mutex::new(Vec::new()));
    let sink = reports.clone();
    let context = GatewayContext::new(
        "0.3.0",
        host.clone(),
        plugins,
        Arc::new(move |event| {
            if let GatewayEvent::RunReport(report) = event {
                sink.lock().unwrap().push(report);
            }
        }),
    );
    let gateway = Gateway::start(context, 0).await.unwrap();
    let chat = chat_for(&dir, &gateway, cli).await;
    let mut prompt = action_prompt(AiId::Opencode, "fake", "echo", "Écho de fond", None, true);
    prompt.push_str("\n\nTâche : appelle l'outil cadence_fake__echo avec text=\"hibou\", puis appelle cadence_cadence__run_report avec text=<la valeur renvoyée> et final=true.");
    let id = chat
        .start_action(ActionStart {
            ai: AiId::Opencode,
            mode: ChatMode::Background,
            title: "Écho de fond".into(),
            origin: "action:fake/echo".into(),
            prompt,
            group: None,
            hidden: false,
            context: None,
        })
        .await
        .unwrap();
    let state = wait_idle(&chat, &id).await;
    let events: Vec<AiEvent> = chat.detail(&id).await.unwrap().events.into_iter().map(|e| e.event).collect();
    assert_eq!(state, ChatState::Idle, "{events:?}");
    let reports = reports.lock().unwrap().clone();
    assert!(reports.iter().any(|r| r.run == id && r.is_final && r.text.to_lowercase().contains("hibou")), "{reports:?}\n{events:?}");
    let start = Instant::now();
    while chat.live_count().await > 0 {
        assert!(start.elapsed() < Duration::from_secs(10), "processus de fond non fermé");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    chat.shutdown().await;
    host.stop_all().await;
    gateway.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}
