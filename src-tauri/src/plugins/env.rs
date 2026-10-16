//! Lancement d'un serveur de plugin : exécutable, arguments, dossier et environnement filtré
//! (plan § 3.7, révision 1.1 : un processus par plugin, plus de variables d'espace).
//!
//! | Variable | Valeur |
//! |---|---|
//! | `CADENCE_API` | `1` |
//! | `CADENCE_VERSION` | version de Cadence |
//! | `CADENCE_DATA_DIR` | `<données>/plugins-data/<id>/` (aussi le dossier courant) |
//! | `CADENCE_SETTINGS` | réglages globaux du plugin (JSON, défauts du schéma compris) |
//! | `CADENCE_SECRET_<ID>` | chaque secret déclaré présent au trousseau |
//! | `CADENCE_LOG` | `info` |
//!
//! L'environnement hérité est filtré : seules passent les variables système utiles (chemins,
//! dossiers de l'utilisateur, langue, proxys). Les jetons des IA (`ANTHROPIC_API_KEY`…) et
//! tout le reste ne passent pas.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;

use super::{
    manifest::{self, API_VERSION},
    registry::PluginRegistry,
    settings::{self, PluginSettingsStore},
};
use crate::{
    error::{AppError, AppResult},
    secrets::{self, SecretStore},
};

/// Niveau de journal demandé aux serveurs.
pub const LOG_LEVEL: &str = "info";

/// Variables héritées transmises au serveur (comparées sans casse : Windows ne la distingue pas).
const INHERITED: &[&str] = &[
    "PATH",
    "PATHEXT",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "USERPROFILE",
    "USERNAME",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "TEMP",
    "TMP",
    "TMPDIR",
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_CTYPE",
    "TZ",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "XDG_RUNTIME_DIR",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

/// Ce qu'il faut pour lancer le serveur d'un plugin.
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub plugin: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    /// `CADENCE_DATA_DIR`, créé au lancement ; dossier courant du serveur.
    pub data_dir: PathBuf,
    /// Variables non secrètes, hérités filtrés compris.
    pub env: Vec<(String, String)>,
    /// `CADENCE_SECRET_<ID>` (jamais journalisés : `SecretString` masque sa valeur).
    pub secrets: Vec<(String, SecretString)>,
    /// `None` = jamais arrêté pour inactivité.
    pub idle_shutdown: Option<Duration>,
    /// Journal du serveur (stderr, `notifications/message`).
    pub log_file: Option<PathBuf>,
}

/// Dossiers communs à tous les serveurs.
#[derive(Debug, Clone)]
pub struct HostPaths {
    /// `<données>/plugins-data` : un sous-dossier par plugin.
    pub data_root: PathBuf,
    /// `<journaux>/plugins` : `<id>.log`.
    pub log_dir: PathBuf,
}

/// Variables héritées gardées, dans l'ordre reçu ; `PATH` complété sur macOS.
pub fn filter_inherited(vars: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    let mut kept: Vec<(String, String)> = vars
        .into_iter()
        .filter(|(key, _)| INHERITED.iter().any(|allowed| allowed.eq_ignore_ascii_case(key)))
        .collect();
    if cfg!(target_os = "macos") {
        // Une app graphique n'hérite pas du PATH du shell : emplacements usuels de Homebrew.
        let extra = ["/opt/homebrew/bin", "/usr/local/bin"];
        match kept.iter_mut().find(|(k, _)| k == "PATH") {
            Some((_, path)) => {
                for dir in extra {
                    if !path.split(':').any(|p| p == dir) {
                        path.push(':');
                        path.push_str(dir);
                    }
                }
            }
            None => kept.push(("PATH".into(), format!("/usr/bin:/bin:/usr/sbin:/sbin:{}", extra.join(":")))),
        }
    }
    kept
}

/// `CADENCE_SECRET_<ID>` : l'id du secret en majuscules.
pub fn secret_var(secret: &str) -> String {
    format!("CADENCE_SECRET_{}", secret.to_ascii_uppercase())
}

/// Environnement complet d'un serveur, hors secrets.
pub fn server_env(
    inherited: impl IntoIterator<Item = (String, String)>,
    app_version: &str,
    data_dir: &Path,
    settings: &Value,
) -> Vec<(String, String)> {
    let mut env = filter_inherited(inherited);
    env.extend([
        ("CADENCE_API".to_string(), API_VERSION.to_string()),
        ("CADENCE_VERSION".to_string(), app_version.to_string()),
        ("CADENCE_DATA_DIR".to_string(), data_dir.to_string_lossy().into_owned()),
        ("CADENCE_SETTINGS".to_string(), settings.to_string()),
        ("CADENCE_LOG".to_string(), LOG_LEVEL.to_string()),
    ]);
    env
}

/// Spécification de lancement du serveur d'un plugin installé, lue au moment du démarrage
/// (secrets et réglages à jour). Erreur si le plugin n'a pas de serveur.
pub fn launch_spec(
    registry: &PluginRegistry,
    settings_store: &PluginSettingsStore,
    secret_store: &dyn SecretStore,
    paths: &HostPaths,
    plugin: &str,
) -> AppResult<LaunchSpec> {
    let entry = registry.entry(plugin)?;
    if !entry.enabled {
        return Err(AppError::Validation(format!("Le plugin « {plugin} » est désactivé.")));
    }
    let dir = registry.plugin_dir(plugin, &entry);
    let m = manifest::load(&dir, registry.app_version())?;
    let server = m
        .server
        .as_ref()
        .ok_or_else(|| AppError::Validation(format!("Le plugin « {plugin} » n'a pas de serveur.")))?;
    let target = manifest::current_target()
        .filter(|t| server.targets.iter().any(|s| s == t))
        .ok_or_else(|| AppError::Validation(format!("Le plugin « {plugin} » n'a pas de serveur pour cette machine.")))?;

    let schema = m.settings_schema.as_deref().and_then(|rel| match settings::read_schema(&dir, rel) {
        Ok(schema) => Some(schema),
        Err(err) => {
            tracing::warn!(plugin, %err, "schéma de réglages ignoré au lancement");
            None
        }
    });
    let settings_value = settings_store.view(plugin, schema)?.value;

    let mut secrets = Vec::new();
    for secret in &m.secrets {
        if let Some(value) = secret_store.get(&secrets::plugin_account(plugin, &secret.id)?)? {
            secrets.push((secret_var(&secret.id), value));
        }
    }

    let data_dir = paths.data_root.join(plugin);
    Ok(LaunchSpec {
        plugin: plugin.to_string(),
        program: server.binary(&dir, target),
        args: server.args.clone(),
        env: server_env(std::env::vars(), registry.app_version(), &data_dir, &settings_value),
        data_dir,
        secrets,
        idle_shutdown: (server.idle_shutdown_s > 0).then(|| Duration::from_secs(server.idle_shutdown_s.into())),
        log_file: Some(paths.log_dir.join(format!("{plugin}.log"))),
    })
}

impl LaunchSpec {
    /// Toutes les variables, secrets compris, pour `Command::envs` après `env_clear`.
    pub fn all_env(&self) -> impl Iterator<Item = (&str, &str)> {
        self.env
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .chain(self.secrets.iter().map(|(k, v)| (k.as_str(), v.expose_secret())))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{plugins::install::tests::plugin_files, secrets::MemorySecrets};

    fn vars(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn keeps_only_system_variables() {
        let inherited = vars(&[
            ("Path", "C:\\Windows"),
            ("SystemRoot", "C:\\Windows"),
            ("HOME", "/home/moi"),
            ("ANTHROPIC_API_KEY", "sk-ant"),
            ("OPENAI_API_KEY", "sk"),
            ("GITHUB_TOKEN", "ghp"),
            ("CADENCE_SECRET_TOKEN", "usurpé"),
            ("https_proxy", "http://proxy:3128"),
        ]);
        let env = server_env(inherited, "0.3.0", Path::new("/data/journal"), &json!({ "a": 1 }));
        let keys: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
        for key in ["Path", "SystemRoot", "HOME", "https_proxy", "CADENCE_API", "CADENCE_VERSION", "CADENCE_DATA_DIR", "CADENCE_SETTINGS", "CADENCE_LOG"] {
            assert!(keys.contains(&key), "{key} manquante dans {keys:?}");
        }
        for key in ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "GITHUB_TOKEN", "CADENCE_SECRET_TOKEN"] {
            assert!(!keys.contains(&key), "{key} ne doit pas passer");
        }
        let get = |k: &str| env.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str());
        assert_eq!(get("CADENCE_API"), Some("1"));
        assert_eq!(get("CADENCE_SETTINGS"), Some(r#"{"a":1}"#));
        assert_eq!(secret_var("github_token"), "CADENCE_SECRET_GITHUB_TOKEN");
    }

    #[test]
    fn builds_the_launch_spec_of_an_installed_plugin() {
        let root = crate::app_settings::temp_dir("plugin-launch-spec");
        let registry = PluginRegistry::load(root.join("plugins"), "0.3.0");
        let dev = root.join("journal");
        for (name, bytes) in plugin_files("journal", "0.1.0") {
            let path = dev.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        crate::plugins::install::install_dev(&registry, &dev).unwrap();
        let settings = PluginSettingsStore::new(root.join("config"));
        settings.write("journal", None, json!({ "rappel": true })).unwrap();
        let store = MemorySecrets::default();
        let paths = HostPaths { data_root: root.join("plugins-data"), log_dir: root.join("logs") };

        let spec = launch_spec(&registry, &settings, &store, &paths, "journal").unwrap();
        assert!(spec.program.starts_with(std::path::absolute(&dev).unwrap().join("bin")));
        assert_eq!(spec.data_dir, root.join("plugins-data").join("journal"));
        assert_eq!(spec.idle_shutdown, Some(Duration::from_secs(600)));
        assert!(spec.secrets.is_empty(), "secret absent du trousseau : pas de variable");
        assert!(spec.env.iter().any(|(k, v)| k == "CADENCE_SETTINGS" && v == r#"{"rappel":true}"#));
        assert_eq!(spec.log_file.as_deref(), Some(root.join("logs").join("journal.log").as_path()));

        store.set("plugin/journal/token", "s3cret").unwrap();
        store.set("plugin/journal/non_declare", "x").unwrap();
        let spec = launch_spec(&registry, &settings, &store, &paths, "journal").unwrap();
        let all: Vec<(&str, &str)> = spec.all_env().collect();
        assert!(all.contains(&("CADENCE_SECRET_TOKEN", "s3cret")));
        assert!(!all.iter().any(|(k, _)| *k == "CADENCE_SECRET_NON_DECLARE"), "seuls les secrets déclarés passent");
        assert!(!format!("{spec:?}").contains("s3cret"), "jamais dans les journaux");
        assert!(launch_spec(&registry, &settings, &store, &paths, "absent").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
