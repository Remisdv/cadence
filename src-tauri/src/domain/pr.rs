use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{agent::AgentKind, UserRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum PrState {
    Draft,
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum CiState {
    None,
    Pending,
    Success,
    Failure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ReviewState {
    Pending,
    Approved,
    ChangesRequested,
    Commented,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrRow {
    pub number: u32,
    pub title: String,
    /// Branche `type/<num>-<slug>`.
    pub branch: String,
    pub base: String,
    pub author: UserRef,
    /// Agent auteur, si la PR a été ouverte par un agent.
    pub agent: Option<AgentKind>,
    pub state: PrState,
    pub ci: CiState,
    pub review: Option<ReviewState>,
    pub linked_issue: Option<u32>,
    pub version: Option<String>,
    pub additions: u32,
    pub deletions: u32,
    pub comments: u32,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, optional_fields)]
pub struct PrFilter {
    pub state: Option<Vec<PrState>>,
    pub author: Option<String>,
    pub version: Option<String>,
}

/// Pour qui la revue est demandée : `reviews_pending` sépare « vous » et « les autres ».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ReviewAudience {
    You,
    Others,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewRow {
    pub pr: PrRow,
    pub audience: ReviewAudience,
    pub requested_at: String,
}
