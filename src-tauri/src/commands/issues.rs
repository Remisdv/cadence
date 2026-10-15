use tauri::State;

use serde_json::json;

use crate::{
    api::github::{issues, repos, writes},
    domain::{
        issue::{CtoAnswer, IssueCreateInput, IssueDetail, IssueFilter, IssueRow},
        sync::SyncResource,
        version::{self, VersionState},
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

/// Crée l'issue sur GitHub, rattachée au milestone de sa version, puis la range dans le
/// cache. Refuse une issue sans titre ou sans version ouverte (règle produit).
/// L'envoi à l'agent CTO arrive avec l'agent (0.1.3).
#[tauri::command]
pub async fn issue_create(state: State<'_, AppState>, input: IssueCreateInput) -> AppResult<IssueRow> {
    input.validate()?;
    let cache = state.repo_cache().await?;
    sync::refresh(&state, &cache, SyncResource::Milestones).await?;
    let wanted = version::parse(&input.version).map(|v| v.to_string()).unwrap_or_else(|| input.version.clone());
    let (versions, _) = repos::to_versions(&cache.milestones().await?);
    let target = versions
        .iter()
        .find(|v| v.title == wanted && v.state == VersionState::Open)
        .ok_or_else(|| AppError::Validation(format!("La version {wanted} n'est pas un milestone ouvert du dépôt.")))?;

    let mut labels: Vec<String> = input.issue_type.map(|t| t.label()).into_iter().collect();
    labels.extend(input.labels.iter().flatten().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()));
    labels.dedup();
    let payload = json!({
        "title": input.title.trim(),
        "body": input.body.as_deref().map(str::trim).filter(|b| !b.is_empty()),
        "milestone": target.milestone_number,
        "labels": labels,
        "assignees": input.assignee.iter().collect::<Vec<_>>(),
    });
    let token = state.github_token()?;
    let (issue, raw) = writes::create_issue(&state.http, &token, &cache.repo, &payload).await?;
    cache.upsert_issues(&[(issue.clone(), raw)]).await?;
    // Compteurs du milestone changés : relus à la prochaine demande.
    cache.invalidate(SyncResource::Milestones).await?;

    let me = state.github.read().await.as_ref().and_then(|g| g.login.clone());
    Ok(issues::to_row(&issue, me.as_deref()))
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
