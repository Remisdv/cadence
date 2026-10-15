//! Événements poussés du back vers le front.
//!
//! Les noms sont dupliqués dans `src/lib/ipc/events.ts` (ts-rs ne génère que les charges).
//! Les vues ne relisent jamais GitHub : elles invalident leur cache TanStack Query
//! sur `sync://updated`, et les écrans d'agent s'abonnent à `agent://*`.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use crate::domain::{
    agent::RunState,
    auth::{AuthFailure, AuthProvider, ProviderStatus},
    sync::{RateLimitMode, SyncResource},
};

pub const SYNC_UPDATED: &str = "sync://updated";
pub const SYNC_RATE_LIMIT: &str = "sync://rate-limit";
pub const AGENT_EVENT: &str = "agent://event";
pub const AGENT_STATE: &str = "agent://state";
pub const AUTH_CHANGED: &str = "auth://changed";

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncUpdated {
    pub resource: SyncResource,
    /// Numéros touchés ; vide = toute la ressource.
    pub ids: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RateLimitInfo {
    pub remaining: u32,
    pub reset_at: String,
    pub mode: RateLimitMode,
}

/// Ligne du flux d'une session d'agent (stream-json de Claude Code, normalisé).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentEvent {
    pub run: String,
    pub kind: AgentEventKind,
    pub text: String,
    /// Charge brute de l'événement, pour les écrans qui veulent le détail.
    #[ts(type = "unknown")]
    pub payload: serde_json::Value,
}

/// Nature d'une ligne de console — détermine sa couleur dans le terminal d'agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AgentEventKind {
    System,
    Read,
    Write,
    Message,
    Success,
    Question,
    Error,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentStateEvent {
    pub run: String,
    pub state: RunState,
    /// Étape courante (1-indexée) et nombre d'étapes, pour les agents à déroulé.
    pub step: Option<u8>,
    pub steps: Option<u8>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthChanged {
    pub provider: AuthProvider,
    pub status: ProviderStatus,
    pub reason: Option<AuthFailure>,
    /// Message lisible, pour une erreur.
    pub message: Option<String>,
}

impl AuthChanged {
    pub fn github(status: ProviderStatus) -> Self {
        Self { provider: AuthProvider::Github, status, reason: None, message: None }
    }

    pub fn github_failed(reason: AuthFailure, message: Option<String>) -> Self {
        Self { provider: AuthProvider::Github, status: ProviderStatus::Error, reason: Some(reason), message }
    }
}

/// Émet un événement en journalisant l'échec plutôt que de le propager :
/// un front fermé ne doit pas faire échouer la sync ou un agent.
pub fn emit<P: Serialize + Clone>(app: &AppHandle, name: &str, payload: P) {
    if let Err(err) = app.emit(name, payload) {
        tracing::warn!(event = name, %err, "émission d'événement impossible");
    }
}
