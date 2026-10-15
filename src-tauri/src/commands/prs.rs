use tauri::State;

use crate::{
    domain::pr::{PrFilter, PrRow, ReviewRow},
    error::{AppError, AppResult},
    state::AppState,
};

#[tauri::command]
pub async fn prs_list(state: State<'_, AppState>, filter: PrFilter) -> AppResult<Vec<PrRow>> {
    let _ = filter;
    state.require_project().await?;
    Err(AppError::NotImplemented("prs_list"))
}

/// Revues en attente, séparées entre « vous » et « les autres ».
#[tauri::command]
pub async fn reviews_pending(state: State<'_, AppState>) -> AppResult<Vec<ReviewRow>> {
    state.require_project().await?;
    Err(AppError::NotImplemented("reviews_pending"))
}
