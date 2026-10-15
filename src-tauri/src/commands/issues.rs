use tauri::State;

use crate::{
    api::github::issues,
    domain::{
        issue::{CtoAnswer, IssueCreateInput, IssueDetail, IssueFilter, IssueRow},
        sync::SyncResource,
    },
    error::{AppError, AppResult},
    state::AppState,
    sync,
};

/// Issues du dépôt ouvert, lues dans le cache SQLite. GitHub n'est interrogé que si la
/// dernière passe date de plus de 60 s, et seulement pour ce qui a changé depuis (`since`
/// + ETag : un 304 ne consomme pas de quota). Hors ligne, le cache est servi tel quel.
#[tauri::command]
pub async fn issues_list(state: State<'_, AppState>, filter: IssueFilter) -> AppResult<Vec<IssueRow>> {
    let cache = state.repo_cache().await?;
    sync::refresh(&state, &cache, SyncResource::Issues).await?;

    let me = state.github.read().await.as_ref().and_then(|g| g.login.clone());
    Ok(cache
        .issues()
        .await?
        .iter()
        .map(|raw| issues::to_row(raw, me.as_deref()))
        .filter(|row| issues::matches(row, &filter))
        .collect())
}

/// Détail avec l'état de l'agent CTO et ses questions.
#[tauri::command]
pub async fn issue_get(state: State<'_, AppState>, number: u32) -> AppResult<IssueDetail> {
    let _ = number;
    state.require_project().await?;
    Err(AppError::NotImplemented("issue_get"))
}

/// Refuse une issue sans titre ou sans version (règle produit).
#[tauri::command]
pub async fn issue_create(state: State<'_, AppState>, input: IssueCreateInput) -> AppResult<IssueRow> {
    input.validate()?;
    state.require_project().await?;
    Err(AppError::NotImplemented("issue_create"))
}

/// Renvoie les réponses dans la session CTO, qui reprend et complète la description.
#[tauri::command]
pub async fn issue_answer_cto(
    state: State<'_, AppState>,
    number: u32,
    answers: Vec<CtoAnswer>,
) -> AppResult<()> {
    let _ = (number, answers);
    state.require_project().await?;
    Err(AppError::NotImplemented("issue_answer_cto"))
}
