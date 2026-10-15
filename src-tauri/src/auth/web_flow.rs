//! Connexion GitHub par le navigateur (flux web OAuth, redirection loopback).
//!
//! 1. Cadence écoute sur `127.0.0.1:<port libre>` et ouvre github.com/login/oauth/authorize
//!    avec un défi PKCE et un `state` aléatoire ;
//! 2. l'utilisateur autorise dans son navigateur, GitHub redirige vers `http://127.0.0.1:<port>/callback` ;
//! 3. Cadence vérifie le `state`, échange le code (client secret + `code_verifier`) contre un jeton,
//!    le range dans le trousseau, et `auth://changed` prévient l'interface.
//!
//! GitHub exige le client secret pour cet échange. Dans une app native il ne peut pas rester
//! confidentiel (RFC 8252) : PKCE et le `state` empêchent qu'un code intercepté soit utilisable.

use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{header, Client};
use secrecy::SecretString;
use sha2::{Digest, Sha256};
use tauri::{async_runtime::JoinHandle, AppHandle, Manager};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use url::Url;

use crate::{
    domain::auth::{AuthFailure, ProviderStatus},
    error::{AppError, AppResult},
    events::{self, AuthChanged},
    state::AppState,
};

use super::ACCESS_TOKEN_URL;

pub const AUTHORIZE_URL: &str = "https://github.com/login/oauth/authorize";
/// Au-delà, on abandonne l'attente et on libère le port.
const WAIT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Client secret de l'OAuth App : lu à l'exécution, sinon figé à la compilation.
pub fn client_secret() -> AppResult<String> {
    std::env::var("CADENCE_GITHUB_CLIENT_SECRET")
        .ok()
        .or_else(|| option_env!("CADENCE_GITHUB_CLIENT_SECRET").map(str::to_string))
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            AppError::Config(
                "Client secret GitHub absent : définir CADENCE_GITHUB_CLIENT_SECRET pour la connexion \
                 par navigateur. Le code d'appairage et le jeton personnel restent disponibles."
                    .into(),
            )
        })
}

/// Chaîne aléatoire URL-safe (43 caractères pour 32 octets).
pub fn random_token() -> AppResult<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| AppError::Internal(format!("aléa indisponible : {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// Défi PKCE S256 : base64url(sha256(verifier)), sans remplissage.
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn authorize_url(client_id: &str, redirect_uri: &str, scopes: &str, state: &str, challenge: &str) -> String {
    Url::parse_with_params(
        AUTHORIZE_URL,
        [
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("scope", scopes),
            ("state", state),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("allow_signup", "true"),
        ],
    )
    .expect("URL d'autorisation valide")
    .into()
}

/// Ce que GitHub renvoie sur `/callback`.
#[derive(Debug, PartialEq, Eq)]
pub enum Callback {
    Code { code: String, state: String },
    Denied,
    Error(String),
    /// Autre requête (favicon…) : on l'ignore et on continue d'attendre.
    Other,
}

/// Lit la ligne de requête HTTP (`GET /callback?code=…&state=… HTTP/1.1`).
pub fn parse_callback(request: &str) -> Callback {
    let Some(target) = request.lines().next().and_then(|l| l.split_whitespace().nth(1)) else {
        return Callback::Other;
    };
    let Ok(url) = Url::parse(&format!("http://127.0.0.1{target}")) else {
        return Callback::Other;
    };
    if url.path() != "/callback" {
        return Callback::Other;
    }
    let param = |key: &str| url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.into_owned());
    match (param("code"), param("state"), param("error")) {
        (_, _, Some(e)) if e == "access_denied" => Callback::Denied,
        (_, _, Some(e)) => Callback::Error(param("error_description").unwrap_or(e)),
        (Some(code), Some(state), None) => Callback::Code { code, state },
        _ => Callback::Error("réponse de GitHub incomplète".into()),
    }
}

pub struct WebFlow {
    pub authorize_url: String,
    listener: TcpListener,
    redirect_uri: String,
    state: String,
    verifier: String,
    client_id: String,
    client_secret: String,
}

/// Prépare le flux : port libre, PKCE, `state`. N'ouvre rien encore.
pub async fn prepare(client_id: String, scopes: &str) -> AppResult<WebFlow> {
    let client_secret = client_secret()?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let state = random_token()?;
    let verifier = random_token()?;
    let authorize_url = authorize_url(&client_id, &redirect_uri, scopes, &state, &pkce_challenge(&verifier));
    Ok(WebFlow { authorize_url, listener, redirect_uri, state, verifier, client_id, client_secret })
}

/// Attend le retour du navigateur en tâche de fond ; l'issue arrive par `auth://changed`.
pub fn spawn_wait(app: AppHandle, flow: WebFlow) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let emit = |event: AuthChanged| events::emit(&app, events::AUTH_CHANGED, event);
        let outcome = tokio::time::timeout(WAIT_TIMEOUT, wait_for_callback(&flow)).await;
        let state = app.state::<AppState>();

        let result = match outcome {
            Err(_) => Err((AuthFailure::Expired, None)),
            Ok(Err(failure)) => Err(failure),
            Ok(Ok((code, mut stream))) => {
                let exchanged = exchange_code(&state.http, &flow, &code).await;
                let signed_in = match exchanged {
                    Ok(token) => super::complete_sign_in(&state, token).await,
                    Err(err) => Err(err),
                };
                let page = match &signed_in {
                    Ok(_) => result_page(true, "Connexion réussie", "Vous pouvez fermer cet onglet et revenir dans Cadence."),
                    Err(err) => result_page(false, "Connexion impossible", &err.to_string()),
                };
                let _ = stream.write_all(page.as_bytes()).await;
                signed_in.map(|_| ()).map_err(|err| (AuthFailure::Error, Some(err.to_string())))
            }
        };

        match result {
            Ok(()) => {
                emit(AuthChanged::github(ProviderStatus::Connected));
                super::focus_main_window(&app);
            }
            Err((reason, message)) => emit(AuthChanged::github_failed(reason, message)),
        }
    })
}

/// Échec du flux : raison + message éventuel, tels que portés par `auth://changed`.
type Failure = (AuthFailure, Option<String>);

/// Accepte des connexions jusqu'au vrai retour de GitHub ; renvoie le code et la connexion
/// ouverte (pour répondre à l'onglet une fois le jeton obtenu).
async fn wait_for_callback(flow: &WebFlow) -> Result<(String, TcpStream), Failure> {
    loop {
        let (mut stream, _) =
            flow.listener.accept().await.map_err(|e| (AuthFailure::Error, Some(e.to_string())))?;
        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);
        match parse_callback(&request) {
            Callback::Other => {
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
            }
            Callback::Code { code, state } if state == flow.state => return Ok((code, stream)),
            Callback::Code { .. } => {
                // `state` inattendu : requête forgée, on l'écarte sans rien échanger.
                let _ = stream.write_all(result_page(false, "Requête refusée", "Paramètre state invalide.").as_bytes()).await;
            }
            Callback::Denied => {
                let _ = stream
                    .write_all(result_page(false, "Autorisation refusée", "Vous pouvez revenir dans Cadence et recommencer.").as_bytes())
                    .await;
                return Err((AuthFailure::Denied, None));
            }
            Callback::Error(message) => {
                let _ = stream.write_all(result_page(false, "Connexion impossible", &message).as_bytes()).await;
                return Err((AuthFailure::Error, Some(message)));
            }
        }
    }
}

async fn exchange_code(http: &Client, flow: &WebFlow, code: &str) -> AppResult<SecretString> {
    let body: serde_json::Value = http
        .post(ACCESS_TOKEN_URL)
        .header(header::ACCEPT, "application/json")
        .form(&[
            ("client_id", flow.client_id.as_str()),
            ("client_secret", flow.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", flow.redirect_uri.as_str()),
            ("code_verifier", flow.verifier.as_str()),
        ])
        .send()
        .await?
        .json()
        .await?;
    if let Some(token) = body.get("access_token").and_then(|v| v.as_str()) {
        return Ok(SecretString::from(token.to_string()));
    }
    let error = body.get("error").and_then(|v| v.as_str()).unwrap_or("réponse inattendue");
    Err(AppError::GitHub(match error {
        "incorrect_client_credentials" => "Client ID ou client secret GitHub incorrect.".into(),
        "redirect_uri_mismatch" => {
            "URL de callback refusée : déclarer http://127.0.0.1/callback dans l'OAuth App.".into()
        }
        other => body.get("error_description").and_then(|v| v.as_str()).unwrap_or(other).to_string(),
    }))
}

/// Page affichée dans l'onglet du navigateur à la fin du flux.
fn result_page(ok: bool, title: &str, message: &str) -> String {
    let escape = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let color = if ok { "#3FB562" } else { "#E0655A" };
    let html = format!(
        r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Cadence</title></head>
<body style="margin:0;height:100vh;display:flex;align-items:center;justify-content:center;background:#0B0D10;color:#E6E9ED;font-family:system-ui,sans-serif">
<div style="text-align:center;max-width:420px;padding:24px">
<div style="width:44px;height:44px;margin:0 auto 18px;border-radius:12px;background:#D97757"></div>
<h1 style="font-size:20px;margin:0;color:{color}">{title}</h1>
<p style="font-size:14px;line-height:1.6;color:#9AA4B0;margin-top:10px">{message}</p></div></body></html>"#,
        title = escape(title),
        message = escape(message),
    );
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_the_rfc7636_pkce_challenge() {
        // RFC 7636, annexe B.
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn random_tokens_are_url_safe_and_distinct() {
        let (a, b) = (random_token().unwrap(), random_token().unwrap());
        assert_eq!(a.len(), 43);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn builds_the_authorize_url() {
        let url = authorize_url("Ov23li", "http://127.0.0.1:5123/callback", "repo read:org", "st", "ch");
        assert!(url.starts_with(AUTHORIZE_URL));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A5123%2Fcallback"));
        assert!(url.contains("scope=repo+read%3Aorg"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn parses_browser_callbacks() {
        assert_eq!(
            parse_callback("GET /callback?code=abc&state=xyz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"),
            Callback::Code { code: "abc".into(), state: "xyz".into() }
        );
        assert_eq!(
            parse_callback("GET /callback?error=access_denied&state=xyz HTTP/1.1\r\n"),
            Callback::Denied
        );
        assert_eq!(parse_callback("GET /favicon.ico HTTP/1.1\r\n"), Callback::Other);
        assert!(matches!(parse_callback("GET /callback?state=xyz HTTP/1.1\r\n"), Callback::Error(_)));
    }
}
