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
    /// Checks réussis (ou neutres / ignorés) sur le dernier commit, parmi `ci_total`.
    pub ci_passed: u32,
    pub ci_total: u32,
    /// Décision de revue : approuvée, changements demandés, ou en attente d'un relecteur.
    pub review: Option<ReviewState>,
    pub approvals: u32,
    pub changes_requested: u32,
    /// Dernière demande de revue encore ouverte (« en attente · 2 j »).
    pub review_requested_at: Option<String>,
    pub linked_issue: Option<u32>,
    pub version: Option<String>,
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
    pub comments: u32,
    pub created_at: String,
    pub updated_at: String,
}

/// Fichier modifié par une PR, avec son patch unifié (absent pour un binaire ou un
/// fichier trop gros).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrFile {
    pub path: String,
    /// `added`, `modified`, `removed`, `renamed`…
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub patch: Option<String>,
}

/// Écran de revue : la PR, sa description et ses fichiers.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrDetail {
    pub pr: PrRow,
    pub body: Option<String>,
    pub url: String,
    pub files: Vec<PrFile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, optional_fields)]
pub struct PrFilter {
    pub state: Option<Vec<PrState>>,
    pub author: Option<String>,
    pub version: Option<String>,
}

impl PrFilter {
    pub fn matches(&self, row: &PrRow) -> bool {
        self.state.as_ref().is_none_or(|s| s.contains(&row.state))
            && self.author.as_ref().is_none_or(|a| &row.author.login == a)
            && self.version.as_ref().is_none_or(|v| row.version.as_ref() == Some(v))
    }
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
