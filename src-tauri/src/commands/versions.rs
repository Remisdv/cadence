use tauri::State;

use crate::{
    api::github::repos,
    domain::version::{CloseReport, Version, VersionCloseInput, VersionDetail},
    error::{AppError, AppResult},
    state::AppState,
};

/// Sprints et paliers, triés par semver.
#[tauri::command]
pub async fn versions_list(state: State<'_, AppState>) -> AppResult<Vec<Version>> {
    let project = state.require_project().await?;
    if state.versions.read().await.is_empty() {
        // Projet rouvert au lancement : on réimporte les milestones.
        let token = state.github_token()?;
        let milestones = repos::milestones(&state.http, &token, &project.repo.full_name).await?;
        *state.versions.write().await = repos::to_versions(&milestones).0;
    }
    Ok(state.versions.read().await.clone())
}

/// Version en cours : portée, burndown, rythme.
#[tauri::command]
pub async fn version_current(state: State<'_, AppState>) -> AppResult<VersionDetail> {
    state.require_project().await?;
    Err(AppError::NotImplemented("version_current"))
}

/// Non réversible, jamais mis en file hors-ligne.
#[tauri::command]
pub async fn version_close(
    state: State<'_, AppState>,
    input: VersionCloseInput,
) -> AppResult<CloseReport> {
    state.require_project().await?;
    if crate::domain::version::parse(&input.next).is_none() {
        return Err(AppError::Validation(format!("« {} » n'est pas une version semver.", input.next)));
    }
    Err(AppError::NotImplemented("version_close"))
}
