//! Dépôts et milestones : ce qu'il faut pour choisir et ouvrir un projet.

use reqwest::{header, Client, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::json;

use super::{API_URL, API_VERSION};
use crate::{
    domain::{
        project::Repo,
        version::{self, Version, VersionKind, VersionState},
    },
    error::{AppError, AppResult},
};

/// Une seule requête GraphQL : 100 dépôts récents avec leur nombre de milestones
/// (en REST, il faudrait un appel par dépôt).
const VIEWER_REPOS: &str = r#"
query {
  viewer {
    repositories(first: 100, orderBy: {field: PUSHED_AT, direction: DESC},
                 affiliations: [OWNER, COLLABORATOR, ORGANIZATION_MEMBER]) {
      nodes {
        nameWithOwner name description isPrivate pushedAt
        owner { login }
        defaultBranchRef { name }
        issues(states: OPEN) { totalCount }
        milestones { totalCount }
      }
    }
  }
}"#;

pub async fn viewer_repos(http: &Client, token: &SecretString) -> AppResult<Vec<Repo>> {
    let resp = http
        .post(format!("{API_URL}/graphql"))
        .bearer_auth(token.expose_secret())
        .json(&json!({ "query": VIEWER_REPOS }))
        .send()
        .await?;
    check_status(resp.status())?;
    let body: serde_json::Value = resp.json().await?;
    if let Some(errors) = body.get("errors") {
        return Err(AppError::GitHub(format!("GraphQL : {errors}")));
    }
    Ok(parse_viewer_repos(&body))
}

pub fn parse_viewer_repos(body: &serde_json::Value) -> Vec<Repo> {
    let nodes = body["data"]["viewer"]["repositories"]["nodes"].as_array();
    nodes
        .into_iter()
        .flatten()
        .filter_map(|n| {
            let count = |v: &serde_json::Value| v["totalCount"].as_u64().unwrap_or(0) as u32;
            Some(Repo {
                full_name: n["nameWithOwner"].as_str()?.to_string(),
                owner: n["owner"]["login"].as_str()?.to_string(),
                name: n["name"].as_str()?.to_string(),
                description: n["description"].as_str().map(str::to_string),
                private: n["isPrivate"].as_bool().unwrap_or(false),
                default_branch: n["defaultBranchRef"]["name"].as_str().unwrap_or("main").to_string(),
                open_issues: count(&n["issues"]),
                milestones: count(&n["milestones"]),
                pushed_at: n["pushedAt"].as_str().map(str::to_string),
            })
        })
        .collect()
}

/// Filtre insensible à la casse sur `owner/name` et la description.
pub fn filter_repos(repos: &[Repo], query: &str) -> Vec<Repo> {
    let q = query.trim().to_lowercase();
    repos
        .iter()
        .filter(|r| {
            q.is_empty()
                || r.full_name.to_lowercase().contains(&q)
                || r.description.as_deref().is_some_and(|d| d.to_lowercase().contains(&q))
        })
        .cloned()
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Milestone {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub description: Option<String>,
    pub due_on: Option<String>,
    pub open_issues: u32,
    pub closed_issues: u32,
}

pub async fn milestones(http: &Client, token: &SecretString, repo: &str) -> AppResult<Vec<Milestone>> {
    let resp = http
        .get(format!("{API_URL}/repos/{repo}/milestones?state=all&per_page=100"))
        .bearer_auth(token.expose_secret())
        .header(header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .send()
        .await?;
    check_status(resp.status())?;
    Ok(resp.json().await?)
}

/// Vrai si le dépôt contient déjà `.cadence/config.toml`.
pub async fn has_cadence_config(http: &Client, token: &SecretString, repo: &str) -> AppResult<bool> {
    let resp = http
        .head(format!("{API_URL}/repos/{repo}/contents/.cadence/config.toml"))
        .bearer_auth(token.expose_secret())
        .header("X-GitHub-Api-Version", API_VERSION)
        .send()
        .await?;
    match resp.status() {
        StatusCode::NOT_FOUND => Ok(false),
        status => check_status(status).map(|_| true),
    }
}

fn check_status(status: StatusCode) -> AppResult<()> {
    match status {
        StatusCode::UNAUTHORIZED => Err(AppError::Unauthenticated("GitHub")),
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => Err(AppError::RateLimited),
        StatusCode::NOT_FOUND => Err(AppError::GitHub("dépôt introuvable ou inaccessible".into())),
        s if !s.is_success() => Err(AppError::GitHub(format!("GitHub a répondu {s}"))),
        _ => Ok(()),
    }
}

/// Milestones → versions : seuls les titres semver comptent, les autres sont signalés.
/// Retourne (versions triées, titres ignorés).
pub fn to_versions(milestones: &[Milestone]) -> (Vec<Version>, Vec<String>) {
    let mut ignored = Vec::new();
    let mut versions: Vec<Version> = milestones
        .iter()
        .filter_map(|m| {
            let Some(v) = version::parse(&m.title) else {
                ignored.push(m.title.clone());
                return None;
            };
            Some(Version {
                title: v.to_string(),
                kind: if m.due_on.is_some() { VersionKind::Sprint } else { VersionKind::Palier },
                state: if m.state == "closed" { VersionState::Closed } else { VersionState::Open },
                milestone_number: m.number,
                name: None,
                start_on: None,
                due_on: m.due_on.clone(),
                open_issues: m.open_issues,
                closed_issues: m.closed_issues,
                description: m.description.clone().filter(|d| !d.trim().is_empty()),
            })
        })
        .collect();
    versions.sort_by(|a, b| version::compare(&a.title, &b.title));
    (versions, ignored)
}

/// Sprint en cours : le plus petit sprint (daté) encore ouvert ; à défaut, la plus petite
/// version ouverte. Les versions sont supposées triées.
pub fn current_version(versions: &[Version]) -> Option<String> {
    let open = || versions.iter().filter(|v| v.state == VersionState::Open);
    open()
        .find(|v| v.kind == VersionKind::Sprint)
        .or_else(|| open().next())
        .map(|v| v.title.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn milestone(number: u32, title: &str, state: &str, due: Option<&str>) -> Milestone {
        Milestone {
            number,
            title: title.into(),
            state: state.into(),
            description: None,
            due_on: due.map(str::to_string),
            open_issues: 0,
            closed_issues: 0,
        }
    }

    #[test]
    fn imports_semver_milestones_and_picks_the_current_sprint() {
        let ms = [
            milestone(6, "0.2.0", "open", None),
            milestone(4, "v0.1.2", "open", Some("2026-09-23T07:00:00Z")),
            milestone(1, "0.1.0", "closed", Some("2026-08-26T07:00:00Z")),
            milestone(5, "0.1.3", "open", Some("2026-10-07T07:00:00Z")),
            milestone(9, "Backlog", "open", None),
        ];
        let (versions, ignored) = to_versions(&ms);
        let titles: Vec<_> = versions.iter().map(|v| v.title.as_str()).collect();
        assert_eq!(titles, ["0.1.0", "0.1.2", "0.1.3", "0.2.0"]);
        assert_eq!(ignored, ["Backlog"]);
        assert_eq!(versions[3].kind, VersionKind::Palier);
        assert_eq!(current_version(&versions).as_deref(), Some("0.1.2"));
    }

    #[test]
    fn falls_back_to_an_open_milestone_without_date() {
        let (versions, _) = to_versions(&[milestone(1, "0.2.0", "open", None)]);
        assert_eq!(current_version(&versions).as_deref(), Some("0.2.0"));
        assert_eq!(current_version(&[]), None);
    }

    #[test]
    fn parses_graphql_repositories_and_filters() {
        let body = json!({"data": {"viewer": {"repositories": {"nodes": [
            {"nameWithOwner": "cadence-app/desktop", "name": "desktop", "description": "Client Tauri",
             "isPrivate": false, "pushedAt": "2026-09-17T08:00:00Z", "owner": {"login": "cadence-app"},
             "defaultBranchRef": {"name": "main"}, "issues": {"totalCount": 18}, "milestones": {"totalCount": 3}},
            {"nameWithOwner": "me/dotfiles", "name": "dotfiles", "description": null, "isPrivate": true,
             "pushedAt": null, "owner": {"login": "me"}, "defaultBranchRef": null,
             "issues": {"totalCount": 0}, "milestones": {"totalCount": 0}}
        ]}}}});
        let repos = parse_viewer_repos(&body);
        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].milestones, 3);
        assert_eq!(repos[1].default_branch, "main");
        assert_eq!(filter_repos(&repos, "TAURI").len(), 1);
        assert_eq!(filter_repos(&repos, "").len(), 2);
    }
}
