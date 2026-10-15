//! Issues d'un dépôt : lecture incrémentale paginée, ETag, conversion en lignes Cadence.

use reqwest::{header, Client, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use super::{API_URL, API_VERSION};
use crate::{
    domain::{
        issue::{IssueFilter, IssueFlag, IssueRow, IssueStatus, IssueType},
        version, UserRef,
    },
    error::{AppError, AppResult},
};

/// Garde-fou de quota par passe (10 pages de 100). Les issues arrivant de la plus
/// ancienne modification à la plus récente, la passe suivante reprend où celle-ci s'arrête.
const MAX_PAGES: usize = 10;

#[derive(Debug, Clone, Deserialize)]
pub struct RawLabel {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawMilestone {
    pub number: u32,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawUser {
    pub login: String,
    pub avatar_url: Option<String>,
}

/// Issue telle que renvoyée par `GET /repos/{repo}/issues`.
#[derive(Debug, Clone, Deserialize)]
pub struct RawIssue {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub body: Option<String>,
    #[serde(default)]
    pub labels: Vec<RawLabel>,
    pub milestone: Option<RawMilestone>,
    #[serde(default)]
    pub assignees: Vec<RawUser>,
    pub updated_at: String,
    #[serde(default)]
    pub created_at: Option<String>,
    /// Fermeture (burndown), absente pour une issue ouverte.
    #[serde(default)]
    pub closed_at: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub user: Option<RawUser>,
    /// Présent quand l'élément est une pull request (l'API mélange les deux).
    pub pull_request: Option<serde_json::Value>,
}

/// Délai minimal entre deux passes de synchronisation (le plan : toutes les 60 s).
pub const REFRESH_EVERY: std::time::Duration = std::time::Duration::from_secs(60);

pub enum IssuesFetch {
    /// `304 Not Modified` : rien n'a changé depuis `since`, aucun quota consommé.
    NotModified,
    Fresh {
        etag: Option<String>,
        /// Issues modifiées depuis `since` (PR exclues), avec leur charge brute.
        issues: Vec<(RawIssue, serde_json::Value)>,
        /// Vrai si la limite de pages a coupé la lecture : il en reste à lire.
        truncated: bool,
    },
}

/// Lit les issues (ouvertes et fermées) modifiées depuis `since` (toutes si `None`),
/// de la plus ancienne modification à la plus récente. Avec `etag`, la première requête
/// est conditionnelle : rien n'est relu si rien n'a changé.
pub async fn fetch_issues(
    http: &Client,
    token: &SecretString,
    repo: &str,
    since: Option<&str>,
    etag: Option<&str>,
) -> AppResult<IssuesFetch> {
    let mut first =
        format!("{API_URL}/repos/{repo}/issues?state=all&per_page=100&sort=updated&direction=asc");
    if let Some(since) = since {
        first.push_str("&since=");
        first.push_str(since);
    }
    let mut url = Some(first);
    let mut first_etag = None;
    let mut issues = Vec::new();

    for page in 0..MAX_PAGES {
        let Some(current) = url.take() else { break };
        let mut req = http
            .get(&current)
            .bearer_auth(token.expose_secret())
            .header(header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION);
        if page == 0
            && let Some(etag) = etag
        {
            req = req.header(header::IF_NONE_MATCH, etag);
        }
        let resp = req.send().await?;
        match resp.status() {
            StatusCode::NOT_MODIFIED => return Ok(IssuesFetch::NotModified),
            StatusCode::UNAUTHORIZED => return Err(AppError::Unauthenticated("GitHub")),
            StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => return Err(AppError::RateLimited),
            StatusCode::NOT_FOUND => {
                return Err(AppError::GitHub("dépôt introuvable ou inaccessible".into()));
            }
            s if !s.is_success() => return Err(AppError::GitHub(format!("GitHub a répondu {s}"))),
            _ => {}
        }
        if page == 0 {
            first_etag = resp.headers().get(header::ETAG).and_then(|v| v.to_str().ok()).map(str::to_string);
        }
        url = resp
            .headers()
            .get(header::LINK)
            .and_then(|v| v.to_str().ok())
            .and_then(next_page);
        let batch: Vec<serde_json::Value> = resp.json().await?;
        issues.extend(parse_page(batch));
    }
    Ok(IssuesFetch::Fresh { etag: first_etag, issues, truncated: url.is_some() })
}

/// Page brute → issues, sans les pull requests (l'API mélange les deux).
pub fn parse_page(batch: Vec<serde_json::Value>) -> Vec<(RawIssue, serde_json::Value)> {
    batch
        .into_iter()
        .filter_map(|raw| match RawIssue::deserialize(&raw) {
            Ok(issue) if issue.pull_request.is_none() => Some((issue, raw)),
            Ok(_) => None,
            Err(err) => {
                tracing::warn!(%err, "issue GitHub illisible, ignorée");
                None
            }
        })
        .collect()
}

/// URL `rel="next"` d'un en-tête `Link` de pagination GitHub.
pub fn next_page(link: &str) -> Option<String> {
    link.split(',').find_map(|part| {
        let (url, rel) = part.split_once(';')?;
        rel.contains(r#"rel="next""#)
            .then(|| url.trim().trim_start_matches('<').trim_end_matches('>').to_string())
    })
}

/// Préfixes de labels conventionnés, affichés autrement que comme labels.
const CONVENTION_PREFIXES: [&str; 3] = ["type/", "status/", "agent/"];

/// Issue GitHub → ligne Cadence. `me` = login de l'utilisateur connecté (« vous »).
pub fn to_row(raw: &RawIssue, me: Option<&str>) -> IssueRow {
    let names: Vec<&str> = raw.labels.iter().map(|l| l.name.as_str()).collect();
    let closed = raw.state == "closed";
    let version = raw.milestone.as_ref().map(|m| {
        version::parse(&m.title).map(|v| v.to_string()).unwrap_or_else(|| m.title.clone())
    });
    let empty_body = raw.body.as_deref().map(str::trim).unwrap_or("").is_empty();
    IssueRow {
        number: raw.number,
        title: raw.title.clone(),
        version,
        issue_type: IssueType::from_labels(names.iter().copied()),
        status: IssueStatus::resolve(closed, names.iter().copied()),
        labels: names
            .iter()
            .filter(|n| !CONVENTION_PREFIXES.iter().any(|p| n.starts_with(p)))
            .map(|n| n.to_string())
            .collect(),
        assignee: raw.assignees.first().map(|u| UserRef {
            login: u.login.clone(),
            name: None,
            avatar_url: u.avatar_url.clone(),
            is_self: me == Some(u.login.as_str()),
        }),
        estimate: names.iter().find_map(|n| n.strip_prefix("estimate/")).map(str::to_string),
        active_agent: None,
        linked_pr: None,
        flags: [
            (empty_body && !closed).then_some(IssueFlag::IncompleteDescription),
            names.contains(&"agent/cto").then_some(IssueFlag::CtoReviewed),
        ]
        .into_iter()
        .flatten()
        .collect(),
        updated_at: raw.updated_at.clone(),
    }
}

/// Filtre appliqué côté Rust (le front peut aussi tout demander et filtrer lui-même).
pub fn matches(row: &IssueRow, filter: &IssueFilter) -> bool {
    let query = filter.query.as_deref().map(str::to_lowercase).unwrap_or_default();
    filter.version.as_ref().is_none_or(|v| row.version.as_ref() == Some(v))
        && filter.status.as_ref().is_none_or(|s| s.contains(&row.status))
        && filter.issue_type.is_none_or(|t| row.issue_type == Some(t))
        && filter.assignee.as_ref().is_none_or(|a| row.assignee.as_ref().is_some_and(|u| &u.login == a))
        && (query.is_empty()
            || row.title.to_lowercase().contains(&query)
            || row.number.to_string() == query.trim_start_matches('#')
            || row.labels.iter().any(|l| l.to_lowercase().contains(&query)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(json: serde_json::Value) -> RawIssue {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn skips_pull_requests_and_keeps_the_raw_payload() {
        let page = vec![
            serde_json::json!({"number": 1, "title": "Issue", "state": "open", "body": null,
                "updated_at": "2026-10-01T10:00:00Z", "html_url": "https://github.com/me/app/issues/1"}),
            serde_json::json!({"number": 2, "title": "PR", "state": "open", "body": null,
                "updated_at": "2026-10-01T10:00:00Z", "pull_request": {"url": "x"}}),
            serde_json::json!({"number": "pas un nombre"}),
        ];
        let issues = parse_page(page);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].0.number, 1);
        assert!(issues[0].1.get("html_url").is_some());
    }

    #[test]
    fn follows_the_next_page_link() {
        let link = r#"<https://api.github.com/repositories/1/issues?page=2>; rel="next", <https://api.github.com/repositories/1/issues?page=5>; rel="last""#;
        assert_eq!(next_page(link).as_deref(), Some("https://api.github.com/repositories/1/issues?page=2"));
        assert_eq!(next_page(r#"<https://x?page=1>; rel="prev""#), None);
    }

    #[test]
    fn converts_labels_milestone_and_assignee() {
        let issue = raw(serde_json::json!({
            "number": 156, "title": "Quota API", "state": "open", "body": "",
            "labels": [{"name": "type/fix"}, {"name": "status/doing"}, {"name": "area/api"}, {"name": "estimate/M"}],
            "milestone": {"number": 4, "title": "v0.1.2"},
            "assignees": [{"login": "remisdv", "avatar_url": null}],
            "updated_at": "2026-10-15T10:00:00Z"
        }));
        let row = to_row(&issue, Some("remisdv"));
        assert_eq!(row.version.as_deref(), Some("0.1.2"));
        assert_eq!(row.issue_type, Some(IssueType::Fix));
        assert_eq!(row.status, IssueStatus::Doing);
        assert_eq!(row.labels, ["area/api", "estimate/M"]);
        assert_eq!(row.estimate.as_deref(), Some("M"));
        assert!(row.assignee.unwrap().is_self);
        assert_eq!(row.flags, [IssueFlag::IncompleteDescription]);
    }

    #[test]
    fn closed_issues_are_done_and_filters_apply() {
        let issue = raw(serde_json::json!({
            "number": 131, "title": "Squelette Tauri", "state": "closed", "body": "ok",
            "labels": [{"name": "status/doing"}], "milestone": null, "assignees": [],
            "updated_at": "2026-09-10T10:00:00Z"
        }));
        let row = to_row(&issue, None);
        assert_eq!(row.status, IssueStatus::Done);
        assert!(row.flags.is_empty());
        let open_only = IssueFilter { status: Some(vec![IssueStatus::Todo]), ..Default::default() };
        assert!(!matches(&row, &open_only));
        let by_number = IssueFilter { query: Some("#131".into()), ..Default::default() };
        assert!(matches(&row, &by_number));
    }
}
