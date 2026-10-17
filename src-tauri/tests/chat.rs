//! Chat du cœur piloté contre `examples/fake-claude.rs`, qui parle le protocole stream-json de
//! Claude Code (aucun appel réseau). `cargo test` construit les exemples d'abord.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use cadence_lib::{
    ai::events::{AiEvent, ApprovalDecision},
    chat::{
        ChatConfig, ChatNotice, ChatSendInput, ChatService,
        store::{ChatState, ChatStore},
    },
};

fn fake_claude() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let path = exe
        .parent()
        .and_then(|deps| deps.parent())
        .unwrap()
        .join("examples")
        .join(format!("fake-claude{}", std::env::consts::EXE_SUFFIX));
    assert!(path.is_file(), "`cargo build --example fake-claude` d'abord ({} absent)", path.display());
    path
}

struct Fixture {
    chat: Arc<ChatService>,
    notices: Arc<Mutex<Vec<ChatNotice>>>,
    dir: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn fixture(name: &str, tweak: impl FnOnce(&mut ChatConfig)) -> Fixture {
    let dir = std::env::temp_dir().join(format!("cadence-chat-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    service(dir, tweak).await
}

async fn service(dir: PathBuf, tweak: impl FnOnce(&mut ChatConfig)) -> Fixture {
    let store = ChatStore::open(&dir.join("chat.sqlite3")).await.unwrap();
    let mut config = ChatConfig::new(dir.join("sessions"), dir.join("staging"));
    config.interrupt_grace = Duration::from_secs(3);
    tweak(&mut config);
    let notices = Arc::new(Mutex::new(Vec::new()));
    let sink = notices.clone();
    let cli = fake_claude();
    let chat = ChatService::new(
        store,
        config,
        Arc::new(move |_| Ok(cli.clone())),
        Arc::new(Vec::new),
        None,
        Arc::new(move |n| sink.lock().unwrap().push(n)),
    );
    Fixture { chat, notices, dir }
}

impl Fixture {
    async fn send(&self, session: Option<&str>, text: &str) -> String {
        let input = ChatSendInput { session: session.map(str::to_string), text: text.into(), ai: None };
        self.chat.send(input).await.unwrap().session
    }

    /// Attend l'état `state` (10 s au plus).
    async fn wait(&self, id: &str, state: ChatState) {
        let start = Instant::now();
        loop {
            let current = self.chat.detail(id).await.unwrap().session.state;
            if current == state {
                return;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "toujours {current:?}, {state:?} attendu");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn detail_origin(&self, id: &str) -> String {
        self.chat.detail(id).await.unwrap().session.origin
    }

    async fn events(&self, id: &str) -> Vec<AiEvent> {
        self.chat.detail(id).await.unwrap().events.into_iter().map(|e| e.event).collect()
    }

    async fn replies(&self, id: &str) -> Vec<String> {
        self.events(id)
            .await
            .into_iter()
            .filter_map(|e| if let AiEvent::MessageCompleted { text, .. } = e { Some(text) } else { None })
            .collect()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_turns_and_resumes_after_a_restart() {
    let f = fixture("turns", |_| {}).await;
    let id = f.send(None, "bonjour\nla suite").await;
    f.wait(&id, ChatState::Idle).await;
    let detail = f.chat.detail(&id).await.unwrap();
    assert_eq!(detail.session.title, "bonjour");
    assert!(matches!(&detail.events[0].event, AiEvent::UserMessage { text } if text == "bonjour\nla suite"));
    assert!(detail.events.iter().any(|e| matches!(e.event, AiEvent::TurnCompleted { ok: true, .. })));
    assert!(detail.events.iter().all(|e| !matches!(e.event, AiEvent::MessageDelta { .. })), "deltas jamais enregistrés");
    let deltas = f
        .notices
        .lock()
        .unwrap()
        .iter()
        .filter(|n| matches!(n, ChatNotice::Event(p) if p.seq.is_none() && matches!(p.event, AiEvent::MessageDelta { .. })))
        .count();
    assert!(deltas > 0, "deltas diffusés au front");

    f.send(Some(&id), "encore").await;
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await, ["écho : bonjour\nla suite", "écho : encore"]);
    assert_eq!(f.chat.live_count().await, 1, "un seul processus pour les deux tours");
    let turns: Vec<u32> = f.chat.detail(&id).await.unwrap().events.iter().map(|e| e.turn).collect();
    assert_eq!((turns.first(), turns.last()), (Some(&1), Some(&2)));

    // Redémarrage de l'app : la conversation reprend la session native (`--resume`).
    f.chat.shutdown().await;
    let dir = f.dir.clone();
    std::mem::forget(f);
    let f = service(dir, |_| {}).await;
    f.send(Some(&id), "après redémarrage").await;
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await.last().unwrap(), "écho [reprise fake-1] : après redémarrage");
    f.chat.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn relays_approvals_both_ways() {
    let f = fixture("approvals", |_| {}).await;
    for (decision, reply) in [(ApprovalDecision::Allow, "autorisé"), (ApprovalDecision::Deny, "refusé")] {
        let id = f.send(None, "approbation").await;
        f.wait(&id, ChatState::WaitingApproval).await;
        let request = f
            .events(&id)
            .await
            .into_iter()
            .find_map(|e| if let AiEvent::ApprovalRequested { request, title, .. } = e { assert_eq!(title, "Write ok.txt"); Some(request) } else { None })
            .unwrap();
        f.chat.approve(&id, &request, decision).await.unwrap();
        f.wait(&id, ChatState::Idle).await;
        assert_eq!(f.replies(&id).await, [reply]);
        assert!(f.events(&id).await.iter().any(|e| matches!(e, AiEvent::ApprovalAnswered { decision: d, .. } if *d == decision)));
        assert!(f.chat.approve(&id, &request, decision).await.is_err(), "déjà répondu");
    }
    f.chat.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn interrupts_a_turn_and_survives_a_crash() {
    let f = fixture("interrupt", |_| {}).await;
    let id = f.send(None, "long").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    f.chat.interrupt(&id).await.unwrap();
    f.wait(&id, ChatState::Interrupted).await;
    assert!(f.events(&id).await.iter().any(|e| matches!(e, AiEvent::TurnCompleted { interrupted: true, .. })));
    assert!(f.chat.interrupt(&id).await.is_err(), "plus de tour en cours");

    // La CLI plante pendant un tour : erreur lisible, puis la conversation repart.
    f.send(Some(&id), "plante").await;
    f.wait(&id, ChatState::Failed).await;
    let crashed = f.events(&id).await.into_iter().any(|e| matches!(e, AiEvent::Error { message, .. } if message.contains("panique simulée")));
    assert!(crashed, "dernières lignes de stderr dans l'erreur");
    f.send(Some(&id), "reprise").await;
    f.wait(&id, ChatState::Idle).await;
    assert!(f.replies(&id).await.last().unwrap().ends_with(": reprise"));
    f.chat.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn closes_idle_processes_caps_them_and_deletes_conversations() {
    let f = fixture("live", |c| {
        c.max_live = 1;
        c.idle_close = Duration::ZERO;
    })
    .await;
    let a = f.send(None, "un").await;
    f.wait(&a, ChatState::Idle).await;
    let b = f.send(None, "deux").await;
    f.wait(&b, ChatState::Idle).await;
    assert!(f.chat.live_count().await <= 1, "plafond de processus vivants");

    assert_eq!(f.chat.close_idle().await.len(), 1);
    assert_eq!(f.chat.live_count().await, 0);
    assert_eq!(f.chat.sessions().await.unwrap().len(), 2, "les conversations restent");

    assert!(f.dir.join("sessions").join(&a).is_dir());
    f.chat.delete(&a).await.unwrap();
    assert!(!f.dir.join("sessions").join(&a).exists(), "dossier de travail supprimé");
    assert!(f.chat.detail(&a).await.is_err());
    assert_eq!(f.chat.rename(&b, "Renommée").await.unwrap().title, "Renommée");
    assert!(f.chat.send(ChatSendInput { session: None, text: "  ".into(), ai: None }).await.is_err());
    f.chat.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opens_granted_worktrees_to_the_cli_from_the_next_turn() {
    use cadence_lib::{
        chat::GatewayAccess,
        gateway::server::GatewayContext,
        plugins::host::{HostConfig, PluginHost},
    };
    let dir = std::env::temp_dir().join(format!("cadence-chat-grants-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let resolve = Arc::new(|_: &str| Err(cadence_lib::error::AppError::Internal("aucun plugin".into())));
    let host = Arc::new(PluginHost::new(HostConfig::new("0.3.0"), resolve, Arc::new(|_| {})));
    let context = GatewayContext::new("0.3.0", host, Arc::new(Vec::new), Arc::new(|_| {}));
    let store = ChatStore::open(&dir.join("chat.sqlite3")).await.unwrap();
    let cli = fake_claude();
    let chat = ChatService::new(
        store,
        ChatConfig::new(dir.join("sessions"), dir.join("staging")),
        Arc::new(move |_| Ok(cli.clone())),
        Arc::new(Vec::new),
        Some(GatewayAccess { url: "http://127.0.0.1:9/mcp".into(), context: context.clone() }),
        Arc::new(|_| {}),
    );
    let f = Fixture { chat, notices: Arc::default(), dir };

    let id = f.send(None, "dossiers").await;
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await, ["dossiers : "], "aucun dossier ouvert d'office");

    // `cadence__repo_checkout` a ouvert un worktree : la CLI est relancée (reprise) avec lui.
    let wt = f.dir.join("repos").join(".worktrees").join("me").join("app").join("issue-42");
    context.grant(&id, wt.clone());
    f.send(Some(&id), "dossiers").await;
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await.last().unwrap(), &format!("dossiers [reprise] : {}", wt.display()));
    assert_eq!(f.chat.live_count().await, 1, "l'ancien processus est fermé");

    // Rien de nouveau : même processus.
    f.send(Some(&id), "encore").await;
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await.last().unwrap(), "écho [reprise fake-1] : encore");

    // Une autre conversation n'en hérite pas ; la suppression oublie l'accès.
    let other = f.send(None, "dossiers").await;
    f.wait(&other, ChatState::Idle).await;
    assert_eq!(f.replies(&other).await, ["dossiers : "]);
    f.chat.delete(&id).await.unwrap();
    assert!(context.granted(&id).is_empty());
    f.chat.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_background_actions_in_a_queue_and_records_reports() {
    use cadence_lib::{
        app_settings::AiId,
        chat::{ActionStart, store::ChatMode},
        gateway::core_tools::RunReport,
    };
    let f = fixture("background", |c| c.max_background = 1).await;
    let start = |n: u32| ActionStart {
        ai: AiId::ClaudeCode,
        mode: ChatMode::Background,
        title: format!("Action {n}"),
        origin: "action:exemple/resumer".into(),
        prompt: format!("tâche {n}"),
    };
    let first = f.chat.start_action(start(1)).await.unwrap();
    let second = f.chat.start_action(start(2)).await.unwrap();
    let row = f.chat.detail(&second).await.unwrap().session;
    assert_eq!((row.mode, row.state), (ChatMode::Background, ChatState::Running), "en file, affichée en cours");
    f.wait(&first, ChatState::Idle).await;
    f.wait(&second, ChatState::Idle).await;
    assert_eq!(f.replies(&second).await, ["écho : tâche 2"]);
    assert_eq!(f.detail_origin(&first).await, "action:exemple/resumer");

    // Processus fermé à la fin du tour : la place est rendue.
    let start_wait = Instant::now();
    while f.chat.live_count().await > 0 {
        assert!(start_wait.elapsed() < Duration::from_secs(10), "processus de fond non fermé");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    f.chat
        .record_report(RunReport {
            run: first.clone(),
            session: first.clone(),
            text: "Terminé".into(),
            progress: Some(100),
            data: None,
            is_final: true,
        })
        .await
        .unwrap();
    assert!(f.events(&first).await.iter().any(|e| matches!(e, AiEvent::Report { is_final: true, text, .. } if text == "Terminé")));

    // Suite dans le chat : la conversation reprend en mode conversation.
    f.send(Some(&first), "et ensuite ?").await;
    f.wait(&first, ChatState::Idle).await;
    let last = f.replies(&first).await.last().cloned().unwrap_or_default();
    assert!(last.contains("reprise fake-1") && last.ends_with("et ensuite ?"), "session native reprise : {last}");
    f.chat.shutdown().await;
}
