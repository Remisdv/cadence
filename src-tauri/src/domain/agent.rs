//! Agents : types partagés. La mécanique (runner, garde-fous) vit dans `crate::agents`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AgentKind {
    Cto,
    Ceo,
    Dev,
    Deploy,
}

impl AgentKind {
    /// Label de trace posé sur GitHub : `agent/cto`.
    pub fn label(self) -> &'static str {
        match self {
            AgentKind::Cto => "agent/cto",
            AgentKind::Ceo => "agent/ceo",
            AgentKind::Dev => "agent/dev",
            AgentKind::Deploy => "agent/deploy",
        }
    }

    /// L'agent CEO est verrouillé en `Propose` ; les autres valident avant d'écrire.
    pub fn default_autonomy(self) -> Autonomy {
        match self {
            AgentKind::Ceo => Autonomy::Propose,
            _ => Autonomy::ApplyAfterReview,
        }
    }

    pub fn allows_autonomy(self, level: Autonomy) -> bool {
        !(self == AgentKind::Ceo && level != Autonomy::Propose)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Autonomy {
    /// Rien n'est appliqué.
    Propose,
    /// Appliqué après validation explicite (défaut).
    ApplyAfterReview,
    /// Appliqué directement, garde-fous toujours actifs.
    Auto,
}

/// Cycle de vie d'une exécution (voir `agents::runner`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum RunState {
    Prepare,
    Running,
    Proposing,
    Review,
    Applying,
    Done,
    Discarded,
    Suspended,
    Failed,
}

/// Activité d'un agent telle que résumée dans le rail et la console.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AgentActivity {
    Idle,
    /// Des actions attendent votre validation.
    Waiting,
    Running,
}

/// Exécution en cours, pour la carte « Agent dev en cours ».
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunSummary {
    pub run: String,
    pub state: RunState,
    pub issue: Option<u32>,
    pub step: Option<u8>,
    pub steps: Option<u8>,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentOverview {
    pub kind: AgentKind,
    pub autonomy: Autonomy,
    pub activity: AgentActivity,
    /// Actions proposées en attente de validation.
    pub pending_actions: u32,
    pub current_run: Option<RunSummary>,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, optional_fields)]
pub struct AgentInput {
    /// Version ciblée (CTO, déploiement).
    pub version: Option<String>,
    /// Issues ciblées (CTO sur une issue, tâches de l'agent dev).
    pub issues: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
#[ts(export)]
pub enum AgentControl {
    Pause,
    Resume,
    Stop,
    /// Intervention en cours de session (terminal de l'agent).
    Message { text: String },
}

/// Action typée produite par un agent. Un agent ne touche jamais GitHub :
/// Cadence exécute l'action après décision, via `agent_apply`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum AgentAction {
    UpdateIssueBody { number: u32, before: String, after: String },
    AskQuestion { number: u32, question: String },
    ProposeIssue { title: String, body: String, milestone: String, labels: Vec<String> },
    CreateBranch { issue: u32, name: String },
    OpenDraftPr { issue: u32, branch: String, title: String },
    PushCommits { branch: String },
    PublishRelease { tag: String, notes: String },
    ReopenIssue { number: u32, milestone: String },
}

/// Action en attente de validation, avec son aperçu lisible.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProposedAction {
    pub id: String,
    pub preview: String,
    pub action: AgentAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApplyFailure {
    pub id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApplyReport {
    pub applied: Vec<String>,
    pub failed: Vec<ApplyFailure>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceo_is_locked_to_propose() {
        assert_eq!(AgentKind::Ceo.default_autonomy(), Autonomy::Propose);
        assert!(!AgentKind::Ceo.allows_autonomy(Autonomy::Auto));
        assert!(AgentKind::Dev.allows_autonomy(Autonomy::Auto));
    }

    #[test]
    fn action_serializes_as_tagged_union() {
        let a = AgentAction::CreateBranch { issue: 142, name: "feat/142-device-flow".into() };
        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json["type"], "create-branch");
        assert_eq!(json["issue"], 142);
    }
}
