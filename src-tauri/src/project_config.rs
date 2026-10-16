//! `.cadence/config.toml` : réglages du projet, versionnés avec le dépôt. Aucun secret ici.
//!
//! Le fichier est écrit en `snake_case` (convention TOML) alors que l'IPC parle `camelCase` :
//! la conversion passe par une valeur JSON intermédiaire. Un fichier partiel ou ancien est
//! complété par les valeurs par défaut, une clé inconnue (dont l'ancienne section `[agents]`)
//! est ignorée.

use serde_json::{Map, Value};

use crate::{
    api::github::writes,
    domain::settings::{Config, IssuesConfig, ProjectConfig, VersioningConfig},
    error::{AppError, AppResult},
    state::{AppState, LoadedConfig},
};

pub const PATH: &str = ".cadence/config.toml";

const HEADER: &str = "# Configuration Cadence — https://github.com/cadence-app/desktop\n\
                      # Versionnée avec le dépôt ; aucun secret ici.\n\n";

/// Configuration d'un projet qui n'a pas encore de `config.toml`.
pub fn defaults(repo: &str) -> Config {
    Config {
        project: ProjectConfig { repo: repo.to_string(), tag_prefix: "v".into() },
        versioning: VersioningConfig { scheme: "semver".into(), confirm_major_bump: true, sprint_days: 14 },
        issues: IssuesConfig { require_milestone: true, template: "contexte-attendu-criteres".into() },
    }
}

/// Configuration du dépôt ouvert, lue sur GitHub au premier appel.
pub async fn load(state: &AppState) -> AppResult<LoadedConfig> {
    let repo = state.require_project().await?.repo.full_name;
    if let Some(loaded) = state.config.read().await.as_ref().filter(|c| c.repo == repo) {
        return Ok(loaded.clone());
    }
    let base = defaults(&repo);
    let token = state.github_token()?;
    let loaded = match writes::get_file(&state.http, &token, &repo, PATH).await? {
        Some((text, sha)) => LoadedConfig { config: parse(&text, &base)?, sha: Some(sha), repo },
        None => LoadedConfig { config: base, sha: None, repo },
    };
    *state.config.write().await = Some(loaded.clone());
    Ok(loaded)
}

/// Lit un `config.toml` par-dessus `base` (valeurs par défaut).
pub fn parse(text: &str, base: &Config) -> AppResult<Config> {
    let file: toml::Value = toml::from_str(text)
        .map_err(|e| AppError::Validation(format!("{PATH} illisible : {}", e.message())))?;
    let file = serde_json::to_value(file).map_err(|e| AppError::Internal(e.to_string()))?;
    let mut merged = serde_json::to_value(base).map_err(|e| AppError::Internal(e.to_string()))?;
    merge(&mut merged, &rename_keys(file, snake_to_camel));
    let config: Config = serde_json::from_value(merged)
        .map_err(|e| AppError::Validation(format!("{PATH} : valeur invalide ({e})")))?;
    let repo = base.project.repo.clone();
    Ok(Config { project: ProjectConfig { repo, ..config.project }, ..config })
}

/// Texte du fichier, clés en `snake_case`.
pub fn to_toml(config: &Config) -> AppResult<String> {
    let json = serde_json::to_value(config).map_err(|e| AppError::Internal(e.to_string()))?;
    let snake = rename_keys(drop_nulls(json), camel_to_snake);
    let body = toml::to_string_pretty(&snake).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(format!("{HEADER}{body}"))
}

/// Bornes raisonnables : on refuse avant d'écrire un fichier que l'app ne saurait pas suivre.
pub fn validate(config: &Config) -> AppResult<()> {
    let prefix = &config.project.tag_prefix;
    if prefix.len() > 10 || prefix.chars().any(|c| c.is_whitespace() || "~^:?*[\\".contains(c)) {
        return Err(AppError::Validation(format!("« {prefix} » ne peut pas préfixer un tag Git.")));
    }
    if !(1..=60).contains(&config.versioning.sprint_days) {
        return Err(AppError::Validation("Un sprint dure entre 1 et 60 jours.".into()));
    }
    Ok(())
}

fn merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                // Clé inconnue : ignorée (fichier d'une version plus récente, faute de frappe).
                if let Some(slot) = b.get_mut(k) {
                    merge(slot, v);
                }
            }
        }
        (slot, v) => *slot = v.clone(),
    }
}

fn drop_nulls(v: Value) -> Value {
    match v {
        Value::Object(map) => {
            Value::Object(map.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k, drop_nulls(v))).collect())
        }
        other => other,
    }
}

fn rename_keys(v: Value, f: fn(&str) -> String) -> Value {
    match v {
        Value::Object(map) => {
            Value::Object(map.into_iter().map(|(k, v)| (f(&k), rename_keys(v, f))).collect::<Map<_, _>>())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(|v| rename_keys(v, f)).collect()),
        other => other,
    }
}

fn camel_to_snake(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for c in key.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn snake_to_camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut upper = false;
    for c in key.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Config {
        defaults("me/app")
    }

    #[test]
    fn round_trips_through_snake_case_toml() {
        let mut config = base();
        config.versioning.sprint_days = 7;
        let text = to_toml(&config).unwrap();
        assert!(text.contains("tag_prefix = \"v\""), "{text}");
        assert!(text.contains("sprint_days = 7"));
        assert!(text.contains("require_milestone = true"));
        assert!(!text.contains("tagPrefix"));
        let back = parse(&text, &base()).unwrap();
        assert_eq!(back.versioning.sprint_days, 7);
    }

    #[test]
    fn completes_a_partial_file_and_ignores_unknown_keys() {
        let config = parse(
            "[project]\ntag_prefix = \"\"\nrepo = \"autre/depot\"\n[agents.cto]\nengine = \"codex\"\n[futur]\nx = 1\n",
            &base(),
        )
        .unwrap();
        assert_eq!(config.project.tag_prefix, "");
        assert_eq!(config.project.repo, "me/app", "le dépôt ouvert fait foi");
        assert_eq!(config.versioning.sprint_days, 14);
    }

    #[test]
    fn rejects_broken_files_and_values() {
        assert!(parse("[project\n", &base()).is_err());
        assert!(parse("[versioning]\nsprint_days = \"deux\"\n", &base()).is_err());
        let mut config = base();
        config.project.tag_prefix = "v ".into();
        assert!(validate(&config).is_err());
        let mut config = base();
        config.versioning.sprint_days = 0;
        assert!(validate(&config).is_err());
        assert!(validate(&base()).is_ok());
    }

    #[test]
    fn converts_key_cases() {
        assert_eq!(camel_to_snake("maxSessionMinutes"), "max_session_minutes");
        assert_eq!(snake_to_camel("max_session_minutes"), "maxSessionMinutes");
        assert_eq!(snake_to_camel("repo"), "repo");
    }
}
