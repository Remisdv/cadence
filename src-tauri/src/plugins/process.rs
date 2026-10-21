//! Processus serveur d'un plugin (plan § 3.2, § 3.4) : un exécutable natif qui parle MCP sur
//! stdin/stdout, client `rmcp` côté Cadence.
//!
//! Cycle : `Stopped` → `Starting` (lancement, `initialize`, `tools/list` mis en cache, 20 s au
//! plus) → `Ready`. Les appels partent en parallèle sur la même connexion (JSON-RPC
//! multiplexé). Un arrêt inattendu passe en `Crashed` : redémarrage après 1 s, 5 s puis 30 s ;
//! un arrêt de plus dans la fenêtre de 5 minutes passe en `Failed` (plus de redémarrage
//! automatique, « Redémarrer » à la main). `stop` ferme stdin, laisse 3 s de grâce puis tue le
//! processus et ses enfants (job object sous Windows, groupe de processus ailleurs).
//!
//! stderr et `notifications/message` vont au journal du plugin (`<journaux>/plugins/<id>.log`,
//! 1 Mo × 3) ; les dernières lignes de stderr accompagnent le message d'un arrêt inattendu.

use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
#[allow(deprecated)] // `notifications/message` (journal MCP) reste utile tant que les serveurs l'envoient.
use rmcp::{
    ClientHandler, Peer, RoleClient, ServiceError,
    model::{
        CallToolRequest, CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig, ClientNotification,
        ClientRequest, CustomNotification, Implementation, JsonObject, LoggingMessageNotificationParam, ServerResult, Tool,
    },
    service::{NotificationContext, PeerRequestOptions},
};
use serde::Serialize;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{oneshot, watch},
    task::JoinHandle,
};
use ts_rs::TS;

use super::host::Shared;
use crate::error::{AppError, AppResult};

/// État du processus serveur d'un plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ProcessState {
    /// Pas lancé (jamais, inactif, désactivé) ; démarré au premier appel.
    Stopped,
    Starting,
    Ready,
    /// Arrêt inattendu, redémarrage programmé.
    Crashed,
    /// Trop d'arrêts, ou lancement impossible : plus de redémarrage automatique.
    Failed,
}

/// Charge de `plugin://state`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginStateEvent {
    pub plugin: String,
    pub state: ProcessState,
    /// Raison d'un `Crashed` ou d'un `Failed`.
    pub message: Option<String>,
}

/// État d'un serveur pour Réglages › Plugins (`plugin_status`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginProcessView {
    pub plugin: String,
    pub state: ProcessState,
    pub pid: Option<u32>,
    /// Redémarrages automatiques depuis le lancement de l'app.
    pub restarts: u32,
    /// Outils annoncés par `tools/list` et visibles des IA (0 tant que le serveur n'est pas
    /// prêt). Les outils réservés à l'interface du plugin (`cadence/visibility = "ui"`) ne
    /// comptent pas : même nombre que la fiche du catalogue.
    pub tools: u32,
    /// Raison du dernier arrêt inattendu ou de l'échec.
    pub message: Option<String>,
    /// Journal du serveur, pour « Voir le journal ».
    pub log_path: Option<String>,
}

/// Fin d'une exécution du processus.
enum Exit {
    /// Arrêt demandé.
    Stopped,
    /// Arrêt inattendu : redémarrage avec back-off.
    Crashed(String),
    /// Lancement impossible (binaire absent, manifeste invalide) : inutile de réessayer.
    Fatal(String),
}

pub struct PluginProcess {
    id: String,
    shared: Arc<Shared>,
    inner: Mutex<Inner>,
    /// (génération, état) : réveille ceux qui attendent `Ready`.
    state_tx: watch::Sender<(u64, ProcessState)>,
}

struct Inner {
    state: ProcessState,
    /// Incrémentée à chaque lancement et à chaque arrêt : une tâche d'une génération
    /// dépassée ne touche plus à l'état.
    generation: u64,
    peer: Option<Peer<RoleClient>>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
    pid: Option<u32>,
    tools: Vec<Tool>,
    crashes: VecDeque<Instant>,
    restarts: u32,
    message: Option<String>,
    idle_shutdown: Option<Duration>,
    last_used: Instant,
    in_flight: u32,
    log_path: Option<PathBuf>,
}

impl PluginProcess {
    pub(crate) fn new(id: &str, shared: Arc<Shared>) -> Arc<Self> {
        Arc::new(Self {
            id: id.to_string(),
            shared,
            inner: Mutex::new(Inner {
                state: ProcessState::Stopped,
                generation: 0,
                peer: None,
                stop: None,
                task: None,
                pid: None,
                tools: Vec::new(),
                crashes: VecDeque::new(),
                restarts: 0,
                message: None,
                idle_shutdown: None,
                last_used: Instant::now(),
                in_flight: 0,
                log_path: None,
            }),
            state_tx: watch::Sender::new((0, ProcessState::Stopped)),
        })
    }

    fn set_state(&self, inner: &mut Inner, state: ProcessState, message: Option<String>) {
        inner.state = state;
        inner.message.clone_from(&message);
        self.state_tx.send_replace((inner.generation, state));
        (self.shared.sink)(PluginStateEvent { plugin: self.id.clone(), state, message });
    }

    fn unavailable(&self, why: Option<&str>) -> AppError {
        AppError::PluginUnavailable(match why {
            Some(why) => format!("Le plugin « {} » est indisponible : {why}", self.id),
            None => format!("Le plugin « {} » est indisponible.", self.id),
        })
    }

    pub fn view(&self) -> PluginProcessView {
        let inner = self.inner.lock().unwrap();
        PluginProcessView {
            plugin: self.id.clone(),
            state: inner.state,
            pid: inner.pid,
            restarts: inner.restarts,
            tools: u32::try_from(inner.tools.iter().filter(|tool| !crate::gateway::scope::is_ui_only(tool)).count())
                .unwrap_or(u32::MAX),
            message: inner.message.clone(),
            log_path: inner.log_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
        }
    }

    /// Lance une nouvelle exécution (verrou tenu par l'appelant).
    fn spawn_run(self: &Arc<Self>, inner: &mut Inner) {
        inner.generation += 1;
        let generation = inner.generation;
        self.set_state(inner, ProcessState::Starting, None);
        let this = self.clone();
        inner.task = Some(tokio::spawn(async move {
            let exit = this.run(generation).await;
            this.after_exit(generation, exit);
        }));
    }

    /// Démarre le serveur s'il est arrêté et attend qu'il soit prêt.
    pub async fn ready(self: &Arc<Self>) -> AppResult<Peer<RoleClient>> {
        let mut rx = self.state_tx.subscribe();
        let generation = {
            let mut inner = self.inner.lock().unwrap();
            match inner.state {
                ProcessState::Ready => return inner.peer.clone().ok_or_else(|| self.unavailable(None)),
                ProcessState::Failed => return Err(self.unavailable(inner.message.as_deref())),
                ProcessState::Crashed => return Err(self.unavailable(Some("redémarrage en cours."))),
                ProcessState::Starting => inner.generation,
                ProcessState::Stopped => {
                    self.spawn_run(&mut inner);
                    inner.generation
                }
            }
        };
        loop {
            let (current, state) = *rx.borrow_and_update();
            if current != generation {
                return Err(self.unavailable(Some("arrêté pendant le démarrage.")));
            }
            match state {
                ProcessState::Starting => {}
                ProcessState::Ready => {
                    let inner = self.inner.lock().unwrap();
                    return match (&inner.peer, inner.generation == generation) {
                        (Some(peer), true) => Ok(peer.clone()),
                        _ => Err(self.unavailable(None)),
                    };
                }
                _ => {
                    let message = self.inner.lock().unwrap().message.clone();
                    return Err(self.unavailable(message.as_deref()));
                }
            }
            if rx.changed().await.is_err() {
                return Err(self.unavailable(None));
            }
        }
    }

    /// Envoie une notification au serveur s'il tourne (sinon rien : il lira l'état à jour au
    /// prochain lancement). Renvoie vrai si elle est partie.
    pub async fn notify(&self, method: &str, params: serde_json::Value) -> bool {
        let peer = {
            let inner = self.inner.lock().unwrap();
            if inner.state != ProcessState::Ready {
                return false;
            }
            inner.peer.clone()
        };
        let Some(peer) = peer else { return false };
        let notification = ClientNotification::CustomNotification(CustomNotification::new(method, Some(params)));
        match peer.send_notification(notification).await {
            Ok(()) => true,
            Err(err) => {
                tracing::warn!(plugin = self.id, method, %err, "notification au serveur non envoyée");
                false
            }
        }
    }

    /// Outils annoncés par le serveur (cache de `tools/list`, relu sur `list_changed`).
    pub async fn tools(self: &Arc<Self>) -> AppResult<Vec<Tool>> {
        self.ready().await?;
        Ok(self.inner.lock().unwrap().tools.clone())
    }

    /// Appelle un outil ; plusieurs appels peuvent être en cours en même temps.
    pub async fn call(self: &Arc<Self>, tool: &str, args: Option<JsonObject>, timeout: Duration) -> AppResult<CallToolResult> {
        let peer = self.ready().await?;
        let _busy = Busy::new(self);
        let mut params = CallToolRequestParams::new(tool.to_string());
        if let Some(args) = args {
            params = params.with_arguments(args);
        }
        // Chaque `notifications/progress` du serveur relance le délai, dans la limite de
        // `PROGRESS_MAX_FACTOR` fois le délai de base.
        let request = ClientRequest::CallToolRequest(CallToolRequest::new(params));
        let mut options = PeerRequestOptions::with_timeout(timeout).reset_timeout_on_progress();
        options.max_total_timeout = Some(timeout * PROGRESS_MAX_FACTOR);
        let response = match peer.send_request_with_option(request, options).await {
            Ok(handle) => handle.await_response().await,
            Err(err) => Err(err),
        };
        match response {
            Ok(ServerResult::CallToolResult(result)) => Ok(result),
            Ok(_) => Err(AppError::ToolError(format!("{}__{tool} : réponse inattendue du serveur.", self.id))),
            // Plafond atteint malgré la progression (rmcp rend le délai total dépassé).
            Err(ServiceError::Timeout { timeout: total }) if total > timeout => Err(AppError::ToolError(format!(
                "{}__{tool} n'a pas fini en {} s malgré sa progression.",
                self.id,
                total.as_secs()
            ))),
            Err(ServiceError::Timeout { .. }) => {
                Err(AppError::ToolError(format!("{}__{tool} n'a pas répondu en {} s.", self.id, timeout.as_secs())))
            }
            Err(ServiceError::McpError(err)) => Err(AppError::ToolError(format!("{}__{tool} : {}", self.id, err.message))),
            Err(err) => Err(self.unavailable(Some(&format!("{err}.")))),
        }
    }

    /// Arrête le serveur (fermeture de stdin, grâce, puis kill) et attend la fin du processus.
    /// Annule aussi un redémarrage programmé et sort de `Failed`.
    pub async fn stop(&self) {
        let (stop, task) = {
            let mut inner = self.inner.lock().unwrap();
            inner.generation += 1;
            let was = inner.state;
            let stop = inner.stop.take();
            let task = inner.task.take();
            inner.peer = None;
            inner.pid = None;
            inner.tools.clear();
            if was != ProcessState::Stopped {
                self.set_state(&mut inner, ProcessState::Stopped, None);
            }
            (stop, task)
        };
        if let Some(stop) = stop {
            let _ = stop.send(());
        }
        if let Some(task) = task {
            let wait = self.shared.config.grace + Duration::from_secs(2);
            if tokio::time::timeout(wait, task).await.is_err() {
                tracing::warn!(plugin = self.id, "le serveur ne s'est pas arrêté à temps");
            }
        }
    }

    /// Redémarrage manuel : repart de zéro (compteur d'arrêts remis à zéro) et attend `Ready`.
    pub async fn restart(self: &Arc<Self>) -> AppResult<()> {
        self.stop().await;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.crashes.clear();
            inner.message = None;
        }
        self.ready().await.map(|_| ())
    }

    /// Prêt, sans appel en cours, et inactif depuis plus que son délai d'arrêt.
    pub(crate) fn idle_expired(&self, now: Instant) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.state == ProcessState::Ready
            && inner.in_flight == 0
            && inner.idle_shutdown.is_some_and(|idle| now.duration_since(inner.last_used) >= idle)
    }

    fn replace_tools(&self, generation: u64, tools: Vec<Tool>) {
        let changed = {
            let mut inner = self.inner.lock().unwrap();
            let current = inner.generation == generation;
            if current {
                inner.tools = tools;
            }
            current
        };
        if changed {
            let _ = self.shared.tools_changed.send(self.id.clone());
        }
    }

    /// Une exécution du processus, du lancement à sa fin.
    async fn run(self: &Arc<Self>, generation: u64) -> Exit {
        let config = &self.shared.config;
        let spec = match (self.shared.resolve)(&self.id) {
            Ok(spec) => spec,
            Err(err) => return Exit::Fatal(err.to_string()),
        };
        if let Err(err) = fs::create_dir_all(&spec.data_dir) {
            return Exit::Fatal(format!("dossier de données impossible à créer ({err})."));
        }
        let log = spec.log_file.as_deref().map(|path| Arc::new(PluginLog::new(path)));

        let mut command = tokio::process::Command::new(&spec.program);
        command
            .args(&spec.args)
            .current_dir(&spec.data_dir)
            .env_clear()
            .envs(spec.all_env())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut wrapped = CommandWrap::from(command);
        wrapped.wrap(KillOnDrop);
        #[cfg(windows)]
        {
            wrapped.wrap(process_wrap::tokio::CreationFlags(windows::Win32::System::Threading::CREATE_NO_WINDOW));
            wrapped.wrap(process_wrap::tokio::JobObject);
        }
        #[cfg(unix)]
        wrapped.wrap(process_wrap::tokio::ProcessGroup::leader());
        let mut child = match wrapped.spawn() {
            Ok(child) => child,
            Err(err) => return Exit::Fatal(format!("lancement de « {} » impossible ({err}).", spec.program.display())),
        };
        let (Some(stdin), Some(stdout)) = (child.stdin().take(), child.stdout().take()) else {
            kill(&mut child).await;
            return Exit::Fatal("entrées-sorties du serveur indisponibles.".into());
        };
        if let Some(log) = &log {
            log.write_line(&format!("--- démarrage du serveur (pid {}) ---", child.id().unwrap_or_default()));
        }
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        if let Some(stderr) = child.stderr().take() {
            tokio::spawn(read_stderr(stderr, log.clone(), tail.clone()));
        }

        let (stop_tx, mut stop_rx) = oneshot::channel();
        let superseded = {
            let mut inner = self.inner.lock().unwrap();
            let superseded = inner.generation != generation;
            if !superseded {
                inner.stop = Some(stop_tx);
                inner.pid = child.id();
                inner.idle_shutdown = spec.idle_shutdown;
                inner.log_path = spec.log_file.clone();
            }
            superseded
        };
        if superseded {
            kill(&mut child).await;
            return Exit::Stopped;
        }

        let handler = Handler {
            process: Arc::downgrade(self),
            generation,
            config: ClientConfig::new(ClientCapabilities::default(), Implementation::new("cadence", &config.app_version)),
            log: log.clone(),
        };
        let started = async {
            let service = rmcp::serve_client(handler, (stdout, stdin)).await.map_err(|e| format!("initialize a échoué ({e})."))?;
            let tools = service.peer().list_all_tools().await.map_err(|e| format!("tools/list a échoué ({e})."))?;
            Ok::<_, String>((service, tools))
        };
        enum Start<T> {
            Up(T),
            Failed(String),
            Exited(String),
            Stop,
        }
        let start = tokio::select! {
            result = tokio::time::timeout(config.start_timeout, started) => match result {
                Ok(Ok(up)) => Start::Up(up),
                Ok(Err(why)) => Start::Failed(why),
                Err(_) => Start::Failed(format!("pas prêt en {} s.", config.start_timeout.as_secs())),
            },
            status = child.wait() => Start::Exited(exit_message(status)),
            _ = &mut stop_rx => Start::Stop,
        };
        let (service, tools) = match start {
            Start::Up(up) => up,
            Start::Failed(why) => {
                kill(&mut child).await;
                return Exit::Crashed(with_tail(why, &tail));
            }
            Start::Exited(why) => return Exit::Crashed(with_tail(why, &tail)),
            Start::Stop => {
                kill(&mut child).await;
                return Exit::Stopped;
            }
        };

        let superseded = {
            let mut inner = self.inner.lock().unwrap();
            let superseded = inner.generation != generation;
            if !superseded {
                inner.peer = Some(service.peer().clone());
                inner.tools = tools;
                inner.last_used = Instant::now();
                self.set_state(&mut inner, ProcessState::Ready, None);
            }
            superseded
        };
        if superseded {
            drop(service);
            kill(&mut child).await;
            return Exit::Stopped;
        }

        let crashed = tokio::select! {
            status = child.wait() => Some(exit_message(status)),
            _ = &mut stop_rx => None,
        };
        match crashed {
            Some(why) => {
                drop(service);
                Exit::Crashed(with_tail(why, &tail))
            }
            None => {
                // Fin du service MCP = stdin fermé : le serveur doit s'arrêter de lui-même.
                let _ = service.cancel().await;
                if tokio::time::timeout(config.grace, child.wait()).await.is_err() {
                    kill(&mut child).await;
                }
                Exit::Stopped
            }
        }
    }

    fn after_exit(self: &Arc<Self>, generation: u64, exit: Exit) {
        let config = &self.shared.config;
        let mut inner = self.inner.lock().unwrap();
        if inner.generation != generation {
            return; // arrêté ou relancé entre-temps : `stop` s'occupe de l'état
        }
        inner.peer = None;
        inner.stop = None;
        inner.pid = None;
        inner.tools.clear();
        match exit {
            Exit::Stopped => self.set_state(&mut inner, ProcessState::Stopped, None),
            Exit::Fatal(why) => {
                tracing::warn!(plugin = self.id, why, "serveur de plugin impossible à lancer");
                self.set_state(&mut inner, ProcessState::Failed, Some(why));
            }
            Exit::Crashed(why) => {
                let now = Instant::now();
                inner.crashes.retain(|at| now.duration_since(*at) < config.crash_window);
                inner.crashes.push_back(now);
                let count = inner.crashes.len();
                tracing::warn!(plugin = self.id, why, count, "serveur de plugin arrêté");
                match config.backoff.get(count - 1).copied() {
                    None => {
                        let message = format!("{why} {count} arrêts en {} min : redémarrage automatique abandonné.", config.crash_window.as_secs() / 60);
                        self.set_state(&mut inner, ProcessState::Failed, Some(message));
                    }
                    Some(delay) => {
                        self.set_state(&mut inner, ProcessState::Crashed, Some(why));
                        let this = self.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(delay).await;
                            let mut inner = this.inner.lock().unwrap();
                            if inner.generation == generation && inner.state == ProcessState::Crashed {
                                inner.restarts += 1;
                                this.spawn_run(&mut inner);
                            }
                        });
                    }
                }
            }
        }
    }
}

/// Appel en cours : retarde l'arrêt sur inactivité.
struct Busy<'a>(&'a PluginProcess);

impl<'a> Busy<'a> {
    fn new(process: &'a PluginProcess) -> Self {
        let mut inner = process.inner.lock().unwrap();
        inner.in_flight += 1;
        inner.last_used = Instant::now();
        Self(process)
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        let mut inner = self.0.inner.lock().unwrap();
        inner.in_flight -= 1;
        inner.last_used = Instant::now();
    }
}

async fn kill(child: &mut Box<dyn ChildWrapper>) {
    if let Err(err) = Box::into_pin(child.kill()).await {
        tracing::debug!(%err, "arrêt forcé du serveur");
    }
}

fn exit_message(status: std::io::Result<std::process::ExitStatus>) -> String {
    match status {
        Ok(status) => match status.code() {
            Some(code) => format!("le serveur s'est arrêté (code {code})."),
            None => "le serveur s'est arrêté (signal).".into(),
        },
        Err(err) => format!("le serveur s'est arrêté ({err})."),
    }
}

const TAIL_LINES: usize = 5;

/// Un appel qui signale sa progression peut durer jusqu'à ce multiple de son délai.
pub const PROGRESS_MAX_FACTOR: u32 = 6;

/// Ajoute les dernières lignes de stderr à un message d'arrêt.
fn with_tail(why: String, tail: &Mutex<VecDeque<String>>) -> String {
    let tail = tail.lock().unwrap();
    if tail.is_empty() {
        why
    } else {
        format!("{why} Dernières lignes du journal : {}", tail.iter().cloned().collect::<Vec<_>>().join(" ⏎ "))
    }
}

async fn read_stderr(stderr: tokio::process::ChildStderr, log: Option<Arc<PluginLog>>, tail: Arc<Mutex<VecDeque<String>>>) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(log) = &log {
            log.write_line(&line);
        }
        let mut tail = tail.lock().unwrap();
        if tail.len() == TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(line);
    }
}

/// Client MCP d'une exécution : notifications du serveur.
struct Handler {
    process: Weak<PluginProcess>,
    generation: u64,
    config: ClientConfig,
    log: Option<Arc<PluginLog>>,
}

impl ClientHandler for Handler {
    fn get_info(&self) -> ClientConfig {
        self.config.clone()
    }

    async fn on_tool_list_changed(&self, context: NotificationContext<RoleClient>) {
        match context.peer.list_all_tools().await {
            Ok(tools) => {
                if let Some(process) = self.process.upgrade() {
                    process.replace_tools(self.generation, tools);
                }
            }
            Err(err) => tracing::warn!(%err, "tools/list après list_changed"),
        }
    }

    #[allow(deprecated)]
    async fn on_logging_message(&self, params: LoggingMessageNotificationParam, _context: NotificationContext<RoleClient>) {
        if let Some(log) = &self.log {
            let text = params.data.as_str().map(str::to_string).unwrap_or_else(|| params.data.to_string());
            log.write_line(&format!("[{:?}] {text}", params.level).to_lowercase());
        }
    }

    async fn on_custom_notification(&self, notification: CustomNotification, _context: NotificationContext<RoleClient>) {
        if notification.method != "notifications/cadence/event" {
            tracing::debug!(method = notification.method, "notification de plugin ignorée");
            return;
        }
        let Some(process) = self.process.upgrade() else { return };
        let params = notification.params.unwrap_or_default();
        let Some(event) = params.get("event").and_then(|e| e.as_str()) else { return };
        if let Some(events) = &process.shared.events {
            events(super::host::PluginEventPayload {
                plugin: process.id.clone(),
                event: event.to_string(),
                data: params.get("data").cloned(),
            });
        }
    }
}

/// Taille d'un fichier de journal avant rotation ; trois fichiers au plus (`.log`, `.1`, `.2`).
const LOG_MAX_BYTES: u64 = 1024 * 1024;
const LOG_FILES: usize = 3;

/// Journal d'un serveur de plugin.
pub(crate) struct PluginLog {
    path: PathBuf,
    file: Mutex<Option<File>>,
}

impl PluginLog {
    pub(crate) fn new(path: &Path) -> Self {
        Self { path: path.to_path_buf(), file: Mutex::new(None) }
    }

    fn rotated(&self, n: usize) -> PathBuf {
        let mut name = self.path.as_os_str().to_owned();
        name.push(format!(".{n}"));
        PathBuf::from(name)
    }

    pub(crate) fn write_line(&self, line: &str) {
        let mut file = self.file.lock().unwrap();
        let size = file.as_ref().and_then(|f| f.metadata().ok()).map(|m| m.len()).or_else(|| fs::metadata(&self.path).ok().map(|m| m.len()));
        if size.is_some_and(|size| size + line.len() as u64 + 1 > LOG_MAX_BYTES) {
            *file = None;
            let _ = fs::remove_file(self.rotated(LOG_FILES - 1));
            for n in (1..LOG_FILES - 1).rev() {
                let _ = fs::rename(self.rotated(n), self.rotated(n + 1));
            }
            let _ = fs::rename(&self.path, self.rotated(1));
        }
        if file.is_none() {
            if let Some(dir) = self.path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            *file = OpenOptions::new().create(true).append(true).open(&self.path).ok();
        }
        if let Some(f) = file.as_mut() {
            let _ = writeln!(f, "{line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_the_log_at_one_megabyte() {
        let dir = crate::app_settings::temp_dir("plugin-log");
        let log = PluginLog::new(&dir.join("journal.log"));
        let line = "x".repeat(1023);
        for _ in 0..(1024 * 3 + 10) {
            log.write_line(&line);
        }
        let size = |p: PathBuf| fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        assert!(size(dir.join("journal.log")) <= LOG_MAX_BYTES);
        assert!(size(log.rotated(1)) > 0 && size(log.rotated(2)) > 0);
        assert!(!log.rotated(3).exists(), "trois fichiers au plus");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn appends_the_end_of_stderr_to_crash_messages() {
        let tail = Mutex::new(VecDeque::from(["panique : jeton absent".to_string()]));
        assert_eq!(with_tail("arrêt.".into(), &tail), "arrêt. Dernières lignes du journal : panique : jeton absent");
        assert_eq!(with_tail("arrêt.".into(), &Mutex::new(VecDeque::new())), "arrêt.");
    }
}
