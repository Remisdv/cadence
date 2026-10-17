//! Contrat commun des adaptateurs (plan § 4.1) : une session d'IA vivante, quelle que soit la
//! CLI. Le chat ne connaît que `AiSession` : démarrer (reprise par `native_id`), envoyer un
//! tour, interrompre, répondre à une approbation, fermer.

use std::{path::PathBuf, time::Duration};

use secrecy::SecretString;
use tokio::sync::mpsc;

use super::{
    claude_code::{ClaudeLaunch, ClaudeSession, Permissions, SessionMode},
    codex::CodexSession,
    events::{AiEvent, ApprovalDecision},
    opencode::OpencodeSession,
    staging::SessionFiles,
};
use crate::{app_settings::AiId, error::AppResult};

/// Lancement d'une session, commun aux trois CLI.
pub struct AiLaunch {
    pub ai: AiId,
    pub cli: PathBuf,
    /// Dossier de travail neutre et persistant de la session.
    pub cwd: PathBuf,
    pub files: SessionFiles,
    /// Passerelle MCP (`http://127.0.0.1:<port>/mcp`), si elle tourne.
    pub gateway_url: Option<String>,
    pub token: Option<SecretString>,
    /// Identifiant natif à reprendre (session Claude, fil Codex, session OpenCode).
    pub resume: Option<String>,
    pub model: Option<String>,
    pub mode: SessionMode,
    pub permissions: Permissions,
    /// Titre de la conversation (OpenCode le garde avec sa session).
    pub title: String,
    /// Dossiers ouverts à la session en plus de son dossier neutre (worktrees du service
    /// Dépôts) : `--add-dir`, racine inscriptible de Codex, permission de chemin d'OpenCode.
    pub add_dirs: Vec<PathBuf>,
}

/// Session vivante d'une des trois CLI.
pub enum AiSession {
    Claude(ClaudeSession),
    Codex(CodexSession),
    Opencode(OpencodeSession),
}

macro_rules! each {
    ($self:ident, $s:ident => $body:expr) => {
        match $self {
            AiSession::Claude($s) => $body,
            AiSession::Codex($s) => $body,
            AiSession::Opencode($s) => $body,
        }
    };
}

impl AiSession {
    /// Lance la CLI ; les événements arrivent sur `events`, dans l'ordre. Codex et OpenCode
    /// ont fini leur poignée de main au retour (fil ou session ouverts).
    pub async fn start(launch: AiLaunch, events: mpsc::UnboundedSender<AiEvent>) -> AppResult<Self> {
        Ok(match launch.ai {
            AiId::ClaudeCode => AiSession::Claude(ClaudeSession::start(
                ClaudeLaunch {
                    cli: launch.cli,
                    cwd: launch.cwd,
                    files: launch.files,
                    token: launch.token,
                    resume: launch.resume,
                    model: launch.model,
                    mode: launch.mode,
                    permissions: launch.permissions,
                    add_dirs: launch.add_dirs,
                },
                events,
            )?),
            AiId::Codex => AiSession::Codex(CodexSession::start(launch, events).await?),
            AiId::Opencode => AiSession::Opencode(OpencodeSession::start(launch, events).await?),
        })
    }

    pub fn native_id(&self) -> Option<String> {
        each!(self, s => s.native_id())
    }

    pub fn is_alive(&self) -> bool {
        each!(self, s => s.is_alive())
    }

    /// Un tour est en cours.
    pub fn is_running(&self) -> bool {
        each!(self, s => s.is_running())
    }

    pub async fn send(&self, text: &str) -> AppResult<()> {
        each!(self, s => s.send(text).await)
    }

    pub async fn interrupt(&self) -> AppResult<()> {
        each!(self, s => s.interrupt().await)
    }

    pub async fn answer(&self, request: &str, decision: ApprovalDecision) -> AppResult<()> {
        each!(self, s => s.answer(request, decision).await)
    }

    pub async fn answer_question(&self, request: &str, answer: &str) -> AppResult<()> {
        each!(self, s => s.answer_question(request, answer).await)
    }

    /// Tue le processus (le tour en cours est marqué interrompu).
    pub async fn kill(&self) {
        each!(self, s => s.kill().await)
    }

    /// Fermeture propre, puis arrêt forcé après `grace`.
    pub async fn close(self, grace: Duration) {
        each!(self, s => s.close(grace).await)
    }
}
