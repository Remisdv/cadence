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

/// Moyens de se connecter à GitHub disponibles sur cette machine.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GithubSources {
    /// Compte actif de la CLI GitHub (`gh`), si elle est connectée.
    pub gh_cli: Option<String>,
    /// Git Credential Manager : connexion par navigateur sans OAuth App propre à Cadence.
    pub credential_manager: bool,
    /// OAuth App de Cadence configurée (client ID + secret) : flux web direct.
    pub oauth_app: bool,
    /// Client ID configuré : code d'appairage (device flow).
    pub device_flow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthStatus {
    pub github: GithubAuth,
    pub github_sources: GithubSources,
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

/// Réponse de `auth_start_web_flow` : la page d'autorisation, déjà ouverte dans le navigateur.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WebFlowStart {
    /// Pour rouvrir la page si l'onglet a été fermé (`None` : c'est Git Credential Manager
    /// qui pilote le navigateur).
    pub authorize_url: Option<String>,
}

/// Raison d'échec d'une connexion GitHub, portée par `auth://changed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AuthFailure {
    /// Code d'appairage expiré, ou navigateur sans retour dans le délai.
    Expired,
    /// L'utilisateur a refusé l'autorisation sur github.com.
    Denied,
    Error,
}
