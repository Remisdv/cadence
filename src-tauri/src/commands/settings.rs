use crate::{
    domain::settings::Config,
    error::{AppError, AppResult},
};

/// Lit `.cadence/config.toml` du projet ouvert.
#[tauri::command]
pub async fn settings_get() -> AppResult<Config> {
    Err(AppError::NotImplemented("settings_get"))
}

/// Écrit `.cadence/config.toml` et renvoie la configuration enregistrée.
#[tauri::command]
pub async fn settings_set(config: Config) -> AppResult<Config> {
    let _ = config;
    Err(AppError::NotImplemented("settings_set"))
}
