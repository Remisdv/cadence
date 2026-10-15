use secrecy::SecretString;
use tauri::{AppHandle, State};

use crate::{
    auth::{self, cli::CliTool, device_flow},
    domain::auth::{AuthProvider, AuthStatus, CliAgentInfo, DeviceFlowStart, GithubAuth, ProviderStatus},
    error::{AppError, AppResult},
    events::{self, AuthChanged},
    state::AppState,
};

/// Démarre le device flow ; le polling continue côté Rust et se termine par `auth://changed`.
/// Un nouvel appel annule le code précédent.
#[tauri::command]
pub async fn auth_start_device_flow(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<DeviceFlowStart> {
    let client_id = auth::client_id()?;
    let code = device_flow::request_code(&state.http, &client_id, &auth::scope_param()).await?;
    let start = DeviceFlowStart {
        user_code: code.user_code.clone(),
        verification_uri: code.verification_uri.clone(),
        expires_in: u32::try_from(code.expires_in).unwrap_or(u32::MAX),
    };
    state.replace_device_flow(device_flow::spawn_polling(app, client_id, code));
    Ok(start)
}

/// Lu au lancement pour décider entre onboarding et app.
#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> AppResult<AuthStatus> {
    Ok(AuthStatus {
        github: auth::github_status(&state).await,
        claude_code: auth::cli_status(&state, CliTool::ClaudeCode, false).await,
        codex: auth::cli_status(&state, CliTool::Codex, false).await,
    })
}

/// Repli par jeton personnel (indispensable pour GitHub Enterprise Server).
/// Le jeton est validé puis rangé dans le trousseau ; il n'est jamais renvoyé.
#[tauri::command]
pub async fn auth_set_token(
    app: AppHandle,
    state: State<'_, AppState>,
    token: String,
) -> AppResult<GithubAuth> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AppError::Validation("Le jeton est vide.".into()));
    }
    let auth = auth::complete_sign_in(&state, SecretString::from(token.to_string())).await?;
    state.abort_device_flow();
    events::emit(&app, events::AUTH_CHANGED, AuthChanged::github(ProviderStatus::Connected));
    Ok(auth)
}

#[tauri::command]
pub async fn auth_sign_out(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    auth::sign_out(&state).await?;
    events::emit(&app, events::AUTH_CHANGED, AuthChanged::github(ProviderStatus::Disconnected));
    Ok(())
}

/// Relance la détection puis la session de test d'un agent de code.
#[tauri::command]
pub async fn cli_agent_test(
    state: State<'_, AppState>,
    provider: AuthProvider,
) -> AppResult<CliAgentInfo> {
    let tool = CliTool::from_provider(provider)?;
    auth::cli_status(&state, tool, true).await;
    auth::test_cli(&state, tool).await
}

/// « Localiser le binaire… » : vérifie le fichier choisi, puis l'enregistre.
#[tauri::command]
pub async fn cli_agent_set_path(
    state: State<'_, AppState>,
    provider: AuthProvider,
    path: String,
) -> AppResult<CliAgentInfo> {
    let tool = CliTool::from_provider(provider)?;
    let candidate = std::path::PathBuf::from(&path);
    if !candidate.is_file() {
        return Err(AppError::Validation(format!("{path} n'est pas un fichier.")));
    }
    auth::cli::read_version(&candidate).await?;
    state.settings.update(|s| {
        s.set_cli_path(tool, Some(path));
        s.set_enabled(tool, true);
    })?;
    Ok(auth::cli_status(&state, tool, true).await)
}

/// « Déconnecter » / « Ignorer » un agent de code, ou le réactiver.
#[tauri::command]
pub async fn cli_agent_set_enabled(
    state: State<'_, AppState>,
    provider: AuthProvider,
    enabled: bool,
) -> AppResult<CliAgentInfo> {
    let tool = CliTool::from_provider(provider)?;
    state.settings.update(|s| s.set_enabled(tool, enabled))?;
    Ok(auth::cli_status(&state, tool, true).await)
}
