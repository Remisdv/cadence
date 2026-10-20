//! Adaptateur Codex CLI (plan § 4.3) : un processus `codex app-server` (JSON-RPC sur stdio) par
//! session, un fil (`thread`) par conversation.
//!
//! ```text
//! codex app-server            cwd = <données>/sessions/<id>/ ; env : CADENCE_MCP_TOKEN
//! → initialize { clientInfo, capabilities.experimentalApi } ; initialized
//! → skills/extraRoots/set { extraRoots: [<prép>/plugin/skills] }
//! → thread/start | thread/resume { cwd, approvalPolicy, sandbox, developerInstructions,
//!     config: { mcp_servers.cadence: { url, bearer_token_env_var, default_tools_approval_mode },
//!               sandbox_workspace_write.writable_roots: [<worktrees ouverts à la session>] } }
//! → turn/start { threadId, input } … turn/interrupt { threadId, turnId }
//! ← item/*, turn/*, thread/tokenUsage/updated, error ; requêtes item/*/requestApproval
//! ```
//!
//! `CODEX_HOME` n'est pas changé : la connexion de l'utilisateur (auth.json ou trousseau) reste
//! celle de Codex ; toute la configuration de Cadence passe en surcharges par fil.

use std::{
    collections::HashMap,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use secrecy::ExposeSecret;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::ChildStdin,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use super::{
    claude_code::{Permissions, SessionMode},
    codex_stream::{
        CodexItem, CodexStream, PendingCodex, approval_reply, notification, question_reply, request, text_input,
    },
    events::{AiErrorKind, AiEvent, ApprovalDecision},
    grants,
    process::{Tail, TreeChild},
    session::AiLaunch,
    staging::{self, SessionFiles},
};
use crate::error::{AppError, AppResult};

/// Délai des requêtes de mise en route (le premier lancement de Codex peut être lent).
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

type Pending = Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>;

#[derive(Default)]
struct Shared {
    pending: Pending,
    approvals: Mutex<HashMap<String, PendingCodex>>,
    turn: Mutex<Option<String>>,
    running: AtomicBool,
    alive: AtomicBool,
    killed: AtomicBool,
}

/// Politique d'approbation et bac à sable d'un profil. « Demander » : toute commande hors
/// lecture et toute modification passent par une carte (`untrusted`) ; « Édition auto » : les
/// modifications du dossier de travail passent, Codex demande pour sortir du bac à sable. En
/// fond, les demandes sont tranchées d'office par Cadence : une modification dans un worktree
/// accordé passe, le reste est refusé.
pub fn policy(permissions: Permissions, mode: SessionMode) -> (&'static str, &'static str) {
    match (permissions, mode) {
        (Permissions::Full, _) => ("never", "danger-full-access"),
        (Permissions::Ask, SessionMode::Chat) => ("untrusted", "workspace-write"),
        _ => ("on-request", "workspace-write"),
    }
}

/// Session Codex vivante.
pub struct CodexSession {
    stdin: Arc<tokio::sync::Mutex<Option<ChildStdin>>>,
    shared: Arc<Shared>,
    child: Arc<tokio::sync::Mutex<TreeChild>>,
    reader: JoinHandle<()>,
    next_id: AtomicU64,
    thread_id: String,
    files: SessionFiles,
}

async fn write_line(stdin: &tokio::sync::Mutex<Option<ChildStdin>>, value: &Value) -> AppResult<()> {
    let mut stdin = stdin.lock().await;
    let pipe = stdin.as_mut().ok_or_else(|| AppError::Internal("entrée de Codex fermée".into()))?;
    let mut line = value.to_string();
    line.push('\n');
    pipe.write_all(line.as_bytes()).await?;
    pipe.flush().await?;
    Ok(())
}

impl CodexSession {
    pub async fn start(launch: AiLaunch, events: mpsc::UnboundedSender<AiEvent>) -> AppResult<Self> {
        std::fs::create_dir_all(&launch.cwd)?;
        let mut command = super::process::command(&launch.cli);
        command.arg("app-server").current_dir(&launch.cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(token) = &launch.token {
            command.env("CADENCE_MCP_TOKEN", token.expose_secret());
        }
        let mut child = TreeChild::spawn(command).map_err(|e| AppError::Internal(format!("Codex CLI ne démarre pas : {e}")))?;
        let stdin = Arc::new(tokio::sync::Mutex::new(child.stdin()));
        let stdout = child.stdout().ok_or_else(|| AppError::Internal("sortie de Codex absente".into()))?;
        let tail = child.stderr().map(|s| Tail::follow(s, "codex")).unwrap_or_default();
        let shared = Arc::new(Shared { alive: AtomicBool::new(true), ..Shared::default() });

        let reader = {
            let (shared, stdin, events) = (shared.clone(), stdin.clone(), events.clone());
            let (granted, background) = (launch.granted.clone(), launch.mode == SessionMode::Background);
            tokio::spawn(async move {
                let mut stream = CodexStream::default();
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let items = stream.push_line(&line);
                    // Réponses d'office : modification dans un worktree accordé, ou refus en fond
                    // (personne pour répondre). Leur carte d'approbation n'est pas montrée.
                    let dirs = granted.as_ref().map(|g| g()).unwrap_or_default();
                    let mut answered = Vec::new();
                    for item in &items {
                        let CodexItem::Approval(pending) = item else { continue };
                        if !pending.questions.is_empty() {
                            continue;
                        }
                        let covered = !pending.paths.is_empty()
                            && pending.paths.iter().all(|p| grants::covering(std::path::Path::new(p), &dirs).is_some());
                        let decision = match (covered, background) {
                            (true, _) => ApprovalDecision::Allow,
                            (false, true) => ApprovalDecision::Deny,
                            (false, false) => continue,
                        };
                        answered.push(pending.request.clone());
                        if let Err(err) = write_line(&stdin, &approval_reply(pending, decision)).await {
                            tracing::warn!(%err, "réponse d'office non transmise à Codex");
                        }
                    }
                    for item in items {
                        match item {
                            CodexItem::Approval(pending) if answered.contains(&pending.request) => {}
                            CodexItem::Event(AiEvent::ApprovalRequested { request, .. }) if answered.contains(&request) => {}
                            CodexItem::Response { id, result } => {
                                if let Some(waiter) = shared.pending.lock().unwrap().remove(&id) {
                                    let _ = waiter.send(result);
                                }
                            }
                            CodexItem::Approval(pending) => {
                                shared.approvals.lock().unwrap().insert(pending.request.clone(), pending);
                            }
                            CodexItem::Reply(reply) => {
                                if let Err(err) = write_line(&stdin, &reply).await {
                                    tracing::warn!(%err, "réponse à Codex non envoyée");
                                }
                            }
                            CodexItem::Event(event) => {
                                if let AiEvent::TurnCompleted { .. } = event {
                                    shared.running.store(false, Ordering::SeqCst);
                                    shared.approvals.lock().unwrap().clear();
                                    shared.turn.lock().unwrap().take();
                                }
                                let _ = events.send(event);
                            }
                        }
                    }
                }
                shared.alive.store(false, Ordering::SeqCst);
                // Requêtes sans réponse : le processus est parti.
                for (_, waiter) in shared.pending.lock().unwrap().drain() {
                    let _ = waiter.send(Err("Codex s'est arrêté.".into()));
                }
                let killed = shared.killed.load(Ordering::SeqCst);
                if shared.running.swap(false, Ordering::SeqCst) {
                    let error = (!killed).then(|| tail.crash_message("Codex CLI"));
                    if let Some(message) = &error {
                        let _ = events.send(AiEvent::Error { kind: AiErrorKind::Crash, message: message.clone() });
                    }
                    let _ = events.send(AiEvent::TurnCompleted { ok: false, interrupted: killed, error, duration_ms: 0 });
                }
            })
        };

        let mut session = Self {
            stdin,
            shared,
            child: Arc::new(tokio::sync::Mutex::new(child)),
            reader,
            next_id: AtomicU64::new(1),
            thread_id: String::new(),
            files: launch.files,
        };
        let opened = session
            .handshake(
                &launch.cwd,
                launch.gateway_url.as_deref(),
                launch.resume.as_deref(),
                launch.model.as_deref(),
                (launch.permissions, launch.mode),
                &launch.add_dirs,
            )
            .await;
        match opened {
            Ok((thread, model)) => {
                session.thread_id = thread.clone();
                let _ = events.send(AiEvent::SessionStarted { native_id: thread, model });
                Ok(session)
            }
            Err(err) => {
                session.kill().await;
                Err(err)
            }
        }
    }

    async fn call_with(&self, method: &str, params: Value, timeout: Duration) -> AppResult<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.shared.pending.lock().unwrap().insert(id, tx);
        if let Err(err) = write_line(&self.stdin, &request(id, method, params)).await {
            self.shared.pending.lock().unwrap().remove(&id);
            return Err(err);
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(message))) => Err(AppError::Internal(format!("Codex ({method}) : {message}"))),
            Ok(Err(_)) => Err(AppError::Internal("Codex s'est arrêté.".into())),
            Err(_) => {
                self.shared.pending.lock().unwrap().remove(&id);
                Err(AppError::Internal(format!("Codex ne répond pas à {method}.")))
            }
        }
    }

    async fn call(&self, method: &str, params: Value) -> AppResult<Value> {
        self.call_with(method, params, REQUEST_TIMEOUT).await
    }

    /// Poignée de main, puis fil ouvert (repris si possible) : `(threadId, modèle)`.
    async fn handshake(
        &self,
        cwd: &std::path::Path,
        gateway_url: Option<&str>,
        resume: Option<&str>,
        model: Option<&str>,
        (permissions, mode): (Permissions, SessionMode),
        add_dirs: &[std::path::PathBuf],
    ) -> AppResult<(String, Option<String>)> {
        let version = env!("CARGO_PKG_VERSION");
        self.call_with(
            "initialize",
            json!({
                "clientInfo": { "name": "cadence", "title": "Cadence", "version": version },
                "capabilities": { "experimentalApi": true, "requestAttestation": false }
            }),
            HANDSHAKE_TIMEOUT,
        )
        .await?;
        write_line(&self.stdin, &notification("initialized")).await?;
        let skills = self.files.skills_root();
        if skills.is_dir()
            && let Err(err) = self.call("skills/extraRoots/set", json!({ "extraRoots": [skills.display().to_string()] })).await
        {
            tracing::warn!(%err, "skills de Cadence non transmis à Codex");
        }

        let (approval, sandbox) = policy(permissions, mode);
        let mut params = json!({
            "cwd": cwd.display().to_string(),
            "approvalPolicy": approval,
            "sandbox": sandbox,
            "config": staging::codex_config(gateway_url, add_dirs),
            "developerInstructions": staging::INSTRUCTIONS,
        });
        if let Some(model) = model {
            params["model"] = json!(model);
        }
        let opened = match resume {
            Some(thread) => {
                let mut resume_params = params.clone();
                resume_params["threadId"] = json!(thread);
                resume_params["excludeTurns"] = json!(true);
                match self.call_with("thread/resume", resume_params, HANDSHAKE_TIMEOUT).await {
                    Ok(value) => value,
                    Err(err) => {
                        // Fil introuvable (CODEX_HOME nettoyé…) : nouveau fil.
                        tracing::warn!(%err, thread, "fil Codex non repris");
                        self.call_with("thread/start", params, HANDSHAKE_TIMEOUT).await?
                    }
                }
            }
            None => self.call_with("thread/start", params, HANDSHAKE_TIMEOUT).await?,
        };
        let thread = opened
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Internal("Codex n'a pas ouvert de fil.".into()))?
            .to_string();
        let model = opened.get("model").and_then(Value::as_str).map(str::to_string);
        Ok((thread, model))
    }

    pub fn native_id(&self) -> Option<String> {
        Some(self.thread_id.clone())
    }

    pub fn is_alive(&self) -> bool {
        self.shared.alive.load(Ordering::SeqCst)
    }

    pub fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::SeqCst)
    }

    /// Nouveau tour du fil.
    pub async fn send(&self, text: &str) -> AppResult<()> {
        if !self.is_alive() {
            return Err(AppError::Internal("Codex CLI s'est arrêté.".into()));
        }
        self.shared.running.store(true, Ordering::SeqCst);
        let started = self.call("turn/start", json!({ "threadId": self.thread_id, "input": text_input(text) })).await;
        match started {
            Ok(value) => {
                if let Some(turn) = value.pointer("/turn/id").and_then(Value::as_str) {
                    *self.shared.turn.lock().unwrap() = Some(turn.to_string());
                }
                Ok(())
            }
            Err(err) => {
                self.shared.running.store(false, Ordering::SeqCst);
                Err(err)
            }
        }
    }

    /// Demande l'arrêt du tour en cours (il se termine en `interrupted`).
    pub async fn interrupt(&self) -> AppResult<()> {
        let turn = self.shared.turn.lock().unwrap().clone();
        let Some(turn) = turn else { return Err(AppError::Validation("Aucun tour Codex en cours.".into())) };
        self.call("turn/interrupt", json!({ "threadId": self.thread_id, "turnId": turn })).await.map(|_| ())
    }

    pub async fn answer(&self, request: &str, decision: ApprovalDecision) -> AppResult<()> {
        let pending = self
            .shared
            .approvals
            .lock()
            .unwrap()
            .remove(request)
            .ok_or_else(|| AppError::Validation("Cette demande d'approbation n'attend plus de réponse.".into()))?;
        write_line(&self.stdin, &approval_reply(&pending, decision)).await
    }

    /// Répond à une question (`item/tool/requestUserInput`).
    pub async fn answer_question(&self, request: &str, answer: &str) -> AppResult<()> {
        let pending = self
            .shared
            .approvals
            .lock()
            .unwrap()
            .remove(request)
            .filter(|p| !p.questions.is_empty())
            .ok_or_else(|| AppError::Validation("Cette question n'attend plus de réponse.".into()))?;
        write_line(&self.stdin, &question_reply(&pending, answer)).await
    }

    pub async fn kill(&self) {
        self.shared.killed.store(true, Ordering::SeqCst);
        self.child.lock().await.kill().await;
    }

    /// Ferme stdin (app-server s'arrête), laisse `grace`, puis tue le processus.
    pub async fn close(self, grace: Duration) {
        self.stdin.lock().await.take();
        let exited = {
            let child = self.child.clone();
            tokio::time::timeout(grace, async move { child.lock().await.wait().await }).await.is_ok()
        };
        if !exited {
            self.kill().await;
        }
        let _ = tokio::time::timeout(Duration::from_secs(2), self.reader).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_profiles_to_codex_policies() {
        assert_eq!(policy(Permissions::Ask, SessionMode::Chat), ("untrusted", "workspace-write"));
        assert_eq!(policy(Permissions::AutoEdit, SessionMode::Chat), ("on-request", "workspace-write"));
        assert_eq!(policy(Permissions::Ask, SessionMode::Background), ("on-request", "workspace-write"), "tranché d'office");
        assert_eq!(policy(Permissions::Full, SessionMode::Chat), ("never", "danger-full-access"));
    }
}
