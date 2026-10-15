//! Pull requests : une requête GraphQL (CI, revues, diffstat, issue liée en un appel),
//! fichiers et patchs en REST pour l'écran de revue.

use reqwest::{header, Client};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::json;

use super::{repos::check_status, API_URL, API_VERSION};
use crate::{
    domain::{
        agent::AgentKind,
        pr::{CiState, PrFile, PrRow, PrState, ReviewAudience, ReviewRow, ReviewState},
        version, UserRef,
    },
    error::{AppError, AppResult},
};

/// PR ouvertes (50 au plus) et 30 dernières fusionnées ou fermées, triées par activité.
/// Coût mesuré : 1 à 3 points de quota GraphQL par passe.
const PULLS_QUERY: &str = r#"
query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    open: pullRequests(states: OPEN, first: 50, orderBy: {field: UPDATED_AT, direction: DESC}) { nodes { ...Pr } }
    recent: pullRequests(states: [MERGED, CLOSED], first: 30, orderBy: {field: UPDATED_AT, direction: DESC}) { nodes { ...Pr } }
  }
}
fragment Pr on PullRequest {
  number title body state isDraft url
  headRefName baseRefName
  author { login avatarUrl ... on User { name } }
  milestone { title }
  labels(first: 20) { nodes { name } }
  additions deletions changedFiles
  comments { totalCount }
  createdAt updatedAt
  closingIssuesReferences(first: 1) { nodes { number } }
  reviewDecision
  latestOpinionatedReviews(first: 20) { nodes { state } }
  reviewRequests(first: 10) { nodes { requestedReviewer { __typename ... on User { login } } } }
  timelineItems(itemTypes: [REVIEW_REQUESTED_EVENT], last: 1) { nodes { ... on ReviewRequestedEvent { createdAt } } }
  commits(last: 1) { nodes { commit { statusCheckRollup {
    state
    contexts(first: 1) { totalCount checkRunCountsByState { state count } statusContextCountsByState { state count } }
  } } } }
}"#;

#[derive(Debug, Clone, Deserialize)]
pub struct Nodes<T> {
    #[serde(default = "Vec::new")]
    pub nodes: Vec<T>,
}

// À la main : le dérivé exigerait `T: Default`.
impl<T> Default for Nodes<T> {
    fn default() -> Self {
        Self { nodes: Vec::new() }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Count {
    #[serde(rename = "totalCount", default)]
    pub total_count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawAuthor {
    pub login: String,
    #[serde(rename = "avatarUrl")]
    pub avatar_url: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Named {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Titled {
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Numbered {
    pub number: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateOnly {
    pub state: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReviewRequest {
    #[serde(rename = "requestedReviewer")]
    pub requested_reviewer: Option<Reviewer>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reviewer {
    /// Absent pour une équipe ou un bot.
    pub login: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dated {
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateCount {
    pub state: String,
    pub count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Contexts {
    #[serde(rename = "totalCount", default)]
    pub total_count: u32,
    #[serde(rename = "checkRunCountsByState", default)]
    pub check_runs: Vec<StateCount>,
    #[serde(rename = "statusContextCountsByState", default)]
    pub statuses: Vec<StateCount>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rollup {
    pub state: String,
    pub contexts: Option<Contexts>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitNode {
    pub commit: CommitRollup,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitRollup {
    #[serde(rename = "statusCheckRollup")]
    pub status_check_rollup: Option<Rollup>,
}

/// PR telle que renvoyée par `PULLS_QUERY` (et rangée telle quelle dans le cache).
#[derive(Debug, Clone, Deserialize)]
pub struct RawPr {
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    /// `OPEN`, `MERGED`, `CLOSED`
    pub state: String,
    #[serde(rename = "isDraft", default)]
    pub is_draft: bool,
    pub url: String,
    #[serde(rename = "headRefName")]
    pub head_ref_name: String,
    #[serde(rename = "baseRefName")]
    pub base_ref_name: String,
    /// Absent pour un compte supprimé.
    pub author: Option<RawAuthor>,
    pub milestone: Option<Titled>,
    #[serde(default)]
    pub labels: Nodes<Named>,
    #[serde(default)]
    pub additions: u32,
    #[serde(default)]
    pub deletions: u32,
    #[serde(rename = "changedFiles", default)]
    pub changed_files: u32,
    #[serde(default)]
    pub comments: Count,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "closingIssuesReferences", default)]
    pub closing_issues: Nodes<Numbered>,
    /// `APPROVED`, `CHANGES_REQUESTED`, `REVIEW_REQUIRED`, ou absent.
    #[serde(rename = "reviewDecision")]
    pub review_decision: Option<String>,
    #[serde(rename = "latestOpinionatedReviews", default)]
    pub reviews: Nodes<StateOnly>,
    #[serde(rename = "reviewRequests", default)]
    pub review_requests: Nodes<ReviewRequest>,
    #[serde(rename = "timelineItems", default)]
    pub review_requested: Nodes<Dated>,
    #[serde(default)]
    pub commits: Nodes<CommitNode>,
}

impl RawPr {
    fn is_open(&self) -> bool {
        self.state == "OPEN"
    }

    fn requested_from(&self, login: &str) -> bool {
        self.review_requests
            .nodes
            .iter()
            .any(|r| r.requested_reviewer.as_ref().and_then(|u| u.login.as_deref()) == Some(login))
    }
}

/// PR ouvertes et récentes du dépôt, avec leur charge brute.
pub async fn fetch_pulls(
    http: &Client,
    token: &SecretString,
    repo: &str,
) -> AppResult<Vec<(RawPr, serde_json::Value)>> {
    let (owner, name) = repo
        .split_once('/')
        .ok_or_else(|| AppError::Validation(format!("« {repo} » n'est pas un dépôt owner/name.")))?;
    let resp = http
        .post(format!("{API_URL}/graphql"))
        .bearer_auth(token.expose_secret())
        .json(&json!({ "query": PULLS_QUERY, "variables": { "owner": owner, "name": name } }))
        .send()
        .await?;
    check_status(resp.status())?;
    let body: serde_json::Value = resp.json().await?;
    if let Some(errors) = body.get("errors") {
        return Err(AppError::GitHub(format!("GraphQL : {errors}")));
    }
    Ok(parse_pulls(&body))
}

pub fn parse_pulls(body: &serde_json::Value) -> Vec<(RawPr, serde_json::Value)> {
    let repo = &body["data"]["repository"];
    ["open", "recent"]
        .iter()
        .filter_map(|key| repo[key]["nodes"].as_array())
        .flatten()
        .filter_map(|node| match RawPr::deserialize(node) {
            Ok(pr) => Some((pr, node.clone())),
            Err(err) => {
                tracing::warn!(%err, "PR GitHub illisible, ignorée");
                None
            }
        })
        .collect()
}

/// PR GitHub → ligne Cadence. `me` = login de l'utilisateur connecté.
pub fn to_row(raw: &RawPr, me: Option<&str>) -> PrRow {
    let state = match raw.state.as_str() {
        "MERGED" => PrState::Merged,
        "CLOSED" => PrState::Closed,
        _ if raw.is_draft => PrState::Draft,
        _ => PrState::Open,
    };
    let (ci, ci_passed, ci_total) = ci_of(raw);
    let count = |s: &str| raw.reviews.nodes.iter().filter(|r| r.state == s).count() as u32;
    let pending_request = !raw.review_requests.nodes.is_empty();
    let review = match raw.review_decision.as_deref() {
        Some("APPROVED") => Some(ReviewState::Approved),
        Some("CHANGES_REQUESTED") => Some(ReviewState::ChangesRequested),
        _ if pending_request => Some(ReviewState::Pending),
        Some("REVIEW_REQUIRED") => Some(ReviewState::Pending),
        _ => None,
    };
    let labels: Vec<&str> = raw.labels.nodes.iter().map(|l| l.name.as_str()).collect();
    PrRow {
        number: raw.number,
        title: raw.title.clone(),
        branch: raw.head_ref_name.clone(),
        base: raw.base_ref_name.clone(),
        author: match &raw.author {
            Some(a) => UserRef {
                login: a.login.clone(),
                name: a.name.clone().filter(|n| !n.is_empty()),
                avatar_url: a.avatar_url.clone(),
                is_self: me == Some(a.login.as_str()),
            },
            None => UserRef { login: "ghost".into(), name: None, avatar_url: None, is_self: false },
        },
        agent: [AgentKind::Cto, AgentKind::Ceo, AgentKind::Dev, AgentKind::Deploy]
            .into_iter()
            .find(|k| labels.contains(&k.label())),
        state,
        ci,
        ci_passed,
        ci_total,
        review,
        approvals: count("APPROVED"),
        changes_requested: count("CHANGES_REQUESTED"),
        review_requested_at: (raw.is_open() && pending_request)
            .then(|| raw.review_requested.nodes.last().and_then(|d| d.created_at.clone()))
            .flatten(),
        linked_issue: raw.closing_issues.nodes.first().map(|i| i.number),
        version: raw.milestone.as_ref().map(|m| {
            version::parse(&m.title).map(|v| v.to_string()).unwrap_or_else(|| m.title.clone())
        }),
        additions: raw.additions,
        deletions: raw.deletions,
        changed_files: raw.changed_files,
        comments: raw.comments.total_count,
        created_at: raw.created_at.clone(),
        updated_at: raw.updated_at.clone(),
    }
}

/// État de CI du dernier commit : (état, checks réussis, total).
fn ci_of(raw: &RawPr) -> (CiState, u32, u32) {
    let Some(rollup) = raw.commits.nodes.last().and_then(|c| c.commit.status_check_rollup.as_ref()) else {
        return (CiState::None, 0, 0);
    };
    let state = match rollup.state.as_str() {
        "SUCCESS" => CiState::Success,
        "FAILURE" | "ERROR" => CiState::Failure,
        _ => CiState::Pending,
    };
    let Some(ctx) = &rollup.contexts else { return (state, 0, 0) };
    let ok = |counts: &[StateCount], states: &[&str]| -> u32 {
        counts.iter().filter(|c| states.contains(&c.state.as_str())).map(|c| c.count).sum()
    };
    let passed = ok(&ctx.check_runs, &["SUCCESS", "NEUTRAL", "SKIPPED"]) + ok(&ctx.statuses, &["SUCCESS"]);
    (state, passed, ctx.total_count)
}

/// Revue à faire (« vous êtes relecteur ») ou attendue des autres (« vos PR »).
/// `None` si la PR n'attend rien de personne.
pub fn review_row(raw: &RawPr, row: &PrRow, me: &str) -> Option<ReviewRow> {
    if !raw.is_open() {
        return None;
    }
    let audience = if raw.requested_from(me) {
        ReviewAudience::You
    } else if row.author.is_self && row.review.is_some_and(|r| r != ReviewState::Approved) {
        ReviewAudience::Others
    } else {
        return None;
    };
    Some(ReviewRow {
        requested_at: row.review_requested_at.clone().unwrap_or_else(|| row.created_at.clone()),
        pr: row.clone(),
        audience,
    })
}

/// Fichiers modifiés et leurs patchs (300 au plus, la limite de l'API).
pub async fn fetch_files(
    http: &Client,
    token: &SecretString,
    repo: &str,
    number: u32,
) -> AppResult<Vec<PrFile>> {
    #[derive(Deserialize)]
    struct RawFile {
        filename: String,
        status: String,
        additions: u32,
        deletions: u32,
        patch: Option<String>,
    }
    let mut files = Vec::new();
    for page in 1..=3 {
        let resp = http
            .get(format!("{API_URL}/repos/{repo}/pulls/{number}/files?per_page=100&page={page}"))
            .bearer_auth(token.expose_secret())
            .header(header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION)
            .send()
            .await?;
        check_status(resp.status())?;
        let batch: Vec<RawFile> = resp.json().await?;
        let last = batch.len() < 100;
        files.extend(batch.into_iter().map(|f| PrFile {
            path: f.filename,
            status: f.status,
            additions: f.additions,
            deletions: f.deletions,
            patch: f.patch,
        }));
        if last {
            break;
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nœud au format exact renvoyé par GitHub (relevé sur un dépôt public).
    fn node(patch: serde_json::Value) -> serde_json::Value {
        let mut base = json!({
            "number": 86, "title": "feat(roadmap) : lanes", "body": "Corrige #138", "state": "OPEN",
            "isDraft": false, "url": "https://github.com/me/app/pull/86",
            "headRefName": "feat/138-roadmap-lanes", "baseRefName": "main",
            "author": {"login": "marc", "avatarUrl": "https://a", "name": "Marc B."},
            "milestone": {"title": "v0.1.2"},
            "labels": {"nodes": [{"name": "agent/dev"}]},
            "additions": 412, "deletions": 87, "changedFiles": 11, "comments": {"totalCount": 3},
            "createdAt": "2026-10-13T09:00:00Z", "updatedAt": "2026-10-15T09:00:00Z",
            "closingIssuesReferences": {"nodes": [{"number": 138}]},
            "reviewDecision": "REVIEW_REQUIRED",
            "latestOpinionatedReviews": {"nodes": [{"state": "COMMENTED"}]},
            "reviewRequests": {"nodes": [{"requestedReviewer": {"__typename": "User", "login": "remisdv"}}]},
            "timelineItems": {"nodes": [{"createdAt": "2026-10-14T08:00:00Z"}]},
            "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "FAILURE", "contexts": {
                "totalCount": 4,
                "checkRunCountsByState": [{"state": "SUCCESS", "count": 2}, {"state": "SKIPPED", "count": 1},
                                          {"state": "FAILURE", "count": 1}],
                "statusContextCountsByState": [{"state": "SUCCESS", "count": 0}]}}}}]}
        });
        if let (Some(base), Some(patch)) = (base.as_object_mut(), patch.as_object()) {
            for (k, v) in patch {
                base.insert(k.clone(), v.clone());
            }
        }
        base
    }

    fn raw(n: serde_json::Value) -> RawPr {
        RawPr::deserialize(&n).unwrap()
    }

    #[test]
    fn reads_both_lists_of_the_query() {
        let body = json!({"data": {"repository": {
            "open": {"nodes": [node(json!({}))]},
            "recent": {"nodes": [node(json!({"number": 80, "state": "MERGED"})), {"number": "illisible"}]}
        }}});
        let pulls = parse_pulls(&body);
        assert_eq!(pulls.iter().map(|(p, _)| p.number).collect::<Vec<_>>(), [86, 80]);
        assert!(pulls[0].1.get("url").is_some());
    }

    #[test]
    fn converts_ci_reviews_version_and_agent() {
        let row = to_row(&raw(node(json!({}))), Some("remisdv"));
        assert_eq!(row.state, PrState::Open);
        assert_eq!((row.ci, row.ci_passed, row.ci_total), (CiState::Failure, 3, 4));
        assert_eq!(row.review, Some(ReviewState::Pending));
        assert_eq!(row.review_requested_at.as_deref(), Some("2026-10-14T08:00:00Z"));
        assert_eq!(row.version.as_deref(), Some("0.1.2"));
        assert_eq!(row.linked_issue, Some(138));
        assert_eq!(row.agent, Some(AgentKind::Dev));
        assert!(!row.author.is_self);
    }

    #[test]
    fn drafts_merges_and_missing_ci() {
        let draft = to_row(&raw(node(json!({"isDraft": true, "commits": {"nodes": []}}))), None);
        assert_eq!((draft.state, draft.ci), (PrState::Draft, CiState::None));
        let merged = to_row(
            &raw(node(json!({"state": "MERGED", "reviewDecision": "APPROVED", "reviewRequests": {"nodes": []},
                "latestOpinionatedReviews": {"nodes": [{"state": "APPROVED"}, {"state": "APPROVED"}]}}))),
            None,
        );
        assert_eq!((merged.state, merged.review, merged.approvals), (PrState::Merged, Some(ReviewState::Approved), 2));
        assert_eq!(merged.review_requested_at, None);
        let ghost = to_row(&raw(node(json!({"author": null}))), None);
        assert_eq!(ghost.author.login, "ghost");
    }

    #[test]
    fn sorts_reviews_between_you_and_others() {
        let requested = raw(node(json!({})));
        let row = to_row(&requested, Some("remisdv"));
        assert_eq!(review_row(&requested, &row, "remisdv").unwrap().audience, ReviewAudience::You);

        let mine = raw(node(json!({"author": {"login": "remisdv", "avatarUrl": null, "name": null},
            "reviewRequests": {"nodes": [{"requestedReviewer": {"__typename": "User", "login": "marc"}}]}})));
        let row = to_row(&mine, Some("remisdv"));
        let others = review_row(&mine, &row, "remisdv").unwrap();
        assert_eq!((others.audience, others.requested_at.as_str()), (ReviewAudience::Others, "2026-10-14T08:00:00Z"));

        let approved = raw(node(json!({"author": {"login": "remisdv", "avatarUrl": null, "name": null},
            "reviewDecision": "APPROVED", "reviewRequests": {"nodes": []}})));
        let row = to_row(&approved, Some("remisdv"));
        assert!(review_row(&approved, &row, "remisdv").is_none());

        let merged = raw(node(json!({"state": "MERGED"})));
        let row = to_row(&merged, Some("remisdv"));
        assert!(review_row(&merged, &row, "remisdv").is_none());
    }
}
