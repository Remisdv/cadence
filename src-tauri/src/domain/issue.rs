//! Issues GitHub et conventions de labels (`type/*`, `status/*`, `agent/*`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{agent::AgentKind, UserRef};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum IssueType {
    Feat,
    Fix,
    Chore,
    Perf,
    Docs,
}

impl IssueType {
    pub const ALL: [IssueType; 5] =
        [IssueType::Feat, IssueType::Fix, IssueType::Chore, IssueType::Perf, IssueType::Docs];

    pub fn slug(self) -> &'static str {
        match self {
            IssueType::Feat => "feat",
            IssueType::Fix => "fix",
            IssueType::Chore => "chore",
            IssueType::Perf => "perf",
            IssueType::Docs => "docs",
        }
    }

    /// Label GitHub conventionné : `type/feat`.
    pub fn label(self) -> String {
        format!("type/{}", self.slug())
    }

    pub fn from_labels<'a>(labels: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        labels.into_iter().find_map(|l| {
            let slug = l.strip_prefix("type/")?;
            Self::ALL.into_iter().find(|t| t.slug() == slug)
        })
    }
}

/// État d'avancement. `Done` n'a pas de label : c'est une issue fermée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum IssueStatus {
    Todo,
    Doing,
    Review,
    Done,
}

impl IssueStatus {
    pub fn label(self) -> Option<&'static str> {
        match self {
            IssueStatus::Todo => Some("status/todo"),
            IssueStatus::Doing => Some("status/doing"),
            IssueStatus::Review => Some("status/review"),
            IssueStatus::Done => None,
        }
    }

    /// Une issue fermée est `Done` ; ouverte, son label décide (`Todo` par défaut).
    pub fn resolve<'a>(closed: bool, labels: impl IntoIterator<Item = &'a str>) -> Self {
        if closed {
            return IssueStatus::Done;
        }
        labels
            .into_iter()
            .find_map(|l| match l {
                "status/doing" => Some(IssueStatus::Doing),
                "status/review" => Some(IssueStatus::Review),
                "status/todo" => Some(IssueStatus::Todo),
                _ => None,
            })
            .unwrap_or(IssueStatus::Todo)
    }
}

/// Signaux affichés sur les cartes d'issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum IssueFlag {
    IncompleteDescription,
    CtoQuestions,
    CiFailed,
    /// Description réécrite et validée avec l'agent CTO (label `agent/cto`).
    CtoReviewed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IssueRow {
    pub number: u32,
    pub title: String,
    /// Titre du milestone. Toujours présent pour une issue créée par Cadence.
    pub version: Option<String>,
    pub issue_type: Option<IssueType>,
    pub status: IssueStatus,
    /// Labels hors conventions `type/`, `status/`, `agent/`.
    pub labels: Vec<String>,
    pub assignee: Option<UserRef>,
    /// Estimation proposée par l'agent CTO (XS, S, M, L).
    pub estimate: Option<String>,
    /// Agent qui travaille dessus en ce moment.
    pub active_agent: Option<AgentKind>,
    pub linked_pr: Option<u32>,
    pub flags: Vec<IssueFlag>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CtoQuestion {
    pub id: String,
    pub question: String,
    pub answer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IssueDetail {
    #[serde(flatten)]
    pub row: IssueRow,
    /// Corps markdown.
    pub body: String,
    pub html_url: String,
    pub author: Option<UserRef>,
    pub created_at: String,
    pub questions: Vec<CtoQuestion>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, optional_fields)]
pub struct IssueFilter {
    pub version: Option<String>,
    pub status: Option<Vec<IssueStatus>>,
    pub issue_type: Option<IssueType>,
    pub assignee: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, optional_fields)]
pub struct IssueCreateInput {
    pub title: String,
    pub version: String,
    pub issue_type: Option<IssueType>,
    pub body: Option<String>,
    pub labels: Option<Vec<String>>,
    pub assignee: Option<String>,
    pub send_to_cto: Option<bool>,
    pub launch_dev: Option<bool>,
}

impl IssueCreateInput {
    /// Règle produit : pas d'issue sans titre ni sans version.
    pub fn validate(&self) -> AppResult<()> {
        if self.title.trim().is_empty() {
            return Err(AppError::Validation("Une issue doit avoir un titre.".into()));
        }
        if self.version.trim().is_empty() {
            return Err(AppError::Validation(
                "Une issue doit être rattachée à une version.".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CtoAnswer {
    pub question_id: String,
    pub answer: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_type_from_labels() {
        let labels = ["area/api", "type/perf", "status/doing"];
        assert_eq!(IssueType::from_labels(labels), Some(IssueType::Perf));
        assert_eq!(IssueType::from_labels(["type/unknown"]), None);
    }

    #[test]
    fn closed_issue_is_done_whatever_its_labels() {
        assert_eq!(IssueStatus::resolve(true, ["status/doing"]), IssueStatus::Done);
        assert_eq!(IssueStatus::resolve(false, ["status/review"]), IssueStatus::Review);
        assert_eq!(IssueStatus::resolve(false, ["area/ui"]), IssueStatus::Todo);
    }

    #[test]
    fn creation_requires_title_and_version() {
        let mut input = IssueCreateInput { title: "Quota".into(), ..Default::default() };
        assert!(matches!(input.validate(), Err(AppError::Validation(_))));
        input.version = "0.1.2".into();
        assert!(input.validate().is_ok());
        input.title = "  ".into();
        assert!(input.validate().is_err());
    }
}
