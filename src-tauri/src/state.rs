//! État partagé, injecté dans les commandes par `tauri::State<AppState>`.

use std::{path::Path, sync::Arc, sync::Mutex};

use secrecy::SecretString;
use tauri::async_runtime::JoinHandle;
use tokio::sync::RwLock;

use crate::{
    api::github::{self, issues::IssueCache},
    auth::{cli::CliTool, token_store::TokenStore},
    domain::{
        auth::{CliAgentInfo, GithubAuth, GithubSources},
        project::{ProjectState, Repo},
        version::Version,
    },
    error::{AppError, AppResult},
    local_settings::SettingsStore,
};

pub struct AppState {
    /// Projet ouvert ; `None` tant que l'onboarding n'est pas terminé.
    pub project: RwLock<Option<ProjectState>>,
    pub http: reqwest::Client,
    pub tokens: Arc<dyn TokenStore>,
    /// Compte GitHub connecté, une fois validé auprès de l'API.
    pub github: RwLock<Option<GithubAuth>>,
    /// Connexion GitHub en cours (device flow ou navigateur) : une seule à la fois.
    pub sign_in: Mutex<Option<JoinHandle<()>>>,
    /// Détection des agents de code, gardée pour la session.
    pub cli: RwLock<CliCache>,
    pub settings: SettingsStore,
    /// Dépôts accessibles au jeton, lus une fois puis filtrés localement.
    pub repos: RwLock<Option<Vec<Repo>>>,
    /// Versions du projet ouvert (en attendant le cache SQLite).
    pub versions: RwLock<Vec<Version>>,
    /// Issues du dépôt ouvert (lecture GitHub avec ETag, rafraîchie au plus toutes les 60 s).
    pub issues: RwLock<Option<IssueCache>>,
    /// Moyens de connexion GitHub détectés (gh, GCM), gardés pour la session.
    pub github_sources: RwLock<Option<GithubSources>>,
}

impl AppState {
    pub fn new(config_dir: &Path, tokens: Arc<dyn TokenStore>) -> Self {
        Self {
            project: RwLock::default(),
            http: github::http_client(),
            tokens,
            github: RwLock::default(),
            sign_in: Mutex::default(),
            cli: RwLock::default(),
            settings: SettingsStore::load(config_dir),
            repos: RwLock::default(),
            versions: RwLock::default(),
            github_sources: RwLock::default(),
            issues: RwLock::default(),
        }
    }

    pub async fn require_project(&self) -> AppResult<ProjectState> {
        self.project.read().await.clone().ok_or(AppError::NoProject)
    }

    /// Oublie tout ce qui appartient au dépôt ouvert (changement de dépôt ou de compte).
    pub async fn clear_project_data(&self) {
        self.versions.write().await.clear();
        *self.issues.write().await = None;
    }

    /// Jeton GitHub du trousseau, lu au moment de la requête.
    pub fn github_token(&self) -> AppResult<SecretString> {
        self.tokens.get()?.ok_or(AppError::Unauthenticated("GitHub"))
    }

    /// Remplace la connexion en cours : un nouveau code ou un nouvel onglet annule le précédent.
    pub fn replace_sign_in(&self, handle: JoinHandle<()>) {
        if let Some(previous) = self.sign_in.lock().unwrap().replace(handle) {
            previous.abort();
        }
    }

    pub fn abort_sign_in(&self) {
        if let Some(handle) = self.sign_in.lock().unwrap().take() {
            handle.abort();
        }
    }
}

#[derive(Default)]
pub struct CliCache {
    claude_code: Option<CliAgentInfo>,
    codex: Option<CliAgentInfo>,
}

impl CliCache {
    pub fn get(&self, tool: CliTool) -> Option<CliAgentInfo> {
        match tool {
            CliTool::ClaudeCode => self.claude_code.clone(),
            CliTool::Codex => self.codex.clone(),
        }
    }

    pub fn set(&mut self, tool: CliTool, info: CliAgentInfo) {
        match tool {
            CliTool::ClaudeCode => self.claude_code = Some(info),
            CliTool::Codex => self.codex = Some(info),
        }
    }
}
