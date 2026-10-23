//! Catalogue en ligne contre un serveur HTTP local : ETag et 304, cache, hors ligne,
//! installation depuis le catalogue (téléchargement en flux, sha256, manifeste conforme).

use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use cadence_lib::plugins::{
    catalog::{self, Catalog},
    install::{InstallPhase, sha256_file},
    registry::{PluginRegistry, PluginSource},
};
use serde_json::json;
use zip::{ZipWriter, write::SimpleFileOptions};

struct Server {
    catalog: String,
    archive: Vec<u8>,
    requests: AtomicUsize,
    down: AtomicBool,
}

async fn catalog_route(State(s): State<Arc<Server>>, headers: HeaderMap) -> Response {
    s.requests.fetch_add(1, Ordering::SeqCst);
    if s.down.load(Ordering::SeqCst) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some("\"v1\"") {
        return StatusCode::NOT_MODIFIED.into_response();
    }
    ([(header::ETAG, "\"v1\""), (header::CONTENT_TYPE, "application/json")], s.catalog.clone()).into_response()
}

async fn archive_route(State(s): State<Arc<Server>>) -> Response {
    s.archive.clone().into_response()
}

fn archive(id: &str, version: &str) -> Vec<u8> {
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("plugin.toml", SimpleFileOptions::default()).unwrap();
    write!(
        zip,
        "api = 1\nid = \"{id}\"\nname = \"Journal\"\nversion = \"{version}\"\nauthors = [\"a\"]\nmin_cadence = \"0.1.0\"\ncategory = \"outils\"\n"
    )
    .unwrap();
    zip.finish().unwrap().into_inner()
}

async fn serve(name: &str, id_in_archive: &str, sha: Option<&str>) -> (String, Arc<Server>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("cadence-catalog-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let bytes = archive(id_in_archive, "0.2.0");
    let zip_path = dir.join("probe.zip");
    std::fs::write(&zip_path, &bytes).unwrap();
    let real_sha = sha256_file(&zip_path, |_, _| {}).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let catalog = json!({ "api": 1, "plugins": [{
        "id": "journal", "name": "Journal", "version": "0.2.0", "description": "Notes",
        "authors": ["Camille"], "url": format!("{base}/journal-0.2.0.zip"),
        "sha256": sha.unwrap_or(&real_sha), "min_cadence": "0.1.0", "category": "outils", "official": true,
        "targets": ["windows-x86_64", "macos-aarch64", "macos-x86_64", "linux-x86_64"]
    }] })
    .to_string();
    let state = Arc::new(Server { catalog, archive: bytes, requests: AtomicUsize::new(0), down: AtomicBool::new(false) });
    let app = Router::new()
        .route("/catalog.json", get(catalog_route))
        .route("/journal-0.2.0.zip", get(archive_route))
        .with_state(state.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("{base}/catalog.json"), state, dir)
}

#[tokio::test(flavor = "multi_thread")]
async fn caches_the_catalog_and_falls_back_offline() {
    let (url, server, dir) = serve("cache", "journal", None).await;
    let catalog = Catalog::new(dir.join("cache"));

    let first = catalog.get(&url, false).await.unwrap();
    assert_eq!((first.catalog.plugins.len(), first.offline), (1, false));
    assert_eq!(server.requests.load(Ordering::SeqCst), 1);

    catalog.get(&url, false).await.unwrap();
    assert_eq!(server.requests.load(Ordering::SeqCst), 1, "cache récent : pas de requête");

    let again = catalog.get(&url, true).await.unwrap();
    assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    assert_eq!(again.catalog.plugins[0].id, "journal", "304 : le cache est resservi");

    server.down.store(true, Ordering::SeqCst);
    let offline = catalog.get(&url, true).await.unwrap();
    assert!(offline.offline);
    assert!(offline.error.unwrap().contains("503"));
    assert_eq!(offline.catalog.plugins.len(), 1);

    let other = Catalog::new(dir.join("vide"));
    assert!(other.get(&url, true).await.is_err(), "ni réseau ni cache : erreur");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn installs_and_updates_from_the_catalog() {
    let (url, _server, dir) = serve("install", "journal", None).await;
    let registry = Arc::new(PluginRegistry::load(dir.join("plugins"), "0.3.0"));
    let catalog = Catalog::new(dir.join("cache"));
    let raw = catalog.get(&url, false).await.unwrap().catalog;

    let view = catalog::view(&url, &catalog.get(&url, false).await.unwrap(), &registry);
    assert_eq!((view.plugins[0].installed.as_deref(), view.plugins[0].official), (None, true));

    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = events.clone();
    let prepared = catalog
        .prepare_install(registry.clone(), &raw, "journal", move |e| sink.lock().unwrap().push(e.phase))
        .await
        .unwrap();
    prepared.commit(&registry).unwrap();
    let entry = registry.entry("journal").unwrap();
    assert_eq!((entry.version.as_str(), entry.source), ("0.2.0", PluginSource::Catalog));
    let phases = events.lock().unwrap().clone();
    assert!(phases.contains(&InstallPhase::Downloading) && phases.contains(&InstallPhase::Extracting), "{phases:?}");

    let view = catalog::view(&url, &catalog.get(&url, false).await.unwrap(), &registry);
    assert_eq!((view.plugins[0].installed.as_deref(), view.updates), (Some("0.2.0"), 0));
    assert!(catalog.prepare_install(registry.clone(), &raw, "absent", |_| {}).await.is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn refuses_a_tampered_or_mismatched_archive() {
    let (url, _server, dir) = serve("tampered", "journal", Some(&"0".repeat(64))).await;
    let registry = Arc::new(PluginRegistry::load(dir.join("plugins"), "0.3.0"));
    let catalog = Catalog::new(dir.join("cache"));
    let raw = catalog.get(&url, false).await.unwrap().catalog;
    let err = catalog.prepare_install(registry.clone(), &raw, "journal", |_| {}).await.unwrap_err();
    assert!(err.to_string().contains("sha256"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);

    let (url, _server, dir) = serve("mismatch", "autre", None).await;
    let registry = Arc::new(PluginRegistry::load(dir.join("plugins"), "0.3.0"));
    let catalog = Catalog::new(dir.join("cache"));
    let raw = catalog.get(&url, false).await.unwrap().catalog;
    let err = catalog.prepare_install(registry.clone(), &raw, "journal", |_| {}).await.unwrap_err();
    assert!(err.to_string().contains("au lieu de"), "{err}");
    assert!(registry.entry("journal").is_err(), "rien d'inscrit");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Catalogue réel de `RemiSDV/cadence-plugins` : chaque plugin publié se télécharge, son
/// empreinte et son manifeste sont validés, puis il s'installe (réseau, ignoré par défaut).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "réseau : release `catalog` de RemiSDV/cadence-plugins"]
async fn installs_every_plugin_of_the_official_catalog() {
    let dir = std::env::temp_dir().join(format!("cadence-official-catalog-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let catalog = Catalog::new(dir.join("cache"));
    let fetched = catalog.get(cadence_lib::plugins::catalog::DEFAULT_URL, true).await.unwrap();
    assert!(fetched.error.is_none(), "{:?}", fetched.error);
    let registry = Arc::new(PluginRegistry::load(dir.join("plugins"), "0.3.0"));
    assert!(!fetched.catalog.plugins.is_empty());
    for entry in &fetched.catalog.plugins {
        let prepared = catalog
            .prepare_install(registry.clone(), &fetched.catalog, &entry.id, |_| {})
            .await
            .unwrap_or_else(|e| panic!("{} : {e}", entry.id));
        prepared.commit(&registry).unwrap();
        let manifest = registry.manifest(&entry.id).unwrap_or_else(|e| panic!("{} : {e}", entry.id));
        // Page d'entrée servie par le protocole `cadence-plugin` (`<id>/<version>/`).
        let page = cadence_lib::protocol::resolve(&registry, &format!("/{}/{}/", manifest.id, manifest.version));
        assert!(page.is_some_and(|p| p.is_file()), "interface de {} introuvable", manifest.id);
        println!("{} {} installé · {} vues · {} actions · {} skills", manifest.id, manifest.version, manifest.views.len(), manifest.actions.len(), manifest.skills.len());
    }

    // Serveurs MCP réels, lancés par l'hôte avec l'environnement de Cadence.
    use cadence_lib::{
        plugins::{
            env::{HostPaths, launch_spec},
            host::{HostConfig, PluginHost},
            settings::PluginSettingsStore,
        },
        secrets::MemorySecrets,
    };
    let settings = Arc::new(PluginSettingsStore::new(dir.join("config")));
    let secrets = Arc::new(MemorySecrets::default());
    let paths = HostPaths { data_root: dir.join("plugins-data"), log_dir: dir.join("logs"), tools: None, repos: None };
    let resolve = {
        let (registry, settings, secrets) = (registry.clone(), settings.clone(), secrets.clone());
        Arc::new(move |plugin: &str| launch_spec(&registry, &settings, secrets.as_ref(), &paths, plugin))
    };
    let host = PluginHost::new(HostConfig::new("0.3.0"), resolve, Arc::new(|_| {}));
    for entry in &fetched.catalog.plugins {
        let tools = host.tools(&entry.id).await.unwrap_or_else(|e| panic!("serveur de {} : {e}", entry.id));
        println!("{} : {} outils ({})", entry.id, tools.len(), tools.iter().take(6).map(|t| t.name.to_string()).collect::<Vec<_>>().join(", "));
        assert!(!tools.is_empty(), "{} n'expose aucun outil", entry.id);
    }
    host.stop_all().await;
    let _ = std::fs::remove_dir_all(&dir);
}
