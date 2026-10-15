use tauri::State;

use crate::{
    api::github::repos,
    domain::{
        sync::SyncResource,
        version::{CloseReport, Version, VersionCloseInput, VersionDetail},
    },
    error::{AppError, AppResult},
    state::AppState,
    sync,
};

/// Sprints et paliers, triés par semver. Lus dans le cache, milestones resynchronisés
/// au plus une fois par minute ; le sprint en cours du projet suit (milestone fermé ou ajouté).
#[tauri::command]
pub async fn versions_list(state: State<'_, AppState>) -> AppResult<Vec<Version>> {
    let cache = state.repo_cache().await?;
    sync::refresh(&state, &cache, SyncResource::Milestones).await?;
    let (versions, _) = repos::to_versions(&cache.milestones().await?);

    let current = repos::current_version(&versions);
    let mut project = state.project.write().await;
    if let Some(p) = project.as_mut().filter(|p| p.current_version != current) {
        p.current_version = current;
        let saved = p.clone();
        state.settings.update(|s| s.last_project = Some(saved))?;
    }
    Ok(versions)
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
