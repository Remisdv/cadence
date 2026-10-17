//! Plugins installés : liste du rail, activation, ordre, réglages et secrets globaux,
//! installation (archive locale, dossier de dev), désinstallation, serveurs MCP.
//!
//! Un changement d'activation ou de secret arrête le serveur du plugin : il redémarre au
//! prochain appel avec son nouvel environnement. Un changement de réglages lui est notifié
//! (`notifications/cadence/settings`) s'il tourne.

use std::path::{Path, PathBuf};

use secrecy::ExposeSecret;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::{
    error::{AppError, AppResult},
    events,
    plugins::{
        catalog::{self, CatalogView},
        install::{self, InstallPhase, PluginInstallEvent},
        process::PluginProcessView,
        registry::{InstalledPluginView, PluginSource},
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

/// Active ou désactive un plugin pour toute l'app (il reste installé). Désactivé, son
/// serveur est arrêté et ne redémarre plus.
#[tauri::command]
pub async fn plugins_set_enabled(state: State<'_, AppState>, id: String, enabled: bool) -> AppResult<InstalledPluginView> {
    state.plugins.set_enabled(&id, enabled)?;
    if !enabled {
        state.plugin_host.stop(&id).await;
    }
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
    let entry = state.plugins.entry(plugin)?;
    settings::read_schema(&state.plugins.plugin_dir(plugin, &entry), relative).map(Some)
}

#[tauri::command]
pub fn plugin_settings_get(state: State<'_, AppState>, plugin: String) -> AppResult<PluginSettingsView> {
    let schema = schema_of(&state, &plugin)?;
    state.plugin_settings.view(&plugin, schema)
}

/// Enregistre les réglages s'ils respectent le schéma ; sinon renvoie les erreurs par champ.
#[tauri::command]
pub async fn plugin_settings_set(state: State<'_, AppState>, plugin: String, value: Value) -> AppResult<PluginSettingsView> {
    let schema = schema_of(&state, &plugin)?;
    let view = state.plugin_settings.write(&plugin, schema, value)?;
    if view.errors.is_empty() {
        // Serveur en marche : il reçoit les nouveaux réglages ; sinon `CADENCE_SETTINGS` au lancement.
        let params = serde_json::json!({ "settings": view.value });
        state.plugin_host.notify(&plugin, SETTINGS_NOTIFICATION, params).await;
    }
    Ok(view)
}

/// Notification au serveur d'un plugin : réglages globaux modifiés, `{ "settings": {…} }`.
pub const SETTINGS_NOTIFICATION: &str = "notifications/cadence/settings";

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
pub async fn plugin_secret_set(state: State<'_, AppState>, plugin: String, secret: String, value: String) -> AppResult<()> {
    let account = declared_account(&state, &plugin, &secret)?;
    let value = secrecy::SecretString::from(value);
    if value.expose_secret().trim().is_empty() {
        return Err(AppError::Validation("Le secret est vide.".into()));
    }
    state.secrets.set(&account, value.expose_secret().trim())?;
    state.plugin_host.stop(&plugin).await;
    Ok(())
}

#[tauri::command]
pub fn plugin_secret_has(state: State<'_, AppState>, plugin: String, secret: String) -> AppResult<bool> {
    let account = declared_account(&state, &plugin, &secret)?;
    state.secrets.has(&account)
}

/// Supprime un secret ; il n'a pas besoin d'être encore déclaré (plugin mis à jour).
#[tauri::command]
pub async fn plugin_secret_delete(state: State<'_, AppState>, plugin: String, secret: String) -> AppResult<()> {
    let account = secrets::plugin_account(&plugin, &secret)?;
    state.secrets.delete(&account)?;
    state.plugin_host.stop(&plugin).await;
    Ok(())
}

fn interrupted(err: tauri::Error) -> AppError {
    AppError::Internal(format!("installation interrompue ({err})"))
}

/// Installe (ou met à jour) un plugin depuis une archive zip locale. `sha256` : empreinte
/// attendue, facultative ; un fichier `<archive>.sha256` voisin est vérifié aussi.
/// Progression par `plugin://install`.
#[tauri::command]
pub async fn plugin_install_zip(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    sha256: Option<String>,
) -> AppResult<InstalledPluginView> {
    let registry = state.plugins.clone();
    let progress = app.clone();
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        install::prepare_zip(&registry, Path::new(&path), sha256.as_deref(), &mut |event| {
            events::emit(&progress, events::PLUGIN_INSTALL, event)
        })
    })
    .await
    .map_err(interrupted)??;

    // Windows refuse de remplacer l'exécutable d'un serveur en cours d'exécution.
    let id = prepared.id.clone();
    state.plugin_host.stop(&id).await;
    let registry = state.plugins.clone();
    tauri::async_runtime::spawn_blocking(move || prepared.commit(&registry)).await.map_err(interrupted)??;
    let done = PluginInstallEvent { id: id.clone(), received: 0, total: 0, phase: InstallPhase::Done };
    events::emit(&app, events::PLUGIN_INSTALL, done);
    view(&state, &id)
}

/// Inscrit un dossier local non zippé comme plugin de développement et le retient dans
/// les préférences (`devPlugins`).
#[tauri::command]
pub async fn plugin_install_dev(state: State<'_, AppState>, path: String) -> AppResult<InstalledPluginView> {
    let id = install::install_dev(&state.plugins, Path::new(&path))?;
    state.plugin_host.stop(&id).await;
    let dir = state.plugins.plugin_dir(&id, &state.plugins.entry(&id)?).to_string_lossy().into_owned();
    state.settings.update(|s| {
        if !s.dev_plugins.contains(&dir) {
            s.dev_plugins.push(dir);
        }
    })?;
    view(&state, &id)
}

/// Désinstalle un plugin : serveur arrêté, fichiers supprimés (jamais le dossier d'un plugin
/// de dev), secrets supprimés si demandé. Réglages et données restent.
#[tauri::command]
pub async fn plugin_uninstall(state: State<'_, AppState>, id: String, delete_secrets: bool) -> AppResult<()> {
    let entry = state.plugins.entry(&id)?;
    state.plugin_host.stop(&id).await;
    install::uninstall(&state.plugins, &id, delete_secrets.then_some(state.secrets.as_ref()))?;
    if entry.source == PluginSource::Dev {
        let dir = state.plugins.plugin_dir(&id, &entry);
        state.settings.update(|s| s.dev_plugins.retain(|d| PathBuf::from(d) != dir))?;
    }
    Ok(())
}

/// État des serveurs des plugins installés qui en déclarent un, dans l'ordre du rail.
#[tauri::command]
pub fn plugin_status(state: State<'_, AppState>) -> Vec<PluginProcessView> {
    state
        .plugins
        .ordered_ids()
        .into_iter()
        .filter(|id| state.plugins.manifest(id).is_ok_and(|m| m.server.is_some()))
        .map(|id| state.plugin_host.status(&id))
        .collect()
}

/// Résultat d'un appel d'outil par l'interface d'un plugin (pont de l'iframe).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginToolResult {
    /// L'outil a répondu par une erreur (`isError`).
    pub is_error: bool,
    /// `structuredContent`, s'il y en a un.
    #[ts(type = "unknown")]
    pub structured: Option<Value>,
    /// Texte des blocs `text` du résultat, mis bout à bout.
    pub text: String,
}

/// Appelle un outil du plugin pour son interface (outils `ui` compris). Le nom est celui du
/// serveur, sans préfixe : l'iframe ne peut viser que son propre plugin.
#[tauri::command]
pub async fn plugin_call(
    state: State<'_, AppState>,
    plugin: String,
    tool: String,
    args: Option<Value>,
) -> AppResult<PluginToolResult> {
    if !state.plugins.entry(&plugin)?.enabled {
        return Err(AppError::Validation(format!("Le plugin « {plugin} » est désactivé.")));
    }
    let args = match args {
        None | Some(Value::Null) => None,
        Some(Value::Object(map)) => Some(map),
        Some(_) => return Err(AppError::Validation("Les arguments d'un outil doivent être un objet.".into())),
    };
    let result = state.plugin_host.call(&plugin, &tool, args).await?;
    let content = serde_json::to_value(&result.content).unwrap_or_default();
    let text = content
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(PluginToolResult { is_error: result.is_error.unwrap_or(false), structured: result.structured_content, text })
}

fn catalog_url(state: &AppState) -> String {
    state.settings.get().catalog_url.unwrap_or_else(|| catalog::DEFAULT_URL.to_string())
}

/// Catalogue en ligne comparé aux plugins installés. `refresh` force la relecture (sinon le
/// cache de moins de 6 h sert) ; hors ligne, le dernier catalogue reçu.
#[tauri::command]
pub async fn catalog_get(app: AppHandle, state: State<'_, AppState>, refresh: bool) -> AppResult<CatalogView> {
    let url = catalog_url(&state);
    let fetched = state.catalog.get(&url, refresh).await?;
    let view = catalog::view(&url, &fetched, &state.plugins);
    events::emit(&app, events::CATALOG_UPDATED, events::CatalogUpdated { updates: view.updates });
    Ok(view)
}

/// Installe un plugin du catalogue (progression par `plugin://install`).
#[tauri::command]
pub async fn plugin_install(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<InstalledPluginView> {
    let url = catalog_url(&state);
    let fetched = state.catalog.get(&url, false).await?;
    let progress = app.clone();
    let prepared = state
        .catalog
        .prepare_install(state.plugins.clone(), &fetched.catalog, &id, move |event| {
            events::emit(&progress, events::PLUGIN_INSTALL, event)
        })
        .await?;
    // Windows refuse de remplacer l'exécutable d'un serveur en cours d'exécution.
    state.plugin_host.stop(&id).await;
    let registry = state.plugins.clone();
    tauri::async_runtime::spawn_blocking(move || prepared.commit(&registry)).await.map_err(interrupted)??;
    let done = PluginInstallEvent { id: id.clone(), received: 0, total: 0, phase: InstallPhase::Done };
    events::emit(&app, events::PLUGIN_INSTALL, done);
    view(&state, &id)
}

/// Met à jour un plugin installé vers la version du catalogue (l'ancienne reste pour un
/// retour arrière).
#[tauri::command]
pub async fn plugin_update(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<InstalledPluginView> {
    let entry = state.plugins.entry(&id)?;
    if entry.source == PluginSource::Dev {
        return Err(AppError::Validation("Un plugin de développement se met à jour depuis son dossier.".into()));
    }
    plugin_install(app, state, id).await
}

/// Relance le serveur d'un plugin activé, même après un échec, et attend qu'il soit prêt.
#[tauri::command]
pub async fn plugin_restart(state: State<'_, AppState>, plugin: String) -> AppResult<()> {
    if !state.plugins.entry(&plugin)?.enabled {
        return Err(AppError::Validation(format!("Le plugin « {plugin} » est désactivé.")));
    }
    state.plugin_host.restart(&plugin).await
}
