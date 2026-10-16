//! Événements poussés du back vers le front.
//!
//! Les noms sont dupliqués dans `src/lib/ipc/events.ts` (ts-rs ne génère que les charges).
//! Les vues ne relisent jamais GitHub : elles invalident leur cache TanStack Query
//! sur `sync://updated`.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use crate::domain::{
    auth::{AuthFailure, AuthProvider, ProviderStatus},
    sync::{RateLimitMode, SyncResource},
};

pub const SYNC_UPDATED: &str = "sync://updated";
pub const SYNC_RATE_LIMIT: &str = "sync://rate-limit";
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
/// un front fermé ne doit pas faire échouer la sync.
pub fn emit<P: Serialize + Clone>(app: &AppHandle, name: &str, payload: P) {
    if let Err(err) = app.emit(name, payload) {
        tracing::warn!(event = name, %err, "émission d'événement impossible");
    }
}
