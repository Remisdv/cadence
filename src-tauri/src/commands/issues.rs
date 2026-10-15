use std::time::Instant;

use tauri::State;

use crate::{
    api::github::issues::{self, IssueCache, IssuesFetch},
    domain::issue::{CtoAnswer, IssueCreateInput, IssueDetail, IssueFilter, IssueRow},
    error::{AppError, AppResult},
    state::AppState,
};

/// Issues du dépôt ouvert. Lit le cache ; GitHub n'est interrogé que si la dernière
/// vérification date de plus de 60 s, et la requête est conditionnelle (ETag : un 304 ne
/// consomme pas de quota). Hors ligne, le cache existant est servi tel quel.
#[tauri::command]
pub async fn issues_list(state: State<'_, AppState>, filter: IssueFilter) -> AppResult<Vec<IssueRow>> {
    let repo = state.require_project().await?.repo.full_name;
    refresh_issues(&state, &repo).await?;

    let me = state.github.read().await.as_ref().and_then(|g| g.login.clone());
    let cache = state.issues.read().await;
    let items = cache.as_ref().map(|c| c.items.as_slice()).unwrap_or_default();
    Ok(items
        .iter()
        .map(|raw| issues::to_row(raw, me.as_deref()))
        .filter(|row| issues::matches(row, &filter))
        .collect())
}

async fn refresh_issues(state: &AppState, repo: &str) -> AppResult<()> {
    let (fresh_enough, etag, has_cache) = match state.issues.read().await.as_ref() {
        Some(c) if c.repo == repo => {
            (c.checked_at.elapsed() < issues::REFRESH_EVERY, c.etag.clone(), true)
        }
        _ => (false, None, false),
    };
    if fresh_enough {
        return Ok(());
    }

    let token = state.github_token()?;
    match issues::fetch_issues(&state.http, &token, repo, etag.as_deref()).await {
        Ok(IssuesFetch::NotModified) => {
            if let Some(cache) = state.issues.write().await.as_mut() {
                cache.checked_at = Instant::now();
            }
            Ok(())
        }
        Ok(IssuesFetch::Fresh { etag, issues: items }) => {
            *state.issues.write().await = Some(IssueCache {
                repo: repo.to_string(),
                etag,
                checked_at: Instant::now(),
                items,
            });
            Ok(())
        }
        // Réseau coupé : on sert le cache existant plutôt que d'échouer.
        Err(AppError::Network(err)) if has_cache => {
            tracing::warn!(%err, "issues : GitHub injoignable, cache servi");
            Ok(())
        }
        Err(err) => Err(err),
    }
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
