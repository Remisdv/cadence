//! État partagé, injecté dans les commandes par `tauri::State<AppState>`.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use secrecy::SecretString;
use tauri::async_runtime::JoinHandle;
use tokio::sync::RwLock;

use crate::{
    api::github,
    auth::{cli::CliTool, token_store::TokenStore},
    domain::{
        auth::{CliAgentInfo, GithubAuth, GithubSources},
        project::{ProjectState, Repo},
        settings::Config,
    },
    error::{AppError, AppResult},
    local_settings::SettingsStore,
    store::RepoCache,
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
    /// Dossier des bases SQLite, une par dépôt.
    pub cache_dir: PathBuf,
    /// Cache SQLite du dépôt ouvert, ouvert à la première lecture.
    pub cache: RwLock<Option<Arc<RepoCache>>>,
    /// Copies des dépôts lues par les agents (`<données>/repos/<owner>/<nom>`).
    pub checkout_dir: PathBuf,
    /// `.cadence/config.toml` du dépôt ouvert, lu une fois puis gardé.
    pub config: RwLock<Option<LoadedConfig>>,
    /// Moyens de connexion GitHub détectés (gh, GCM), gardés pour la session.
    pub github_sources: RwLock<Option<GithubSources>>,
}

impl AppState {
    pub fn new(config_dir: &Path, cache_dir: PathBuf, tokens: Arc<dyn TokenStore>) -> Self {
        Self {
            project: RwLock::default(),
            http: github::http_client(),
            tokens,
            github: RwLock::default(),
            sign_in: Mutex::default(),
            cli: RwLock::default(),
            settings: SettingsStore::load(config_dir),
            repos: RwLock::default(),
            checkout_dir: cache_dir.with_file_name("repos"),
            cache_dir,
            cache: RwLock::default(),
            config: RwLock::default(),
            github_sources: RwLock::default(),
        }
    }

    pub async fn require_project(&self) -> AppResult<ProjectState> {
        self.project.read().await.clone().ok_or(AppError::NoProject)
    }

    /// Cache du dépôt ouvert, ouvert (et migré) au premier appel.
    pub async fn repo_cache(&self) -> AppResult<Arc<RepoCache>> {
        let repo = self.require_project().await?.repo.full_name;
        if let Some(cache) = self.cache.read().await.as_ref().filter(|c| c.repo == repo) {
            return Ok(cache.clone());
        }
        let mut slot = self.cache.write().await;
        if let Some(cache) = slot.as_ref().filter(|c| c.repo == repo) {
            return Ok(cache.clone());
        }
        let cache = Arc::new(RepoCache::open(&self.cache_dir, &repo).await?);
        *slot = Some(cache.clone());
        Ok(cache)
    }

    /// Ouvre le cache d'un dépôt sans l'installer (`project_open` le remplit d'abord).
    pub async fn open_cache(&self, repo: &str) -> AppResult<Arc<RepoCache>> {
        Ok(Arc::new(RepoCache::open(&self.cache_dir, repo).await?))
    }

    /// Le dépôt ouvert change : son cache remplace le précédent.
    pub async fn install_cache(&self, cache: Arc<RepoCache>) {
        *self.cache.write().await = Some(cache);
    }

    /// Oublie le dépôt ouvert ; les bases restent sur le disque pour le prochain lancement.
    pub async fn clear_project_data(&self) {
        *self.cache.write().await = None;
        *self.config.write().await = None;
    }

    /// Déconnexion : supprime les bases de tous les dépôts (issues privées comprises).
    pub async fn delete_caches(&self) {
        *self.config.write().await = None;
        if self.checkout_dir.exists()
            && let Err(err) = std::fs::remove_dir_all(&self.checkout_dir)
        {
            tracing::warn!(%err, "suppression des copies de dépôts incomplète");
        }
        if let Some(cache) = self.cache.write().await.take() {
            cache.close().await;
        }
        if self.cache_dir.exists()
            && let Err(err) = std::fs::remove_dir_all(&self.cache_dir)
        {
            tracing::warn!(%err, "suppression du cache local incomplète");
        }
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

/// Configuration lue sur GitHub, avec de quoi la réécrire.
#[derive(Clone)]
pub struct LoadedConfig {
    pub repo: String,
    pub config: Config,
    /// SHA du blob lu ; `None` tant que le fichier n'existe pas (valeurs par défaut).
    pub sha: Option<String>,
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
