//! État partagé, injecté dans les commandes par `tauri::State<AppState>`.

use std::{path::Path, sync::Arc, sync::Mutex};

use secrecy::SecretString;
use tauri::async_runtime::JoinHandle;
use tokio::sync::RwLock;

use crate::{
    api::github,
    auth::{cli::CliTool, token_store::TokenStore},
    domain::{
        auth::{CliAgentInfo, GithubAuth},
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
    /// Polling du device flow en cours (un seul à la fois).
    pub device_flow: Mutex<Option<JoinHandle<()>>>,
    /// Détection des agents de code, gardée pour la session.
    pub cli: RwLock<CliCache>,
    pub settings: SettingsStore,
    /// Dépôts accessibles au jeton, lus une fois puis filtrés localement.
    pub repos: RwLock<Option<Vec<Repo>>>,
    /// Versions du projet ouvert (en attendant le cache SQLite).
    pub versions: RwLock<Vec<Version>>,
}

impl AppState {
    pub fn new(config_dir: &Path, tokens: Arc<dyn TokenStore>) -> Self {
        Self {
            project: RwLock::default(),
            http: github::http_client(),
            tokens,
            github: RwLock::default(),
            device_flow: Mutex::default(),
            cli: RwLock::default(),
            settings: SettingsStore::load(config_dir),
            repos: RwLock::default(),
            versions: RwLock::default(),
        }
    }

    pub async fn require_project(&self) -> AppResult<ProjectState> {
        self.project.read().await.clone().ok_or(AppError::NoProject)
    }

    /// Jeton GitHub du trousseau, lu au moment de la requête.
    pub fn github_token(&self) -> AppResult<SecretString> {
        self.tokens.get()?.ok_or(AppError::Unauthenticated("GitHub"))
    }

    /// Remplace le polling en cours (un nouveau code annule l'ancien).
    pub fn replace_device_flow(&self, handle: JoinHandle<()>) {
        if let Some(previous) = self.device_flow.lock().unwrap().replace(handle) {
            previous.abort();
        }
    }

    pub fn abort_device_flow(&self) {
        if let Some(handle) = self.device_flow.lock().unwrap().take() {
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
