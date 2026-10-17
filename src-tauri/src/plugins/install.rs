//! Installation des plugins (plan § 3.2) : archive zip locale, dossier de développement,
//! désinstallation.
//!
//! Une archive est vérifiée (sha256 attendu et fichier `<archive>.sha256` voisin), extraite
//! sans risque dans `<plugins>/.tmp/` (chemins normalisés, ni `..` ni racine ni flux NTFS, liens
//! symboliques refusés, 500 Mo au plus une fois extraite), validée (manifeste, skills, binaire
//! de chaque cible annoncée), puis renommée d'un bloc vers `<plugins>/<id>/<version>/` et
//! inscrite au registre. Aucun processus n'est lancé. L'installation se fait en deux temps,
//! `prepare_zip` puis `Prepared::commit`, pour que l'appelant arrête le serveur du plugin entre
//! les deux (Windows refuse de remplacer un exécutable en cours d'exécution).

use std::{
    fs::{self, File},
    io::{self, BufReader, Read},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use ts_rs::TS;
use zip::ZipArchive;

use super::{
    manifest::{self, Manifest},
    registry::{InstalledEntry, PluginRegistry, PluginSource},
};
use crate::{
    error::{AppError, AppResult},
    secrets::{self, SecretStore},
};

/// Taille maximale d'un plugin extrait.
pub const MAX_EXTRACTED_BYTES: u64 = 500 * 1024 * 1024;

/// Étape d'une installation, pour la barre de progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum InstallPhase {
    /// Téléchargement depuis le catalogue (étape suivante du plan).
    Downloading,
    /// Calcul et comparaison du sha256.
    Verifying,
    Extracting,
    Done,
}

/// Charge de `plugin://install`. Octets en `u32` (pas d'entier 64 bits dans l'IPC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginInstallEvent {
    pub id: String,
    pub received: u32,
    pub total: u32,
    pub phase: InstallPhase,
}

impl PluginInstallEvent {
    fn new(id: &str, phase: InstallPhase, received: u64, total: u64) -> Self {
        let clamp = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
        Self { id: id.to_string(), received: clamp(received), total: clamp(total), phase }
    }
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::Validation(message.into())
}

/// Archive extraite et validée, prête à prendre sa place. Supprimée si on l'abandonne.
#[derive(Debug)]
pub struct Prepared {
    pub id: String,
    pub version: String,
    /// Dossier racine du plugin extrait (contient `plugin.toml`).
    root: PathBuf,
    /// Dossier temporaire à supprimer après coup.
    staging: PathBuf,
    /// Provenance inscrite au registre (`file` pour une archive locale).
    source: PluginSource,
}

impl Drop for Prepared {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.staging);
    }
}

fn nonce() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

/// sha256 d'un fichier en hexadécimal minuscule ; `progress(lus, total)` pendant la lecture.
pub fn sha256_file(path: &Path, mut progress: impl FnMut(u64, u64)) -> AppResult<String> {
    let file = File::open(path)?;
    let total = file.metadata()?.len();
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let mut read = 0u64;
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        read += n as u64;
        progress(read, total);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Premier mot d'un fichier `.sha256` (`<hex>  <nom>` ou `<hex>`), s'il ressemble à un sha256.
fn sidecar_sha256(zip: &Path) -> AppResult<Option<String>> {
    let mut name = zip.as_os_str().to_owned();
    name.push(".sha256");
    let Ok(text) = fs::read_to_string(PathBuf::from(name)) else { return Ok(None) };
    let hex = text.split_whitespace().next().unwrap_or_default().to_ascii_lowercase();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid("Le fichier .sha256 voisin de l'archive est illisible."));
    }
    Ok(Some(hex))
}

/// Chemin d'une entrée de l'archive, relatif à la racine du plugin ; `None` hors de la racine.
/// Refuse tout ce qui pourrait sortir du dossier d'extraction.
fn entry_path(name: &str, enclosed: Option<PathBuf>, prefix: &str) -> AppResult<Option<PathBuf>> {
    let unsafe_path = || invalid(format!("Archive refusée : chemin dangereux « {name} »."));
    // `enclosed_name` ramène `/etc/x` à `etc/x` et résout `a/../b` : on refuse avant lui
    // toute racine et tout `..`, même inoffensif.
    if name.starts_with(['/', '\\']) || name.split(['/', '\\']).any(|part| part == "..") {
        return Err(unsafe_path());
    }
    let enclosed = enclosed.ok_or_else(unsafe_path)?;
    let mut parts = Vec::new();
    for component in enclosed.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str().ok_or_else(unsafe_path)?;
                if part.contains(':') || part.contains('\\') {
                    return Err(unsafe_path());
                }
                parts.push(part.to_string());
            }
            Component::CurDir => {}
            _ => return Err(unsafe_path()),
        }
    }
    let relative = parts.join("/");
    let Some(rest) = (if prefix.is_empty() { Some(relative.as_str()) } else { relative.strip_prefix(prefix) }) else {
        return Ok(None);
    };
    Ok((!rest.is_empty()).then(|| PathBuf::from(rest)))
}

/// Préfixe de la racine du plugin dans l'archive : `""` si `plugin.toml` est à la racine,
/// `"<dossier>/"` si l'archive contient un seul dossier qui le porte.
fn root_prefix<R: Read + io::Seek>(archive: &ZipArchive<R>) -> AppResult<String> {
    let names = archive
        .file_names()
        .map(|n| n.map(|n| n.into_owned()))
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| invalid(format!("Archive zip invalide ({e}).")))?;
    if names.iter().any(|n| n == manifest::FILE_NAME) {
        return Ok(String::new());
    }
    let nested: Vec<&str> = names
        .iter()
        .filter_map(|n| n.strip_suffix(manifest::FILE_NAME))
        .filter(|dir| dir.ends_with('/') && dir.matches('/').count() == 1)
        .collect();
    match nested.as_slice() {
        [dir] => Ok(dir.to_string()),
        [] => Err(invalid(format!("Archive refusée : `{}` introuvable.", manifest::FILE_NAME))),
        _ => Err(invalid(format!("Archive refusée : plusieurs `{}`.", manifest::FILE_NAME))),
    }
}

/// Vérifie, extrait et valide une archive locale. `expected_sha256` vient du catalogue (ou de
/// l'utilisateur) ; un fichier `<archive>.sha256` voisin est comparé lui aussi.
pub fn prepare_zip(
    registry: &PluginRegistry,
    zip_path: &Path,
    expected_sha256: Option<&str>,
    progress: &mut dyn FnMut(PluginInstallEvent),
) -> AppResult<Prepared> {
    prepare_zip_limited(registry, zip_path, expected_sha256, progress, MAX_EXTRACTED_BYTES)
}

fn prepare_zip_limited(
    registry: &PluginRegistry,
    zip_path: &Path,
    expected_sha256: Option<&str>,
    progress: &mut dyn FnMut(PluginInstallEvent),
    max_bytes: u64,
) -> AppResult<Prepared> {
    let file = File::open(zip_path).map_err(|e| invalid(format!("Archive illisible ({e}).")))?;
    let mut archive = ZipArchive::new(BufReader::new(file)).map_err(|e| invalid(format!("Archive zip invalide ({e}).")))?;

    // Identité d'abord, pour nommer la progression et refuser tôt un manifeste invalide.
    let prefix = root_prefix(&archive)?;
    let manifest_text = {
        let mut entry = archive
            .by_name(&format!("{prefix}{}", manifest::FILE_NAME))
            .map_err(|e| invalid(format!("`{}` illisible ({e}).", manifest::FILE_NAME)))?;
        let mut text = String::new();
        entry.by_ref().take(1024 * 1024).read_to_string(&mut text)?;
        text
    };
    let parsed = manifest::parse(&manifest_text, registry.app_version())?;
    let (id, version) = (parsed.id.clone(), parsed.version.clone());
    if !prefix.is_empty() && prefix.trim_end_matches('/') != id {
        return Err(invalid(format!("Archive refusée : le dossier « {} » ne porte pas l'id « {id} ».", prefix.trim_end_matches('/'))));
    }

    progress(PluginInstallEvent::new(&id, InstallPhase::Verifying, 0, 0));
    let actual = sha256_file(zip_path, |read, total| progress(PluginInstallEvent::new(&id, InstallPhase::Verifying, read, total)))?;
    for (expected, origin) in [
        (expected_sha256.map(|s| s.trim().to_ascii_lowercase()), "attendu"),
        (sidecar_sha256(zip_path)?, "du fichier .sha256"),
    ] {
        if let Some(expected) = expected
            && expected != actual
        {
            return Err(invalid(format!("Empreinte sha256 différente de celle {origin} : archive corrompue ou modifiée.")));
        }
    }

    let declared: u64 = (0..archive.len())
        .filter_map(|i| archive.by_index_raw(i).ok().map(|e| e.size()))
        .fold(0u64, u64::saturating_add);
    if declared > max_bytes {
        return Err(invalid("Archive refusée : plus de 500 Mo une fois extraite."));
    }

    let staging = registry.root().join(".tmp").join(format!("{id}-{version}-{}", nonce()));
    fs::create_dir_all(&staging)?;
    let mut prepared =
        Prepared { id: id.clone(), version: version.clone(), root: staging.join(&id), staging, source: PluginSource::File };
    fs::create_dir_all(&prepared.root)?;

    let mut written = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| invalid(format!("Archive zip invalide ({e}).")))?;
        let name = entry.name().map_err(|e| invalid(format!("Archive zip invalide ({e}).")))?.into_owned();
        if entry.is_symlink() {
            return Err(invalid(format!("Archive refusée : lien symbolique « {name} ».")));
        }
        let Some(relative) = entry_path(&name, entry.enclosed_name(), &prefix)? else { continue };
        let target = prepared.root.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&target)?;
        let budget = max_bytes - written;
        let copied = io::copy(&mut entry.by_ref().take(budget + 1), &mut out)?;
        written += copied;
        if copied > budget {
            return Err(invalid("Archive refusée : plus de 500 Mo une fois extraite."));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o755 | 0o600))?;
            }
        }
        progress(PluginInstallEvent::new(&id, InstallPhase::Extracting, written, declared));
    }

    let extracted = manifest::load(&prepared.root, registry.app_version())?;
    if (extracted.id.as_str(), extracted.version.as_str()) != (id.as_str(), version.as_str()) {
        return Err(invalid("Archive refusée : le manifeste extrait ne correspond pas."));
    }
    manifest::check_all_targets(&prepared.root, &extracted)?;
    make_binaries_executable(&prepared.root, &extracted)?;
    prepared.version = extracted.version;
    Ok(prepared)
}

#[cfg(unix)]
fn make_binaries_executable(dir: &Path, m: &Manifest) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(server) = &m.server {
        for target in &server.targets {
            fs::set_permissions(server.binary(dir, target), fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_binaries_executable(_dir: &Path, _m: &Manifest) -> AppResult<()> {
    Ok(())
}

impl Prepared {
    /// Provenance à inscrire au registre (catalogue).
    pub fn with_source(mut self, source: PluginSource) -> Self {
        self.source = source;
        self
    }

    /// Met le plugin en place (`<plugins>/<id>/<version>/`) et l'inscrit au registre. La version
    /// précédente est gardée pour un retour arrière, les plus anciennes sont supprimées.
    pub fn commit(self, registry: &PluginRegistry) -> AppResult<()> {
        let plugin_dir = registry.root().join(&self.id);
        let final_dir = plugin_dir.join(&self.version);
        fs::create_dir_all(&plugin_dir)?;

        // Même version déjà là (réinstallation) : on la met de côté le temps du renommage.
        let old = self.staging.join("previous");
        let replaced = final_dir.exists();
        if replaced {
            fs::rename(&final_dir, &old)?;
        }
        if let Err(err) = fs::rename(&self.root, &final_dir) {
            if replaced {
                let _ = fs::rename(&old, &final_dir);
            }
            return Err(err.into());
        }

        let existing = registry.entry(&self.id).ok();
        let previous = match &existing {
            Some(e) if e.source != PluginSource::Dev && e.version != self.version => Some(e.version.clone()),
            Some(e) if e.source != PluginSource::Dev => e.previous.clone(),
            _ => None,
        };
        let entry = InstalledEntry {
            version: self.version.clone(),
            previous: previous.clone(),
            source: self.source,
            dev_path: None,
            enabled: existing.as_ref().is_none_or(|e| e.enabled),
        };
        registry.put(&self.id, entry)?;

        // Ménage : seules la version installée et la précédente restent.
        if let Ok(versions) = fs::read_dir(&plugin_dir) {
            for dir in versions.flatten() {
                let name = dir.file_name().to_string_lossy().into_owned();
                if name != self.version && Some(&name) != previous.as_ref() {
                    let _ = fs::remove_dir_all(dir.path());
                }
            }
        }
        Ok(())
    }
}

/// Inscrit un dossier local non zippé comme plugin de développement (il n'est jamais copié ni
/// supprimé par Cadence). Le dossier doit porter l'id du plugin.
pub fn install_dev(registry: &PluginRegistry, dir: &Path) -> AppResult<String> {
    let dir = std::path::absolute(dir)?;
    if !dir.is_dir() {
        return Err(invalid(format!("« {} » n'est pas un dossier.", dir.display())));
    }
    let m = manifest::load(&dir, registry.app_version())?;
    let folder = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if folder != m.id {
        return Err(invalid(format!("Le dossier « {folder} » doit porter l'id du plugin (« {} »).", m.id)));
    }
    let existing = registry.entry(&m.id).ok();
    registry.put(
        &m.id,
        InstalledEntry {
            version: m.version.clone(),
            previous: existing.as_ref().and_then(|e| e.previous.clone()),
            source: PluginSource::Dev,
            dev_path: Some(dir.to_string_lossy().into_owned()),
            enabled: existing.as_ref().is_none_or(|e| e.enabled),
        },
    )?;
    Ok(m.id)
}

/// Supprime les fichiers d'un plugin installé (jamais le dossier d'un plugin de dev) et son
/// entrée du registre ; avec `secrets`, supprime aussi ses secrets du trousseau. Ses réglages
/// et ses données (`plugins-data/<id>/`) restent. Le serveur doit être arrêté avant.
pub fn uninstall(registry: &PluginRegistry, id: &str, secrets: Option<&dyn SecretStore>) -> AppResult<()> {
    let entry = registry.entry(id)?;
    if let Some(store) = secrets
        && let Ok(m) = registry.manifest(id)
    {
        for secret in &m.secrets {
            store.delete(&secrets::plugin_account(id, &secret.id)?)?;
        }
    }
    if entry.source != PluginSource::Dev {
        match fs::remove_dir_all(registry.root().join(id)) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(AppError::Internal(format!("Fichiers du plugin « {id} » non supprimés ({err})."))),
        }
    }
    registry.remove(id)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Write;

    use secrecy::ExposeSecret;
    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::*;
    use crate::secrets::MemorySecrets;

    pub(crate) fn manifest_text(id: &str, version: &str) -> String {
        format!(
            "api = 1\nid = \"{id}\"\nname = \"Plugin {id}\"\nversion = \"{version}\"\nauthors = [\"a\"]\nmin_cadence = \"0.1.0\"\n\
             category = \"outils\"\n\n[server]\nbin = \"srv\"\n\n[[secrets]]\nid = \"token\"\nlabel = \"Jeton\"\n\n\
             [[actions]]\nid = \"resumer\"\nlabel = \"Résumer\"\nskill = \"resumer\"\nmode = \"chat\"\n"
        )
    }

    /// Fichiers d'un plugin complet (binaire factice pour les quatre cibles).
    pub(crate) fn plugin_files(id: &str, version: &str) -> Vec<(String, Vec<u8>)> {
        let mut files = vec![
            ("plugin.toml".to_string(), manifest_text(id, version).into_bytes()),
            ("skills/resumer/SKILL.md".into(), b"---\nname: resumer\ndescription: Resume.\n---\n".to_vec()),
        ];
        for target in manifest::TARGETS {
            let bin = if target.contains("windows") { "srv.exe" } else { "srv" };
            files.push((format!("bin/{target}/{bin}"), b"binaire".to_vec()));
        }
        files
    }

    fn write_zip(path: &Path, prefix: &str, files: &[(String, Vec<u8>)]) {
        let mut zip = ZipWriter::new(File::create(path).unwrap());
        for (name, bytes) in files {
            zip.start_file(format!("{prefix}{name}"), SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    fn setup(name: &str) -> (PathBuf, PluginRegistry) {
        let root = crate::app_settings::temp_dir(name);
        fs::create_dir_all(&root).unwrap();
        let registry = PluginRegistry::load(root.join("plugins"), "0.3.0");
        (root, registry)
    }

    fn install(registry: &PluginRegistry, zip: &Path, sha: Option<&str>) -> AppResult<String> {
        let mut events = Vec::new();
        let prepared = prepare_zip(registry, zip, sha, &mut |e| events.push(e))?;
        assert!(events.iter().any(|e| e.phase == InstallPhase::Extracting));
        let id = prepared.id.clone();
        prepared.commit(registry)?;
        Ok(id)
    }

    #[test]
    fn installs_updates_and_uninstalls_an_archive() {
        let (root, registry) = setup("plugin-install");
        let zip = root.join("journal-0.1.0.zip");
        write_zip(&zip, "", &plugin_files("journal", "0.1.0"));
        let sha = sha256_file(&zip, |_, _| {}).unwrap();

        assert!(install(&registry, &zip, Some(&"0".repeat(64))).is_err(), "sha256 différent");
        assert_eq!(install(&registry, &zip, Some(&sha.to_uppercase())).unwrap(), "journal");
        let dir = registry.root().join("journal/0.1.0");
        assert!(dir.join("skills/resumer/SKILL.md").is_file());
        let entry = registry.entry("journal").unwrap();
        assert_eq!((entry.version.as_str(), entry.source, entry.previous.as_ref()), ("0.1.0", PluginSource::File, None));
        assert_eq!(fs::read_dir(registry.root().join(".tmp")).unwrap().count(), 0, "dossier temporaire nettoyé");

        // Réinstallation de la même version, puis deux mises à jour : seules deux versions restent.
        registry.set_enabled("journal", false).unwrap();
        install(&registry, &zip, None).unwrap();
        for version in ["0.2.0", "0.3.0"] {
            let update = root.join(format!("journal-{version}.zip"));
            write_zip(&update, "journal/", &plugin_files("journal", version));
            install(&registry, &update, None).unwrap();
        }
        let entry = registry.entry("journal").unwrap();
        assert_eq!((entry.version.as_str(), entry.previous.as_deref(), entry.enabled), ("0.3.0", Some("0.2.0"), false));
        let mut versions: Vec<String> =
            fs::read_dir(registry.root().join("journal")).unwrap().map(|d| d.unwrap().file_name().to_string_lossy().into_owned()).collect();
        versions.sort();
        assert_eq!(versions, ["0.2.0", "0.3.0"]);

        let store = MemorySecrets::default();
        store.set("plugin/journal/token", "s3cret").unwrap();
        store.set("plugin/autre/token", "garde").unwrap();
        uninstall(&registry, "journal", Some(&store)).unwrap();
        assert!(!registry.root().join("journal").exists());
        assert!(registry.entry("journal").is_err());
        assert!(!store.has("plugin/journal/token").unwrap());
        assert_eq!(store.get("plugin/autre/token").unwrap().unwrap().expose_secret(), "garde");
        assert!(uninstall(&registry, "journal", None).is_err(), "déjà désinstallé");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn checks_the_sidecar_sha256() {
        let (root, registry) = setup("plugin-install-sidecar");
        let zip = root.join("journal.zip");
        write_zip(&zip, "", &plugin_files("journal", "0.1.0"));
        fs::write(root.join("journal.zip.sha256"), format!("{}  journal.zip\n", "a".repeat(64))).unwrap();
        assert!(install(&registry, &zip, None).unwrap_err().to_string().contains(".sha256"));
        let sha = sha256_file(&zip, |_, _| {}).unwrap();
        fs::write(root.join("journal.zip.sha256"), format!("{sha}\n")).unwrap();
        assert!(install(&registry, &zip, None).is_ok());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_unsafe_or_invalid_archives() {
        let (root, registry) = setup("plugin-install-unsafe");
        let refuse = |name: &str, files: Vec<(String, Vec<u8>)>, prefix: &str, why: &str| {
            let zip = root.join(format!("{name}.zip"));
            write_zip(&zip, prefix, &files);
            let err = install(&registry, &zip, None).expect_err(name);
            assert!(err.to_string().contains(why), "{name} : {err}");
        };
        let with = |extra: (&str, &[u8])| {
            let mut files = plugin_files("journal", "0.1.0");
            files.push((extra.0.to_string(), extra.1.to_vec()));
            files
        };
        refuse("slip", with(("../evil.txt", b"x")), "", "chemin dangereux");
        refuse("slip-nested", with(("skills/../../evil.txt", b"x")), "", "chemin dangereux");
        refuse("absolu", with(("/etc/evil", b"x")), "", "chemin dangereux");
        refuse("flux", with(("ui/index.html:secret", b"x")), "", "chemin dangereux");
        refuse("sans-manifeste", plugin_files("journal", "0.1.0").into_iter().skip(1).collect(), "", "introuvable");
        refuse("mauvais-dossier", plugin_files("journal", "0.1.0"), "autre/", "ne porte pas l'id");
        let no_linux: Vec<_> = plugin_files("journal", "0.1.0").into_iter().filter(|(n, _)| !n.contains("linux")).collect();
        refuse("cible-manquante", no_linux, "", "Binaire du serveur absent");
        let no_skill: Vec<_> = plugin_files("journal", "0.1.0").into_iter().filter(|(n, _)| !n.starts_with("skills")).collect();
        refuse("skill-manquant", no_skill, "", "skill");

        // Lien symbolique.
        let zip_path = root.join("lien.zip");
        let mut zip = ZipWriter::new(File::create(&zip_path).unwrap());
        for (name, bytes) in plugin_files("journal", "0.1.0") {
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            zip.write_all(&bytes).unwrap();
        }
        zip.add_symlink("ui/lien", "/etc/passwd", SimpleFileOptions::default()).unwrap();
        zip.finish().unwrap();
        assert!(install(&registry, &zip_path, None).unwrap_err().to_string().contains("lien symbolique"));

        assert!(registry.snapshot().plugins.is_empty(), "rien d'inscrit");
        assert!(!registry.root().join("journal").exists(), "rien de mis en place");
        assert_eq!(fs::read_dir(registry.root().join(".tmp")).unwrap().count(), 0, "extractions abandonnées supprimées");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_archives_over_the_size_limit() {
        // Limite abaissée pour le test : 64 Kio. Les données compressées (zéros) dépassent la
        // limite une fois extraites, sans que l'archive soit grosse.
        let (root, registry) = setup("plugin-install-size");
        let zip_path = root.join("gros.zip");
        let mut files = plugin_files("journal", "0.1.0");
        files.push(("data/zeros.bin".into(), vec![0u8; 128 * 1024]));
        let mut zip = ZipWriter::new(File::create(&zip_path).unwrap());
        for (name, bytes) in &files {
            let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        assert!(fs::metadata(&zip_path).unwrap().len() < 64 * 1024);
        let err = prepare_zip_limited(&registry, &zip_path, None, &mut |_| {}, 64 * 1024).unwrap_err();
        assert!(err.to_string().contains("500 Mo"), "{err}");
        assert!(prepare_zip_limited(&registry, &zip_path, None, &mut |_| {}, 256 * 1024).is_ok());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn registers_a_dev_folder_without_copying_it() {
        let (root, registry) = setup("plugin-install-dev");
        let dev = root.join("src").join("journal");
        for (name, bytes) in plugin_files("journal", "0.1.0") {
            let path = dev.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        assert_eq!(install_dev(&registry, &dev).unwrap(), "journal");
        let entry = registry.entry("journal").unwrap();
        assert_eq!(entry.source, PluginSource::Dev);
        assert_eq!(registry.plugin_dir("journal", &entry), std::path::absolute(&dev).unwrap());
        assert!(!registry.root().join("journal").exists(), "aucune copie");
        uninstall(&registry, "journal", None).unwrap();
        assert!(dev.join("plugin.toml").is_file(), "le dossier de dev n'est jamais supprimé");

        let renamed = root.join("src").join("autre-nom");
        fs::rename(&dev, &renamed).unwrap();
        assert!(install_dev(&registry, &renamed).unwrap_err().to_string().contains("porter l'id"));
        assert!(install_dev(&registry, &root.join("absent")).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_entry_paths_outside_the_root() {
        let ok = |name: &str, prefix: &str| entry_path(name, Some(PathBuf::from(name)), prefix).unwrap();
        assert_eq!(ok("ui/index.html", ""), Some(PathBuf::from("ui/index.html")));
        assert_eq!(ok("journal/ui/index.html", "journal/"), Some(PathBuf::from("ui/index.html")));
        assert_eq!(ok("__MACOSX/x", "journal/"), None, "hors de la racine : ignoré");
        assert_eq!(ok("journal/", "journal/"), None);
        assert!(entry_path("../x", None, "").is_err(), "enclosed_name refusé");
        assert!(entry_path("a:b", Some(PathBuf::from("a:b")), "").is_err());
    }
}
