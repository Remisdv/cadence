//! État partagé, injecté dans les commandes par `tauri::State<AppState>`.

use std::{
    path::PathBuf,
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
    app_settings::AppSettingsStore,
    error::{AppError, AppResult},
    chat::ChatService,
    gateway::server::Gateway,
    plugins::{
        env::{self, HostPaths},
        host::{HostConfig, PluginEventSink, PluginHost, StateSink},
        registry::PluginRegistry,
        settings::PluginSettingsStore,
    },
    secrets::SecretStore,
    store::RepoCache,
};

/// Dossiers de l'app, résolus au démarrage par Tauri.
pub struct AppPaths {
    /// `settings.json`, réglages des plugins (`plugins/<id>.json`).
    pub config_dir: PathBuf,
    /// Plugins installés (`plugins/`), leurs données (`plugins-data/`).
    pub data_dir: PathBuf,
    /// Caches SQLite de Cadence 1, un par dépôt.
    pub cache_dir: PathBuf,
    /// Journaux (`plugins/<id>.log` des serveurs de plugin).
    pub log_dir: PathBuf,
}

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
    /// Préférences de la machine (`settings.json` v2).
    pub settings: Arc<AppSettingsStore>,
    /// Trousseau du système (secrets des plugins, jeton GitHub).
    pub secrets: Arc<dyn SecretStore>,
    /// Plugins installés, activation et ordre du rail.
    pub plugins: Arc<PluginRegistry>,
    /// Réglages globaux des plugins.
    pub plugin_settings: Arc<PluginSettingsStore>,
    /// Serveurs MCP des plugins, un processus par plugin.
    pub plugin_host: Arc<PluginHost>,
    /// Passerelle MCP des IA ; `None` si elle n'a pas pu démarrer.
    pub gateway: Option<Gateway>,
    /// Chat du cœur ; `None` si son historique n'a pas pu s'ouvrir.
    pub chat: Option<Arc<ChatService>>,
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
    pub fn new(
        paths: AppPaths,
        app_version: &str,
        secrets: Arc<dyn SecretStore>,
        tokens: Arc<dyn TokenStore>,
        plugin_events: StateSink,
        plugin_notifications: PluginEventSink,
    ) -> Self {
        let AppPaths { config_dir, data_dir, cache_dir, log_dir } = paths;
        let plugins = Arc::new(PluginRegistry::load(data_dir.join("plugins"), app_version));
        let plugin_settings = Arc::new(PluginSettingsStore::new(config_dir.join("plugins")));
        let host_paths = HostPaths { data_root: data_dir.join("plugins-data"), log_dir: log_dir.join("plugins") };
        let resolve = {
            let (plugins, plugin_settings, secrets) = (plugins.clone(), plugin_settings.clone(), secrets.clone());
            Arc::new(move |plugin: &str| env::launch_spec(&plugins, &plugin_settings, secrets.as_ref(), &host_paths, plugin))
        };
        let plugin_host = Arc::new(
            PluginHost::new(HostConfig::new(app_version), resolve, plugin_events).with_plugin_events(plugin_notifications),
        );
        Self {
            project: RwLock::default(),
            http: github::http_client(),
            tokens,
            github: RwLock::default(),
            sign_in: Mutex::default(),
            cli: RwLock::default(),
            settings: Arc::new(AppSettingsStore::load(&config_dir)),
            secrets,
            plugins,
            plugin_settings,
            plugin_host,
            gateway: None,
            chat: None,
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
