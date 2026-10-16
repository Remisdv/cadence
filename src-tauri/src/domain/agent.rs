//! Agents : types partagés. La mécanique (runner, garde-fous) vit dans `crate::agents`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::issue::{CtoQuestion, IssueType};
use crate::events::AgentEventKind;

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
    /// Agent développeur : faire lancer les tests par l'agent (étape 4). Vrai par défaut.
    pub run_tests: Option<bool>,
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

/* ---------- Exécution de l'agent CTO ---------- */

/// Où en est une issue dans la file de l'agent CTO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum CtoItemState {
    /// Pas encore traitée.
    Queued,
    /// Session Claude Code en cours sur cette issue.
    Running,
    /// Description proposée, prête à relire.
    Ready,
    /// L'agent attend vos réponses avant de conclure.
    Questions,
    /// Écrite sur GitHub.
    Applied,
    /// Passée sans rien écrire.
    Skipped,
    Failed,
}

impl CtoItemState {
    /// Proposition à relire (prête, ou en attente de réponses).
    pub fn awaits_review(self) -> bool {
        matches!(self, CtoItemState::Ready | CtoItemState::Questions)
    }
}

/// Une issue dans la file de l'agent CTO, avec ce qu'il propose.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CtoItem {
    pub number: u32,
    pub title: String,
    pub version: Option<String>,
    pub issue_type: Option<IssueType>,
    pub state: CtoItemState,
    /// Description actuelle sur GitHub.
    pub before: String,
    /// Description proposée.
    pub after: Option<String>,
    pub questions: Vec<CtoQuestion>,
    /// Estimation : `S`, `M`, `L`, `XL`.
    pub estimate: Option<String>,
    pub estimate_hours: Option<u32>,
    /// Labels proposés (hors conventions `type/`, `agent/`).
    pub labels: Vec<String>,
    /// `faible`, `moyen`, `élevé`.
    pub risk: Option<String>,
    pub related: Vec<u32>,
    pub error: Option<String>,
    /// Session Claude Code de l'issue, reprise pour intégrer vos réponses.
    #[ts(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Ligne du journal d'une exécution (la console de l'écran d'agent).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentLogLine {
    pub kind: AgentEventKind,
    pub text: String,
}

/// Exécution d'un agent : état, file d'issues, journal. Gardée dans `agent_runs`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentRunDetail {
    pub run: String,
    pub kind: AgentKind,
    pub state: RunState,
    pub autonomy: Autonomy,
    pub version: Option<String>,
    pub model: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub items: Vec<CtoItem>,
    /// Tâches de l'agent développeur.
    #[serde(default)]
    pub tasks: Vec<DevTask>,
    pub log: Vec<AgentLogLine>,
    /// Pourquoi l'exécution a échoué, le cas échéant.
    pub error: Option<String>,
    /// En pause : la file s'arrête après l'étape en cours.
    #[serde(default)]
    pub paused: bool,
    /// Pipeline de l'agent déploiement.
    #[serde(default)]
    pub deploy: Option<DeployState>,
    /// Issues proposées par l'agent CEO.
    #[serde(default)]
    pub proposals: Vec<CeoProposal>,
}

/* ---------- Exécution de l'agent développeur ---------- */

/// Où en est une tâche de l'agent développeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum DevTaskState {
    Queued,
    Running,
    /// Code commité dans le worktree : le push attend votre validation.
    Review,
    Done,
    Skipped,
    Failed,
}

/// Fichier touché par une tâche (diffstat du worktree).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DevFile {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
    /// Fichier créé par la tâche.
    pub added: bool,
}

/// Une issue confiée à l'agent développeur : six étapes, de l'analyse à la demande de revue.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DevTask {
    pub number: u32,
    pub title: String,
    pub issue_type: Option<IssueType>,
    pub estimate: Option<String>,
    pub state: DevTaskState,
    /// Étape courante (1 à 6) ; 0 tant que la tâche est en file.
    pub step: u8,
    /// Résumé de l'étape terminée, par étape (index 0 = étape 1).
    pub step_notes: Vec<String>,
    pub branch: String,
    pub worktree: Option<String>,
    pub pr: Option<u32>,
    pub pr_url: Option<String>,
    pub files: Vec<DevFile>,
    pub additions: u32,
    pub deletions: u32,
    /// Ce que l'agent dit avoir fait (corps du commentaire sur la PR).
    pub summary: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    #[ts(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[ts(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_node_id: Option<String>,
}

/* ---------- Exécution de l'agent déploiement ---------- */

/// État d'une étape du pipeline de déploiement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum DeployStageState {
    Pending,
    Running,
    Success,
    Failure,
}

/// Une des cinq étapes : pré-vol, build, tests de vérification, documentation, tag et release.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeployStage {
    pub state: DeployStageState,
    /// Résumé d'une ligne (« 14 issues fermées · tag v0.1.1 »).
    pub detail: Option<String>,
}

/// Verdict d'un test d'acceptation rejoué sur le binaire construit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum DeployTestStatus {
    Queued,
    Running,
    Passed,
    Failed,
    /// Pas de critère d'acceptation, ou vérification impossible sur le binaire.
    Unverifiable,
}

/// Ce que vous avez tranché pour un test en échec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum DeployDecision {
    /// Issue rouverte sur la version suivante, retirée des notes.
    Reopened,
    /// Échec ignoré : l'issue reste close et figure dans les notes.
    Ignored,
}

/// Un test par issue close de la version, dérivé de ses critères d'acceptation.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeployTest {
    pub number: u32,
    pub title: String,
    pub issue_type: Option<IssueType>,
    /// Cases `- [ ]` de la section « Critères d'acceptation ».
    pub criteria: Vec<String>,
    pub status: DeployTestStatus,
    /// Ce que l'agent a constaté (cause d'un échec, ce qui a été vérifié).
    pub detail: Option<String>,
    pub duration_ms: Option<u32>,
    pub decision: Option<DeployDecision>,
}

/// Fichier produit par le build (installeur, binaire, archive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeployArtifact {
    /// Chemin relatif au worktree.
    pub path: String,
    pub size_kb: Option<u32>,
}

/// Pipeline de l'agent déploiement sur une version close (son tag).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeployState {
    pub tag: String,
    /// Libellé du sprint (« Lecture du projet »).
    pub name: Option<String>,
    /// Version où rouvrir une issue dont le test échoue.
    pub next_version: Option<String>,
    /// Les cinq étapes, dans l'ordre.
    pub stages: Vec<DeployStage>,
    /// Étape courante (1 à 5) ; 0 avant le pré-vol.
    pub step: u8,
    pub tests: Vec<DeployTest>,
    pub artifacts: Vec<DeployArtifact>,
    /// Notes de version (markdown), sans les issues en échec.
    pub notes: Option<String>,
    /// Notes réécrites à la main : plus régénérées après vos décisions.
    pub notes_edited: bool,
    /// Release existante (brouillon créé à la clôture) ou publiée.
    pub release_url: Option<String>,
    pub published: bool,
    pub worktree: Option<String>,
    #[ts(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/* ---------- Exécution de l'agent CEO ---------- */

/// Où en est une issue proposée par l'agent CEO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum CeoProposalState {
    /// Création demandée, en cours d'écriture sur GitHub (évite un doublon).
    Queued,
    /// À relire.
    Ready,
    /// Issue créée sur GitHub.
    Applied,
    /// Rejetée sans rien écrire.
    Skipped,
    /// La création a échoué (on peut réessayer).
    Failed,
}

/// Issue que l'agent CEO propose de créer, avec sa justification.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CeoProposal {
    /// Rang dans l'exécution ; l'action correspondante est `propose-<index>`.
    pub index: u32,
    pub title: String,
    pub body: String,
    /// Version (titre semver du milestone) où créer l'issue.
    pub milestone: String,
    pub issue_type: Option<IssueType>,
    /// Labels proposés (hors conventions `type/`, `agent/`…).
    pub labels: Vec<String>,
    /// Thème de la roadmap dont elle découle.
    pub theme: Option<String>,
    /// Pourquoi rien ne la couvre déjà.
    pub rationale: String,
    /// Recouvrement éventuel avec une issue existante.
    pub overlap: Option<String>,
    /// Issues existantes citées dans le recouvrement.
    pub overlap_issues: Vec<u32>,
    pub state: CeoProposalState,
    /// Numéro de l'issue créée.
    pub issue: Option<u32>,
    pub issue_url: Option<String>,
    pub error: Option<String>,
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
