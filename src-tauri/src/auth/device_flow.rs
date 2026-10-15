//! Device flow OAuth GitHub.
//!
//! 1. `POST /login/device/code` → `device_code`, `user_code`, `interval`
//! 2. l'utilisateur saisit le code sur github.com/login/device
//! 3. polling de `POST /login/oauth/access_token` au rythme `interval`, ralenti sur `slow_down`
//! 4. jeton → trousseau, puis `auth://changed`

use std::time::{Duration, Instant};

use reqwest::{header, Client};
use secrecy::SecretString;
use serde::Deserialize;
use tauri::{async_runtime::JoinHandle, AppHandle, Manager};

use crate::{
    domain::auth::{AuthFailure, ProviderStatus},
    error::{AppError, AppResult},
    events::{self, AuthChanged},
    state::AppState,
};

use super::{ACCESS_TOKEN_URL, DEVICE_CODE_URL};

const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Réponse d'un tour de polling.
#[derive(Debug, PartialEq, Eq)]
pub enum PollOutcome {
    Token(String),
    Pending,
    SlowDown { interval: Option<u64> },
    Expired,
    Denied,
    Failed(String),
}

pub async fn request_code(http: &Client, client_id: &str, scopes: &str) -> AppResult<DeviceCode> {
    let resp = http
        .post(DEVICE_CODE_URL)
        .header(header::ACCEPT, "application/json")
        .form(&[("client_id", client_id), ("scope", scopes)])
        .send()
        .await?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();

    if let Some(error) = body.get("error").and_then(|v| v.as_str()) {
        return Err(AppError::GitHub(describe_code_error(error)));
    }
    if !status.is_success() {
        return Err(AppError::GitHub(describe_code_error(status.as_str())));
    }
    serde_json::from_value(body)
        .map_err(|err| AppError::GitHub(format!("réponse du device flow illisible : {err}")))
}

fn describe_code_error(error: &str) -> String {
    match error {
        "device_flow_disabled" => "Le device flow n'est pas activé sur l'application OAuth GitHub \
             (Settings › Developer settings › OAuth Apps › Enable Device Flow)."
            .into(),
        "Not Found" | "404" | "incorrect_client_credentials" => {
            "Client ID GitHub inconnu : vérifier CADENCE_GITHUB_CLIENT_ID.".into()
        }
        other => format!("GitHub a refusé la demande de code ({other})."),
    }
}

pub async fn poll_token(
    http: &Client,
    client_id: &str,
    device_code: &str,
) -> Result<PollOutcome, reqwest::Error> {
    let body: serde_json::Value = http
        .post(ACCESS_TOKEN_URL)
        .header(header::ACCEPT, "application/json")
        .form(&[("client_id", client_id), ("device_code", device_code), ("grant_type", GRANT_TYPE)])
        .send()
        .await?
        .json()
        .await?;
    Ok(parse_poll(&body))
}

pub fn parse_poll(body: &serde_json::Value) -> PollOutcome {
    if let Some(token) = body.get("access_token").and_then(|v| v.as_str()) {
        return PollOutcome::Token(token.to_string());
    }
    let error = body.get("error").and_then(|v| v.as_str());
    match error {
        Some("authorization_pending") => PollOutcome::Pending,
        Some("slow_down") => {
            PollOutcome::SlowDown { interval: body.get("interval").and_then(|v| v.as_u64()) }
        }
        Some("expired_token") => PollOutcome::Expired,
        Some("access_denied") => PollOutcome::Denied,
        Some(other) => PollOutcome::Failed(
            body.get("error_description").and_then(|v| v.as_str()).unwrap_or(other).to_string(),
        ),
        None => PollOutcome::Failed("réponse inattendue de GitHub".into()),
    }
}

/// Délai après un `slow_down` : celui que donne GitHub, sinon le double de l'actuel.
pub fn next_interval(current: Duration, given: Option<u64>) -> Duration {
    given.map(Duration::from_secs).unwrap_or(current * 2).max(current)
}

/// Lance le polling en tâche de fond ; l'issue arrive par `auth://changed`.
pub fn spawn_polling(app: AppHandle, client_id: String, code: DeviceCode) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let deadline = Instant::now() + Duration::from_secs(code.expires_in);
        let mut interval = Duration::from_secs(code.interval.max(1));
        let emit = |event: AuthChanged| events::emit(&app, events::AUTH_CHANGED, event);

        loop {
            tokio::time::sleep(interval).await;
            if Instant::now() >= deadline {
                emit(AuthChanged::github_failed(AuthFailure::Expired, None));
                return;
            }

            let outcome = match poll_token(&state.http, &client_id, &code.device_code).await {
                Ok(outcome) => outcome,
                // Réseau coupé : on réessaie au prochain tour, le code reste valable.
                Err(err) => {
                    tracing::warn!(%err, "polling du device flow");
                    continue;
                }
            };

            match outcome {
                PollOutcome::Pending => {}
                PollOutcome::SlowDown { interval: given } => interval = next_interval(interval, given),
                PollOutcome::Token(token) => {
                    match super::complete_sign_in(&state, SecretString::from(token)).await {
                        Ok(_) => emit(AuthChanged::github(ProviderStatus::Connected)),
                        Err(err) => emit(AuthChanged::github_failed(AuthFailure::Error, Some(err.to_string()))),
                    }
                    return;
                }
                PollOutcome::Expired => {
                    emit(AuthChanged::github_failed(AuthFailure::Expired, None));
                    return;
                }
                PollOutcome::Denied => {
                    emit(AuthChanged::github_failed(AuthFailure::Denied, None));
                    return;
                }
                PollOutcome::Failed(message) => {
                    emit(AuthChanged::github_failed(AuthFailure::Error, Some(message)));
                    return;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_every_polling_answer() {
        assert_eq!(
            parse_poll(&json!({"access_token": "gho_x", "scope": "repo"})),
            PollOutcome::Token("gho_x".into())
        );
        assert_eq!(parse_poll(&json!({"error": "authorization_pending"})), PollOutcome::Pending);
        assert_eq!(
            parse_poll(&json!({"error": "slow_down", "interval": 10})),
            PollOutcome::SlowDown { interval: Some(10) }
        );
        assert_eq!(parse_poll(&json!({"error": "expired_token"})), PollOutcome::Expired);
        assert_eq!(parse_poll(&json!({"error": "access_denied"})), PollOutcome::Denied);
        assert_eq!(
            parse_poll(&json!({"error": "incorrect_device_code", "error_description": "bad code"})),
            PollOutcome::Failed("bad code".into())
        );
    }

    #[test]
    fn slows_down_as_asked_or_doubles() {
        let five = Duration::from_secs(5);
        assert_eq!(next_interval(five, Some(10)), Duration::from_secs(10));
        assert_eq!(next_interval(five, None), Duration::from_secs(10));
        assert_eq!(next_interval(five, Some(1)), five, "ne jamais accélérer");
    }

    #[test]
    fn explains_misconfigured_oauth_app() {
        assert!(describe_code_error("device_flow_disabled").contains("Enable Device Flow"));
        assert!(describe_code_error("Not Found").contains("CADENCE_GITHUB_CLIENT_ID"));
    }
}
