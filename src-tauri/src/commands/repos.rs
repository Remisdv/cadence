//! Service Dépôts GitHub : état des clones (Réglages), clone désigné, et `repos.checkout` du
//! pont de l'iframe (le même service que l'outil du cœur `cadence__repo_checkout`).

use std::path::PathBuf;

use tauri::State;

use crate::{
    api::github::repos as gh_repos,
    error::{AppError, AppResult},
    domain::project::{OwnerKind, Repo, RepoCreateInput},
    plugins::manifest::PluginService,
    repos::{RepoCheckout, RepoCloneView},
    state::AppState,
};

/// Copie locale de chaque dépôt suivi (tous plugins confondus).
#[tauri::command]
pub fn repos_status(state: State<'_, AppState>) -> Vec<RepoCloneView> {
    state.repo_service.status(&state.repo_service.tracked())
}

/// Clone au besoin et prépare le worktree de `task` pour l'interface d'un plugin, qui doit
/// déclarer `services = ["repos"]` et suivre le dépôt.
#[tauri::command]
pub async fn repos_checkout(
    state: State<'_, AppState>,
    plugin: String,
    repo: String,
    task: Option<String>,
) -> AppResult<RepoCheckout> {
    let manifest = state.plugins.manifest(&plugin)?;
    if !manifest.services.contains(&PluginService::Repos) {
        return Err(AppError::Validation(format!("Le plugin « {plugin} » ne déclare pas services = [\"repos\"].")));
    }
    if !state.repo_service.is_tracked(&repo, Some(&plugin)) {
        return Err(AppError::Validation(format!("Le plugin « {plugin} » ne suit pas le dépôt {repo}.")));
    }
    let path = state.repo_service.checkout(&repo, task.as_deref()).await?;
    Ok(RepoCheckout { path: path.display().to_string() })
}

/// « Désigner un clone existant… » ; `path` nul : revenir au clone géré par Cadence.
#[tauri::command]
pub async fn repos_link(state: State<'_, AppState>, repo: String, path: Option<String>) -> AppResult<RepoCloneView> {
    let path = path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).map(PathBuf::from);
    state.repo_service.link(&repo, path.as_deref()).await
}

/// Crée un dépôt GitHub (compte ou organisation) depuis le sélecteur de la barre d'un plugin ;
/// il rejoint en tête la liste des dépôts du compte. Le plugin le choisit ensuite (suivi et
/// sélectionné), sans passer par l'ancien « projet ouvert ».
#[tauri::command]
pub async fn repos_create(state: State<'_, AppState>, input: RepoCreateInput) -> AppResult<Repo> {
    input.validate()?;
    let token = state.github_token()?;
    let me = crate::api::github::fetch_user(&state.http, &token).await?;
    let kind = if input.owner.eq_ignore_ascii_case(&me.login) { OwnerKind::User } else { OwnerKind::Organization };
    let repo = gh_repos::create_repo(&state.http, &token, &input, kind).await?;
    if let Some(list) = state.repos.write().await.as_mut() {
        list.insert(0, repo.clone());
    }
    Ok(repo)
}
