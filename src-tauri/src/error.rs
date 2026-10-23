//! Erreur unique de la couche IPC.
//!
//! Toute commande renvoie `AppResult<T>`. Côté front, l'erreur arrive sérialisée en
//! `IpcError { kind, message }` : le `kind` permet un traitement ciblé (bandeau de quota,
//! retour à l'onboarding…), le `message` est lisible tel quel.

use serde::{Serialize, Serializer};
use ts_rs::TS;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0} n'est pas encore implémenté")]
    NotImplemented(&'static str),

    /// Règle métier non respectée (ex. issue sans version).
    #[error("{0}")]
    Validation(String),

    #[error("non authentifié auprès de {0}")]
    Unauthenticated(&'static str),

    #[error("quota de l'API GitHub épuisé")]
    RateLimited,

    #[error("GitHub : {0}")]
    GitHub(String),

    #[error("aucun projet ouvert")]
    NoProject,

    /// Réseau indisponible ou GitHub injoignable : l'app reste utilisable sur le cache.
    #[error("réseau indisponible : {0}")]
    Network(String),

    /// Configuration de l'app incomplète (ex. client ID GitHub absent).
    #[error("{0}")]
    Config(String),

    /// Serveur de plugin arrêté, en échec ou injoignable.
    #[error("{0}")]
    PluginUnavailable(String),

    /// Un outil de plugin a répondu par une erreur MCP ou n'a pas répondu à temps.
    #[error("{0}")]
    ToolError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ErrorKind {
    NotImplemented,
    Validation,
    Unauthenticated,
    RateLimited,
    Github,
    NoProject,
    Network,
    Config,
    PluginUnavailable,
    ToolError,
    Io,
    Internal,
}

/// Forme sérialisée d'une `AppError`, telle que reçue par `invoke()` côté front.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct IpcError {
    pub kind: ErrorKind,
    pub message: String,
}

impl AppError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            AppError::NotImplemented(_) => ErrorKind::NotImplemented,
            AppError::Validation(_) => ErrorKind::Validation,
            AppError::Unauthenticated(_) => ErrorKind::Unauthenticated,
            AppError::RateLimited => ErrorKind::RateLimited,
            AppError::GitHub(_) => ErrorKind::Github,
            AppError::NoProject => ErrorKind::NoProject,
            AppError::Network(_) => ErrorKind::Network,
            AppError::Config(_) => ErrorKind::Config,
            AppError::PluginUnavailable(_) => ErrorKind::PluginUnavailable,
            AppError::ToolError(_) => ErrorKind::ToolError,
            AppError::Io(_) => ErrorKind::Io,
            AppError::Internal(_) => ErrorKind::Internal,
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_connect() || err.is_timeout() {
            AppError::Network(err.to_string())
        } else {
            AppError::GitHub(err.to_string())
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        IpcError { kind: self.kind(), message: self.to_string() }.serialize(serializer)
    }
}
