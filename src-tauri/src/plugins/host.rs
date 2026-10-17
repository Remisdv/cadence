//! Hôte des serveurs de plugin (plan § 2.2, § 3.2) : un `PluginProcess` par plugin installé,
//! créé à la demande, démarré au premier appel (`call`, `tools`, `restart`), arrêté sur
//! inactivité (`server.idle_shutdown_s`), à la désactivation, à la désinstallation et à la
//! fermeture de l'app.
//!
//! L'hôte ne connaît ni Tauri ni le registre : la spécification de lancement vient d'un
//! `Resolver` (lu à chaque démarrage, donc secrets et réglages à jour) et les changements
//! d'état partent dans un `StateSink` (événement `plugin://state` dans l'app).

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use rmcp::model::{CallToolResult, JsonObject, Tool};

use super::{
    env::LaunchSpec,
    process::{PluginProcess, PluginProcessView, PluginStateEvent, ProcessState},
};
use crate::error::AppResult;

/// Spécification de lancement du serveur d'un plugin.
pub type Resolver = Arc<dyn Fn(&str) -> AppResult<LaunchSpec> + Send + Sync>;
/// Reçoit chaque changement d'état d'un serveur.
pub type StateSink = Arc<dyn Fn(PluginStateEvent) + Send + Sync>;
/// Reçoit les `notifications/cadence/event` des serveurs (relayées aux iframes du plugin).
pub type PluginEventSink = Arc<dyn Fn(PluginEventPayload) + Send + Sync>;

/// Charge de `plugin://event` : notification `notifications/cadence/event` d'un serveur.
#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginEventPayload {
    pub plugin: String,
    pub event: String,
    #[ts(type = "unknown")]
    pub data: Option<serde_json::Value>,
}

/// Délais de l'hôte (valeurs du plan par défaut ; raccourcis dans les tests).
#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Version de Cadence annoncée dans `initialize`.
    pub app_version: String,
    /// Lancement + `initialize` + `tools/list`.
    pub start_timeout: Duration,
    /// Entre la fermeture de stdin et le kill.
    pub grace: Duration,
    /// Délai avant chaque redémarrage automatique ; un arrêt de plus → `Failed`.
    pub backoff: Vec<Duration>,
    /// Fenêtre dans laquelle les arrêts inattendus s'additionnent.
    pub crash_window: Duration,
    /// Période de la vérification d'inactivité.
    pub idle_check: Duration,
    /// Délai par défaut d'un appel d'outil (UI : 120 s ; l'IA passera 600 s).
    pub call_timeout: Duration,
}

impl HostConfig {
    pub fn new(app_version: &str) -> Self {
        Self {
            app_version: app_version.to_string(),
            start_timeout: Duration::from_secs(20),
            grace: Duration::from_secs(3),
            backoff: vec![Duration::from_secs(1), Duration::from_secs(5), Duration::from_secs(30)],
            crash_window: Duration::from_secs(5 * 60),
            idle_check: Duration::from_secs(30),
            call_timeout: Duration::from_secs(120),
        }
    }
}

pub(crate) struct Shared {
    pub(crate) config: HostConfig,
    pub(crate) resolve: Resolver,
    pub(crate) sink: StateSink,
    pub(crate) events: Option<PluginEventSink>,
    /// Id du plugin dont les outils viennent de changer (`notifications/tools/list_changed`).
    pub(crate) tools_changed: tokio::sync::broadcast::Sender<String>,
}

pub struct PluginHost {
    shared: Arc<Shared>,
    processes: Mutex<BTreeMap<String, Arc<PluginProcess>>>,
}

impl PluginHost {
    pub fn new(config: HostConfig, resolve: Resolver, sink: StateSink) -> Self {
        let (tools_changed, _) = tokio::sync::broadcast::channel(16);
        Self { shared: Arc::new(Shared { config, resolve, sink, events: None, tools_changed }), processes: Mutex::default() }
    }

    /// Plugins dont les outils changent en cours de route (relayé aux sessions d'IA).
    pub fn subscribe_tools_changed(&self) -> tokio::sync::broadcast::Receiver<String> {
        self.shared.tools_changed.subscribe()
    }

    /// Relaie les `notifications/cadence/event` des serveurs (à appeler avant tout lancement).
    pub fn with_plugin_events(mut self, events: PluginEventSink) -> Self {
        if let Some(shared) = Arc::get_mut(&mut self.shared) {
            shared.events = Some(events);
        }
        self
    }

    pub fn config(&self) -> &HostConfig {
        &self.shared.config
    }

    fn process(&self, plugin: &str) -> Arc<PluginProcess> {
        self.processes
            .lock()
            .unwrap()
            .entry(plugin.to_string())
            .or_insert_with(|| PluginProcess::new(plugin, self.shared.clone()))
            .clone()
    }

    fn existing(&self, plugin: &str) -> Option<Arc<PluginProcess>> {
        self.processes.lock().unwrap().get(plugin).cloned()
    }

    /// Appelle `tool` (nom sans préfixe) sur le serveur du plugin, démarré au besoin.
    pub async fn call(&self, plugin: &str, tool: &str, args: Option<JsonObject>) -> AppResult<CallToolResult> {
        self.call_with_timeout(plugin, tool, args, self.shared.config.call_timeout).await
    }

    pub async fn call_with_timeout(
        &self,
        plugin: &str,
        tool: &str,
        args: Option<JsonObject>,
        timeout: Duration,
    ) -> AppResult<CallToolResult> {
        self.process(plugin).call(tool, args, timeout).await
    }

    /// Outils du serveur (cache de `tools/list`), démarré au besoin.
    pub async fn tools(&self, plugin: &str) -> AppResult<Vec<Tool>> {
        self.process(plugin).tools().await
    }

    /// Notification `method` au serveur du plugin s'il tourne ; un serveur arrêté n'est pas
    /// démarré pour autant. Renvoie vrai si elle est partie.
    pub async fn notify(&self, plugin: &str, method: &str, params: serde_json::Value) -> bool {
        match self.existing(plugin) {
            Some(process) => process.notify(method, params).await,
            None => false,
        }
    }

    /// Arrête le serveur d'un plugin s'il tourne, et attend la fin du processus.
    pub async fn stop(&self, plugin: &str) {
        if let Some(process) = self.existing(plugin) {
            process.stop().await;
        }
    }

    /// Arrête puis relance le serveur, même en `Failed`, et attend qu'il soit prêt.
    pub async fn restart(&self, plugin: &str) -> AppResult<()> {
        self.process(plugin).restart().await
    }

    /// Arrête tous les serveurs (fermeture de l'app).
    pub async fn stop_all(&self) {
        let all: Vec<Arc<PluginProcess>> = self.processes.lock().unwrap().values().cloned().collect();
        let mut stopping = tokio::task::JoinSet::new();
        for process in all {
            stopping.spawn(async move { process.stop().await });
        }
        while stopping.join_next().await.is_some() {}
    }

    /// État du serveur d'un plugin (`Stopped` s'il n'a jamais été lancé).
    pub fn status(&self, plugin: &str) -> PluginProcessView {
        match self.existing(plugin) {
            Some(process) => process.view(),
            None => PluginProcessView {
                plugin: plugin.to_string(),
                state: ProcessState::Stopped,
                pid: None,
                restarts: 0,
                tools: 0,
                message: None,
                log_path: None,
            },
        }
    }

    /// Arrête les serveurs inactifs depuis plus que leur délai ; renvoie leurs ids.
    pub async fn stop_idle(&self) -> Vec<String> {
        let now = Instant::now();
        let idle: Vec<(String, Arc<PluginProcess>)> = self
            .processes
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, p)| p.idle_expired(now))
            .map(|(id, p)| (id.clone(), p.clone()))
            .collect();
        for (id, process) in &idle {
            tracing::info!(plugin = id, "serveur de plugin inactif, arrêté");
            process.stop().await;
        }
        idle.into_iter().map(|(id, _)| id).collect()
    }
}

/// Boucle de l'arrêt sur inactivité ; s'arrête quand l'hôte disparaît.
pub async fn run_idle_reaper(host: Weak<PluginHost>) {
    loop {
        let Some(period) = host.upgrade().map(|h| h.shared.config.idle_check) else { return };
        tokio::time::sleep(period).await;
        let Some(host) = host.upgrade() else { return };
        host.stop_idle().await;
    }
}
