use tauri::State;

use crate::{
    api::github::repos,
    domain::project::{OwnerKind, ProjectPreview, ProjectState, Repo, RepoCreateInput, RepoOwner},
    error::AppResult,
    state::AppState,
};

/// Projet ouvert, s'il y en a un : décide au lancement entre l'app et l'onboarding.
/// Au premier appel, rouvre le dernier projet choisi.
#[tauri::command]
pub async fn project_current(state: State<'_, AppState>) -> AppResult<Option<ProjectState>> {
    if let Some(project) = state.project.read().await.clone() {
        return Ok(Some(project));
    }
    let last = state.settings.get().last_project;
    if let Some(project) = &last {
        *state.project.write().await = Some(project.clone());
    }
    Ok(last)
}

/// Dépôts accessibles au jeton, filtrés par `query` (nom, propriétaire, description).
/// La liste est lue une fois par session (une requête GraphQL), puis filtrée localement.
#[tauri::command]
pub async fn project_list_repos(state: State<'_, AppState>, query: String) -> AppResult<Vec<Repo>> {
    if state.repos.read().await.is_none() {
        let token = state.github_token()?;
        let list = repos::viewer_repos(&state.http, &token).await?;
        *state.repos.write().await = Some(list);
    }
    let cached = state.repos.read().await;
    Ok(repos::filter_repos(cached.as_deref().unwrap_or_default(), &query))
}

/// Comptes sous lesquels créer un dépôt : l'utilisateur, puis ses organisations.
#[tauri::command]
pub async fn project_owners(state: State<'_, AppState>) -> AppResult<Vec<RepoOwner>> {
    let token = state.github_token()?;
    let me = crate::api::github::fetch_user(&state.http, &token).await?;
    let mut owners =
        vec![RepoOwner { login: me.login, kind: OwnerKind::User, avatar_url: me.avatar_url }];
    owners.extend(repos::user_orgs(&state.http, &token).await?);
    Ok(owners)
}

/// Crée un dépôt sur GitHub, puis l'ouvre (et le mémorise) comme `project_open`.
#[tauri::command]
pub async fn project_create(
    state: State<'_, AppState>,
    input: RepoCreateInput,
) -> AppResult<ProjectState> {
    input.validate()?;
    let token = state.github_token()?;
    let me = crate::api::github::fetch_user(&state.http, &token).await?;
    let kind = if input.owner == me.login { OwnerKind::User } else { OwnerKind::Organization };
    let repo = repos::create_repo(&state.http, &token, &input, kind).await?;
    // Le nouveau dépôt rejoint la liste en cache, en tête (le plus récent).
    if let Some(list) = state.repos.write().await.as_mut() {
        list.insert(0, repo.clone());
    }
    project_open(state, repo.full_name).await
}

/// Ouvre un dépôt : importe les milestones et mémorise le choix pour le prochain lancement.
#[tauri::command]
pub async fn project_open(state: State<'_, AppState>, repo: String) -> AppResult<ProjectState> {
    let preview = load_preview(&state, &repo).await?;
    let project = ProjectState {
        repo: preview.repo,
        current_version: preview.current_version,
        has_config: preview.has_config,
    };
    state.clear_project_data().await;
    *state.versions.write().await = preview.versions;
    *state.project.write().await = Some(project.clone());
    state.settings.update(|s| s.last_project = Some(project.clone()))?;
    Ok(project)
}

async fn load_preview(state: &AppState, full_name: &str) -> AppResult<ProjectPreview> {
    let token = state.github_token()?;
    let repo = match find_repo(state, full_name).await {
        Some(repo) => repo,
        None => {
            // Dépôt absent du cache (saisi à la main, ou liste périmée) : on relit la liste.
            *state.repos.write().await = Some(repos::viewer_repos(&state.http, &token).await?);
            find_repo(state, full_name).await.ok_or_else(|| {
                crate::error::AppError::Validation(format!("Dépôt {full_name} inaccessible avec ce compte."))
            })?
        }
    };
    let (milestones, has_config) = tokio::try_join!(
        repos::milestones(&state.http, &token, full_name),
        repos::has_cadence_config(&state.http, &token, full_name),
    )?;
    let (versions, ignored_milestones) = repos::to_versions(&milestones);
    Ok(ProjectPreview {
        current_version: repos::current_version(&versions),
        repo,
        versions,
        ignored_milestones,
        has_config,
    })
}

async fn find_repo(state: &AppState, full_name: &str) -> Option<Repo> {
    state.repos.read().await.as_ref()?.iter().find(|r| r.full_name == full_name).cloned()
}
