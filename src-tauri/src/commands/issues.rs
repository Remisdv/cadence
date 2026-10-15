use tauri::State;

use crate::{
    domain::issue::{CtoAnswer, IssueCreateInput, IssueDetail, IssueFilter, IssueRow},
    error::{AppError, AppResult},
    state::AppState,
};

/// Lit le cache, jamais l'API.
#[tauri::command]
pub async fn issues_list(state: State<'_, AppState>, filter: IssueFilter) -> AppResult<Vec<IssueRow>> {
    let _ = filter;
    state.require_project().await?;
    Err(AppError::NotImplemented("issues_list"))
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
