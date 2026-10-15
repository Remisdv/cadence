//! Dépôts et milestones : ce qu'il faut pour choisir et ouvrir un projet.

use reqwest::{header, Client, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::json;

use super::{API_URL, API_VERSION};
use crate::{
    domain::{
        project::{OwnerKind, Repo, RepoCreateInput, RepoOwner},
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

/// Organisations de l'utilisateur (portée `read:org` ; vide sans elle).
pub async fn user_orgs(http: &Client, token: &SecretString) -> AppResult<Vec<RepoOwner>> {
    #[derive(Deserialize)]
    struct Org {
        login: String,
        avatar_url: Option<String>,
    }
    let resp = http
        .get(format!("{API_URL}/user/orgs?per_page=100"))
        .bearer_auth(token.expose_secret())
        .header(header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .send()
        .await?;
    // Sans `read:org`, GitHub répond 403 : on propose alors seulement le compte personnel.
    if resp.status() == StatusCode::FORBIDDEN {
        return Ok(vec![]);
    }
    check_status(resp.status())?;
    let orgs: Vec<Org> = resp.json().await?;
    Ok(orgs
        .into_iter()
        .map(|o| RepoOwner { login: o.login, kind: OwnerKind::Organization, avatar_url: o.avatar_url })
        .collect())
}

/// `POST /user/repos` ou `POST /orgs/{org}/repos`.
pub async fn create_repo(
    http: &Client,
    token: &SecretString,
    input: &RepoCreateInput,
    owner_kind: OwnerKind,
) -> AppResult<Repo> {
    let url = match owner_kind {
        OwnerKind::User => format!("{API_URL}/user/repos"),
        OwnerKind::Organization => format!("{API_URL}/orgs/{}/repos", input.owner),
    };
    let resp = http
        .post(url)
        .bearer_auth(token.expose_secret())
        .header(header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .json(&json!({
            "name": input.name.trim(),
            "description": input.description.as_deref().map(str::trim).filter(|d| !d.is_empty()),
            "private": input.private,
            "auto_init": input.auto_init,
        }))
        .send()
        .await?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    match status {
        s if s.is_success() => {
            parse_rest_repo(&body).ok_or_else(|| AppError::GitHub("réponse de création illisible".into()))
        }
        StatusCode::UNPROCESSABLE_ENTITY => Err(AppError::Validation(describe_create_error(&body))),
        StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => Err(AppError::Validation(format!(
            "Création refusée sur {} : droits insuffisants pour ce compte.",
            input.owner
        ))),
        other => {
            check_status(other)?;
            Err(AppError::GitHub(format!("GitHub a répondu {other}")))
        }
    }
}

/// Message lisible d'un 422 (« name already exists on this account »…).
pub fn describe_create_error(body: &serde_json::Value) -> String {
    let detail = body["errors"]
        .as_array()
        .and_then(|errs| errs.iter().find_map(|e| e["message"].as_str()))
        .or_else(|| body["message"].as_str())
        .unwrap_or("requête refusée");
    if detail.contains("already exists") {
        "Un dépôt porte déjà ce nom sur ce compte.".into()
    } else {
        format!("GitHub refuse la création : {detail}")
    }
}

/// Dépôt au format REST → `Repo`.
pub fn parse_rest_repo(v: &serde_json::Value) -> Option<Repo> {
    Some(Repo {
        full_name: v["full_name"].as_str()?.to_string(),
        owner: v["owner"]["login"].as_str()?.to_string(),
        name: v["name"].as_str()?.to_string(),
        description: v["description"].as_str().map(str::to_string),
        private: v["private"].as_bool().unwrap_or(false),
        default_branch: v["default_branch"].as_str().unwrap_or("main").to_string(),
        open_issues: v["open_issues_count"].as_u64().unwrap_or(0) as u32,
        milestones: 0,
        pushed_at: v["pushed_at"].as_str().map(str::to_string),
    })
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
    #[serde(default)]
    pub updated_at: Option<String>,
}

pub enum MilestonesFetch {
    /// `304 Not Modified` : aucun quota consommé.
    NotModified,
    Fresh { etag: Option<String>, milestones: Vec<(Milestone, serde_json::Value)> },
}

/// Milestones ouverts et fermés (100 au plus), requête conditionnelle avec `etag`.
pub async fn fetch_milestones(
    http: &Client,
    token: &SecretString,
    repo: &str,
    etag: Option<&str>,
) -> AppResult<MilestonesFetch> {
    let mut req = http
        .get(format!("{API_URL}/repos/{repo}/milestones?state=all&per_page=100"))
        .bearer_auth(token.expose_secret())
        .header(header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION);
    if let Some(etag) = etag {
        req = req.header(header::IF_NONE_MATCH, etag);
    }
    let resp = req.send().await?;
    if resp.status() == StatusCode::NOT_MODIFIED {
        return Ok(MilestonesFetch::NotModified);
    }
    check_status(resp.status())?;
    let etag = resp.headers().get(header::ETAG).and_then(|v| v.to_str().ok()).map(str::to_string);
    let page: Vec<serde_json::Value> = resp.json().await?;
    let milestones = page
        .into_iter()
        .filter_map(|raw| Milestone::deserialize(&raw).ok().map(|m| (m, raw)))
        .collect();
    Ok(MilestonesFetch::Fresh { etag, milestones })
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

pub(super) fn check_status(status: StatusCode) -> AppResult<()> {
    match status {
        StatusCode::UNAUTHORIZED => Err(AppError::Unauthenticated("GitHub")),
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => Err(AppError::RateLimited),
        StatusCode::NOT_FOUND => Err(AppError::GitHub("dépôt introuvable ou inaccessible".into())),
        s if !s.is_success() => Err(AppError::GitHub(format!("GitHub a répondu {s}"))),
        _ => Ok(()),
    }
}

/// Milestones → versions : seuls les titres semver comptent, les autres sont signalés.
/// Retourne (versions triées, titres ignorés). Un sprint commence à l'échéance du sprint
/// précédent (GitHub ne connaît que l'échéance d'un milestone).
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
                name: m.description.as_deref().and_then(milestone_name),
                start_on: None,
                due_on: m.due_on.clone(),
                open_issues: m.open_issues,
                closed_issues: m.closed_issues,
                description: m.description.clone().filter(|d| !d.trim().is_empty()),
            })
        })
        .collect();
    versions.sort_by(|a, b| version::compare(&a.title, &b.title));
    let mut previous_due: Option<String> = None;
    for v in versions.iter_mut().filter(|v| v.kind == VersionKind::Sprint) {
        v.start_on = previous_due.clone();
        previous_due = v.due_on.clone();
    }
    (versions, ignored)
}

/// Libellé d'une version : première ligne de la description du milestone, si c'est un
/// titre (`# Cockpit Tauri`) ou une ligne courte qui n'est pas un élément de liste.
pub fn milestone_name(description: &str) -> Option<String> {
    let first = description.lines().map(str::trim).find(|l| !l.is_empty())?;
    if let Some(title) = first.strip_prefix('#') {
        return Some(title.trim_start_matches('#').trim().to_string()).filter(|t| !t.is_empty());
    }
    let is_item = first.starts_with(['-', '*', '+']) || first.chars().next().is_some_and(|c| c.is_ascii_digit());
    (!is_item && first.chars().count() <= 60).then(|| first.to_string())
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
            updated_at: None,
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
    fn reads_created_repositories_and_explains_conflicts() {
        let body = json!({"full_name": "me/new", "name": "new", "owner": {"login": "me"},
            "description": null, "private": true, "default_branch": "main", "open_issues_count": 0,
            "pushed_at": "2026-10-15T10:00:00Z"});
        let repo = parse_rest_repo(&body).unwrap();
        assert_eq!((repo.full_name.as_str(), repo.private, repo.milestones), ("me/new", true, 0));
        let conflict = json!({"message": "Repository creation failed.",
            "errors": [{"message": "name already exists on this account"}]});
        assert!(describe_create_error(&conflict).contains("déjà"));
    }

    #[test]
    fn names_versions_and_chains_sprint_dates() {
        let mut first = milestone(1, "0.1.0", "closed", Some("2026-08-26T07:00:00Z"));
        first.description = Some("# Première fenêtre\n\n- Squelette".into());
        let mut palier = milestone(6, "0.2.0", "open", None);
        palier.description = Some("- Rôles\n- Multi-dépôts".into());
        let (versions, _) =
            to_versions(&[palier, milestone(2, "0.1.1", "open", Some("2026-09-09T07:00:00Z")), first]);
        assert_eq!(versions[0].name.as_deref(), Some("Première fenêtre"));
        assert_eq!(versions[0].start_on, None);
        assert_eq!(versions[1].start_on.as_deref(), Some("2026-08-26T07:00:00Z"));
        assert_eq!((versions[2].name.as_deref(), versions[2].start_on.as_deref()), (None, None));

        assert_eq!(milestone_name("Lecture du projet\n- PR").as_deref(), Some("Lecture du projet"));
        assert_eq!(milestone_name("1. Étape"), None);
        assert_eq!(milestone_name("   \n"), None);
        assert_eq!(milestone_name(&"x".repeat(80)), None);
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
