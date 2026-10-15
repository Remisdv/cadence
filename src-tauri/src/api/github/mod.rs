//! Client GitHub typé : pagination, ETag, quota.
//!
//! Prévu (0.1.0) : `octocrab` pour l'API REST. Chaque requête porte l'ETag stocké en cache ;
//! un `304 Not Modified` ne consomme pas de quota et ne déclenche aucun événement.
//! Le jeton est lu dans le trousseau au moment de construire la requête et circule en
//! `SecretString` — jamais exposé au webview.

pub mod issues;
pub mod pulls;
pub mod rate_limit;
pub mod repos;
pub mod writes;

use std::time::Duration;

use reqwest::{header, Client, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::error::{AppError, AppResult};

pub const API_URL: &str = "https://api.github.com";
pub const API_VERSION: &str = "2022-11-28";

/// Client HTTP partagé (connexions réutilisées), avec l'User-Agent exigé par GitHub.
pub fn http_client() -> Client {
    Client::builder()
        .user_agent(concat!("Cadence/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
        .expect("client HTTP")
}

/// Compte associé à un jeton.
#[derive(Debug, Clone)]
pub struct GithubUser {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub scopes: Vec<String>,
}

/// `GET /user` : valide le jeton et lit ses portées (`x-oauth-scopes`).
pub async fn fetch_user(http: &Client, token: &SecretString) -> AppResult<GithubUser> {
    #[derive(Deserialize)]
    struct Raw {
        login: String,
        name: Option<String>,
        avatar_url: Option<String>,
    }

    let resp = http
        .get(format!("{API_URL}/user"))
        .bearer_auth(token.expose_secret())
        .header(header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .send()
        .await?;

    match resp.status() {
        StatusCode::UNAUTHORIZED => return Err(AppError::Unauthenticated("GitHub")),
        status if !status.is_success() => {
            return Err(AppError::GitHub(format!("GET /user a répondu {status}")));
        }
        _ => {}
    }

    let scopes = resp
        .headers()
        .get("x-oauth-scopes")
        .and_then(|v| v.to_str().ok())
        .map(parse_scopes)
        .unwrap_or_default();
    let raw: Raw = resp.json().await?;
    Ok(GithubUser { login: raw.login, name: raw.name, avatar_url: raw.avatar_url, scopes })
}

/// `"repo, read:org, workflow"` → `["repo", "read:org", "workflow"]`
pub fn parse_scopes(header: &str) -> Vec<String> {
    header.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

/// Portées requises absentes d'un jeton classique. Un jeton à permissions fines
/// n'annonce aucune portée : on ne peut pas le vérifier ainsi, on le laisse passer.
pub fn missing_scopes<'a>(scopes: &[String], required: &[&'a str]) -> Vec<&'a str> {
    if scopes.is_empty() {
        return vec![];
    }
    required
        .iter()
        .copied()
        // `repo` couvre les sous-portées `repo:*` ; `admin:org` couvre `read:org`.
        .filter(|req| {
            !scopes.iter().any(|s| s == req || (*req == "read:org" && s == "admin:org"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_oauth_scopes_header() {
        assert_eq!(parse_scopes("repo, read:org,workflow "), ["repo", "read:org", "workflow"]);
        assert!(parse_scopes("").is_empty());
    }

    #[test]
    fn reports_missing_scopes_on_classic_tokens_only() {
        let required = ["repo", "read:org", "workflow"];
        let classic = parse_scopes("repo, admin:org");
        assert_eq!(missing_scopes(&classic, &required), ["workflow"]);
        assert!(missing_scopes(&[], &required).is_empty());
    }
}
