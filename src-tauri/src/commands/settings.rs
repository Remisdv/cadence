use tauri::State;

use crate::{
    api::github::writes,
    domain::settings::Config,
    error::AppResult,
    project_config,
    state::{AppState, LoadedConfig},
};

/// `.cadence/config.toml` du projet ouvert, ou les valeurs par défaut s'il n'existe pas.
#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> AppResult<Config> {
    Ok(load(&state).await?.config)
}

/// Valide puis écrit `.cadence/config.toml` (un commit sur la branche par défaut) et renvoie
/// la configuration enregistrée.
#[tauri::command]
pub async fn settings_set(state: State<'_, AppState>, config: Config) -> AppResult<Config> {
    let loaded = load(&state).await?;
    let config = Config {
        project: crate::domain::settings::ProjectConfig { repo: loaded.repo.clone(), ..config.project },
        ..config
    };
    project_config::validate(&config)?;
    let text = project_config::to_toml(&config)?;
    let token = state.github_token()?;
    let message = if loaded.sha.is_some() {
        "chore(cadence): mise à jour de la configuration"
    } else {
        "chore(cadence): configuration du projet"
    };
    let sha = writes::put_file(
        &state.http,
        &token,
        &loaded.repo,
        project_config::PATH,
        &text,
        loaded.sha.as_deref(),
        message,
    )
    .await?;
    *state.config.write().await =
        Some(LoadedConfig { repo: loaded.repo.clone(), config: config.clone(), sha: Some(sha) });
    let mut project = state.project.write().await;
    if let Some(p) = project.as_mut().filter(|p| !p.has_config) {
        p.has_config = true;
        let saved = p.clone();
        state.settings.update(|s| s.last_project = Some(saved))?;
    }
    Ok(config)
}

/// Configuration du dépôt ouvert, lue sur GitHub au premier appel.
pub async fn load(state: &AppState) -> AppResult<LoadedConfig> {
    let repo = state.require_project().await?.repo.full_name;
    if let Some(loaded) = state.config.read().await.as_ref().filter(|c| c.repo == repo) {
        return Ok(loaded.clone());
    }
    let base = project_config::defaults(&repo, &state.settings.get().agent_engines);
    let token = state.github_token()?;
    let loaded = match writes::get_file(&state.http, &token, &repo, project_config::PATH).await? {
        Some((text, sha)) => LoadedConfig { config: project_config::parse(&text, &base)?, sha: Some(sha), repo },
        None => LoadedConfig { config: base, sha: None, repo },
    };
    *state.config.write().await = Some(loaded.clone());
    Ok(loaded)
}
