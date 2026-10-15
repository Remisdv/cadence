//! Journal des exécutions d'agents : une ligne par exécution, tous agents confondus
//! (console des agents). Calculée depuis le détail gardé dans `agent_runs`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::agent::{AgentKind, AgentRunDetail, Autonomy, CtoItemState, DevTaskState, RunState};

/// Résumé d'une exécution : ce qu'elle a examiné, écrit, laissé en attente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentRunEntry {
    pub run: String,
    pub kind: AgentKind,
    pub state: RunState,
    pub autonomy: Autonomy,
    pub version: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    /// En pause : la file s'arrête après l'étape en cours.
    pub paused: bool,
    /// Issues (agent CTO) ou tâches (agent développeur) de l'exécution.
    pub total: u32,
    /// Traitées : ni en file ni en cours.
    pub processed: u32,
    /// En attente de validation : propositions, questions, pushes.
    pub pending: u32,
    /// Questions sans réponse posées par l'agent.
    pub questions: u32,
    /// Écrites sur GitHub : descriptions appliquées, branches poussées.
    pub applied: u32,
    /// Passées sans rien écrire.
    pub skipped: u32,
    pub failed: u32,
    /// Issue en cours, sinon la première en attente de validation.
    pub focus: Option<u32>,
    /// Étape de la tâche en cours (agent développeur, 1 à 6).
    pub step: Option<u8>,
    /// Dernière PR ouverte par l'exécution (agent développeur).
    pub pr: Option<u32>,
    pub error: Option<String>,
}

fn count<T>(items: &[T], pred: impl Fn(&T) -> bool) -> u32 {
    items.iter().filter(|i| pred(i)).count().min(u32::MAX as usize) as u32
}

impl AgentRunEntry {
    /// Agrège la file d'issues (CTO) et les tâches (développeur) ; un agent sans file
    /// (CEO, déploiement) garde des compteurs à zéro et son état.
    pub fn of(d: &AgentRunDetail) -> Self {
        let items = &d.items;
        let tasks = &d.tasks;
        let running_item = items.iter().find(|i| i.state == CtoItemState::Running);
        let running_task = tasks.iter().find(|t| t.state == DevTaskState::Running);
        let waiting_item = items.iter().find(|i| i.state.awaits_review());
        let waiting_task = tasks.iter().find(|t| t.state == DevTaskState::Review);
        let focus = running_task
            .map(|t| t.number)
            .or(running_item.map(|i| i.number))
            .or(waiting_task.map(|t| t.number))
            .or(waiting_item.map(|i| i.number));
        AgentRunEntry {
            run: d.run.clone(),
            kind: d.kind,
            state: d.state,
            autonomy: d.autonomy,
            version: d.version.clone(),
            started_at: d.started_at.clone(),
            ended_at: d.ended_at.clone(),
            paused: d.paused,
            total: (items.len() + tasks.len()).min(u32::MAX as usize) as u32,
            processed: count(items, |i| !matches!(i.state, CtoItemState::Queued | CtoItemState::Running))
                + count(tasks, |t| !matches!(t.state, DevTaskState::Queued | DevTaskState::Running)),
            pending: count(items, |i| i.state.awaits_review()) + count(tasks, |t| t.state == DevTaskState::Review),
            questions: items
                .iter()
                .filter(|i| i.state == CtoItemState::Questions)
                .map(|i| count(&i.questions, |q| q.answer.is_none()))
                .sum(),
            applied: count(items, |i| i.state == CtoItemState::Applied) + count(tasks, |t| t.state == DevTaskState::Done),
            skipped: count(items, |i| i.state == CtoItemState::Skipped) + count(tasks, |t| t.state == DevTaskState::Skipped),
            failed: count(items, |i| i.state == CtoItemState::Failed) + count(tasks, |t| t.state == DevTaskState::Failed),
            focus,
            step: running_task.map(|t| t.step),
            pr: tasks.iter().rev().find_map(|t| t.pr),
            error: d.error.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        agent::{CtoItem, DevTask},
        issue::CtoQuestion,
    };

    fn run(kind: AgentKind, state: RunState) -> AgentRunDetail {
        AgentRunDetail {
            run: format!("{kind:?}-1").to_lowercase(),
            kind,
            state,
            autonomy: Autonomy::ApplyAfterReview,
            version: Some("0.1.2".into()),
            model: None,
            started_at: "2026-10-15T08:40:00Z".into(),
            ended_at: None,
            items: vec![],
            tasks: vec![],
            log: vec![],
            error: None,
            paused: false,
        }
    }

    fn item(number: u32, state: CtoItemState, unanswered: usize) -> CtoItem {
        CtoItem {
            number,
            title: format!("Issue {number}"),
            version: Some("0.1.2".into()),
            issue_type: None,
            state,
            before: String::new(),
            after: None,
            questions: (0..unanswered)
                .map(|n| CtoQuestion { id: format!("q{n}"), question: "?".into(), answer: None })
                .chain([CtoQuestion { id: "done".into(), question: "?".into(), answer: Some("oui".into()) }])
                .collect(),
            estimate: None,
            estimate_hours: None,
            labels: vec![],
            risk: None,
            related: vec![],
            error: None,
            session_id: None,
        }
    }

    fn task(number: u32, state: DevTaskState, step: u8, pr: Option<u32>) -> DevTask {
        DevTask {
            number,
            title: format!("Tâche {number}"),
            issue_type: None,
            estimate: None,
            state,
            step,
            step_notes: vec![],
            branch: format!("feat/{number}-x"),
            worktree: None,
            pr,
            pr_url: None,
            files: vec![],
            additions: 0,
            deletions: 0,
            summary: None,
            error: None,
            started_at: None,
            session_id: None,
            pr_node_id: None,
        }
    }

    #[test]
    fn summarizes_a_cto_queue() {
        let mut d = run(AgentKind::Cto, RunState::Running);
        d.items = vec![
            item(151, CtoItemState::Applied, 0),
            item(153, CtoItemState::Ready, 0),
            item(156, CtoItemState::Questions, 2),
            item(158, CtoItemState::Running, 0),
            item(160, CtoItemState::Queued, 0),
            item(161, CtoItemState::Skipped, 0),
            item(162, CtoItemState::Failed, 0),
        ];
        let e = AgentRunEntry::of(&d);
        assert_eq!((e.total, e.processed, e.pending, e.questions), (7, 5, 2, 2));
        assert_eq!((e.applied, e.skipped, e.failed), (1, 1, 1));
        assert_eq!(e.focus, Some(158), "l'issue en cours passe avant celles à relire");
        assert_eq!((e.step, e.pr), (None, None));
    }

    #[test]
    fn summarizes_dev_tasks() {
        let mut d = run(AgentKind::Dev, RunState::Running);
        d.tasks = vec![
            task(140, DevTaskState::Done, 6, Some(87)),
            task(142, DevTaskState::Running, 3, Some(88)),
            task(145, DevTaskState::Queued, 0, None),
        ];
        let e = AgentRunEntry::of(&d);
        assert_eq!((e.total, e.processed, e.applied, e.pending), (3, 1, 1, 0));
        assert_eq!((e.focus, e.step, e.pr), (Some(142), Some(3), Some(88)));

        d.tasks[1].state = DevTaskState::Review;
        let e = AgentRunEntry::of(&d);
        assert_eq!((e.pending, e.focus, e.step), (1, Some(142), None));
    }

    #[test]
    fn keeps_the_state_of_an_agent_without_queue() {
        let mut d = run(AgentKind::Deploy, RunState::Failed);
        d.error = Some("tag déjà pris".into());
        d.paused = true;
        let e = AgentRunEntry::of(&d);
        assert_eq!((e.total, e.processed, e.focus), (0, 0, None));
        assert_eq!((e.state, e.paused, e.error.as_deref()), (RunState::Failed, true, Some("tag déjà pris")));
    }
}
