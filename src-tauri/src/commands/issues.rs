use tauri::State;

use serde_json::json;

use crate::{
    agents::engine,
    api::github::{issues, repos, writes},
    domain::{
        agent::{AgentKind, CtoItemState},
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

/// Détail d'une issue (cache), avec les questions de l'agent CTO en attente sur elle.
#[tauri::command]
pub async fn issue_get(state: State<'_, AppState>, number: u32) -> AppResult<IssueDetail> {
    let cache = state.repo_cache().await?;
    sync::refresh(&state, &cache, SyncResource::Issues).await?;
    let raw = cache
        .issues()
        .await?
        .into_iter()
        .find(|i| i.number == number)
        .ok_or_else(|| AppError::Validation(format!("Issue #{number} introuvable dans ce dépôt.")))?;
    let me = state.github.read().await.as_ref().and_then(|g| g.login.clone());
    let mut row = issues::to_row(&raw, me.as_deref());
    let mut questions = vec![];
    if let Some(handle) = engine::latest(&state, AgentKind::Cto).await? {
        let d = handle.detail.read().await;
        if let Some(item) = d.items.iter().find(|i| i.number == number) {
            match item.state {
                CtoItemState::Questions => questions = item.questions.clone(),
                CtoItemState::Running => row.active_agent = Some(AgentKind::Cto),
                _ => {}
            }
        }
    }
    Ok(IssueDetail {
        body: raw.body.clone().unwrap_or_default(),
        html_url: raw.html_url.clone().unwrap_or_else(|| format!("https://github.com/{}/issues/{number}", cache.repo)),
        author: raw.user.as_ref().map(|u| crate::domain::UserRef {
            login: u.login.clone(),
            name: None,
            avatar_url: u.avatar_url.clone(),
            is_self: me.as_deref() == Some(u.login.as_str()),
        }),
        created_at: raw.created_at.clone().unwrap_or_else(|| raw.updated_at.clone()),
        questions,
        row,
    })
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    number: u32,
    answers: Vec<CtoAnswer>,
) -> AppResult<()> {
    engine::answer(&app, &state, number, answers).await
}
