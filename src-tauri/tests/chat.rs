//! Chat du cœur piloté contre `examples/fake-claude.rs`, qui parle le protocole stream-json de
//! Claude Code (aucun appel réseau). `cargo test` construit les exemples d'abord.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use cadence_lib::{
    ai::{
        claude_code::Permissions,
        events::{AiEvent, ApprovalDecision},
    },
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
        let input = ChatSendInput { session: session.map(str::to_string), text: text.into(), ai: None, attachments: None, permissions: None, plugin: None };
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

impl Fixture {
    /// Demandes d'accord encore sans réponse : (requête, portée de « pour la session »).
    async fn open_approvals(&self, id: &str) -> Vec<(String, Option<String>)> {
        let events = self.events(id).await;
        let answered: Vec<String> =
            events.iter().filter_map(|e| if let AiEvent::ApprovalAnswered { request, .. } = e { Some(request.clone()) } else { None }).collect();
        events
            .into_iter()
            .filter_map(|e| match e {
                AiEvent::ApprovalRequested { request, scope, .. } if !answered.contains(&request) => Some((request, scope)),
                _ => None,
            })
            .collect()
    }

    async fn asked(&self, id: &str) -> usize {
        self.events(id).await.iter().filter(|e| matches!(e, AiEvent::ApprovalRequested { .. })).count()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn allows_a_command_family_for_the_rest_of_the_session() {
    let f = fixture("session-rules", |_| {}).await;
    let id = f.send(None, "commandes cd /r; git fetch origin ;; git -C /r log --oneline ;; python3 a.py ;; git status").await;
    f.wait(&id, ChatState::WaitingApproval).await;
    let open = f.open_approvals(&id).await;
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].1.as_deref(), Some("commande git"), "portée affichée sous la demande");
    f.chat.approve(&id, &open[0].0, ApprovalDecision::AllowSession).await.unwrap();

    // `git log` passe d'office ; `python3` n'est pas couvert : nouvelle demande.
    f.wait(&id, ChatState::WaitingApproval).await;
    let open = f.open_approvals(&id).await;
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].1.as_deref(), Some("commande python3"));
    f.chat.approve(&id, &open[0].0, ApprovalDecision::Allow).await.unwrap();
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await, ["commandes : autorisé | autorisé | autorisé | autorisé [règles : git:*]"]);
    assert_eq!(f.asked(&id).await, 2, "git log et git status n'ont pas de carte");

    // Tour suivant : l'accord tient toujours ; python3 (accordé une fois) redemande.
    f.send(Some(&id), "commandes git push origin main ;; python3 b.py").await;
    f.wait(&id, ChatState::WaitingApproval).await;
    assert_eq!(f.asked(&id).await, 3);
    let open = f.open_approvals(&id).await;
    f.chat.approve(&id, &open[0].0, ApprovalDecision::Deny).await.unwrap();
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await.last().unwrap(), "commandes : autorisé | refusé [règles : ]");

    // Une autre conversation n'hérite de rien.
    let other = f.send(None, "commandes git status").await;
    f.wait(&other, ChatState::WaitingApproval).await;
    let open = f.open_approvals(&other).await;
    f.chat.approve(&other, &open[0].0, ApprovalDecision::Allow).await.unwrap();
    f.wait(&other, ChatState::Idle).await;
    f.chat.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_chosen_during_a_turn_accepts_the_pending_and_following_requests() {
    let f = fixture("auto-pending", |_| {}).await;
    let id = f.send(None, "commandes rm -rf build ;; npm test ;; curl https://exemple.test").await;
    f.wait(&id, ChatState::WaitingApproval).await;
    // « Édition auto » n'accepte que les modifications de fichiers : la commande attend toujours.
    f.chat.set_permissions(&id, Permissions::AutoEdit).await.unwrap();
    assert_eq!(f.chat.detail(&id).await.unwrap().session.state, ChatState::WaitingApproval);
    assert_eq!(f.open_approvals(&id).await.len(), 1);

    let row = f.chat.set_permissions(&id, Permissions::Full).await.unwrap();
    assert_eq!(row.permissions, Permissions::Full);
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await, ["commandes : autorisé | autorisé | autorisé [règles : ]"]);
    assert_eq!(f.asked(&id).await, 1, "les demandes suivantes du tour n'ont pas de carte");
    assert!(f.open_approvals(&id).await.is_empty(), "la demande en attente est acceptée et marquée");
    assert!(
        f.events(&id).await.iter().any(|e| matches!(e, AiEvent::ApprovalAnswered { decision: ApprovalDecision::Allow, .. })),
        "réponse enregistrée pour la carte"
    );
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
    assert_eq!(f.chat.sessions(None, false).await.unwrap().len(), 2, "les conversations restent");

    assert!(f.dir.join("sessions").join(&a).is_dir());
    f.chat.delete(&a).await.unwrap();
    assert!(!f.dir.join("sessions").join(&a).exists(), "dossier de travail supprimé");
    assert!(f.chat.detail(&a).await.is_err());
    assert_eq!(f.chat.rename(&b, "Renommée").await.unwrap().title, "Renommée");
    assert!(f.chat.send(ChatSendInput { session: None, text: "  ".into(), ai: None, attachments: None, permissions: None, plugin: None }).await.is_err());
    f.chat.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn applies_a_new_mode_from_the_next_turn_by_resuming_the_session() {
    let f = fixture("modes", |_| {}).await;
    let input = ChatSendInput { session: None, text: "mode".into(), ai: None, attachments: None, permissions: Some(Permissions::AutoEdit), plugin: None };
    let id = f.chat.send(input).await.unwrap().session;
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await, ["mode : acceptEdits"], "mode choisi à la création");
    assert_eq!(f.chat.detail(&id).await.unwrap().session.permissions, Permissions::AutoEdit);

    // Mode changé entre deux tours : la CLI est relancée avec lui, la session native reprise.
    assert_eq!(f.chat.set_permissions(&id, Permissions::Full).await.unwrap().permissions, Permissions::Full);
    f.send(Some(&id), "mode").await;
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await.last().unwrap(), "mode [reprise] : bypassPermissions");
    assert_eq!(f.chat.live_count().await, 1, "l'ancien processus est fermé");

    // Même mode : même processus.
    f.send(Some(&id), "encore").await;
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await.last().unwrap(), "écho [reprise fake-1] : encore");

    // Changé pendant un tour : le tour en cours garde son processus, le suivant a le nouveau mode.
    f.send(Some(&id), "long").await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    f.chat.set_permissions(&id, Permissions::Ask).await.unwrap();
    f.chat.interrupt(&id).await.unwrap();
    f.wait(&id, ChatState::Interrupted).await;
    f.send(Some(&id), "mode").await;
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await.last().unwrap(), "mode [reprise] : default");

    // Une nouvelle conversation part en « Demander ».
    let other = f.send(None, "mode").await;
    f.wait(&other, ChatState::Idle).await;
    assert_eq!(f.replies(&other).await, ["mode : default"]);
    assert!(f.chat.set_permissions("absente", Permissions::Full).await.is_err());
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
    assert_eq!(f.chat.detail(&id).await.unwrap().worktrees, [wt.display().to_string()], "montré dans le panneau");

    // Pendant le tour, un accès au worktree est accordé d'office (pas de carte d'approbation) ;
    // un autre dossier reste soumis à l'utilisateur.
    let readme = wt.join("README.md");
    f.send(Some(&id), &format!("lis {}", readme.display())).await;
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await.last().unwrap(), &format!("lu : autorisé [{}]", wt.display()));
    assert!(!f.events(&id).await.iter().any(|e| matches!(e, AiEvent::ApprovalRequested { .. })));
    let outside = f.dir.join("ailleurs.txt");
    f.send(Some(&id), &format!("lis {}", outside.display())).await;
    f.wait(&id, ChatState::WaitingApproval).await;
    let request = f
        .events(&id)
        .await
        .into_iter()
        .find_map(|e| if let AiEvent::ApprovalRequested { request, .. } = e { Some(request) } else { None })
        .unwrap();
    f.chat.approve(&id, &request, ApprovalDecision::Deny).await.unwrap();
    f.wait(&id, ChatState::Idle).await;
    assert_eq!(f.replies(&id).await.last().unwrap(), "lu : refusé");

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
        group: None,
        hidden: false,
        context: None,
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

    // En fond, personne pour répondre : un accès hors worktree est refusé d'office.
    let denied = f
        .chat
        .start_action(ActionStart { prompt: format!("lis {}", f.dir.join("ailleurs.txt").display()), ..start(3) })
        .await
        .unwrap();
    f.wait(&denied, ChatState::Idle).await;
    assert_eq!(f.replies(&denied).await, ["lu : refusé"]);
    assert!(!f.events(&denied).await.iter().any(|e| matches!(e, AiEvent::ApprovalRequested { .. })));

    // Suite dans le chat : la conversation reprend en mode conversation.
    f.send(Some(&first), "et ensuite ?").await;
    f.wait(&first, ChatState::Idle).await;
    let last = f.replies(&first).await.last().cloned().unwrap_or_default();
    assert!(last.contains("reprise fake-1") && last.ends_with("et ensuite ?"), "session native reprise : {last}");
    f.chat.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn keeps_group_conversations_out_of_the_global_chat_and_inside_their_plugin() {
    use cadence_lib::{
        app_settings::AiId,
        chat::{
            ActionStart, ChatStatePayload,
            conversations::{ConversationRole, action_group},
            store::ChatMode,
        },
    };
    let f = fixture("groups", |_| {}).await;
    let context = serde_json::json!({ "type": "issue", "repo": "o/n", "issue": 42,
        "group": { "id": "sprint:o/n:3", "label": "Sprint 3 · o/n" } });
    let (group, hidden) = action_group("gestion-projet", Some(&context)).unwrap();
    let start = |origin: &str, prompt: &str| ActionStart {
        ai: AiId::ClaudeCode,
        mode: ChatMode::Chat,
        title: "Développer l'issue 42".into(),
        origin: origin.into(),
        prompt: prompt.into(),
        group: group.clone(),
        hidden,
        context: Some(context.clone()),
    };
    let sprint = f.chat.start_action(start("action:gestion-projet/developper-issue", "issue 42")).await.unwrap();
    let free = f.send(None, "bonjour").await;
    f.wait(&sprint, ChatState::Idle).await;
    f.wait(&free, ChatState::Idle).await;

    let global: Vec<String> = f.chat.sessions(None, false).await.unwrap().into_iter().map(|s| s.id).collect();
    assert_eq!(global, std::slice::from_ref(&free), "conversation de groupe cachée du chat global");
    let explicit = f.chat.sessions(Some("gestion-projet:sprint:o/n:3"), false).await.unwrap();
    assert_eq!(explicit.len(), 1, "visible avec un filtre de groupe explicite");
    let all: Vec<String> = f.chat.sessions(None, true).await.unwrap().into_iter().map(|s| s.id).collect();
    assert!(all.contains(&sprint) && all.contains(&free), "toutes sur demande (recherche du chat)");
    assert!(f.chat.detail(&sprint).await.is_ok(), "elle s'ouvre quand même dans le chat");

    let listed = f.chat.conversations("gestion-projet", Some("sprint:o/n:3")).await.unwrap();
    assert_eq!(listed.len(), 1);
    let info = &listed[0];
    assert_eq!((info.id.as_str(), info.turns, info.state), (sprint.as_str(), 1, ChatState::Idle));
    assert_eq!(info.action.as_deref(), Some("developper-issue"));
    assert_eq!(info.group.as_ref().map(|g| (g.id.as_str(), g.label.as_str())), Some(("sprint:o/n:3", "Sprint 3 · o/n")));
    assert_eq!(info.context.as_ref().and_then(|c| c.get("issue")), Some(&serde_json::json!(42)));
    assert!(f.chat.conversations("gestion-projet", None).await.unwrap().is_empty(), "sans groupe : pas les cachées");

    // Isolation : un autre plugin ne voit ni le groupe ni la conversation.
    assert!(f.chat.conversations("documents", Some("sprint:o/n:3")).await.unwrap().is_empty());
    assert!(f.chat.conversation("documents", &sprint).await.is_err());
    assert!(f.chat.conversation("gestion-projet", &free).await.is_err(), "conversation libre : pas au plugin");

    let detail = f.chat.conversation("gestion-projet", &sprint).await.unwrap();
    let roles: Vec<ConversationRole> = detail.messages.iter().map(|m| m.role).collect();
    assert_eq!(roles, [ConversationRole::User, ConversationRole::Assistant]);
    assert_eq!(detail.messages[1].text, "écho : issue 42");

    // L'état relayé porte le plugin et le groupe vu par lui (événement `conversations`).
    let states: Vec<ChatStatePayload> = f
        .notices
        .lock()
        .unwrap()
        .iter()
        .filter_map(|n| match n {
            ChatNotice::State(s) if s.session == sprint => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert!(states.iter().any(|s| s.state == ChatState::Running), "création : en cours");
    assert!(states.iter().all(|s| s.plugin.as_deref() == Some("gestion-projet")
        && s.group.as_ref().map(|g| g.id.as_str()) == Some("sprint:o/n:3")));
    let free_state = f.notices.lock().unwrap().iter().find_map(|n| match n {
        ChatNotice::State(s) if s.session == free => Some(s.clone()),
        _ => None,
    });
    assert_eq!(free_state.map(|s| (s.plugin, s.group)), Some((None, None)));
    f.chat.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn asks_choice_questions_and_returns_the_answers_even_in_auto_mode() {
    use cadence_lib::{
        ai::events::QuestionAnswer,
        app_settings::AiId,
        chat::{ActionStart, store::ChatMode},
    };
    let f = fixture("questions", |_| {}).await;
    let answer = |id: &str, answers: &[&str]| QuestionAnswer { id: id.into(), answers: answers.iter().map(|s| s.to_string()).collect() };
    // « Auto » (bypassPermissions) compris : la question est posée à l'utilisateur.
    for permissions in [Permissions::Ask, Permissions::Full] {
        let input = ChatSendInput { session: None, text: "question".into(), ai: None, attachments: None, permissions: Some(permissions), plugin: None };
        let id = f.chat.send(input).await.unwrap().session;
        f.wait(&id, ChatState::WaitingApproval).await;
        let (request, questions) = f
            .events(&id)
            .await
            .into_iter()
            .find_map(|e| if let AiEvent::Question { request, questions, .. } = e { Some((request, questions)) } else { None })
            .expect("carte de question");
        assert_eq!(questions.len(), 2);
        assert_eq!((questions[0].header.as_deref(), questions[1].multi_select), (Some("Format"), true));
        assert_eq!(questions[0].options[0].description.as_deref(), Some("Court"));
        assert!(!f.events(&id).await.iter().any(|e| matches!(e, AiEvent::ApprovalRequested { .. })), "pas d'approbation");

        // Réponse incomplète : refusée, la question attend toujours.
        let partial = f.chat.answer(&id, &request, vec![answer("Quel format ?", &["Résumé"])]).await;
        assert!(partial.unwrap_err().to_string().contains("Quelles sections ?"));
        f.chat
            .answer(&id, &request, vec![answer("Quelles sections ?", &["Intro", " Conclusion "]), answer("Quel format ?", &["Résumé"])])
            .await
            .unwrap();
        f.wait(&id, ChatState::Idle).await;
        assert_eq!(f.replies(&id).await, ["réponses (2 questions) : Quel format ?=Résumé | Quelles sections ?=Intro, Conclusion"]);
        let answered = f.events(&id).await.into_iter().find_map(|e| match e {
            AiEvent::QuestionAnswered { answer, answers, .. } => Some((answer, answers.len())),
            _ => None,
        });
        assert_eq!(answered, Some(("Format : Résumé · Sections : Intro, Conclusion".to_string(), 2)));
        assert!(f.chat.answer(&id, &request, vec![answer("Quel format ?", &["Détaillé"])]).await.is_err(), "déjà répondu");
    }

    // En fond, personne pour répondre : refus d'office, avec la consigne de trancher seul.
    let run = f
        .chat
        .start_action(ActionStart {
            ai: AiId::ClaudeCode,
            mode: ChatMode::Background,
            title: "Question de fond".into(),
            origin: "action:exemple/rapport".into(),
            prompt: "question".into(),
            group: None,
            hidden: false,
            context: None,
        })
        .await
        .unwrap();
    f.wait(&run, ChatState::Idle).await;
    let reply = f.replies(&run).await.join("");
    assert!(reply.starts_with("question refusée : Personne ne peut répondre"), "{reply}");
    assert!(!f.events(&run).await.iter().any(|e| matches!(e, AiEvent::Question { .. })));
    f.chat.shutdown().await;
}
