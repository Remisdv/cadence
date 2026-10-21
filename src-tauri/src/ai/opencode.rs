//! Adaptateur OpenCode (plan § 4.4) : un serveur `opencode serve` par session (révision 1.1 :
//! plus d'espaces, chaque conversation a son dossier neutre, ses skills et son jeton de
//! passerelle), piloté en HTTP, événements lus sur le flux SSE `GET /event`.
//!
//! ```text
//! opencode serve --port 0 --hostname 127.0.0.1     cwd = <données>/sessions/<id>/
//!   env : OPENCODE_SERVER_PASSWORD (aléatoire, authentification Basic `opencode:<mot de passe>`),
//!         OPENCODE_CONFIG_CONTENT (passerelle, permissions, consignes, skills), CADENCE_MCP_TOKEN
//! POST /session { title } | GET /session/:id (reprise)
//! POST /session/:id/prompt_async { parts } ; POST /session/:id/abort
//! POST /permission/:id/reply { reply: once | always | reject }
//! ```

use std::{
    collections::HashSet,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use secrecy::ExposeSecret;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use super::{
    claude_code::{Permissions, SessionMode},
    events::{AiErrorKind, AiEvent, ApprovalDecision, QuestionAnswer},
    grants::{self, DirGrants},
    opencode_stream::{OpencodeStream, sse_data},
    process::{Tail, TreeChild, random_secret},
    session::AiLaunch,
    staging::{self, OpencodePermissions, SessionFiles},
};
use crate::error::{AppError, AppResult};

const START_TIMEOUT: Duration = Duration::from_secs(45);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const USER: &str = "opencode";

#[derive(Default)]
struct Shared {
    approvals: Mutex<HashSet<String>>,
    questions: Mutex<HashSet<String>>,
    running: AtomicBool,
    alive: AtomicBool,
    killed: AtomicBool,
}

/// Règles `edit` / `bash` / `cadence_*` d'un profil. Les outils de la passerelle ne sont demandés
/// qu'en « Demander » : le chat accorde d'office ceux du cœur et ceux des plugins en lecture
/// seule, et montre une carte pour ceux qui écrivent (N12). En fond, personne ne répond : ce qui
/// demanderait une approbation est refusé.
pub fn permissions(permissions: Permissions, mode: SessionMode) -> OpencodePermissions {
    match (permissions, mode) {
        (Permissions::Full, _) => OpencodePermissions { edit: "allow", bash: "allow", mcp: "allow" },
        (Permissions::AutoEdit, SessionMode::Chat) => OpencodePermissions { edit: "allow", bash: "ask", mcp: "allow" },
        (Permissions::Ask, SessionMode::Chat) => OpencodePermissions { edit: "ask", bash: "ask", mcp: "ask" },
        (_, SessionMode::Background) => OpencodePermissions { edit: "allow", bash: "deny", mcp: "allow" },
    }
}

/// Session OpenCode vivante.
pub struct OpencodeSession {
    http: reqwest::Client,
    base: String,
    password: String,
    session_id: String,
    shared: Arc<Shared>,
    child: Arc<tokio::sync::Mutex<TreeChild>>,
    events_task: JoinHandle<()>,
    _files: SessionFiles,
}

impl OpencodeSession {
    pub async fn start(launch: AiLaunch, events: mpsc::UnboundedSender<AiEvent>) -> AppResult<Self> {
        let granted = launch.granted.clone();
        std::fs::create_dir_all(&launch.cwd)?;
        let password = random_secret();
        let config = staging::opencode_config(
            launch.gateway_url.as_deref(),
            &launch.files,
            permissions(launch.permissions, launch.mode),
            &launch.add_dirs,
        );
        let mut command = super::process::command(&launch.cli);
        command
            .args(["serve", "--port", "0", "--hostname", "127.0.0.1"])
            .current_dir(&launch.cwd)
            .env("OPENCODE_SERVER_PASSWORD", &password)
            .env("OPENCODE_CONFIG_CONTENT", config.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(token) = &launch.token {
            command.env("CADENCE_MCP_TOKEN", token.expose_secret());
        }
        let mut child = TreeChild::spawn(command).map_err(|e| AppError::Internal(format!("OpenCode ne démarre pas : {e}")))?;
        let stdout = child.stdout().ok_or_else(|| AppError::Internal("sortie d'OpenCode absente".into()))?;
        let tail = child.stderr().map(|s| Tail::follow(s, "opencode")).unwrap_or_default();
        let shared = Arc::new(Shared { alive: AtomicBool::new(true), ..Shared::default() });
        let child = Arc::new(tokio::sync::Mutex::new(child));

        // Adresse annoncée sur la sortie standard, puis lecture jusqu'à l'arrêt du serveur.
        let (address_tx, address_rx) = oneshot::channel::<String>();
        {
            let (shared, tail, events) = (shared.clone(), tail.clone(), events.clone());
            tokio::spawn(async move {
                let mut address_tx = Some(address_tx);
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(at) = line.find("http://127.0.0.1:")
                        && let Some(tx) = address_tx.take()
                    {
                        let address = line[at..].split_whitespace().next().unwrap_or_default().to_string();
                        let _ = tx.send(address);
                    } else {
                        tail.push(line);
                    }
                }
                shared.alive.store(false, Ordering::SeqCst);
                let killed = shared.killed.load(Ordering::SeqCst);
                if shared.running.swap(false, Ordering::SeqCst) {
                    let error = (!killed).then(|| tail.crash_message("OpenCode"));
                    if let Some(message) = &error {
                        let _ = events.send(AiEvent::Error { kind: AiErrorKind::Crash, message: message.clone() });
                    }
                    let _ = events.send(AiEvent::TurnCompleted { ok: false, interrupted: killed, error, duration_ms: 0 });
                }
            });
        }
        let kill = |child: Arc<tokio::sync::Mutex<TreeChild>>| async move {
            child.lock().await.kill().await;
        };
        let base = match tokio::time::timeout(START_TIMEOUT, address_rx).await {
            Ok(Ok(address)) => address.trim_end_matches('/').to_string(),
            _ => {
                kill(child.clone()).await;
                return Err(AppError::Internal(format!(
                    "OpenCode n'a pas démarré son serveur ({}).",
                    tail.crash_message("OpenCode")
                )));
            }
        };
        let http = reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(|e| AppError::Internal(format!("client HTTP d'OpenCode : {e}")))?;

        let mut session = Self {
            http,
            base,
            password,
            session_id: String::new(),
            shared,
            child,
            events_task: tokio::spawn(async {}),
            _files: launch.files,
        };
        match session.open(launch.resume.as_deref(), &launch.title).await {
            Ok(id) => session.session_id = id,
            Err(err) => {
                session.kill().await;
                return Err(err);
            }
        }

        // Flux d'événements : branché avant le premier message pour ne rien perdre.
        let (ready_tx, ready_rx) = oneshot::channel();
        session.events_task = tokio::spawn(follow_events(
            session.http.clone(),
            session.base.clone(),
            session.password.clone(),
            session.session_id.clone(),
            session.shared.clone(),
            (events.clone(), granted),
            ready_tx,
        ));
        if tokio::time::timeout(REQUEST_TIMEOUT, ready_rx).await.is_err() {
            session.kill().await;
            return Err(AppError::Internal("Le flux d'événements d'OpenCode ne répond pas.".into()));
        }
        let _ = events.send(AiEvent::SessionStarted { native_id: session.session_id.clone(), model: None });
        Ok(session)
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.base))
            .basic_auth(USER, Some(&self.password))
            .timeout(REQUEST_TIMEOUT)
    }

    async fn send_json(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> AppResult<Option<Value>> {
        let mut request = self.request(method, path);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|e| AppError::Internal(format!("OpenCode ne répond pas : {e}")))?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AppError::Internal(format!("OpenCode ({path}) : {status} {}", text.chars().take(300).collect::<String>())));
        }
        Ok(serde_json::from_str(&text).ok())
    }

    /// Session reprise si elle existe encore, sinon créée.
    async fn open(&self, resume: Option<&str>, title: &str) -> AppResult<String> {
        if let Some(id) = resume {
            match self.send_json(reqwest::Method::GET, &format!("/session/{id}"), None).await {
                Ok(Some(_)) => return Ok(id.to_string()),
                _ => tracing::warn!(session = id, "session OpenCode non reprise"),
            }
        }
        let created = self.send_json(reqwest::Method::POST, "/session", Some(json!({ "title": title }))).await?;
        created
            .as_ref()
            .and_then(|v| v.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| AppError::Internal("OpenCode n'a pas créé de session.".into()))
    }

    pub fn native_id(&self) -> Option<String> {
        Some(self.session_id.clone())
    }

    pub fn is_alive(&self) -> bool {
        self.shared.alive.load(Ordering::SeqCst)
    }

    pub fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::SeqCst)
    }

    pub async fn send(&self, text: &str) -> AppResult<()> {
        if !self.is_alive() {
            return Err(AppError::Internal("OpenCode s'est arrêté.".into()));
        }
        self.shared.running.store(true, Ordering::SeqCst);
        let body = json!({ "parts": [{ "type": "text", "text": text }] });
        self.send_json(reqwest::Method::POST, &format!("/session/{}/prompt_async", self.session_id), Some(body))
            .await
            .map(|_| ())
            .inspect_err(|_| self.shared.running.store(false, Ordering::SeqCst))
    }

    pub async fn interrupt(&self) -> AppResult<()> {
        self.send_json(reqwest::Method::POST, &format!("/session/{}/abort", self.session_id), None).await.map(|_| ())
    }

    pub async fn answer(&self, request: &str, decision: ApprovalDecision) -> AppResult<()> {
        if !self.shared.approvals.lock().unwrap().remove(request) {
            return Err(AppError::Validation("Cette demande d'approbation n'attend plus de réponse.".into()));
        }
        let reply = match decision {
            ApprovalDecision::Allow => "once",
            ApprovalDecision::AllowSession => "always",
            ApprovalDecision::Deny => "reject",
        };
        self.send_json(reqwest::Method::POST, &format!("/permission/{request}/reply"), Some(json!({ "reply": reply })))
            .await
            .map(|_| ())
    }

    /// Répond à une question (`question.asked`) : une liste de réponses par question, dans
    /// l'ordre des questions posées.
    pub async fn answer_question(&self, request: &str, answers: &[QuestionAnswer]) -> AppResult<()> {
        if !self.shared.questions.lock().unwrap().remove(request) {
            return Err(AppError::Validation("Cette question n'attend plus de réponse.".into()));
        }
        let body = json!({ "answers": answers.iter().map(|a| a.answers.clone()).collect::<Vec<_>>() });
        self.send_json(reqwest::Method::POST, &format!("/question/{request}/reply"), Some(body)).await.map(|_| ())
    }

    pub async fn kill(&self) {
        self.shared.killed.store(true, Ordering::SeqCst);
        self.events_task.abort();
        self.child.lock().await.kill().await;
    }

    /// Le serveur n'a pas d'état à sauver : arrêt direct (la session reste reprenable).
    pub async fn close(self, _grace: Duration) {
        self.kill().await;
        let _ = tokio::time::timeout(Duration::from_secs(2), self.child.lock().await.wait()).await;
    }
}

/// Lit `GET /event` et pousse les événements de la session ; `ready` part au premier événement.
/// Une demande `external_directory` qui vise un worktree ouvert à la session est accordée
/// d'office (`always`), sans carte d'approbation.
async fn follow_events(
    http: reqwest::Client,
    base: String,
    password: String,
    session: String,
    shared: Arc<Shared>,
    (events, granted): (mpsc::UnboundedSender<AiEvent>, Option<DirGrants>),
    ready: oneshot::Sender<()>,
) {
    let mut ready = Some(ready);
    let url = format!("{base}/event");
    let response = match http.get(&url).basic_auth(USER, Some(&password)).header("accept", "text/event-stream").send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            tracing::warn!(status = %r.status(), "flux d'OpenCode refusé");
            return;
        }
        Err(err) => {
            tracing::warn!(%err, "flux d'OpenCode injoignable");
            return;
        }
    };
    let mut response = response;
    let mut stream = OpencodeStream::new(&session);
    let mut buffer: Vec<u8> = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        buffer.extend_from_slice(&chunk);
        while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buffer.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            let Some(event) = sse_data(line.trim_end()) else { continue };
            if let Some(tx) = ready.take() {
                let _ = tx.send(());
            }
            for event in stream.push(&event) {
                match &event {
                    AiEvent::ApprovalRequested { request, detail, .. }
                        if granted.as_ref().is_some_and(|g| grants::opencode_covered(detail, &g())) =>
                    {
                        let reply = http
                            .post(format!("{base}/permission/{request}/reply"))
                            .basic_auth(USER, Some(&password))
                            .json(&serde_json::json!({ "reply": "always" }))
                            .send()
                            .await;
                        match reply {
                            Ok(r) if r.status().is_success() => continue,
                            Ok(r) => tracing::warn!(status = %r.status(), "accès au worktree non accordé à OpenCode"),
                            Err(err) => tracing::warn!(%err, "accès au worktree non accordé à OpenCode"),
                        }
                        shared.approvals.lock().unwrap().insert(request.clone());
                    }
                    AiEvent::ApprovalRequested { request, .. } => {
                        shared.approvals.lock().unwrap().insert(request.clone());
                    }
                    AiEvent::Question { request, .. } => {
                        shared.questions.lock().unwrap().insert(request.clone());
                    }
                    AiEvent::TurnCompleted { .. } => {
                        shared.running.store(false, Ordering::SeqCst);
                        shared.approvals.lock().unwrap().clear();
                    }
                    _ => {}
                }
                let _ = events.send(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_profiles_to_opencode_rules() {
        assert_eq!(permissions(Permissions::Ask, SessionMode::Chat), OpencodePermissions { edit: "ask", bash: "ask", mcp: "ask" });
        assert_eq!(permissions(Permissions::AutoEdit, SessionMode::Chat), OpencodePermissions { edit: "allow", bash: "ask", mcp: "allow" });
        assert_eq!(permissions(Permissions::Ask, SessionMode::Background).mcp, "allow", "personne pour répondre en fond");
        assert_eq!(permissions(Permissions::Ask, SessionMode::Background).bash, "deny");
        assert_eq!(permissions(Permissions::Full, SessionMode::Chat).bash, "allow");
    }
}
