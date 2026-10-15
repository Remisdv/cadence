//! Moteur d'agents.
//!
//! Les quatre agents partagent un `AgentRunner` : il construit un contexte, lance Claude Code
//! en headless (`--output-format stream-json`), réémet chaque ligne sous `agent://event`
//! et transforme les sorties en `AgentAction` proposées. Un agent se distingue par son prompt
//! système, le contexte qu'il reçoit et les actions qu'il a le droit de produire.

pub mod runner;

use crate::domain::agent::{AgentAction, AgentKind};

/// Actions qu'un agent a le droit de produire ; toute autre sortie est rejetée.
pub fn may_produce(kind: AgentKind, action: &AgentAction) -> bool {
    use AgentAction::*;
    matches!(
        (kind, action),
        (AgentKind::Cto, UpdateIssueBody { .. } | AskQuestion { .. })
            | (AgentKind::Ceo, ProposeIssue { .. })
            | (AgentKind::Dev, CreateBranch { .. } | OpenDraftPr { .. } | PushCommits { .. })
            | (AgentKind::Deploy, PublishRelease { .. } | ReopenIssue { .. })
    )
}

/// Garde-fou : aucun agent ne pousse sur la branche par défaut, même en autonomie `auto`.
pub fn is_protected_branch(branch: &str, default_branch: &str) -> bool {
    branch == default_branch || branch == "main" || branch == "master"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_agent_is_restricted_to_its_actions() {
        let push = AgentAction::PushCommits { branch: "feat/1-x".into() };
        assert!(may_produce(AgentKind::Dev, &push));
        assert!(!may_produce(AgentKind::Ceo, &push));
        let release = AgentAction::PublishRelease { tag: "v0.1.2".into(), notes: String::new() };
        assert!(may_produce(AgentKind::Deploy, &release));
        assert!(!may_produce(AgentKind::Cto, &release));
    }

    #[test]
    fn default_branch_is_protected() {
        assert!(is_protected_branch("main", "main"));
        assert!(is_protected_branch("develop", "develop"));
        assert!(!is_protected_branch("feat/142-device-flow", "main"));
    }
}
