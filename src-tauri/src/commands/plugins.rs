//! Plugins installés : liste du rail, activation, ordre, réglages et secrets globaux.

use secrecy::ExposeSecret;
use serde_json::Value;
use tauri::State;

use crate::{
    error::{AppError, AppResult},
    plugins::{
        registry::InstalledPluginView,
        settings::{self, PluginSettingsView},
    },
    secrets,
    state::AppState,
};

fn has_secret(state: &AppState, plugin: &str, secret: &str) -> bool {
    secrets::plugin_account(plugin, secret)
        .and_then(|account| state.secrets.has(&account))
        .unwrap_or_else(|err| {
            tracing::warn!(%err, plugin, secret, "trousseau illisible");
            false
        })
}

fn view(state: &AppState, id: &str) -> AppResult<InstalledPluginView> {
    state.plugins.view(id, |secret| has_secret(state, id, secret))
}

/// Plugins installés, dans l'ordre du rail (catégories, puis ordre choisi).
#[tauri::command]
pub fn plugins_installed(state: State<'_, AppState>) -> Vec<InstalledPluginView> {
    state.plugins.views(|plugin, secret| has_secret(&state, plugin, secret))
}

/// Active ou désactive un plugin pour toute l'app (il reste installé).
#[tauri::command]
pub fn plugins_set_enabled(state: State<'_, AppState>, id: String, enabled: bool) -> AppResult<InstalledPluginView> {
    state.plugins.set_enabled(&id, enabled)?;
    view(&state, &id)
}

/// Nouvel ordre du rail ; renvoie la liste réordonnée.
#[tauri::command]
pub fn plugins_reorder(state: State<'_, AppState>, order: Vec<String>) -> AppResult<Vec<InstalledPluginView>> {
    state.plugins.reorder(order)?;
    Ok(plugins_installed(state))
}

/// Schéma déclaré par le plugin (absent si le plugin n'en déclare pas).
fn schema_of(state: &AppState, plugin: &str) -> AppResult<Option<Value>> {
    let manifest = state.plugins.manifest(plugin)?;
    let Some(relative) = manifest.settings_schema.as_deref() else { return Ok(None) };
    let entry = state
        .plugins
        .snapshot()
        .plugins
        .get(plugin)
        .cloned()
        .ok_or_else(|| AppError::Validation(format!("Le plugin « {plugin} » n'est pas installé.")))?;
    settings::read_schema(&state.plugins.plugin_dir(plugin, &entry), relative).map(Some)
}

#[tauri::command]
pub fn plugin_settings_get(state: State<'_, AppState>, plugin: String) -> AppResult<PluginSettingsView> {
    let schema = schema_of(&state, &plugin)?;
    state.plugin_settings.view(&plugin, schema)
}

/// Enregistre les réglages s'ils respectent le schéma ; sinon renvoie les erreurs par champ.
#[tauri::command]
pub fn plugin_settings_set(state: State<'_, AppState>, plugin: String, value: Value) -> AppResult<PluginSettingsView> {
    let schema = schema_of(&state, &plugin)?;
    state.plugin_settings.write(&plugin, schema, value)
}

/// Le secret doit être déclaré par le manifeste du plugin installé.
fn declared_account(state: &AppState, plugin: &str, secret: &str) -> AppResult<String> {
    let account = secrets::plugin_account(plugin, secret)?;
    let manifest = state.plugins.manifest(plugin)?;
    if !manifest.secrets.iter().any(|s| s.id == secret) {
        return Err(AppError::Validation(format!("Le plugin « {plugin} » ne déclare pas le secret « {secret} ».")));
    }
    Ok(account)
}

/// Écrit un secret au trousseau. Écriture seule : aucune commande ne le relit pour le front.
#[tauri::command]
pub fn plugin_secret_set(state: State<'_, AppState>, plugin: String, secret: String, value: String) -> AppResult<()> {
    let account = declared_account(&state, &plugin, &secret)?;
    let value = secrecy::SecretString::from(value);
    if value.expose_secret().trim().is_empty() {
        return Err(AppError::Validation("Le secret est vide.".into()));
    }
    state.secrets.set(&account, value.expose_secret().trim())
}

#[tauri::command]
pub fn plugin_secret_has(state: State<'_, AppState>, plugin: String, secret: String) -> AppResult<bool> {
    let account = declared_account(&state, &plugin, &secret)?;
    state.secrets.has(&account)
}

/// Supprime un secret ; il n'a pas besoin d'être encore déclaré (plugin mis à jour).
#[tauri::command]
pub fn plugin_secret_delete(state: State<'_, AppState>, plugin: String, secret: String) -> AppResult<()> {
    let account = secrets::plugin_account(&plugin, &secret)?;
    state.secrets.delete(&account)
}
