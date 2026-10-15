//! Authentification GitHub (device flow, jeton personnel) et agents de code locaux.
//!
//! Le jeton GitHub va dans le trousseau de l'OS. Il n'est jamais renvoyé au webview,
//! jamais écrit dans `config.toml`, jamais passé en argument de ligne de commande.

pub mod cli;
pub mod device_flow;
pub mod token_store;

use secrecy::{ExposeSecret, SecretString};

use crate::{
    api::github,
    domain::auth::{CliAgentInfo, GithubAuth, ProviderStatus},
    error::{AppError, AppResult},
    state::AppState,
};
use cli::CliTool;

pub const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
pub const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

/// Portées demandées. `read:user` est optionnelle (avatar, nom affiché).
pub const REQUIRED_SCOPES: [&str; 3] = ["repo", "read:org", "workflow"];
pub const OPTIONAL_SCOPES: [&str; 1] = ["read:user"];

/// Service sous lequel le jeton est rangé dans le trousseau.
pub const KEYRING_SERVICE: &str = "app.cadence.desktop";

/// Binaires des agents de code.
pub const CLAUDE_CODE_BIN: &str = "claude";
pub const CODEX_BIN: &str = "codex";

/// Client ID de l'application OAuth GitHub (public par construction).
/// Lu à l'exécution, sinon figé à la compilation (`CADENCE_GITHUB_CLIENT_ID`).
pub fn client_id() -> AppResult<String> {
    std::env::var("CADENCE_GITHUB_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("CADENCE_GITHUB_CLIENT_ID").map(str::to_string))
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| {
            AppError::Config(
                "Client ID GitHub absent : créer une OAuth App avec le device flow activé, \
                 puis définir CADENCE_GITHUB_CLIENT_ID. En attendant, un jeton personnel fonctionne."
                    .into(),
            )
        })
}

pub fn scope_param() -> String {
    REQUIRED_SCOPES.iter().chain(OPTIONAL_SCOPES.iter()).copied().collect::<Vec<_>>().join(" ")
}

/// Valide un jeton auprès de GitHub, le range dans le trousseau et met le compte en cache.
pub async fn complete_sign_in(state: &AppState, token: SecretString) -> AppResult<GithubAuth> {
    let user = github::fetch_user(&state.http, &token).await.map_err(|err| match err {
        AppError::Unauthenticated(_) => AppError::Validation("GitHub refuse ce jeton.".into()),
        other => other,
    })?;
    let missing = github::missing_scopes(&user.scopes, &REQUIRED_SCOPES);
    if !missing.is_empty() {
        return Err(AppError::Validation(format!(
            "Portées manquantes sur le jeton : {}.",
            missing.join(", ")
        )));
    }

    state.tokens.set(token.expose_secret())?;
    let auth = GithubAuth {
        status: ProviderStatus::Connected,
        login: Some(user.login),
        name: user.name,
        avatar_url: user.avatar_url,
        scopes: user.scopes,
    };
    *state.github.write().await = Some(auth.clone());
    Ok(auth)
}

/// État du compte GitHub. Hors ligne avec un jeton : connecté, compte inconnu (réessayé plus tard).
pub async fn github_status(state: &AppState) -> GithubAuth {
    if let Some(cached) = state.github.read().await.clone() {
        return cached;
    }
    let token = match state.tokens.get() {
        Ok(Some(token)) => token,
        Ok(None) => return GithubAuth::disconnected(),
        Err(err) => {
            tracing::error!(%err, "lecture du trousseau");
            return GithubAuth { status: ProviderStatus::Error, ..GithubAuth::disconnected() };
        }
    };
    match github::fetch_user(&state.http, &token).await {
        Ok(user) => {
            let auth = GithubAuth {
                status: ProviderStatus::Connected,
                login: Some(user.login),
                name: user.name,
                avatar_url: user.avatar_url,
                scopes: user.scopes,
            };
            *state.github.write().await = Some(auth.clone());
            auth
        }
        Err(AppError::Unauthenticated(_)) => {
            // Jeton révoqué côté GitHub : on l'oublie.
            let _ = state.tokens.delete();
            GithubAuth::disconnected()
        }
        Err(err) => {
            tracing::warn!(%err, "compte GitHub injoignable");
            GithubAuth { status: ProviderStatus::Connected, ..GithubAuth::disconnected() }
        }
    }
}

/// Oublie le jeton et le projet ouvert : l'autre compte n'a peut-être pas accès au même dépôt.
pub async fn sign_out(state: &AppState) -> AppResult<()> {
    state.abort_device_flow();
    state.tokens.delete()?;
    *state.github.write().await = None;
    *state.repos.write().await = None;
    *state.project.write().await = None;
    state.versions.write().await.clear();
    state.settings.update(|s| s.last_project = None)?;
    Ok(())
}

/* ---------- Agents de code ---------- */

/// État d'un agent de code, détecté au premier appel puis gardé en cache pour la session.
pub async fn cli_status(state: &AppState, tool: CliTool, refresh: bool) -> CliAgentInfo {
    if !refresh {
        if let Some(info) = state.cli.read().await.get(tool) {
            return info;
        }
    }
    let settings = state.settings.get();
    let info = cli::detect(tool, settings.cli_path(tool), settings.is_enabled(tool)).await;
    state.cli.write().await.set(tool, info.clone());
    info
}

/// Lance la session de test et enregistre son résultat.
pub async fn test_cli(state: &AppState, tool: CliTool) -> AppResult<CliAgentInfo> {
    let mut info = cli_status(state, tool, false).await;
    let path = info
        .binary_path
        .clone()
        .ok_or_else(|| AppError::Validation("Binaire introuvable : impossible de le tester.".into()))?;
    if !tool.supports_test() {
        return Ok(info);
    }

    let (test, auth_method) = cli::run_test(std::path::Path::new(&path)).await;
    if info.status != ProviderStatus::Disconnected {
        info.status = if test.ok { ProviderStatus::Connected } else { ProviderStatus::Error };
    }
    info.auth_method = auth_method.or(info.auth_method);
    info.test = Some(test);
    state.cli.write().await.set(tool, info.clone());
    Ok(info)
}
