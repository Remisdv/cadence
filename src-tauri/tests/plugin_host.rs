//! Hôte des plugins piloté contre un vrai serveur MCP : `examples/fake-plugin.rs` (plan § 11).
//!
//! `cargo test` construit les exemples avant de lancer les tests ; avec un filtre `--test`,
//! construire d'abord `cargo build --example fake-plugin`.

use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use cadence_lib::{
    error::AppError,
    plugins::{
        env::{LaunchSpec, server_env},
        host::{HostConfig, PluginEventPayload, PluginHost, run_idle_reaper},
        process::{PluginProcessView, PluginStateEvent, ProcessState},
    },
};
use secrecy::SecretString;
use serde_json::{Value, json};

const PLUGIN: &str = "fake";

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// `target/<profil>/deps/plugin_host-….exe` → `target/<profil>/examples/fake-plugin(.exe)`.
fn fake_plugin() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let path = exe
        .parent()
        .and_then(|deps| deps.parent())
        .unwrap()
        .join("examples")
        .join(format!("fake-plugin{}", std::env::consts::EXE_SUFFIX));
    assert!(path.is_file(), "`cargo build --example fake-plugin` d'abord ({} absent)", path.display());
    path
}

struct Fixture {
    host: Arc<PluginHost>,
    events: Arc<Mutex<Vec<PluginStateEvent>>>,
    notices: Arc<Mutex<Vec<PluginEventPayload>>>,
    dir: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

impl Fixture {
    fn new(name: &str, args: &[&str], idle: Option<Duration>, tweak: impl FnOnce(&mut HostConfig)) -> Self {
        let dir = std::env::temp_dir().join(format!("cadence-host-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let program = if args.contains(&"--binaire-absent") { dir.join("absent.exe") } else { fake_plugin() };
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let (data_root, log_file) = (dir.join("plugins-data"), dir.join("logs").join("fake.log"));
        let resolve = Arc::new(move |plugin: &str| {
            let data_dir = data_root.join(plugin);
            let inherited = std::env::vars().chain([("FAKE_PLUGIN_LEAK".to_string(), "fuite".to_string())]);
            Ok(LaunchSpec {
                plugin: plugin.to_string(),
                program: program.clone(),
                args: args.clone(),
                env: server_env(inherited, "0.3.0", &data_dir, &json!({ "rappel": true })),
                data_dir,
                secrets: vec![("CADENCE_SECRET_TOKEN".to_string(), SecretString::from("s3cret"))],
                idle_shutdown: idle,
                log_file: Some(log_file.clone()),
            })
        });
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let mut config = HostConfig::new("0.3.0");
        config.backoff = vec![ms(50), ms(100), ms(150)];
        config.grace = ms(1000);
        config.start_timeout = Duration::from_secs(15);
        tweak(&mut config);
        let notices = Arc::new(Mutex::new(Vec::new()));
        let relay = notices.clone();
        let host = Arc::new(
            PluginHost::new(config, resolve, Arc::new(move |e| sink.lock().unwrap().push(e)))
                .with_plugin_events(Arc::new(move |e| relay.lock().unwrap().push(e))),
        );
        Self { host, events, notices, dir }
    }

    fn status(&self) -> PluginProcessView {
        self.host.status(PLUGIN)
    }

    /// Attend que l'état du serveur vérifie `done` (10 s au plus).
    async fn wait(&self, what: &str, done: impl Fn(&PluginProcessView) -> bool) -> PluginProcessView {
        let start = Instant::now();
        loop {
            let status = self.status();
            if done(&status) {
                return status;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "{what} : toujours {status:?}");
            tokio::time::sleep(ms(20)).await;
        }
    }

    async fn call(&self, tool: &str, args: Value) -> Result<Value, AppError> {
        let Value::Object(args) = args else { panic!("arguments objet attendus") };
        let result = self.host.call(PLUGIN, tool, Some(args)).await?;
        Ok(result.structured_content.unwrap_or(Value::Null))
    }

    fn states(&self) -> Vec<ProcessState> {
        self.events.lock().unwrap().iter().map(|e| e.state).collect()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn starts_on_demand_calls_in_parallel_and_stops() {
    let f = Fixture::new("cycle", &[], None, |_| {});
    assert_eq!(f.status().state, ProcessState::Stopped, "rien n'est lancé avant le premier appel");

    let tools = f.host.tools(PLUGIN).await.unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(names, ["echo", "slow", "crash", "ui_only", "env", "grow", "fail", "emit"]);
    let ui_only = tools.iter().find(|t| t.name == "ui_only").unwrap();
    assert_eq!(ui_only.meta.as_ref().unwrap().0.get("cadence/visibility"), Some(&json!("ui")));

    let status = f.status();
    assert_eq!((status.state, status.tools), (ProcessState::Ready, 8));
    assert!(status.pid.is_some());
    assert_eq!(f.call("echo", json!({ "text": "bonjour" })).await.unwrap(), json!({ "echo": { "text": "bonjour" } }));

    // Cinq appels de 600 ms en même temps : bien moins que 3 s s'ils partent en parallèle.
    let start = Instant::now();
    let calls = (0..5).map(|_| f.call("slow", json!({ "ms": 600 })));
    for result in futures_all(calls).await {
        assert_eq!(result.unwrap(), json!({ "slept": 600 }));
    }
    assert!(start.elapsed() < ms(2000), "appels sérialisés : {:?}", start.elapsed());

    let env = f.call("env", json!({})).await.unwrap();
    assert_eq!(env["CADENCE_API"], "1");
    assert_eq!(env["CADENCE_VERSION"], "0.3.0");
    assert_eq!(env["CADENCE_LOG"], "info");
    assert_eq!(env["CADENCE_SETTINGS"], r#"{"rappel":true}"#);
    assert_eq!(env["CADENCE_SECRET_TOKEN"], "s3cret");
    assert_eq!(env["FAKE_PLUGIN_LEAK"], Value::Null, "environnement hérité filtré");
    let data_dir = f.dir.join("plugins-data").join(PLUGIN);
    assert_eq!(env["CADENCE_DATA_DIR"], data_dir.to_string_lossy().as_ref());
    assert!(data_dir.is_dir(), "dossier de données créé");
    assert!(env["cwd"].as_str().unwrap().ends_with(PLUGIN), "dossier courant = CADENCE_DATA_DIR : {}", env["cwd"]);

    let err = f.call("fail", json!({})).await.unwrap_err();
    assert!(matches!(&err, AppError::ToolError(m) if m.contains("échec demandé")), "{err:?}");
    let err = f.host.call_with_timeout(PLUGIN, "slow", Some(json!({ "ms": 2000 }).as_object().unwrap().clone()), ms(200)).await.unwrap_err();
    assert!(matches!(err, AppError::ToolError(_)), "délai dépassé : {err:?}");

    f.host.stop(PLUGIN).await;
    let status = f.status();
    assert_eq!((status.state, status.pid, status.tools), (ProcessState::Stopped, None, 0));
    assert_eq!(f.states(), [ProcessState::Starting, ProcessState::Ready, ProcessState::Stopped]);
    let log = fs::read_to_string(f.dir.join("logs").join("fake.log")).unwrap();
    assert!(log.contains("fake-plugin prêt"), "stderr au journal : {log}");

    // Relancé à la demande.
    assert!(f.call("echo", json!({})).await.is_ok());
    assert_eq!(f.status().state, ProcessState::Ready);
    f.host.stop_all().await;
    assert_eq!(f.status().state, ProcessState::Stopped);
}

#[tokio::test(flavor = "multi_thread")]
async fn restarts_after_crashes_then_gives_up() {
    let f = Fixture::new("crash", &[], None, |_| {});
    f.call("echo", json!({})).await.unwrap();

    for restarts in 1..=3 {
        let err = f.call("crash", json!({})).await.unwrap_err();
        assert!(matches!(err, AppError::PluginUnavailable(_)), "appel en cours : {err:?}");
        let status = f.wait("redémarrage", |s| s.state == ProcessState::Ready && s.restarts == restarts).await;
        assert!(status.pid.is_some());
    }
    let crashed = f.events.lock().unwrap().iter().find(|e| e.state == ProcessState::Crashed).cloned().unwrap();
    let message = crashed.message.unwrap();
    assert!(message.contains("code 3"), "{message}");

    // Quatrième arrêt dans la fenêtre : plus de redémarrage automatique.
    f.call("crash", json!({})).await.unwrap_err();
    let status = f.wait("échec", |s| s.state == ProcessState::Failed).await;
    assert!(status.message.as_deref().unwrap().contains("4 arrêts"), "{status:?}");
    let err = f.call("echo", json!({})).await.unwrap_err();
    assert!(matches!(err, AppError::PluginUnavailable(_)), "{err:?}");
    tokio::time::sleep(ms(400)).await;
    assert_eq!(f.status().state, ProcessState::Failed, "aucun redémarrage après l'échec");

    // « Redémarrer » repart de zéro.
    f.host.restart(PLUGIN).await.unwrap();
    assert_eq!(f.status().state, ProcessState::Ready);
    assert!(f.call("echo", json!({})).await.is_ok());
    f.host.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reports_start_failures() {
    // Plantage avant `initialize` : chaque essai compte, puis `Failed`.
    let f = Fixture::new("crash-start", &["--crash-on-start"], None, |c| c.backoff = vec![ms(10), ms(10)]);
    let err = f.host.tools(PLUGIN).await.unwrap_err();
    assert!(matches!(err, AppError::PluginUnavailable(_)), "{err:?}");
    let status = f.wait("échec", |s| s.state == ProcessState::Failed).await;
    assert_eq!(status.restarts, 2);

    // Serveur muet : délai de démarrage dépassé, sans redémarrage (back-off vide).
    let f = Fixture::new("hang", &["--hang-on-start"], None, |c| {
        c.start_timeout = ms(500);
        c.backoff = vec![];
    });
    let err = f.host.tools(PLUGIN).await.unwrap_err();
    assert!(err.to_string().contains("pas prêt"), "{err}");
    assert_eq!(f.status().state, ProcessState::Failed);

    // Binaire absent : échec immédiat, inutile de réessayer.
    let f = Fixture::new("absent", &["--binaire-absent"], None, |_| {});
    let err = f.host.tools(PLUGIN).await.unwrap_err();
    assert!(err.to_string().contains("lancement"), "{err}");
    let status = f.status();
    assert_eq!((status.state, status.restarts), (ProcessState::Failed, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn stops_idle_servers_but_not_busy_ones() {
    let f = Fixture::new("idle", &[], Some(ms(400)), |c| c.idle_check = ms(50));
    tokio::spawn(run_idle_reaper(Arc::downgrade(&f.host)));

    // Un appel plus long que le délai d'inactivité garde le serveur en vie.
    f.call("slow", json!({ "ms": 900 })).await.unwrap();
    assert!(f.events.lock().unwrap().iter().all(|e| e.state != ProcessState::Stopped), "arrêté en plein appel");
    f.wait("arrêt sur inactivité", |s| s.state == ProcessState::Stopped).await;

    // Relancé au prochain appel.
    assert!(f.call("echo", json!({})).await.is_ok());
    f.host.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn relays_plugin_events() {
    let f = Fixture::new("events", &[], None, |_| {});
    f.call("emit", json!({ "event": "sync.updated", "data": { "ids": [12] } })).await.unwrap();
    let start = Instant::now();
    while f.notices.lock().unwrap().is_empty() && start.elapsed() < Duration::from_secs(5) {
        tokio::time::sleep(ms(20)).await;
    }
    let notices = f.notices.lock().unwrap().clone();
    let expected = PluginEventPayload { plugin: PLUGIN.into(), event: "sync.updated".into(), data: Some(json!({ "ids": [12] })) };
    assert_eq!(notices, [expected]);
    f.host.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rereads_tools_on_list_changed() {
    let f = Fixture::new("list-changed", &[], None, |_| {});
    assert_eq!(f.host.tools(PLUGIN).await.unwrap().len(), 8);
    f.call("grow", json!({})).await.unwrap();
    f.wait("outil ajouté", |s| s.tools == 9).await;
    let tools = f.host.tools(PLUGIN).await.unwrap();
    assert!(tools.iter().any(|t| t.name == "extra"));
    assert_eq!(f.call("extra", json!({})).await.unwrap(), json!({ "ok": true }));
    f.host.stop_all().await;
}

/// Lance toutes les futures en même temps et rend leurs résultats dans l'ordre.
async fn futures_all<F, T>(futures: impl Iterator<Item = F>) -> Vec<T>
where
    F: std::future::Future<Output = T>,
{
    let mut pinned: Vec<_> = futures.map(Box::pin).collect();
    let mut results: Vec<Option<T>> = pinned.iter().map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut pending = false;
        for (future, slot) in pinned.iter_mut().zip(results.iter_mut()) {
            if slot.is_none() {
                match future.as_mut().poll(cx) {
                    std::task::Poll::Ready(value) => *slot = Some(value),
                    std::task::Poll::Pending => pending = true,
                }
            }
        }
        if pending { std::task::Poll::Pending } else { std::task::Poll::Ready(()) }
    })
    .await;
    results.into_iter().map(Option::unwrap).collect()
}
