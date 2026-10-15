use tauri::State;

use crate::{
    api::github::pulls,
    domain::{
        pr::{PrDetail, PrFilter, PrRow, ReviewRow},
        sync::SyncResource,
    },
    error::{AppError, AppResult},
    state::AppState,
    sync,
};

/// PR ouvertes et 30 dernières fusionnées ou fermées, lues dans le cache (resynchronisé au
/// plus une fois par minute), triées par activité.
#[tauri::command]
pub async fn prs_list(state: State<'_, AppState>, filter: PrFilter) -> AppResult<Vec<PrRow>> {
    let cache = state.repo_cache().await?;
    sync::refresh(&state, &cache, SyncResource::PullRequests).await?;
    let me = viewer(&state).await;
    Ok(cache
        .pulls()
        .await?
        .iter()
        .map(|raw| pulls::to_row(raw, me.as_deref()))
        .filter(|row| filter.matches(row))
        .collect())
}

/// Revues en attente, séparées entre « vous » (relecteur demandé) et « les autres »
/// (vos PR qui attendent une revue), la plus ancienne demande d'abord.
#[tauri::command]
pub async fn reviews_pending(state: State<'_, AppState>) -> AppResult<Vec<ReviewRow>> {
    let cache = state.repo_cache().await?;
    sync::refresh(&state, &cache, SyncResource::PullRequests).await?;
    let Some(me) = viewer(&state).await else { return Ok(vec![]) };
    let mut rows: Vec<ReviewRow> = cache
        .pulls()
        .await?
        .iter()
        .filter_map(|raw| pulls::review_row(raw, &pulls::to_row(raw, Some(&me)), &me))
        .collect();
    rows.sort_by(|a, b| a.requested_at.cmp(&b.requested_at));
    Ok(rows)
}

/// Écran de revue : la PR (cache) et ses fichiers avec leurs patchs (lus sur GitHub).
#[tauri::command]
pub async fn pr_detail(state: State<'_, AppState>, number: u32) -> AppResult<PrDetail> {
    let cache = state.repo_cache().await?;
    sync::refresh(&state, &cache, SyncResource::PullRequests).await?;
    let raw = cache.pull(number).await?.ok_or_else(|| {
        AppError::Validation(format!("PR #{number} absente : seules les PR ouvertes et récentes sont lues."))
    })?;
    let token = state.github_token()?;
    let files = pulls::fetch_files(&state.http, &token, &cache.repo, number).await?;
    let me = viewer(&state).await;
    Ok(PrDetail { pr: pulls::to_row(&raw, me.as_deref()), body: raw.body.clone(), url: raw.url.clone(), files })
}

async fn viewer(state: &AppState) -> Option<String> {
    state.github.read().await.as_ref().and_then(|g| g.login.clone())
}
