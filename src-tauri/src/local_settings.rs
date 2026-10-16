//! Préférences propres à la machine (`<config de l'app>/settings.json`) : chemins des CLI,
//! agents de code désactivés. Distinct de `.cadence/config.toml`, qui est
//! versionné avec le dépôt. Aucun secret ici.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use crate::{
    auth::cli::CliTool,
    domain::{auth::AuthProvider, project::ProjectState},
    error::AppResult,
};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LocalSettings {
    /// Chemin choisi à la main (« Localiser le binaire… »).
    pub claude_code_path: Option<String>,
    pub codex_path: Option<String>,
    /// Agents de code écartés par l'utilisateur (« Déconnecter », « Ignorer »).
    pub disabled_tools: Vec<AuthProvider>,
    /// Dernier projet ouvert, rouvert au lancement suivant.
    pub last_project: Option<ProjectState>,
}

impl LocalSettings {
    pub fn cli_path(&self, tool: CliTool) -> Option<&str> {
        match tool {
            CliTool::ClaudeCode => self.claude_code_path.as_deref(),
            CliTool::Codex => self.codex_path.as_deref(),
        }
    }

    pub fn set_cli_path(&mut self, tool: CliTool, path: Option<String>) {
        match tool {
            CliTool::ClaudeCode => self.claude_code_path = path,
            CliTool::Codex => self.codex_path = path,
        }
    }

    pub fn is_enabled(&self, tool: CliTool) -> bool {
        !self.disabled_tools.contains(&tool.provider())
    }

    pub fn set_enabled(&mut self, tool: CliTool, enabled: bool) {
        self.disabled_tools.retain(|p| *p != tool.provider());
        if !enabled {
            self.disabled_tools.push(tool.provider());
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    inner: Mutex<LocalSettings>,
}

impl SettingsStore {
    /// Lit le fichier ; absent ou illisible → valeurs par défaut (sans écraser le fichier).
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("settings.json");
        let settings = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|err| {
                tracing::warn!(%err, path = %path.display(), "réglages locaux illisibles, valeurs par défaut");
                LocalSettings::default()
            }),
            Err(_) => LocalSettings::default(),
        };
        Self { path, inner: Mutex::new(settings) }
    }

    pub fn get(&self) -> LocalSettings {
        self.inner.lock().unwrap().clone()
    }

    /// Modifie puis écrit le fichier (écriture atomique : fichier temporaire puis renommage).
    pub fn update(&self, change: impl FnOnce(&mut LocalSettings)) -> AppResult<LocalSettings> {
        let mut guard = self.inner.lock().unwrap();
        let mut next = guard.clone();
        change(&mut next);
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&next).expect("réglages sérialisables"))?;
        fs::rename(&tmp, &self.path)?;
        *guard = next.clone();
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cadence-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn persists_and_reloads() {
        let dir = temp_dir("settings");
        let store = SettingsStore::load(&dir);
        assert_eq!(store.get(), LocalSettings::default());

        store
            .update(|s| {
                s.set_cli_path(CliTool::Codex, Some("/usr/local/bin/codex".into()));
                s.set_enabled(CliTool::ClaudeCode, false);
            })
            .unwrap();

        let reloaded = SettingsStore::load(&dir).get();
        assert_eq!(reloaded.cli_path(CliTool::Codex), Some("/usr/local/bin/codex"));
        assert!(!reloaded.is_enabled(CliTool::ClaudeCode));
        assert!(reloaded.is_enabled(CliTool::Codex));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn re_enabling_does_not_duplicate() {
        let mut s = LocalSettings::default();
        s.set_enabled(CliTool::Codex, false);
        s.set_enabled(CliTool::Codex, false);
        assert_eq!(s.disabled_tools.len(), 1);
        s.set_enabled(CliTool::Codex, true);
        assert!(s.disabled_tools.is_empty());
    }
}
