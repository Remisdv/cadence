use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AuthProvider {
    Github,
    ClaudeCode,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ProviderStatus {
    Connected,
    /// GitHub : aucun jeton. Agent de code : désactivé par l'utilisateur.
    Disconnected,
    /// Agent de code détecté, session de test pas encore lancée.
    Pending,
    /// Binaire CLI introuvable.
    NotFound,
    /// Jeton refusé, trousseau inaccessible, ou session de test en échec.
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GithubAuth {
    pub status: ProviderStatus,
    pub login: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    /// Portées du jeton (vide pour un jeton à permissions fines).
    pub scopes: Vec<String>,
}

impl GithubAuth {
    pub fn disconnected() -> Self {
        Self { status: ProviderStatus::Disconnected, login: None, name: None, avatar_url: None, scopes: vec![] }
    }
}

/// Résultat de la session de test d'un agent de code.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CliTest {
    pub ok: bool,
    /// Durée totale, démarrage du binaire compris.
    pub latency_ms: u32,
    /// Message d'erreur renvoyé par le CLI, en cas d'échec.
    pub message: Option<String>,
}

/// Agent de code local (Claude Code, Codex CLI) détecté sur la machine.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CliAgentInfo {
    pub status: ProviderStatus,
    pub binary_path: Option<String>,
    pub version: Option<String>,
    /// Ex. « Compte Claude », « Clé API ».
    pub auth_method: Option<String>,
    pub test: Option<CliTest>,
}

impl CliAgentInfo {
    pub fn not_found() -> Self {
        Self { status: ProviderStatus::NotFound, binary_path: None, version: None, auth_method: None, test: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthStatus {
    pub github: GithubAuth,
    pub claude_code: CliAgentInfo,
    pub codex: CliAgentInfo,
}

/// Réponse de `auth_start_device_flow` ; le polling du jeton continue côté Rust
/// et se termine par un événement `auth://changed`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceFlowStart {
    pub user_code: String,
    pub verification_uri: String,
    /// Secondes avant expiration du code.
    pub expires_in: u32,
}

/// Raison d'échec d'une connexion GitHub, portée par `auth://changed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AuthFailure {
    /// Le code d'appairage a expiré avant autorisation.
    Expired,
    /// L'utilisateur a refusé l'autorisation sur github.com.
    Denied,
    Error,
}
