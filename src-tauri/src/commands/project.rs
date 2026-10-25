use tauri::State;

use crate::{
    api::github::repos,
    domain::{
        project::{OwnerKind, ProjectState, Repo, RepoCreateInput, RepoOwner},
        sync::SyncResource,
    },
    error::{AppError, AppResult},
    state::AppState,
    sync,
};

/// Projet ouvert, s'il y en a un : décide au lancement entre l'app et l'onboarding.
/// Au premier appel, rouvre le dernier projet choisi.
#[tauri::command]
pub async fn project_current(state: State<'_, AppState>) -> AppResult<Option<ProjectState>> {
    if let Some(project) = state.project.read().await.clone() {
        return Ok(Some(project));
    }
    let last = state.settings.get().legacy.last_project;
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
    let repo = create(&state, &input).await?;
    project_open(state, repo.full_name).await
}

/// Crée un dépôt sur GitHub, sans rien ouvrir : le sélecteur de dépôt d'un plugin le choisit
/// ensuite (`app_settings_set`, `repoSelection`).
#[tauri::command]
pub async fn repo_create(state: State<'_, AppState>, input: RepoCreateInput) -> AppResult<Repo> {
    create(&state, &input).await
}

async fn create(state: &AppState, input: &RepoCreateInput) -> AppResult<Repo> {
    input.validate()?;
    let token = state.github_token()?;
    let me = crate::api::github::fetch_user(&state.http, &token).await?;
    let kind = if input.owner == me.login { OwnerKind::User } else { OwnerKind::Organization };
    let repo = repos::create_repo(&state.http, &token, input, kind).await?;
    // Le nouveau dépôt rejoint la liste en cache, en tête (le plus récent).
    if let Some(list) = state.repos.write().await.as_mut() {
        list.insert(0, repo.clone());
    }
    Ok(repo)
}

/// Ouvre un dépôt : synchronise ses milestones dans son cache et mémorise le choix pour le
/// prochain lancement. Le projet courant ne change qu'une fois tout chargé.
#[tauri::command]
pub async fn project_open(state: State<'_, AppState>, repo: String) -> AppResult<ProjectState> {
    let repo = resolve_repo(&state, &repo).await?;
    let token = state.github_token()?;
    let cache = state.open_cache(&repo.full_name).await?;
    let (has_config, ()) = tokio::try_join!(
        repos::has_cadence_config(&state.http, &token, &repo.full_name),
        sync::refresh(&state, &cache, SyncResource::Milestones),
    )?;
    let (versions, ignored) = repos::to_versions(&cache.milestones().await?);
    if !ignored.is_empty() {
        tracing::info!(?ignored, "milestones sans numéro semver ignorés");
    }
    let project =
        ProjectState { repo, current_version: repos::current_version(&versions), has_config };
    state.install_cache(cache).await;
    *state.project.write().await = Some(project.clone());
    state.settings.update(|s| s.legacy.last_project = Some(project.clone()))?;
    Ok(project)
}

/// Dépôt `full_name` parmi ceux accessibles au jeton.
async fn resolve_repo(state: &AppState, full_name: &str) -> AppResult<Repo> {
    if let Some(repo) = find_repo(state, full_name).await {
        return Ok(repo);
    }
    // Dépôt absent du cache (saisi à la main, ou liste périmée) : on relit la liste.
    let token = state.github_token()?;
    *state.repos.write().await = Some(repos::viewer_repos(&state.http, &token).await?);
    find_repo(state, full_name)
        .await
        .ok_or_else(|| AppError::Validation(format!("Dépôt {full_name} inaccessible avec ce compte.")))
}

async fn find_repo(state: &AppState, full_name: &str) -> Option<Repo> {
    state.repos.read().await.as_ref()?.iter().find(|r| r.full_name == full_name).cloned()
}
