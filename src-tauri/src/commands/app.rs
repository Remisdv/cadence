//! État de l'app et préférences de la machine (`settings.json` v2).

use serde_json::json;
use tauri::State;

use crate::{
    app_settings::{AppSettings, AppSettingsPatch, AppStatus},
    error::AppResult,
    events::{self, ReposChanged},
    plugins::manifest::PluginService,
    repos::REPOS_NOTIFICATION,
    state::AppState,
};

/// Écran de départ : onboarding de Cadence 2 tant qu'il n'est pas terminé.
#[tauri::command]
pub fn app_status(app: tauri::AppHandle, state: State<'_, AppState>) -> AppStatus {
    AppStatus {
        onboarded: state.settings.get().onboarded,
        version: app.package_info().version.to_string(),
        gateway: state.gateway.as_ref().map(|g| g.url()),
        dirs: state.dirs.clone(),
    }
}

#[tauri::command]
pub fn app_settings_get(state: State<'_, AppState>) -> AppSettings {
    state.settings.get()
}

/// Exporte la configuration de Cadence dans `path` (JSON lisible) : réglages de la machine,
/// plugins installés et leurs réglages. Jamais de secret ni de jeton (ils restent au trousseau).
#[tauri::command]
pub fn app_export_config(app: tauri::AppHandle, state: State<'_, AppState>, path: String) -> AppResult<()> {
    let installed = state.plugins.snapshot();
    let plugins: Vec<serde_json::Value> = state
        .plugins
        .ordered_ids()
        .into_iter()
        .filter_map(|id| {
            let entry = installed.plugins.get(&id)?;
            let settings = super::plugins::settings_value(&state, &id).ok();
            Some(json!({
                "id": id,
                "version": entry.version,
                "enabled": entry.enabled,
                "source": entry.source,
                "settings": settings,
            }))
        })
        .collect();
    let mut settings = serde_json::to_value(state.settings.get()).map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
    if let Some(map) = settings.as_object_mut() {
        map.remove("legacy");
    }
    let export = json!({
        "cadence": app.package_info().version.to_string(),
        "exportedAt": crate::domain::version::now_iso(),
        "settings": settings,
        "plugins": plugins,
    });
    let text = serde_json::to_string_pretty(&export).map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
    std::fs::write(&path, text + "\n")?;
    Ok(())
}

/// Modification partielle ; une valeur refusée ne change rien.
///
/// Dépôts suivis changés : événement `repos://changed` (iframes du plugin) et notification
/// `notifications/cadence/repos` à son serveur s'il tourne (sinon `CADENCE_REPOS` au lancement).
#[tauri::command]
pub async fn app_settings_set(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    patch: AppSettingsPatch,
) -> AppResult<AppSettings> {
    let changes_cli = patch.ai.is_some();
    let mut repos_of: Vec<String> = patch.tracked_repos.iter().flat_map(|m| m.keys().cloned()).collect();
    repos_of.extend(patch.repo_selection.iter().flat_map(|m| m.keys().cloned()));
    repos_of.sort();
    repos_of.dedup();
    let tracked_changed: Vec<String> = patch.tracked_repos.iter().flat_map(|m| m.keys().cloned()).collect();
    let settings = state.settings.try_update(|s| s.apply(patch))?;
    if changes_cli {
        // Chemins ou activation changés : la détection des CLI est refaite au prochain appel.
        *state.cli.write().await = Default::default();
    }
    for plugin in repos_of {
        let repos = settings.tracked_repos.get(&plugin).cloned().unwrap_or_default();
        let selection = settings.repo_selection.get(&plugin).cloned();
        events::emit(&app, events::REPOS_CHANGED, ReposChanged { plugin: plugin.clone(), repos: repos.clone(), selection });
        let uses_repos = state.plugins.manifest(&plugin).is_ok_and(|m| m.services.contains(&PluginService::Repos));
        // Le serveur ne connaît que la liste ; la sélection ne concerne que l'interface.
        if uses_repos && tracked_changed.contains(&plugin) {
            state.plugin_host.notify(&plugin, REPOS_NOTIFICATION, json!({ "repos": repos })).await;
        }
    }
    Ok(settings)
}
