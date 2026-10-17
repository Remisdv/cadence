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
    let repos_of: Vec<String> = patch.tracked_repos.iter().flat_map(|m| m.keys().cloned()).collect();
    let settings = state.settings.try_update(|s| s.apply(patch))?;
    if changes_cli {
        // Chemins ou activation changés : la détection des CLI est refaite au prochain appel.
        *state.cli.write().await = Default::default();
    }
    for plugin in repos_of {
        let repos = settings.tracked_repos.get(&plugin).cloned().unwrap_or_default();
        events::emit(&app, events::REPOS_CHANGED, ReposChanged { plugin: plugin.clone(), repos: repos.clone() });
        let uses_repos = state.plugins.manifest(&plugin).is_ok_and(|m| m.services.contains(&PluginService::Repos));
        if uses_repos {
            state.plugin_host.notify(&plugin, REPOS_NOTIFICATION, json!({ "repos": repos })).await;
        }
    }
    Ok(settings)
}
