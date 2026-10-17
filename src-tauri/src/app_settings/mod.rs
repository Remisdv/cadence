//! Préférences propres à la machine (`<config de l'app>/settings.json`, format v2) : IA par
//! défaut, chemins et activation des CLI d'IA, IA par action, URL du catalogue, plugins en
//! mode développement. Aucun secret ici (ils vont au trousseau, voir `secrets`).
//!
//! Le format v1 (`local_settings.rs` de Cadence 1) est converti au premier chargement
//! (`migrate.rs`) ; l'ancien fichier est gardé à côté sous `settings.v1.json`.

pub mod migrate;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    auth::cli::CliTool,
    domain::project::ProjectState,
    error::{AppError, AppResult},
    tools::ToolId,
};

/// Version du format écrit sur disque.
pub const VERSION: u32 = 2;

/// IA de code prise en charge (adaptateurs du plan Cadence 2, § 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AiId {
    ClaudeCode,
    Codex,
    Opencode,
}

impl AiId {
    pub const ALL: [AiId; 3] = [AiId::ClaudeCode, AiId::Codex, AiId::Opencode];
}

impl From<CliTool> for AiId {
    fn from(tool: CliTool) -> Self {
        match tool {
            CliTool::ClaudeCode => AiId::ClaudeCode,
            CliTool::Codex => AiId::Codex,
        }
    }
}

/// Réglages d'une CLI d'IA sur cette machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AiCliSettings {
    /// Chemin choisi à la main (« Localiser le binaire… ») ; sinon détection automatique.
    pub path: Option<String>,
    /// Faux après « Déconnecter » / « Ignorer » : l'IA n'est plus proposée.
    pub enabled: bool,
}

impl Default for AiCliSettings {
    fn default() -> Self {
        Self { path: None, enabled: true }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AiSettings {
    pub claude_code: AiCliSettings,
    pub codex: AiCliSettings,
    pub opencode: AiCliSettings,
}

impl AiSettings {
    pub fn get(&self, ai: AiId) -> &AiCliSettings {
        match ai {
            AiId::ClaudeCode => &self.claude_code,
            AiId::Codex => &self.codex,
            AiId::Opencode => &self.opencode,
        }
    }

    pub fn get_mut(&mut self, ai: AiId) -> &mut AiCliSettings {
        match ai {
            AiId::ClaudeCode => &mut self.claude_code,
            AiId::Codex => &mut self.codex,
            AiId::Opencode => &mut self.opencode,
        }
    }
}

/// Ce que Cadence 1 avait mémorisé et que le plugin Gestion de projet reprendra (plan § 9.4) :
/// le dépôt ouvert deviendra son premier dépôt suivi. Absent du contrat TS.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Legacy {
    pub last_project: Option<ProjectState>,
}

impl Legacy {
    pub fn is_empty(&self) -> bool {
        self.last_project.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AppSettings {
    /// Version du format (toujours `VERSION` une fois chargé).
    pub version: u32,
    /// Onboarding de Cadence 2 terminé (IA choisie, plugins proposés).
    pub onboarded: bool,
    /// IA lancée par défaut (chat et actions) ; `None` tant qu'aucune n'a été choisie.
    pub default_ai: Option<AiId>,
    pub ai: AiSettings,
    /// IA imposée pour une action, clé `<plugin>/<action>`.
    pub action_ai: BTreeMap<String, AiId>,
    /// Catalogue des plugins ; `None` = catalogue public `RemiSDV/cadence-plugins`.
    pub catalog_url: Option<String>,
    /// Dossiers de plugins chargés en mode développement (non zippés).
    pub dev_plugins: Vec<String>,
    /// Chemin choisi à la main pour un outil hors IA (Git, GitHub CLI, Node.js).
    pub tool_paths: BTreeMap<ToolId, String>,
    /// Outils optionnels écartés (« Ignorer ») ; les IA, elles, passent par `ai.<id>.enabled`.
    pub ignored_tools: Vec<ToolId>,
    /// Service `repos` : dépôts GitHub suivis (`owner/name`) par plugin.
    pub tracked_repos: BTreeMap<String, Vec<String>>,
    /// Service `repos` : clone local désigné par l'utilisateur, par dépôt (`owner/name` en
    /// minuscules → dossier). Écrit seulement par `repos_link`, après vérification du clone.
    pub repo_clones: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Legacy::is_empty")]
    #[ts(skip)]
    pub legacy: Legacy,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: VERSION,
            onboarded: false,
            default_ai: None,
            ai: AiSettings::default(),
            action_ai: BTreeMap::new(),
            catalog_url: None,
            dev_plugins: Vec::new(),
            tool_paths: BTreeMap::new(),
            ignored_tools: Vec::new(),
            tracked_repos: BTreeMap::new(),
            repo_clones: BTreeMap::new(),
            legacy: Legacy::default(),
        }
    }
}

impl AppSettings {
    /* Accès des CLI détectées par `auth::cli` (Claude Code, Codex). */

    pub fn cli_path(&self, tool: CliTool) -> Option<&str> {
        self.ai.get(tool.into()).path.as_deref()
    }

    pub fn set_cli_path(&mut self, tool: CliTool, path: Option<String>) {
        self.ai.get_mut(tool.into()).path = path;
    }

    pub fn is_enabled(&self, tool: CliTool) -> bool {
        self.ai.get(tool.into()).enabled
    }

    pub fn set_enabled(&mut self, tool: CliTool, enabled: bool) {
        self.ai.get_mut(tool.into()).enabled = enabled;
    }

    /// Applique une modification partielle venue du front, après validation.
    pub fn apply(&mut self, patch: AppSettingsPatch) -> AppResult<()> {
        if let Some(onboarded) = patch.onboarded {
            self.onboarded = onboarded;
        }
        if let Some(ai) = patch.default_ai {
            self.default_ai = Some(ai);
        }
        for change in patch.ai.into_iter().flatten() {
            let slot = self.ai.get_mut(change.ai);
            if let Some(path) = change.path {
                slot.path = Some(path.trim().to_string()).filter(|p| !p.is_empty());
            }
            if let Some(enabled) = change.enabled {
                slot.enabled = enabled;
            }
        }
        for (key, ai) in patch.action_ai.into_iter().flatten() {
            validate_action_key(&key)?;
            match ai {
                Some(ai) => self.action_ai.insert(key, ai),
                None => self.action_ai.remove(&key),
            };
        }
        if let Some(url) = patch.catalog_url {
            let url = url.trim();
            if !url.is_empty() && !url.starts_with("https://") {
                return Err(AppError::Validation("L'URL du catalogue doit commencer par https://.".into()));
            }
            self.catalog_url = Some(url.to_string()).filter(|u| !u.is_empty());
        }
        if let Some(dirs) = patch.dev_plugins {
            let mut dirs: Vec<String> =
                dirs.into_iter().map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).collect();
            dirs.dedup();
            self.dev_plugins = dirs;
        }
        for (plugin, repos) in patch.tracked_repos.into_iter().flatten() {
            if !is_plugin_id(&plugin) {
                return Err(AppError::Validation(format!("« {plugin} » n'est pas un identifiant de plugin.")));
            }
            let mut list: Vec<String> = Vec::new();
            for repo in repos {
                let repo = repo.trim().trim_end_matches(".git").to_string();
                if !is_repo_name(&repo) {
                    return Err(AppError::Validation(format!("« {repo} » n'est pas un dépôt owner/nom.")));
                }
                if !list.iter().any(|r| r.eq_ignore_ascii_case(&repo)) {
                    list.push(repo);
                }
            }
            if list.is_empty() {
                self.tracked_repos.remove(&plugin);
            } else {
                self.tracked_repos.insert(plugin, list);
            }
        }
        if let Some(mut ignored) = patch.ignored_tools {
            ignored.sort();
            ignored.dedup();
            self.ignored_tools = ignored;
        }
        Ok(())
    }
}

fn is_plugin_id(id: &str) -> bool {
    !id.is_empty() && id.split('-').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
}

/// `owner/name` d'un dépôt GitHub.
pub fn is_repo_name(repo: &str) -> bool {
    let ok = |part: &str| {
        !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    matches!(repo.split_once('/'), Some((owner, name)) if ok(owner) && ok(name) && !name.contains('/'))
}

/// `<plugin>/<action>`, deux identifiants en minuscules et tirets.
fn validate_action_key(key: &str) -> AppResult<()> {
    let ok = |part: &str| {
        !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    };
    match key.split_once('/') {
        Some((plugin, action)) if ok(plugin) && ok(action) => Ok(()),
        _ => Err(AppError::Validation(format!("« {key} » n'est pas une action <plugin>/<action>."))),
    }
}

/// Changement d'une CLI d'IA : `path` vide = revenir à la détection automatique.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct AiCliPatch {
    pub ai: AiId,
    pub path: Option<String>,
    pub enabled: Option<bool>,
}

/// Modification partielle des réglages : un champ absent ne change rien.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, optional_fields)]
pub struct AppSettingsPatch {
    pub onboarded: Option<bool>,
    pub default_ai: Option<AiId>,
    pub ai: Option<Vec<AiCliPatch>>,
    /// `null` pour une action = revenir à l'IA par défaut.
    #[ts(optional, type = "Record<string, AiId | null>")]
    pub action_ai: Option<BTreeMap<String, Option<AiId>>>,
    /// Chaîne vide = catalogue public.
    pub catalog_url: Option<String>,
    pub dev_plugins: Option<Vec<String>>,
    pub ignored_tools: Option<Vec<ToolId>>,
    /// Liste complète des dépôts suivis d'un plugin ; liste vide = n'en suivre aucun.
    #[ts(optional, type = "Record<string, string[]>")]
    pub tracked_repos: Option<BTreeMap<String, Vec<String>>>,
}

/// État de l'app au démarrage : choisit l'écran de départ.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppStatus {
    pub onboarded: bool,
    /// Version de Cadence (semver).
    pub version: String,
    /// Adresse de la passerelle MCP locale (`http://127.0.0.1:<port>/mcp`), si elle tourne.
    pub gateway: Option<String>,
    pub dirs: AppDirs,
}

/// Dossiers de l'app, affichés dans les réglages (lecture seule).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppDirs {
    /// Archives extraites des plugins, par version.
    pub plugins: String,
    /// Un dossier par plugin, transmis au serveur dans `CADENCE_DATA_DIR`.
    pub plugins_data: String,
    /// Dossier neutre où chaque session d'IA est lancée.
    pub sessions: String,
    /// Clones et worktrees du service Dépôts, sous la configuration de l'app.
    pub repos: String,
}

/// Fichier `settings.json`, lu au démarrage et réécrit à chaque modification.
pub struct AppSettingsStore {
    path: PathBuf,
    inner: Mutex<AppSettings>,
}

impl AppSettingsStore {
    /// Lit le fichier et le migre si besoin. Absent → valeurs par défaut ; illisible → valeurs
    /// par défaut sans écraser le fichier (une modification le remplacera).
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("settings.json");
        let settings = match fs::read_to_string(&path) {
            Ok(text) => match migrate::parse(&text) {
                Ok((settings, migrated)) => {
                    if migrated {
                        if let Err(err) = save_migrated(&path, &text, &settings) {
                            tracing::warn!(%err, "réglages migrés en mémoire seulement");
                        } else {
                            tracing::info!(path = %path.display(), "réglages migrés en v2");
                        }
                    }
                    settings
                }
                Err(err) => {
                    tracing::warn!(%err, path = %path.display(), "réglages illisibles, valeurs par défaut");
                    AppSettings::default()
                }
            },
            Err(_) => AppSettings::default(),
        };
        Self { path, inner: Mutex::new(settings) }
    }

    pub fn get(&self) -> AppSettings {
        self.inner.lock().unwrap().clone()
    }

    /// Modifie puis écrit le fichier (écriture atomique : fichier temporaire puis renommage).
    /// En cas d'erreur, rien n'est changé.
    pub fn try_update(&self, change: impl FnOnce(&mut AppSettings) -> AppResult<()>) -> AppResult<AppSettings> {
        let mut guard = self.inner.lock().unwrap();
        let mut next = guard.clone();
        change(&mut next)?;
        write_atomic(&self.path, &next)?;
        *guard = next.clone();
        Ok(next)
    }

    pub fn update(&self, change: impl FnOnce(&mut AppSettings)) -> AppResult<AppSettings> {
        self.try_update(|s| {
            change(s);
            Ok(())
        })
    }
}

fn save_migrated(path: &Path, original: &str, settings: &AppSettings) -> AppResult<()> {
    let backup = path.with_file_name("settings.v1.json");
    if !backup.exists() {
        fs::write(&backup, original)?;
    }
    write_atomic(path, settings)
}

pub(crate) fn write_atomic<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| AppError::Internal(e.to_string()))?;
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cadence-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_and_reloads() {
        let dir = temp_dir("app-settings");
        let store = AppSettingsStore::load(&dir);
        assert_eq!(store.get(), AppSettings::default());

        store
            .update(|s| {
                s.set_cli_path(CliTool::Codex, Some("/usr/local/bin/codex".into()));
                s.set_enabled(CliTool::ClaudeCode, false);
                s.default_ai = Some(AiId::Codex);
            })
            .unwrap();

        let reloaded = AppSettingsStore::load(&dir).get();
        assert_eq!(reloaded.cli_path(CliTool::Codex), Some("/usr/local/bin/codex"));
        assert!(!reloaded.is_enabled(CliTool::ClaudeCode));
        assert!(reloaded.is_enabled(CliTool::Codex));
        assert_eq!(reloaded.default_ai, Some(AiId::Codex));
        let text = fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(text.contains("\"version\": 2"), "{text}");
        assert!(!text.contains("legacy"), "section vide non écrite");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn applies_a_partial_patch() {
        let mut s = AppSettings::default();
        let patch: AppSettingsPatch = serde_json::from_value(serde_json::json!({
            "onboarded": true,
            "defaultAi": "claude-code",
            "ai": [{ "ai": "opencode", "path": " /opt/opencode ", "enabled": false }],
            "actionAi": { "gestion-projet/preparer-issue": "codex" },
            "catalogUrl": "https://example.org/catalog.json",
            "devPlugins": ["C:/dev/exemple", "", "C:/dev/exemple"]
        }))
        .unwrap();
        s.apply(patch).unwrap();
        assert!(s.onboarded);
        assert_eq!(s.default_ai, Some(AiId::ClaudeCode));
        assert_eq!(s.ai.opencode, AiCliSettings { path: Some("/opt/opencode".into()), enabled: false });
        assert_eq!(s.action_ai.get("gestion-projet/preparer-issue"), Some(&AiId::Codex));
        assert_eq!(s.dev_plugins, ["C:/dev/exemple"]);

        let clear: AppSettingsPatch = serde_json::from_value(serde_json::json!({
            "ai": [{ "ai": "opencode", "path": "" }],
            "actionAi": { "gestion-projet/preparer-issue": null },
            "catalogUrl": ""
        }))
        .unwrap();
        s.apply(clear).unwrap();
        assert_eq!(s.ai.opencode.path, None);
        assert!(!s.ai.opencode.enabled, "champ absent : inchangé");
        assert!(s.action_ai.is_empty());
        assert_eq!(s.catalog_url, None);
        assert!(s.onboarded);
    }

    #[test]
    fn tracks_repos_per_plugin() {
        let mut s = AppSettings::default();
        let track = |repos: &[&str]| AppSettingsPatch {
            tracked_repos: Some(BTreeMap::from([(
                "gestion-projet".to_string(),
                repos.iter().map(|r| r.to_string()).collect(),
            )])),
            ..Default::default()
        };
        s.apply(track(&["RemiSDV/cadence", " remisdv/cadence ", "RemiSDV/cadence-plugins.git"])).unwrap();
        assert_eq!(s.tracked_repos["gestion-projet"], ["RemiSDV/cadence", "RemiSDV/cadence-plugins"]);
        s.apply(track(&[])).unwrap();
        assert!(s.tracked_repos.is_empty());
    }

    #[test]
    fn rejects_invalid_patches_without_writing() {
        let dir = temp_dir("app-settings-invalid");
        let store = AppSettingsStore::load(&dir);
        let bad_url = AppSettingsPatch { catalog_url: Some("http://example.org".into()), ..Default::default() };
        assert!(store.try_update(|s| s.apply(bad_url)).is_err());
        let bad_key = AppSettingsPatch {
            action_ai: Some(BTreeMap::from([("pas une action".to_string(), Some(AiId::Codex))])),
            ..Default::default()
        };
        assert!(store.try_update(|s| s.apply(bad_key)).is_err());
        let bad_repo = AppSettingsPatch {
            tracked_repos: Some(BTreeMap::from([("gestion-projet".to_string(), vec!["pas-un-depot".to_string()])])),
            ..Default::default()
        };
        assert!(store.try_update(|s| s.apply(bad_repo)).is_err());
        assert!(!dir.join("settings.json").exists());
        assert_eq!(store.get(), AppSettings::default());
    }
}
