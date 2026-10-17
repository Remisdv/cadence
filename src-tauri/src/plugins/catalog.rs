//! Catalogue en ligne des plugins (plan § 6.2) : `catalog.json` publié sur la release fixe
//! `catalog` du dépôt `RemiSDV/cadence-plugins` (URL surchargeable dans les réglages).
//!
//! - téléchargé avec `If-None-Match` ; le dernier reçu est gardé dans le cache de l'app et
//!   resservi hors ligne ;
//! - comparé aux plugins installés : version installée, mise à jour disponible ;
//! - chaque entrée dit si elle convient à cette machine (`api`, `min_cadence`, binaire pour la
//!   cible) ;
//! - l'archive d'un plugin est téléchargée en flux (500 Mo au plus), puis installée par
//!   `install::prepare_zip` avec le sha256 du catalogue.
//!
//! Champs reconnus en plus de ceux du plan (tous facultatifs, à valider au contrat) :
//! `category`, `color`, `icon` (icône du kit), `official`, `services`, `views`, `actions`,
//! `tools` — de quoi remplir la fiche d'un plugin sans le télécharger.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use semver::Version;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    install::{InstallPhase, MAX_EXTRACTED_BYTES, PluginInstallEvent},
    install::{self, Prepared},
    manifest::{self, ActionMode, PluginCategory, PluginColor, PluginService},
    registry::{PluginRegistry, PluginSource},
};
use crate::{
    domain::version::now_iso,
    error::{AppError, AppResult},
};

/// Catalogue officiel.
pub const DEFAULT_URL: &str = "https://github.com/RemiSDV/cadence-plugins/releases/download/catalog/catalog.json";
/// Âge au-delà duquel le catalogue en cache est relu (vérification au lancement puis toutes les 6 h).
pub const MAX_AGE: Duration = Duration::from_secs(6 * 3600);

#[derive(Debug, Clone, Deserialize)]
pub struct RawCatalog {
    #[serde(default = "api_one")]
    pub api: u32,
    pub generated_at: Option<String>,
    #[serde(default)]
    pub plugins: Vec<RawEntry>,
}

fn api_one() -> u32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    pub api: Option<u32>,
    pub icon_url: Option<String>,
    pub url: String,
    #[serde(default)]
    pub sha256: String,
    pub min_cadence: Option<String>,
    pub targets: Option<Vec<String>>,
    pub homepage: Option<String>,
    pub size: Option<u64>,
    pub category: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    #[serde(default)]
    pub official: bool,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub views: Vec<RawView>,
    #[serde(default)]
    pub actions: Vec<RawAction>,
    #[serde(default)]
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawView {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawAction {
    pub id: String,
    pub label: String,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogViewInfo {
    pub id: String,
    pub title: String,
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogActionInfo {
    pub id: String,
    pub label: String,
    pub mode: ActionMode,
}

/// Plugin du catalogue, comparé à ce qui est installé sur cette machine.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogEntryView {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub icon_url: Option<String>,
    pub homepage: Option<String>,
    /// Taille de l'archive en octets (saturée : pas d'entier 64 bits dans l'IPC).
    pub size: Option<u32>,
    pub category: PluginCategory,
    pub color: PluginColor,
    /// Icône du kit pour la tuile.
    pub rail_icon: String,
    /// Publié par le mainteneur du dépôt.
    pub official: bool,
    pub services: Vec<PluginService>,
    pub views: Vec<CatalogViewInfo>,
    pub actions: Vec<CatalogActionInfo>,
    pub tools: Vec<String>,
    pub min_cadence: Option<String>,
    /// Nom du fichier de l'archive (dernier segment de l'URL).
    pub archive: String,
    /// Installable sur cette machine.
    pub compatible: bool,
    /// Pourquoi il ne l'est pas.
    pub incompatibility: Option<String>,
    /// Version installée, si le plugin l'est.
    pub installed: Option<String>,
    pub update_available: bool,
    pub enabled: Option<bool>,
}

/// Réponse de `catalog_get`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogView {
    pub url: String,
    pub plugins: Vec<CatalogEntryView>,
    /// Date de la dernière réception du catalogue.
    pub fetched_at: Option<String>,
    /// Le catalogue montré vient du cache : le réseau ou le dépôt est injoignable.
    pub offline: bool,
    /// Raison de l'échec de la dernière lecture, s'il y en a un.
    pub error: Option<String>,
    /// Plugins installés qui ont une version plus récente au catalogue.
    pub updates: u32,
}

/// Cibles du catalogue (`windows-x86_64`) ou du manifeste (`x86_64-pc-windows-msvc`).
fn target_matches(target: &str, current: &str) -> bool {
    target == current
        || match current {
            "x86_64-pc-windows-msvc" => target == "windows-x86_64",
            "aarch64-apple-darwin" => target == "macos-aarch64",
            "x86_64-apple-darwin" => target == "macos-x86_64",
            "x86_64-unknown-linux-gnu" => target == "linux-x86_64",
            _ => false,
        }
}

fn enum_value<T: for<'de> Deserialize<'de>>(value: Option<&str>) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(value?.to_string())).ok()
}

/// Entrée du catalogue vue depuis cette machine.
pub fn entry_view(raw: &RawEntry, registry: &PluginRegistry) -> CatalogEntryView {
    let app = registry.app_version();
    let mut incompatibility = None;
    if raw.api.unwrap_or(1) > manifest::API_VERSION {
        incompatibility = Some("Écrit pour une version plus récente de l'API des plugins.".to_string());
    } else if let (Some(min), Ok(current)) = (raw.min_cadence.as_deref(), Version::parse(app))
        && Version::parse(min).is_ok_and(|min| current < min)
    {
        incompatibility = Some(format!("Nécessite Cadence {min} ou plus récent."));
    } else if let Some(targets) = &raw.targets {
        let ok = manifest::current_target().is_some_and(|current| targets.iter().any(|t| target_matches(t, current)));
        if !ok {
            incompatibility = Some("Aucun binaire pour cette machine.".into());
        }
    }
    let entry = registry.entry(&raw.id).ok();
    let installed = entry.as_ref().map(|e| e.version.clone());
    let update_available = match (&entry, Version::parse(&raw.version)) {
        (Some(e), Ok(latest)) if e.source != PluginSource::Dev => Version::parse(&e.version).is_ok_and(|v| v < latest),
        _ => false,
    };
    let rail_icon = raw
        .icon
        .clone()
        .filter(|i| !i.ends_with(".svg"))
        .or_else(|| raw.views.iter().find(|v| !v.hidden).map(|v| v.icon.clone()))
        .filter(|i| !i.is_empty())
        .unwrap_or_else(|| "layers".into());
    CatalogEntryView {
        id: raw.id.clone(),
        name: raw.name.clone(),
        version: raw.version.clone(),
        description: raw.description.clone(),
        authors: raw.authors.clone(),
        icon_url: raw.icon_url.clone(),
        homepage: raw.homepage.clone().filter(|h| h.starts_with("https://")),
        size: raw.size.map(|s| u32::try_from(s).unwrap_or(u32::MAX)),
        category: enum_value(raw.category.as_deref()).unwrap_or(PluginCategory::Outils),
        color: enum_value(raw.color.as_deref()).unwrap_or_else(|| PluginColor::derived(&raw.id)),
        rail_icon,
        official: raw.official,
        services: raw.services.iter().filter_map(|s| enum_value(Some(s))).collect(),
        views: raw
            .views
            .iter()
            .filter(|v| !v.hidden)
            .map(|v| CatalogViewInfo { id: v.id.clone(), title: v.title.clone(), icon: v.icon.clone() })
            .collect(),
        actions: raw
            .actions
            .iter()
            .map(|a| CatalogActionInfo {
                id: a.id.clone(),
                label: a.label.clone(),
                mode: if a.mode.as_deref() == Some("background") { ActionMode::Background } else { ActionMode::Chat },
            })
            .collect(),
        tools: raw.tools.clone(),
        min_cadence: raw.min_cadence.clone(),
        archive: raw.url.rsplit('/').next().unwrap_or_default().to_string(),
        compatible: incompatibility.is_none(),
        incompatibility,
        installed,
        update_available,
        enabled: entry.map(|e| e.enabled),
    }
}

/// Métadonnées du catalogue en cache.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheMeta {
    url: String,
    etag: Option<String>,
    fetched_at: Option<String>,
    /// Secondes depuis l'époque, pour l'âge du cache.
    fetched_secs: u64,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Catalogue en ligne, avec son cache disque.
pub struct Catalog {
    http: reqwest::Client,
    /// `<cache>/catalog.json` et `catalog.meta.json`.
    cache_dir: PathBuf,
}

/// Catalogue lu, et d'où il vient.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub catalog: RawCatalog,
    pub fetched_at: Option<String>,
    pub offline: bool,
    pub error: Option<String>,
}

fn network(err: reqwest::Error) -> AppError {
    AppError::Network(err.to_string())
}

impl Catalog {
    pub fn new(cache_dir: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("Cadence/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .expect("client HTTP");
        Self { http, cache_dir }
    }

    fn meta_path(&self) -> PathBuf {
        self.cache_dir.join("catalog.meta.json")
    }

    fn body_path(&self) -> PathBuf {
        self.cache_dir.join("catalog.json")
    }

    fn cached(&self, url: &str) -> Option<(CacheMeta, RawCatalog)> {
        let meta: CacheMeta = serde_json::from_str(&fs::read_to_string(self.meta_path()).ok()?).ok()?;
        if meta.url != url {
            return None;
        }
        let catalog = serde_json::from_str(&fs::read_to_string(self.body_path()).ok()?).ok()?;
        Some((meta, catalog))
    }

    /// Catalogue de `url` : le cache s'il est récent (et `refresh` faux), sinon le réseau ;
    /// en cas d'échec, le cache marqué hors ligne.
    pub async fn get(&self, url: &str, refresh: bool) -> AppResult<Fetched> {
        let cached = self.cached(url);
        if let Some((meta, catalog)) = &cached
            && !refresh
            && now_secs().saturating_sub(meta.fetched_secs) < MAX_AGE.as_secs()
        {
            return Ok(Fetched { catalog: catalog.clone(), fetched_at: meta.fetched_at.clone(), offline: false, error: None });
        }
        match self.download(url, cached.as_ref().and_then(|(m, _)| m.etag.as_deref())).await {
            Ok(Some((body, etag))) => {
                let catalog: RawCatalog = serde_json::from_slice(&body)
                    .map_err(|e| AppError::Validation(format!("Catalogue illisible ({e}).")))?;
                let fetched_at = now_iso();
                fs::create_dir_all(&self.cache_dir)?;
                fs::write(self.body_path(), &body)?;
                let meta = CacheMeta { url: url.into(), etag, fetched_at: Some(fetched_at.clone()), fetched_secs: now_secs() };
                crate::app_settings::write_atomic(&self.meta_path(), &meta)?;
                Ok(Fetched { catalog, fetched_at: Some(fetched_at), offline: false, error: None })
            }
            Ok(None) => {
                // 304 : le cache est à jour.
                let (mut meta, catalog) = cached.ok_or_else(|| AppError::Internal("304 sans catalogue en cache".into()))?;
                meta.fetched_at = Some(now_iso());
                meta.fetched_secs = now_secs();
                crate::app_settings::write_atomic(&self.meta_path(), &meta)?;
                Ok(Fetched { catalog, fetched_at: meta.fetched_at, offline: false, error: None })
            }
            Err(err) => match cached {
                Some((meta, catalog)) => {
                    Ok(Fetched { catalog, fetched_at: meta.fetched_at, offline: true, error: Some(err.to_string()) })
                }
                None => Err(err),
            },
        }
    }

    /// `None` = 304 (non modifié).
    async fn download(&self, url: &str, etag: Option<&str>) -> AppResult<Option<(Vec<u8>, Option<String>)>> {
        check_url(url)?;
        let mut request = self.http.get(url).timeout(Duration::from_secs(30));
        if let Some(etag) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let response = request.send().await.map_err(network)?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(AppError::Network(format!("catalogue indisponible (HTTP {}).", response.status().as_u16())));
        }
        let etag = response.headers().get(reqwest::header::ETAG).and_then(|v| v.to_str().ok()).map(str::to_string);
        let body = response.bytes().await.map_err(network)?;
        Ok(Some((body.to_vec(), etag)))
    }

    /// Télécharge l'archive d'un plugin en flux vers `target`, avec la progression.
    pub async fn download_archive(
        &self,
        entry: &RawEntry,
        target: &Path,
        progress: &mut (dyn FnMut(PluginInstallEvent) + Send),
    ) -> AppResult<()> {
        check_url(&entry.url)?;
        let mut response = self.http.get(&entry.url).send().await.map_err(network)?;
        if !response.status().is_success() {
            return Err(AppError::Network(format!("archive indisponible (HTTP {}).", response.status().as_u16())));
        }
        let total = response.content_length().or(entry.size).unwrap_or(0);
        if total > MAX_EXTRACTED_BYTES {
            return Err(AppError::Validation("Archive refusée : plus de 500 Mo.".into()));
        }
        if let Some(dir) = target.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut file = fs::File::create(target)?;
        let mut received = 0u64;
        let clamp = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
        while let Some(chunk) = response.chunk().await.map_err(network)? {
            received += chunk.len() as u64;
            if received > MAX_EXTRACTED_BYTES {
                drop(file);
                let _ = fs::remove_file(target);
                return Err(AppError::Validation("Archive refusée : plus de 500 Mo.".into()));
            }
            file.write_all(&chunk)?;
            progress(PluginInstallEvent {
                id: entry.id.clone(),
                received: clamp(received),
                total: clamp(total),
                phase: InstallPhase::Downloading,
            });
        }
        file.flush()?;
        Ok(())
    }
}

/// https seulement (http accepté pour 127.0.0.1 et localhost : catalogue de développement).
fn check_url(url: &str) -> AppResult<()> {
    let parsed = url::Url::parse(url).map_err(|_| AppError::Validation(format!("URL invalide : {url}")))?;
    let local = matches!(parsed.host_str(), Some("127.0.0.1" | "localhost"));
    if parsed.scheme() == "https" || (parsed.scheme() == "http" && local) {
        Ok(())
    } else {
        Err(AppError::Validation("Le catalogue et les archives doivent être servis en https.".into()))
    }
}

/// Vue complète du catalogue pour le front.
pub fn view(url: &str, fetched: &Fetched, registry: &PluginRegistry) -> CatalogView {
    let plugins: Vec<CatalogEntryView> = fetched.catalog.plugins.iter().map(|e| entry_view(e, registry)).collect();
    let updates = u32::try_from(plugins.iter().filter(|p| p.update_available).count()).unwrap_or(u32::MAX);
    CatalogView {
        url: url.into(),
        plugins,
        fetched_at: fetched.fetched_at.clone(),
        offline: fetched.offline,
        error: fetched.error.clone(),
        updates,
    }
}

/// sha256 attendu d'une entrée, s'il ressemble à un sha256.
pub fn expected_sha256(entry: &RawEntry) -> Option<&str> {
    let s = entry.sha256.trim();
    (s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())).then_some(s)
}

impl Catalog {
    /// Télécharge et prépare l'installation (ou la mise à jour) d'un plugin du catalogue :
    /// compatibilité, archive en flux, sha256 du catalogue, manifeste conforme à l'entrée.
    /// L'appelant arrête le serveur du plugin puis appelle `Prepared::commit`.
    pub async fn prepare_install(
        &self,
        registry: Arc<PluginRegistry>,
        catalog: &RawCatalog,
        id: &str,
        mut progress: impl FnMut(PluginInstallEvent) + Send + 'static,
    ) -> AppResult<Prepared> {
        let entry = catalog
            .plugins
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| AppError::Validation(format!("« {id} » n'est pas au catalogue.")))?
            .clone();
        let view = entry_view(&entry, &registry);
        if let Some(why) = view.incompatibility {
            return Err(AppError::Validation(why));
        }
        if !manifest::is_plugin_id(&entry.id) {
            return Err(AppError::Validation(format!("« {} » n'est pas un identifiant de plugin.", entry.id)));
        }
        let zip = registry.root().join(".tmp").join(format!("{}-{}.zip", entry.id, entry.version));
        self.download_archive(&entry, &zip, &mut progress).await?;
        let sha = expected_sha256(&entry).map(str::to_string);
        let prepared = tokio::task::spawn_blocking({
            let (registry, zip) = (registry.clone(), zip.clone());
            move || install::prepare_zip(&registry, &zip, sha.as_deref(), &mut progress)
        })
        .await
        .map_err(|e| AppError::Internal(format!("installation interrompue ({e})")))?;
        let _ = fs::remove_file(&zip);
        let prepared = prepared?;
        if (prepared.id.as_str(), prepared.version.as_str()) != (entry.id.as_str(), entry.version.as_str()) {
            return Err(AppError::Validation(format!(
                "L'archive contient {} {} au lieu de {} {}.",
                prepared.id, prepared.version, entry.id, entry.version
            )));
        }
        Ok(prepared.with_source(PluginSource::Catalog))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::registry::InstalledEntry;

    fn entry(id: &str, version: &str) -> RawEntry {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "version": version, "url": format!("https://x/{id}-{version}.zip"),
            "sha256": "a".repeat(64), "min_cadence": "0.3.0",
            "targets": ["windows-x86_64", "macos-aarch64", "macos-x86_64", "linux-x86_64"],
            "category": "etudes", "views": [{ "id": "a", "title": "A", "icon": "book" }],
            "actions": [{ "id": "r", "label": "Résumer", "mode": "background" }]
        }))
        .unwrap()
    }

    #[test]
    fn compares_entries_with_installed_plugins() {
        let root = crate::app_settings::temp_dir("catalog-view");
        let registry = PluginRegistry::load(root.clone(), "0.3.0");
        let installed = |version: &str, source| InstalledEntry { version: version.into(), previous: None, source, dev_path: None, enabled: true };
        registry.put("documents", installed("0.1.0", PluginSource::Catalog)).unwrap();
        registry.put("journal", installed("0.1.0", PluginSource::Dev)).unwrap();

        let docs = entry_view(&entry("documents", "0.1.1"), &registry);
        assert_eq!((docs.installed.as_deref(), docs.update_available, docs.compatible), (Some("0.1.0"), true, true));
        assert_eq!((docs.category, docs.rail_icon.as_str(), docs.archive.as_str()), (PluginCategory::Etudes, "book", "documents-0.1.1.zip"));
        assert_eq!(docs.actions[0].mode, ActionMode::Background);
        assert!(!entry_view(&entry("journal", "9.0.0"), &registry).update_available, "un plugin de dev n'est jamais mis à jour");
        assert!(entry_view(&entry("autre", "1.0.0"), &registry).installed.is_none());

        let mut newer = entry("x", "1.0.0");
        newer.min_cadence = Some("9.0.0".into());
        assert!(entry_view(&newer, &registry).incompatibility.unwrap().contains("9.0.0"));
        let mut no_binary = entry("x", "1.0.0");
        no_binary.targets = Some(vec!["riscv64".into()]);
        assert!(!entry_view(&no_binary, &registry).compatible);
        let mut future = entry("x", "1.0.0");
        future.api = Some(2);
        assert!(!entry_view(&future, &registry).compatible);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn accepts_https_and_local_http_only() {
        assert!(check_url("https://github.com/a.json").is_ok());
        assert!(check_url("http://127.0.0.1:5000/catalog.json").is_ok());
        assert!(check_url("http://example.org/catalog.json").is_err());
        assert!(check_url("file:///etc/passwd").is_err());
        assert_eq!(expected_sha256(&entry("a", "1.0.0")).map(str::len), Some(64));
        let mut fake = entry("a", "1.0.0");
        fake.sha256 = "…".into();
        assert_eq!(expected_sha256(&fake), None);
    }
}
