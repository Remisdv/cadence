//! Lecture de `settings.json` et migration v1 → v2.
//!
//! v1 (Cadence 0.1/0.2, sans champ `version`) :
//! `{ claudeCodePath, codexPath, disabledTools: ["claude-code" | "codex"], agentEngines, lastProject }`.
//! Les chemins et désactivations deviennent `ai.<id>`, le dépôt ouvert part dans `legacy`
//! (repris par le plugin Gestion de projet), l'affectation des rôles d'agents est abandonnée.
//! `onboarded` reste faux : l'onboarding de Cadence 2 (IA, plugins) est à faire.

use serde::Deserialize;
use serde_json::Value;

use super::{AiId, AppSettings, Legacy, VERSION};
use crate::{
    domain::project::ProjectState,
    error::{AppError, AppResult},
};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct V1 {
    claude_code_path: Option<String>,
    codex_path: Option<String>,
    disabled_tools: Vec<String>,
    last_project: Option<ProjectState>,
}

/// Réglages lus depuis le texte du fichier, et `true` s'ils viennent d'être migrés.
pub fn parse(text: &str) -> AppResult<(AppSettings, bool)> {
    let value: Value = serde_json::from_str(text).map_err(|e| AppError::Validation(format!("settings.json : {e}")))?;
    let version = value.get("version").and_then(Value::as_u64).unwrap_or(1);
    match version {
        1 => {
            let v1: V1 = serde_json::from_value(value).map_err(|e| AppError::Validation(format!("settings.json v1 : {e}")))?;
            Ok((from_v1(v1), true))
        }
        2 => {
            let settings: AppSettings =
                serde_json::from_value(value).map_err(|e| AppError::Validation(format!("settings.json : {e}")))?;
            Ok((settings, false))
        }
        newer => Err(AppError::Validation(format!(
            "settings.json en version {newer} : écrit par une version plus récente de Cadence."
        ))),
    }
}

fn from_v1(v1: V1) -> AppSettings {
    let mut settings = AppSettings { version: VERSION, ..AppSettings::default() };
    let path = |p: Option<String>| p.filter(|p| !p.trim().is_empty());
    settings.ai.claude_code.path = path(v1.claude_code_path);
    settings.ai.codex.path = path(v1.codex_path);
    for disabled in &v1.disabled_tools {
        match disabled.as_str() {
            "claude-code" => settings.ai.claude_code.enabled = false,
            "codex" => settings.ai.codex.enabled = false,
            _ => {}
        }
    }
    // Claude Code était la seule IA pilotée par Cadence 1 : on la garde comme IA par défaut
    // si elle n'avait pas été écartée.
    if settings.ai.claude_code.enabled {
        settings.default_ai = Some(AiId::ClaudeCode);
    }
    settings.legacy = Legacy { last_project: v1.last_project };
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_settings::AppSettingsStore, auth::cli::CliTool};

    const V1_FILE: &str = r#"{
      "claudeCodePath": null,
      "codexPath": "C:\\Users\\me\\AppData\\Roaming\\npm\\codex.cmd",
      "disabledTools": ["claude-code"],
      "agentEngines": { "cto": { "engine": "claude-code", "model": "opus" } },
      "lastProject": {
        "repo": {
          "fullName": "cadence-app/desktop", "owner": "cadence-app", "name": "desktop",
          "description": null, "private": false, "defaultBranch": "main",
          "openIssues": 18, "milestones": 4, "pushedAt": "2026-10-15T10:00:00Z"
        },
        "currentVersion": "0.1.2",
        "hasConfig": true
      }
    }"#;

    #[test]
    fn converts_a_v1_file() {
        let (settings, migrated) = parse(V1_FILE).unwrap();
        assert!(migrated);
        assert_eq!(settings.version, VERSION);
        assert!(!settings.onboarded);
        assert!(!settings.ai.claude_code.enabled);
        assert_eq!(settings.ai.codex.path.as_deref(), Some("C:\\Users\\me\\AppData\\Roaming\\npm\\codex.cmd"));
        assert!(settings.ai.codex.enabled && settings.ai.opencode.enabled);
        assert_eq!(settings.default_ai, None, "Claude Code écarté : pas d'IA par défaut");
        let project = settings.legacy.last_project.unwrap();
        assert_eq!(project.repo.full_name, "cadence-app/desktop");
        assert_eq!(project.current_version.as_deref(), Some("0.1.2"));
    }

    #[test]
    fn an_empty_v1_file_keeps_claude_code_by_default() {
        let (settings, migrated) = parse("{}").unwrap();
        assert!(migrated);
        assert_eq!(settings.default_ai, Some(AiId::ClaudeCode));
        assert!(settings.legacy.is_empty());
    }

    #[test]
    fn reads_v2_as_is_and_refuses_newer_files() {
        let (settings, migrated) = parse(r#"{ "version": 2, "onboarded": true, "defaultAi": "opencode" }"#).unwrap();
        assert!(!migrated);
        assert!(settings.onboarded);
        assert_eq!(settings.default_ai, Some(AiId::Opencode));
        assert!(parse(r#"{ "version": 3 }"#).is_err());
        assert!(parse("pas du json").is_err());
    }

    #[test]
    fn the_store_migrates_once_and_keeps_a_backup() {
        let dir = crate::app_settings::temp_dir("migrate");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), V1_FILE).unwrap();

        let settings = AppSettingsStore::load(&dir).get();
        assert!(!settings.is_enabled(CliTool::ClaudeCode));
        assert_eq!(std::fs::read_to_string(dir.join("settings.v1.json")).unwrap(), V1_FILE);
        let written = std::fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(written.contains("\"version\": 2") && written.contains("lastProject"), "{written}");
        assert!(!written.contains("agentEngines"));

        // Deuxième lancement : fichier déjà en v2, rien à migrer.
        let again = AppSettingsStore::load(&dir).get();
        assert_eq!(again, settings);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
