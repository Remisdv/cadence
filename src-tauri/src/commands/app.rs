//! État de l'app et préférences de la machine (`settings.json` v2).

use tauri::State;

use crate::{
    app_settings::{AppSettings, AppSettingsPatch, AppStatus},
    error::AppResult,
    state::AppState,
};

/// Écran de départ : onboarding de Cadence 2 tant qu'il n'est pas terminé.
#[tauri::command]
pub fn app_status(app: tauri::AppHandle, state: State<'_, AppState>) -> AppStatus {
    AppStatus { onboarded: state.settings.get().onboarded, version: app.package_info().version.to_string() }
}

#[tauri::command]
pub fn app_settings_get(state: State<'_, AppState>) -> AppSettings {
    state.settings.get()
}

/// Modification partielle ; une valeur refusée ne change rien.
#[tauri::command]
pub async fn app_settings_set(state: State<'_, AppState>, patch: AppSettingsPatch) -> AppResult<AppSettings> {
    let changes_cli = patch.ai.is_some();
    let settings = state.settings.try_update(|s| s.apply(patch))?;
    if changes_cli {
        // Chemins ou activation changés : la détection des CLI est refaite au prochain appel.
        *state.cli.write().await = Default::default();
    }
    Ok(settings)
}
