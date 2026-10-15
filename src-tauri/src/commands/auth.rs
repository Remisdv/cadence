use secrecy::SecretString;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::{
    auth::{self, cli::CliTool, device_flow, web_flow},
    domain::auth::{AuthProvider, AuthStatus, CliAgentInfo, DeviceFlowStart, GithubAuth, ProviderStatus, WebFlowStart},
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
    state.replace_sign_in(device_flow::spawn_polling(app, client_id, code));
    Ok(start)
}

/// Connexion par le navigateur, avec retour automatique dans l'app. L'issue arrive par
/// `auth://changed` ; un nouvel appel annule le précédent.
/// - OAuth App de Cadence configurée : flux web direct (redirection 127.0.0.1 + PKCE) ;
/// - sinon, Git Credential Manager, qui a sa propre application OAuth chez GitHub.
#[tauri::command]
pub async fn auth_start_web_flow(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<WebFlowStart> {
    let sources = auth::github_sources(&state).await;
    if sources.oauth_app {
        let flow = web_flow::prepare(auth::client_id()?, &auth::scope_param()).await?;
        let authorize_url = flow.authorize_url.clone();
        app.opener()
            .open_url(&authorize_url, None::<&str>)
            .map_err(|e| AppError::Internal(format!("navigateur introuvable : {e}")))?;
        state.replace_sign_in(web_flow::spawn_wait(app, flow));
        return Ok(WebFlowStart { authorize_url: Some(authorize_url) });
    }
    if sources.credential_manager {
        state.replace_sign_in(auth::spawn_credential_manager(app));
        return Ok(WebFlowStart { authorize_url: None });
    }
    Err(AppError::Config(
        "Connexion par navigateur indisponible : installez Git (avec Git Credential Manager) ou \
         connectez la CLI GitHub (`gh auth login`). Le jeton personnel reste possible."
            .into(),
    ))
}

/// Reprend la session de la CLI GitHub (`gh`) : aucun navigateur, aucun code.
#[tauri::command]
pub async fn auth_use_gh_cli(app: AppHandle, state: State<'_, AppState>) -> AppResult<GithubAuth> {
    let token = auth::external::gh_token().await?;
    let account = auth::complete_sign_in(&state, token).await?;
    state.abort_sign_in();
    events::emit(&app, events::AUTH_CHANGED, AuthChanged::github(ProviderStatus::Connected));
    Ok(account)
}

/// Lu au lancement pour décider entre onboarding et app.
#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> AppResult<AuthStatus> {
    Ok(AuthStatus {
        github: auth::github_status(&state).await,
        github_sources: auth::github_sources(&state).await,
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
    state.abort_sign_in();
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
