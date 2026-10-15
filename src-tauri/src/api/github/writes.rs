//! Écritures sur GitHub : issues, milestones, tags, brouillons de release.
//!
//! Chaque écriture part d'abord sur GitHub ; l'appelant range ensuite la réponse dans le
//! cache. Les refus sont traduits en messages lisibles (droits, issues désactivées, 422).

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{header, Client, RequestBuilder, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{issues::RawIssue, repos::Milestone, API_URL, API_VERSION};
use crate::error::{AppError, AppResult};

fn authed(req: RequestBuilder, token: &SecretString) -> RequestBuilder {
    req.bearer_auth(token.expose_secret())
        .header(header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
}

/// Envoie une écriture et renvoie le corps JSON, ou une erreur explicite.
async fn send(req: RequestBuilder, what: &str) -> AppResult<Value> {
    let resp = req.send().await?;
    let status = resp.status();
    let exhausted = resp
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "0");
    let body: Value = resp.json().await.unwrap_or_default();
    match status {
        s if s.is_success() => Ok(body),
        StatusCode::UNAUTHORIZED => Err(AppError::Unauthenticated("GitHub")),
        StatusCode::TOO_MANY_REQUESTS => Err(AppError::RateLimited),
        StatusCode::FORBIDDEN if exhausted => Err(AppError::RateLimited),
        StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => Err(AppError::Validation(format!(
            "{what} refusé : ce compte n'a pas les droits d'écriture sur le dépôt."
        ))),
        StatusCode::GONE => Err(AppError::Validation("Les issues sont désactivées sur ce dépôt.".into())),
        StatusCode::UNPROCESSABLE_ENTITY => {
            Err(AppError::Validation(format!("{what} refusé par GitHub : {}", describe_422(&body))))
        }
        other => Err(AppError::GitHub(format!("{what} : GitHub a répondu {other}"))),
    }
}

/// Premier message utile d'un 422 (« already_exists », « invalid »…).
pub fn describe_422(body: &Value) -> String {
    body["errors"]
        .as_array()
        .and_then(|errs| {
            errs.iter().find_map(|e| {
                e["message"].as_str().map(str::to_string).or_else(|| {
                    let field = e["field"].as_str()?;
                    let code = e["code"].as_str()?;
                    Some(format!("{field} {code}"))
                })
            })
        })
        .or_else(|| body["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| "requête invalide".into())
}

fn parse<T: for<'de> Deserialize<'de>>(body: Value, what: &str) -> AppResult<(T, Value)> {
    let item = T::deserialize(&body).map_err(|e| AppError::GitHub(format!("{what} : réponse illisible ({e})")))?;
    Ok((item, body))
}

/* ---------- Issues ---------- */

pub async fn create_issue(
    http: &Client,
    token: &SecretString,
    repo: &str,
    payload: &Value,
) -> AppResult<(RawIssue, Value)> {
    let body = send(authed(http.post(format!("{API_URL}/repos/{repo}/issues")), token).json(payload), "Création de l'issue").await?;
    parse(body, "issue créée")
}

/// `PATCH` d'une issue (milestone, état…).
pub async fn update_issue(
    http: &Client,
    token: &SecretString,
    repo: &str,
    number: u32,
    patch: &Value,
) -> AppResult<(RawIssue, Value)> {
    let url = format!("{API_URL}/repos/{repo}/issues/{number}");
    let body = send(authed(http.patch(url), token).json(patch), &format!("Mise à jour de #{number}")).await?;
    parse(body, "issue modifiée")
}

/// Commentaire sur une issue (trace signée d'un agent).
pub async fn create_comment(http: &Client, token: &SecretString, repo: &str, number: u32, body: &str) -> AppResult<()> {
    let url = format!("{API_URL}/repos/{repo}/issues/{number}/comments");
    send(authed(http.post(url), token).json(&json!({ "body": body })), &format!("Commentaire sur #{number}"))
        .await
        .map(drop)
}

/* ---------- Milestones ---------- */

pub async fn create_milestone(
    http: &Client,
    token: &SecretString,
    repo: &str,
    title: &str,
    due_on: Option<&str>,
) -> AppResult<(Milestone, Value)> {
    let payload = json!({ "title": title, "state": "open", "due_on": due_on });
    let body = send(
        authed(http.post(format!("{API_URL}/repos/{repo}/milestones")), token).json(&payload),
        &format!("Création du milestone {title}"),
    )
    .await?;
    parse(body, "milestone créé")
}

pub async fn update_milestone(
    http: &Client,
    token: &SecretString,
    repo: &str,
    number: u32,
    patch: &Value,
) -> AppResult<(Milestone, Value)> {
    let url = format!("{API_URL}/repos/{repo}/milestones/{number}");
    let body = send(authed(http.patch(url), token).json(patch), "Mise à jour du milestone").await?;
    parse(body, "milestone modifié")
}

/* ---------- Tags et releases ---------- */

/// SHA du dernier commit d'une branche.
pub async fn branch_sha(http: &Client, token: &SecretString, repo: &str, branch: &str) -> AppResult<String> {
    let url = format!("{API_URL}/repos/{repo}/git/ref/heads/{branch}");
    let body = send(authed(http.get(url), token), &format!("Lecture de la branche {branch}")).await?;
    body["object"]["sha"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::GitHub(format!("branche {branch} illisible")))
}

pub async fn tag_exists(http: &Client, token: &SecretString, repo: &str, tag: &str) -> AppResult<bool> {
    let resp = authed(http.get(format!("{API_URL}/repos/{repo}/git/ref/tags/{tag}")), token).send().await?;
    match resp.status() {
        StatusCode::NOT_FOUND => Ok(false),
        s if s.is_success() => Ok(true),
        StatusCode::UNAUTHORIZED => Err(AppError::Unauthenticated("GitHub")),
        other => Err(AppError::GitHub(format!("lecture du tag {tag} : GitHub a répondu {other}"))),
    }
}

/// Tag léger `refs/tags/<tag>` sur `sha`.
pub async fn create_tag(http: &Client, token: &SecretString, repo: &str, tag: &str, sha: &str) -> AppResult<()> {
    let payload = json!({ "ref": format!("refs/tags/{tag}"), "sha": sha });
    send(
        authed(http.post(format!("{API_URL}/repos/{repo}/git/refs")), token).json(&payload),
        &format!("Pose du tag {tag}"),
    )
    .await
    .map(drop)
}

/// Brouillon de release sur un tag existant, notes générées par GitHub. Un brouillon ne
/// publie rien : la publication reste un geste explicite. Renvoie l'URL du brouillon.
pub async fn create_draft_release(
    http: &Client,
    token: &SecretString,
    repo: &str,
    tag: &str,
) -> AppResult<String> {
    let payload = json!({ "tag_name": tag, "name": tag, "draft": true, "generate_release_notes": true });
    let body = send(
        authed(http.post(format!("{API_URL}/repos/{repo}/releases")), token).json(&payload),
        "Création des notes de version",
    )
    .await?;
    Ok(body["html_url"].as_str().unwrap_or_default().to_string())
}

/* ---------- Fichiers du dépôt ---------- */

/// Fichier texte du dépôt sur sa branche par défaut : (contenu, SHA du blob), `None` s'il
/// n'existe pas.
pub async fn get_file(
    http: &Client,
    token: &SecretString,
    repo: &str,
    path: &str,
) -> AppResult<Option<(String, String)>> {
    let resp = authed(http.get(format!("{API_URL}/repos/{repo}/contents/{path}")), token).send().await?;
    match resp.status() {
        StatusCode::NOT_FOUND => return Ok(None),
        StatusCode::UNAUTHORIZED => return Err(AppError::Unauthenticated("GitHub")),
        s if !s.is_success() => return Err(AppError::GitHub(format!("lecture de {path} : GitHub a répondu {s}"))),
        _ => {}
    }
    let body: Value = resp.json().await?;
    let sha = body["sha"].as_str().unwrap_or_default().to_string();
    let encoded: String = body["content"].as_str().unwrap_or_default().split_whitespace().collect();
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|e| AppError::GitHub(format!("{path} : contenu illisible ({e})")))?;
    let text = String::from_utf8(bytes).map_err(|_| AppError::Validation(format!("{path} n'est pas du texte UTF-8.")))?;
    Ok(Some((text, sha)))
}

/// Crée ou remplace un fichier par un commit sur la branche par défaut. `sha` = blob
/// remplacé (obligatoire pour un fichier existant : GitHub refuse si le fichier a changé
/// entre-temps). Renvoie le nouveau SHA du blob.
pub async fn put_file(
    http: &Client,
    token: &SecretString,
    repo: &str,
    path: &str,
    text: &str,
    sha: Option<&str>,
    message: &str,
) -> AppResult<String> {
    let payload = json!({ "message": message, "content": STANDARD.encode(text), "sha": sha });
    let req = authed(http.put(format!("{API_URL}/repos/{repo}/contents/{path}")), token).json(&payload);
    let resp = req.send().await?;
    if resp.status() == StatusCode::CONFLICT {
        return Err(AppError::Validation(format!(
            "{path} a changé sur GitHub depuis son ouverture : rechargez les réglages."
        )));
    }
    // Remet la réponse dans le circuit commun d'erreurs.
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or_default();
    if !status.is_success() {
        return Err(match status {
            StatusCode::UNAUTHORIZED => AppError::Unauthenticated("GitHub"),
            StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => AppError::Validation(format!(
                "Écriture de {path} refusée : ce compte n'a pas les droits d'écriture sur le dépôt."
            )),
            StatusCode::UNPROCESSABLE_ENTITY => {
                AppError::Validation(format!("Écriture de {path} refusée par GitHub : {}", describe_422(&body)))
            }
            other => AppError::GitHub(format!("écriture de {path} : GitHub a répondu {other}")),
        });
    }
    Ok(body["content"]["sha"].as_str().unwrap_or_default().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_validation_failures() {
        let field = json!({"message": "Validation Failed",
            "errors": [{"resource": "Milestone", "field": "title", "code": "already_exists"}]});
        assert_eq!(describe_422(&field), "title already_exists");
        let message = json!({"errors": [{"message": "Reference already exists"}]});
        assert_eq!(describe_422(&message), "Reference already exists");
        assert_eq!(describe_422(&json!({})), "requête invalide");
    }
}
