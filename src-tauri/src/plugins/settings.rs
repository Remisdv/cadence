//! Réglages globaux d'un plugin : `<config de l'app>/plugins/<id>.json` (révision 1.1 : plus
//! de réglages par espace). Le serveur du plugin les recevra par `CADENCE_SETTINGS` puis
//! `notifications/cadence/settings`.
//!
//! Validation par le sous-ensemble de JSON Schema du plan (§ 3.6) : racine `object`,
//! `properties`, `required`, types `string` (`enum`, `minLength`, `maxLength`), `integer` /
//! `number` (`minimum`, `maximum`), `boolean`, `array` de chaînes (`enum`). Une valeur
//! invalide n'est pas écrite : les erreurs reviennent par champ.

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;
use serde_json::{Map, Value};
use ts_rs::TS;

use super::manifest;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingsFieldError {
    /// Clé du réglage ; vide pour une erreur sur l'ensemble.
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginSettingsView {
    pub plugin: String,
    /// JSON Schema déclaré par le plugin (`settings.schema`), s'il y en a un.
    #[ts(type = "Record<string, unknown> | null")]
    pub schema: Option<Value>,
    /// Valeurs enregistrées, complétées par les défauts du schéma.
    #[ts(type = "Record<string, unknown>")]
    pub value: Value,
    /// Vide si la valeur est valide (et enregistrée, pour `plugin_settings_set`).
    pub errors: Vec<SettingsFieldError>,
}

pub struct PluginSettingsStore {
    /// `<config de l'app>/plugins`.
    dir: PathBuf,
}

impl PluginSettingsStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, plugin: &str) -> AppResult<PathBuf> {
        if !manifest::is_plugin_id(plugin) {
            return Err(AppError::Validation(format!("« {plugin} » n'est pas un identifiant de plugin.")));
        }
        Ok(self.dir.join(format!("{plugin}.json")))
    }

    /// Valeurs enregistrées (objet vide si rien n'est encore réglé).
    pub fn read(&self, plugin: &str) -> AppResult<Map<String, Value>> {
        match fs::read_to_string(self.path(plugin)?) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(Value::Object(map)) => Ok(map),
                _ => {
                    tracing::warn!(plugin, "réglages du plugin illisibles, ignorés");
                    Ok(Map::new())
                }
            },
            Err(_) => Ok(Map::new()),
        }
    }

    pub fn view(&self, plugin: &str, schema: Option<Value>) -> AppResult<PluginSettingsView> {
        let stored = self.read(plugin)?;
        let value = with_defaults(stored, schema.as_ref());
        Ok(PluginSettingsView { plugin: plugin.to_string(), schema, value: Value::Object(value), errors: vec![] })
    }

    /// Valide puis enregistre `value` ; avec des erreurs, rien n'est écrit.
    pub fn write(&self, plugin: &str, schema: Option<Value>, value: Value) -> AppResult<PluginSettingsView> {
        let Value::Object(map) = value else {
            return Ok(PluginSettingsView {
                plugin: plugin.to_string(),
                schema,
                value: Value::Object(self.read(plugin)?),
                errors: vec![SettingsFieldError { field: String::new(), message: "Les réglages doivent être un objet.".into() }],
            });
        };
        let errors = schema.as_ref().map(|s| validate(s, &map)).unwrap_or_default();
        if errors.is_empty() {
            crate::app_settings::write_atomic(&self.path(plugin)?, &map)?;
        }
        Ok(PluginSettingsView {
            plugin: plugin.to_string(),
            value: Value::Object(with_defaults(map, schema.as_ref())),
            schema,
            errors,
        })
    }
}

/// Lit le schéma déclaré par le manifeste, dans le dossier du plugin seulement.
pub fn read_schema(plugin_dir: &Path, relative: &str) -> AppResult<Value> {
    let rel = Path::new(relative);
    if rel.is_absolute() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(AppError::Validation(format!("Schéma de réglages « {relative} » hors du dossier du plugin.")));
    }
    let text = fs::read_to_string(plugin_dir.join(rel))
        .map_err(|e| AppError::Validation(format!("Schéma de réglages « {relative} » illisible ({e}).")))?;
    let schema: Value = serde_json::from_str(&text)
        .map_err(|e| AppError::Validation(format!("Schéma de réglages « {relative} » : {e}")))?;
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err(AppError::Validation("Le schéma de réglages doit décrire un objet (`type: \"object\"`).".into()));
    }
    Ok(schema)
}

fn properties(schema: &Value) -> Option<&Map<String, Value>> {
    schema.get("properties").and_then(Value::as_object)
}

fn with_defaults(mut value: Map<String, Value>, schema: Option<&Value>) -> Map<String, Value> {
    if let Some(props) = schema.and_then(properties) {
        for (key, prop) in props {
            if let (false, Some(default)) = (value.contains_key(key), prop.get("default")) {
                value.insert(key.clone(), default.clone());
            }
        }
    }
    value
}

/// Erreurs de `value` au regard du sous-ensemble de JSON Schema pris en charge.
pub fn validate(schema: &Value, value: &Map<String, Value>) -> Vec<SettingsFieldError> {
    let mut errors = Vec::new();
    let error = |field: &str, message: String| SettingsFieldError { field: field.to_string(), message };
    let props = properties(schema).cloned().unwrap_or_default();

    for required in schema.get("required").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        if value.get(required).is_none_or(Value::is_null) {
            errors.push(error(required, "Réglage obligatoire.".into()));
        }
    }
    for (key, v) in value {
        let Some(prop) = props.get(key) else {
            errors.push(error(key, "Réglage inconnu de ce plugin.".into()));
            continue;
        };
        if let Some(message) = check(prop, v) {
            errors.push(error(key, message));
        }
    }
    errors
}

fn check(prop: &Value, v: &Value) -> Option<String> {
    let number = |key: &str| prop.get(key).and_then(Value::as_f64);
    let allowed = prop.get("enum").and_then(Value::as_array);
    match prop.get("type").and_then(Value::as_str) {
        Some("string") => {
            let Some(s) = v.as_str() else { return Some("Texte attendu.".into()) };
            let len = s.chars().count() as f64;
            if let Some(values) = allowed.filter(|e| !e.contains(v)) {
                return Some(format!("Valeur parmi : {}.", list(values)));
            }
            if number("minLength").is_some_and(|min| len < min) {
                return Some(format!("Au moins {} caractères.", number("minLength")?));
            }
            if number("maxLength").is_some_and(|max| len > max) {
                return Some(format!("Au plus {} caractères.", number("maxLength")?));
            }
            None
        }
        Some(kind @ ("integer" | "number")) => {
            let Some(n) = v.as_f64() else { return Some("Nombre attendu.".into()) };
            if kind == "integer" && n.fract() != 0.0 {
                return Some("Nombre entier attendu.".into());
            }
            if number("minimum").is_some_and(|min| n < min) {
                return Some(format!("Au moins {}.", number("minimum")?));
            }
            if number("maximum").is_some_and(|max| n > max) {
                return Some(format!("Au plus {}.", number("maximum")?));
            }
            None
        }
        Some("boolean") => (!v.is_boolean()).then(|| "Oui ou non attendu.".into()),
        Some("array") => {
            let Some(items) = v.as_array() else { return Some("Liste attendue.".into()) };
            if items.iter().any(|i| !i.is_string()) {
                return Some("Liste de textes attendue.".into());
            }
            let item_enum = prop.pointer("/items/enum").and_then(Value::as_array);
            if let Some(values) = item_enum
                && items.iter().any(|i| !values.contains(i))
            {
                return Some(format!("Valeurs parmi : {}.", list(values)));
            }
            None
        }
        // Type absent ou hors du sous-ensemble : accepté tel quel.
        _ => None,
    }
}

fn list(values: &[Value]) -> String {
    values.iter().map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn schema() -> Value {
        json!({
            "type": "object",
            "required": ["sprintDays"],
            "properties": {
                "sprintDays": { "type": "integer", "minimum": 1, "maximum": 60, "default": 14 },
                "tagPrefix": { "type": "string", "maxLength": 10, "default": "v" },
                "scheme": { "type": "string", "enum": ["semver", "calver"] },
                "notes": { "type": "boolean" },
                "labels": { "type": "array", "items": { "type": "string", "enum": ["bug", "feat"] } }
            }
        })
    }

    #[test]
    fn validates_the_supported_subset() {
        let ok = json!({ "sprintDays": 7, "tagPrefix": "", "scheme": "semver", "notes": true, "labels": ["bug"] });
        assert!(validate(&schema(), ok.as_object().unwrap()).is_empty());

        let ko = json!({ "sprintDays": 7.5, "tagPrefix": "trop-long-pour-un-tag", "scheme": "autre",
                         "notes": "oui", "labels": ["docs"], "inconnu": 1 });
        let fields: Vec<String> = validate(&schema(), ko.as_object().unwrap()).into_iter().map(|e| e.field).collect();
        assert_eq!(fields, ["inconnu", "labels", "notes", "scheme", "sprintDays", "tagPrefix"]);

        let missing = validate(&schema(), json!({ "notes": false }).as_object().unwrap());
        assert_eq!(missing, [SettingsFieldError { field: "sprintDays".into(), message: "Réglage obligatoire.".into() }]);
        assert_eq!(validate(&schema(), json!({ "sprintDays": 0 }).as_object().unwrap())[0].message, "Au moins 1.");
    }

    #[test]
    fn stores_valid_values_and_fills_defaults() {
        let dir = crate::app_settings::temp_dir("plugin-settings");
        let store = PluginSettingsStore::new(dir.clone());

        let view = store.view("gestion-projet", Some(schema())).unwrap();
        assert_eq!(view.value, json!({ "sprintDays": 14, "tagPrefix": "v" }));

        let refused = store.write("gestion-projet", Some(schema()), json!({ "sprintDays": 99 })).unwrap();
        assert_eq!(refused.errors.len(), 1);
        assert!(!dir.join("gestion-projet.json").exists(), "valeur invalide non écrite");

        let saved = store.write("gestion-projet", Some(schema()), json!({ "sprintDays": 7 })).unwrap();
        assert!(saved.errors.is_empty());
        assert_eq!(store.view("gestion-projet", Some(schema())).unwrap().value, json!({ "sprintDays": 7, "tagPrefix": "v" }));
        // Sans schéma, tout objet est accepté.
        assert!(store.write("journal", None, json!({ "libre": [1, 2] })).unwrap().errors.is_empty());
        assert_eq!(store.write("journal", None, json!([1])).unwrap().errors[0].field, "");
        assert!(store.view("../evil", None).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_the_schema_inside_the_plugin_only() {
        let dir = crate::app_settings::temp_dir("plugin-schema");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("settings.schema.json"), schema().to_string()).unwrap();
        fs::write(dir.join("liste.json"), "[]").unwrap();
        assert!(read_schema(&dir, "settings.schema.json").is_ok());
        assert!(read_schema(&dir, "liste.json").is_err());
        assert!(read_schema(&dir, "../settings.schema.json").is_err());
        assert!(read_schema(&dir, "absent.json").is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
