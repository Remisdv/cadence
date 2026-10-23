//! Adaptateur Claude Code (plan § 4.2) : un processus `claude` vivant par session, entrée et
//! sortie stream-json avec le protocole de contrôle (interruption, approbations).
//!
//! ```text
//! claude -p --input-format stream-json --output-format stream-json --verbose
//!   --include-partial-messages --mcp-config <prép>/mcp.json --strict-mcp-config
//!   --plugin-dir <prép>/plugin --append-system-prompt-file <prép>/CADENCE.md
//!   --settings <prép>/settings.json --allowedTools mcp__cadence (Demander : les outils du cœur)
//!   --permission-mode default|acceptEdits|bypassPermissions --permission-prompt-tool stdio
//!   [--resume <id>] [--model <alias>] [--add-dir <worktree>]…
//! cwd = <données>/sessions/<id>/ ; env : CADENCE_MCP_TOKEN, sans ANTHROPIC_API_KEY
//! ```
//!
//! Chaque tour est une ligne `user` sur stdin ; les événements normalisés partent dans un
//! canal, dans l'ordre. En fond, une seule entrée puis fermeture de stdin.
//!
//! Approbations (`--permission-prompt-tool stdio`, dans tous les modes) : un outil de
//! fichiers qui vise un worktree ouvert à la session est accordé d'office (`addDirectories`) ;
//! le reste devient une carte dans le chat, ou est refusé en fond (personne pour répondre).
//!
//! Outils de la passerelle (recette du 21/10, N12) : en « Demander », seuls ceux du cœur sont
//! pré-autorisés ; un outil de plugin passe par `can_use_tool`, et le chat accorde d'office ceux
//! en lecture seule (`readOnlyHint`) et montre une carte pour ceux qui écrivent. En « Édition
//! auto », « Auto » et en fond, tout le serveur `cadence` est pré-autorisé.
//!
//! Questions à choix (`AskUserQuestion`, posées même en accès complet) : une carte de question
//! dans le chat ; les réponses repartent dans `updatedInput.answers`. En fond, la question est
//! refusée avec la consigne de trancher seul.

use std::{
    collections::{HashMap, VecDeque},
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::ChildStdin,
    sync::mpsc,
    task::JoinHandle,
};
use ts_rs::TS;

use super::{
    claude_stream::{
        ASK_USER_QUESTION, ClaudeStream, PendingApproval, StreamItem, approval_response, background_denial,
        background_question_denial, grant_response, interrupt_request, question_response, user_message,
    },
    events::{AiErrorKind, AiEvent, ApprovalDecision, QuestionAnswer},
    grants::{self, DirGrants},
    process::TreeChild,
    staging::SessionFiles,
};
use crate::error::{AppError, AppResult};

/// Profil de permissions des outils natifs de la CLI (ceux de la passerelle sont toujours
/// autorisés). C'est le « mode » d'une conversation, choisi dans le composeur du chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, rename = "ChatPermissions")]
pub enum Permissions {
    /// Chaque demande devient une carte d'approbation dans le chat.
    #[default]
    Ask,
    /// Édition de fichiers autorisée, le reste demandé (refusé en fond).
    AutoEdit,
    /// Tout autorisé.
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    Chat,
    /// Une entrée, puis stdin fermé ; personne ne répond aux approbations.
    Background,
}

/// Lancement d'une session.
pub struct ClaudeLaunch {
    pub cli: PathBuf,
    /// Dossier de travail neutre et persistant de la session.
    pub cwd: PathBuf,
    pub files: SessionFiles,
    pub token: Option<SecretString>,
    pub resume: Option<String>,
    pub model: Option<String>,
    pub mode: SessionMode,
    pub permissions: Permissions,
    /// Worktrees ouverts à la session (`--add-dir`).
    pub add_dirs: Vec<PathBuf>,
    /// Worktrees ouverts depuis le lancement, accordés d'office pendant le tour.
    pub granted: Option<DirGrants>,
}

/// Outils pré-autorisés (`--allowedTools`) : tout le serveur de la passerelle, sauf en
/// « Demander » où seuls les outils du cœur le sont (les outils des plugins passent par
/// `can_use_tool`, N12).
pub fn allowed_tools(permissions: Permissions, mode: SessionMode) -> String {
    match (permissions, mode) {
        (Permissions::Ask, SessionMode::Chat) => crate::gateway::core_tools::names()
            .iter()
            .map(|name| format!("mcp__cadence__{name}"))
            .collect::<Vec<_>>()
            .join(","),
        _ => "mcp__cadence".into(),
    }
}

/// Arguments de `claude` (hors binaire).
pub fn args(launch: &ClaudeLaunch) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--strict-mcp-config",
        "--allowedTools",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.push(allowed_tools(launch.permissions, launch.mode).into());
    let path = |flag: &str, p: &Path, args: &mut Vec<OsString>| {
        args.push(flag.into());
        args.push(p.as_os_str().to_owned());
    };
    path("--mcp-config", &launch.files.mcp_config, &mut args);
    path("--plugin-dir", &launch.files.plugin_dir, &mut args);
    path("--append-system-prompt-file", &launch.files.instructions, &mut args);
    path("--settings", &launch.files.settings, &mut args);
    let mode = match (launch.permissions, launch.mode) {
        (Permissions::Full, _) => "bypassPermissions",
        (Permissions::AutoEdit, _) | (Permissions::Ask, SessionMode::Background) => "acceptEdits",
        (Permissions::Ask, SessionMode::Chat) => "default",
    };
    args.extend(["--permission-mode".into(), mode.into()]);
    // Toujours, même en accès complet : `AskUserQuestion` (et les suppressions de chemins
    // critiques) passent par `can_use_tool` dans tous les modes ; sans outil de demande, la
    // question reviendrait vide à l'IA.
    args.extend(["--permission-prompt-tool".into(), "stdio".into()]);
    if let Some(resume) = &launch.resume {
        args.extend(["--resume".into(), resume.into()]);
    }
    if let Some(model) = &launch.model {
        args.extend(["--model".into(), model.into()]);
    }
    for dir in &launch.add_dirs {
        path("--add-dir", dir, &mut args);
    }
    args
}

/// Session Claude Code vivante.
pub struct ClaudeSession {
    stdin: Arc<tokio::sync::Mutex<Option<ChildStdin>>>,
    shared: Arc<Shared>,
    child: Arc<tokio::sync::Mutex<TreeChild>>,
    reader: JoinHandle<()>,
    next_request: AtomicU32,
    /// Fichiers de préparation, supprimés à la fin de la session.
    _files: SessionFiles,
}

#[derive(Default)]
struct Shared {
    approvals: Mutex<HashMap<String, PendingApproval>>,
    native_id: Mutex<Option<String>>,
    running: AtomicBool,
    alive: AtomicBool,
    /// Le processus a été tué par Cadence (interruption qui n'aboutit pas, fermeture).
    killed: AtomicBool,
}

const STDERR_LINES: usize = 8;

async fn write_line(stdin: &tokio::sync::Mutex<Option<ChildStdin>>, value: &Value) -> AppResult<()> {
    let mut stdin = stdin.lock().await;
    let pipe = stdin.as_mut().ok_or_else(|| AppError::Internal("entrée de Claude Code fermée".into()))?;
    let mut line = value.to_string();
    line.push('\n');
    pipe.write_all(line.as_bytes()).await?;
    pipe.flush().await?;
    Ok(())
}

impl ClaudeSession {
    /// Lance `claude` ; les événements arrivent sur `events` dans l'ordre du flux.
    pub fn start(launch: ClaudeLaunch, events: mpsc::UnboundedSender<AiEvent>) -> AppResult<Self> {
        std::fs::create_dir_all(&launch.cwd)?;
        let mut command = super::process::command(&launch.cli);
        command
            .args(args(&launch))
            .current_dir(&launch.cwd)
            .env_remove("ANTHROPIC_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(token) = &launch.token {
            command.env("CADENCE_MCP_TOKEN", token.expose_secret());
        }
        let mut child = TreeChild::spawn(command).map_err(|e| AppError::Internal(format!("Claude Code ne démarre pas : {e}")))?;
        let stdin = Arc::new(tokio::sync::Mutex::new(child.stdin()));
        let stdout = child.stdout().ok_or_else(|| AppError::Internal("sortie de Claude Code absente".into()))?;
        let stderr = child.stderr();

        let shared = Arc::new(Shared { alive: AtomicBool::new(true), ..Shared::default() });
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        if let Some(stderr) = stderr {
            let tail = tail.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(line, "stderr de Claude Code");
                    let mut tail = tail.lock().unwrap();
                    if tail.len() == STDERR_LINES {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
            });
        }

        let reader = {
            let (shared, stdin) = (shared.clone(), stdin.clone());
            let (granted, background) = (launch.granted.clone(), launch.mode == SessionMode::Background);
            tokio::spawn(async move {
                let mut stream = ClaudeStream::default();
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let items = stream.push_line(&line);
                    // Réponses d'office : worktree accordé, ou refus en fond. Leur carte
                    // d'approbation n'est pas montrée.
                    let dirs = granted.as_ref().map(|g| g()).unwrap_or_default();
                    let mut answered = Vec::new();
                    for item in &items {
                        let StreamItem::Approval(pending) = item else { continue };
                        let response = if pending.tool == ASK_USER_QUESTION {
                            if !background {
                                continue;
                            }
                            background_question_denial(pending)
                        } else {
                            let dir = grants::claude_tool_path(&pending.tool, &pending.input)
                                .and_then(|path| grants::covering(&path, &dirs));
                            match dir {
                                Some(dir) => grant_response(pending, &dir),
                                None if background => background_denial(pending),
                                None => continue,
                            }
                        };
                        answered.push(pending.request_id.clone());
                        if let Err(err) = write_line(&stdin, &response).await {
                            tracing::warn!(%err, "réponse d'office non transmise à Claude Code");
                        }
                    }
                    // Demandes gardées avant la diffusion de leur événement : le chat peut y
                    // répondre d'office dès qu'il le reçoit (accord de session, mode Auto).
                    for item in &items {
                        if let StreamItem::Approval(pending) = item
                            && !answered.contains(&pending.request_id)
                        {
                            shared.approvals.lock().unwrap().insert(pending.request_id.clone(), pending.clone());
                        }
                    }
                    for item in items {
                        match item {
                            StreamItem::Approval(_) => {}
                            StreamItem::Event(
                                AiEvent::ApprovalRequested { request, .. } | AiEvent::Question { request, .. },
                            ) if answered.contains(&request) => {}
                            StreamItem::Event(event) => {
                                match &event {
                                    AiEvent::SessionStarted { native_id, .. } => {
                                        *shared.native_id.lock().unwrap() = Some(native_id.clone());
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
                // Fin du flux : le processus s'arrête.
                shared.alive.store(false, Ordering::SeqCst);
                let killed = shared.killed.load(Ordering::SeqCst);
                if shared.running.swap(false, Ordering::SeqCst) {
                    let error = (!killed).then(|| {
                        let tail = tail.lock().unwrap();
                        if tail.is_empty() {
                            "Claude Code s'est arrêté pendant le tour.".to_string()
                        } else {
                            format!("Claude Code s'est arrêté pendant le tour : {}", tail.iter().cloned().collect::<Vec<_>>().join(" ⏎ "))
                        }
                    });
                    if let Some(message) = &error {
                        let _ = events.send(AiEvent::Error { kind: AiErrorKind::Crash, message: message.clone() });
                    }
                    let _ = events.send(AiEvent::TurnCompleted { ok: false, interrupted: killed, error, duration_ms: 0 });
                }
            })
        };

        Ok(Self {
            stdin,
            shared,
            child: Arc::new(tokio::sync::Mutex::new(child)),
            reader,
            next_request: AtomicU32::new(1),
            _files: launch.files,
        })
    }

    pub fn native_id(&self) -> Option<String> {
        self.shared.native_id.lock().unwrap().clone()
    }

    pub fn is_alive(&self) -> bool {
        self.shared.alive.load(Ordering::SeqCst)
    }

    /// Un tour est en cours.
    pub fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::SeqCst)
    }

    async fn write(&self, value: &Value) -> AppResult<()> {
        write_line(&self.stdin, value).await
    }

    /// Nouveau tour (mis en file par la CLI si un tour est en cours).
    pub async fn send(&self, text: &str) -> AppResult<()> {
        if !self.is_alive() {
            return Err(AppError::Internal("Claude Code s'est arrêté.".into()));
        }
        self.shared.running.store(true, Ordering::SeqCst);
        self.write(&user_message(text)).await.inspect_err(|_| self.shared.running.store(false, Ordering::SeqCst))
    }

    /// Fin des entrées (mode fond) : la CLI termine le tour puis s'arrête.
    pub async fn close_input(&self) {
        self.stdin.lock().await.take();
    }

    /// Demande l'arrêt du tour en cours.
    pub async fn interrupt(&self) -> AppResult<()> {
        let id = format!("cadence_{}", self.next_request.fetch_add(1, Ordering::SeqCst));
        self.write(&interrupt_request(&id)).await
    }

    /// Demande en attente de ce type (approbation, ou question `AskUserQuestion`).
    fn take_pending(&self, request: &str, question: bool) -> Option<PendingApproval> {
        let mut approvals = self.shared.approvals.lock().unwrap();
        let matches = approvals.get(request).is_some_and(|p| (p.tool == ASK_USER_QUESTION) == question);
        matches.then(|| approvals.remove(request)).flatten()
    }

    /// Répond à une demande d'approbation.
    pub async fn answer(&self, request: &str, decision: ApprovalDecision) -> AppResult<()> {
        let pending = self
            .take_pending(request, false)
            .ok_or_else(|| AppError::Validation("Cette demande d'approbation n'attend plus de réponse.".into()))?;
        self.write(&approval_response(&pending, decision)).await
    }

    /// Répond à un `AskUserQuestion` : les réponses rejoignent l'entrée de l'outil (`answers`).
    pub async fn answer_question(&self, request: &str, answers: &[QuestionAnswer]) -> AppResult<()> {
        let pending = self
            .take_pending(request, true)
            .ok_or_else(|| AppError::Validation("Cette question n'attend plus de réponse.".into()))?;
        self.write(&question_response(&pending, answers)).await
    }

    /// Tue le processus (le tour en cours est marqué interrompu).
    pub async fn kill(&self) {
        self.shared.killed.store(true, Ordering::SeqCst);
        self.child.lock().await.kill().await;
    }

    /// Ferme stdin, laisse `grace` à la CLI pour finir, puis la tue.
    pub async fn close(self, grace: Duration) {
        self.close_input().await;
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

    fn launch(permissions: Permissions, mode: SessionMode, resume: Option<&str>) -> ClaudeLaunch {
        let root = crate::app_settings::temp_dir(&format!("claude-args-{permissions:?}-{mode:?}"));
        ClaudeLaunch {
            cli: "claude".into(),
            cwd: root.join("cwd"),
            files: super::super::staging::stage(&root, "s", None, &[]).unwrap(),
            token: None,
            resume: resume.map(str::to_string),
            model: Some("haiku".into()),
            mode,
            permissions,
            add_dirs: Vec::new(),
            granted: None,
        }
    }

    fn strings(launch: &ClaudeLaunch) -> Vec<String> {
        args(launch).into_iter().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn builds_the_command_line_per_profile() {
        let chat = strings(&launch(Permissions::Ask, SessionMode::Chat, Some("abc")));
        let after = |flag: &str| chat.iter().position(|a| a == flag).map(|i| chat[i + 1].as_str());
        assert_eq!(after("--permission-mode"), Some("default"));
        assert_eq!(after("--permission-prompt-tool"), Some("stdio"));
        assert_eq!(after("--resume"), Some("abc"));
        assert_eq!(after("--model"), Some("haiku"));
        // « Demander » : seuls les outils du cœur ; ceux des plugins passent par `can_use_tool`.
        let allowed: Vec<&str> = after("--allowedTools").unwrap().split(',').collect();
        assert!(allowed.contains(&"mcp__cadence__cadence__run_report") && allowed.contains(&"mcp__cadence__cadence__repo_checkout"));
        assert!(allowed.iter().all(|t| t.starts_with("mcp__cadence__cadence__")), "{allowed:?}");
        assert!(after("--mcp-config").unwrap().ends_with("mcp.json"));
        assert!(chat.contains(&"--strict-mcp-config".to_string()));
        assert!(!chat.iter().any(|a| a.contains("CADENCE_MCP_TOKEN") && !a.contains("${")), "jeton jamais en argument");

        let background = strings(&launch(Permissions::Ask, SessionMode::Background, None));
        assert!(background.windows(2).any(|w| w == ["--permission-prompt-tool", "stdio"]), "réponses d'office en fond");
        assert!(background.windows(2).any(|w| w == ["--permission-mode", "acceptEdits"]));
        assert!(background.windows(2).any(|w| w == ["--allowedTools", "mcp__cadence"]), "personne pour répondre en fond");
        let auto_edit = strings(&launch(Permissions::AutoEdit, SessionMode::Chat, None));
        assert!(auto_edit.windows(2).any(|w| w == ["--permission-mode", "acceptEdits"]));
        assert!(auto_edit.windows(2).any(|w| w == ["--allowedTools", "mcp__cadence"]), "outils des plugins sans accord");
        assert!(auto_edit.windows(2).any(|w| w == ["--permission-prompt-tool", "stdio"]), "le shell reste demandé");
        let full = strings(&launch(Permissions::Full, SessionMode::Chat, None));
        assert!(full.windows(2).any(|w| w == ["--permission-mode", "bypassPermissions"]));
        assert!(full.windows(2).any(|w| w == ["--allowedTools", "mcp__cadence"]));
        assert!(full.windows(2).any(|w| w == ["--permission-prompt-tool", "stdio"]), "les questions passent même en accès complet");
        assert!(!full.contains(&"--add-dir".to_string()));

        let mut granted = launch(Permissions::Ask, SessionMode::Chat, None);
        granted.add_dirs = vec![PathBuf::from("/repos/.worktrees/me/app/issue-42"), PathBuf::from("/repos/me/site")];
        let granted = strings(&granted);
        let dirs: Vec<&str> = granted.windows(2).filter(|w| w[0] == "--add-dir").map(|w| w[1].as_str()).collect();
        assert_eq!(dirs.len(), 2);
        assert!(dirs[0].ends_with("issue-42") && dirs[1].ends_with("site"));
    }
}
