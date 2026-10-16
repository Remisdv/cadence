//! Écran « Détail d'une version » (et tableau du sprint en cours) : la version, son rythme,
//! ses issues reliées à leurs PR, ses voisines dans la roadmap.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    burndown,
    issue::{IssueFlag, IssueRow, IssueStatus},
    pr::{CiState, PrRow, PrState},
    version::{days_between, Version, VersionDetail, VersionKind, VersionState},
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VersionReport {
    /// Version, avancement, rythme et burndown (vide pour un palier).
    pub detail: VersionDetail,
    /// Vrai pour la version en cours du projet : la seule qu'on peut clôturer.
    pub current: bool,
    /// Issues de la version, ouvertes d'abord, avec leur PR liée.
    pub issues: Vec<IssueRow>,
    /// PR rattachées à la version (milestone) ou à l'une de ses issues, parmi les PR ouvertes
    /// et récentes du cache.
    pub pulls: Vec<PrRow>,
    pub previous: Option<String>,
    pub next: Option<String>,
    /// Durée du sprint en jours (axe du burndown), `None` pour un palier.
    pub sprint_days: Option<u32>,
    /// Tag de livraison (`v0.1.1`) d'une version close.
    pub tag: Option<String>,
}

/// Issue de la version et son jour de fermeture (`None` si elle est ouverte).
pub struct ScopeIssue {
    pub row: IssueRow,
    pub closed_at: Option<String>,
}

/// Assemble le rapport de `version`. `versions` : toutes les versions triées (voisines) ;
/// `pulls` : toutes les PR du cache.
pub fn build(
    version: &Version,
    versions: &[Version],
    scope: Vec<ScopeIssue>,
    pulls: &[PrRow],
    current: bool,
    tag_prefix: &str,
    today: &str,
) -> VersionReport {
    let closed_on: Vec<Option<&str>> = scope.iter().map(|i| i.closed_at.as_deref()).collect();
    let detail = burndown::version_detail(version, &closed_on, today);
    let sprint_days = match (&detail.version.start_on, &detail.version.due_on) {
        (Some(start), Some(due)) if version.kind == VersionKind::Sprint => {
            days_between(start, due).and_then(|d| u32::try_from(d).ok())
        }
        _ => None,
    };

    let mut issues: Vec<IssueRow> = scope.into_iter().map(|i| i.row).collect();
    let numbers: Vec<u32> = issues.iter().map(|i| i.number).collect();
    let pulls: Vec<PrRow> = pulls
        .iter()
        .filter(|p| {
            p.version.as_deref() == Some(version.title.as_str())
                || p.linked_issue.is_some_and(|n| numbers.contains(&n))
        })
        .cloned()
        .collect();
    link_pulls(&mut issues, &pulls);
    issues.sort_by(|a, b| {
        (a.status == IssueStatus::Done).cmp(&(b.status == IssueStatus::Done)).then(b.number.cmp(&a.number))
    });

    let index = versions.iter().position(|v| v.title == version.title);
    let neighbour = |i: Option<usize>| i.and_then(|i| versions.get(i)).map(|v| v.title.clone());
    VersionReport {
        current,
        previous: neighbour(index.and_then(|i| i.checked_sub(1))),
        next: neighbour(index.map(|i| i + 1)),
        sprint_days,
        tag: (version.state == VersionState::Closed).then(|| format!("{tag_prefix}{}", version.title)),
        issues,
        pulls,
        detail,
    }
}

/// PR liée à chaque issue (une PR ouverte avant une PR fusionnée) ; une PR ouverte dont la
/// CI échoue marque l'issue « CI en échec ».
pub fn link_pulls(issues: &mut [IssueRow], pulls: &[PrRow]) {
    let rank = |p: &PrRow| match p.state {
        PrState::Open | PrState::Draft => 0,
        PrState::Merged => 1,
        PrState::Closed => 2,
    };
    for issue in issues.iter_mut() {
        let Some(pr) = pulls.iter().filter(|p| p.linked_issue == Some(issue.number)).min_by_key(|p| rank(p))
        else {
            continue;
        };
        issue.linked_pr.get_or_insert(pr.number);
        if rank(pr) == 0 && pr.ci == CiState::Failure && !issue.flags.contains(&IssueFlag::CiFailed) {
            issue.flags.push(IssueFlag::CiFailed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::UserRef;

    fn version(title: &str, kind: VersionKind, state: VersionState, due: Option<&str>) -> Version {
        Version {
            title: title.into(),
            kind,
            state,
            milestone_number: 4,
            name: None,
            start_on: None,
            due_on: due.map(str::to_string),
            open_issues: 0,
            closed_issues: 0,
            description: None,
        }
    }

    fn issue(number: u32, status: IssueStatus) -> IssueRow {
        IssueRow {
            number,
            title: format!("Issue {number}"),
            version: Some("0.1.2".into()),
            issue_type: None,
            status,
            labels: vec![],
            assignee: None,
            estimate: None,
            linked_pr: None,
            flags: vec![],
            updated_at: "2026-09-16T10:00:00Z".into(),
        }
    }

    fn pr(number: u32, state: PrState, ci: CiState, linked: Option<u32>, version: Option<&str>) -> PrRow {
        PrRow {
            number,
            title: format!("PR {number}"),
            branch: "feat/x".into(),
            base: "main".into(),
            author: UserRef { login: "me".into(), name: None, avatar_url: None, is_self: true },
            state,
            ci,
            ci_passed: 0,
            ci_total: 0,
            review: None,
            approvals: 0,
            changes_requested: 0,
            review_requested_at: None,
            linked_issue: linked,
            version: version.map(str::to_string),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            comments: 0,
            created_at: "2026-09-10T10:00:00Z".into(),
            updated_at: "2026-09-12T10:00:00Z".into(),
        }
    }

    #[test]
    fn links_the_open_pull_request_first_and_flags_failed_ci() {
        let mut issues = vec![issue(140, IssueStatus::Review), issue(131, IssueStatus::Done)];
        let pulls = [
            pr(80, PrState::Merged, CiState::Success, Some(140), None),
            pr(85, PrState::Open, CiState::Failure, Some(140), None),
            pr(79, PrState::Merged, CiState::Failure, Some(131), None),
        ];
        link_pulls(&mut issues, &pulls);
        assert_eq!((issues[0].linked_pr, issues[0].flags.as_slice()), (Some(85), [IssueFlag::CiFailed].as_slice()));
        assert_eq!((issues[1].linked_pr, issues[1].flags.is_empty()), (Some(79), true));
    }

    #[test]
    fn builds_a_sprint_report_with_neighbours_and_related_pulls() {
        let versions = [
            version("0.1.1", VersionKind::Sprint, VersionState::Closed, Some("2026-09-09T07:00:00Z")),
            version("0.1.2", VersionKind::Sprint, VersionState::Open, Some("2026-09-23T07:00:00Z")),
            version("0.2.0", VersionKind::Palier, VersionState::Open, None),
        ];
        let mut sprint = versions[1].clone();
        sprint.start_on = Some("2026-09-09T07:00:00Z".into());
        let scope = vec![
            ScopeIssue { row: issue(131, IssueStatus::Done), closed_at: Some("2026-09-11T10:00:00Z".into()) },
            ScopeIssue { row: issue(138, IssueStatus::Review), closed_at: None },
            ScopeIssue { row: issue(156, IssueStatus::Todo), closed_at: None },
        ];
        let pulls = [
            pr(86, PrState::Open, CiState::Success, Some(138), None),
            pr(90, PrState::Merged, CiState::Success, None, Some("0.1.2")),
            pr(76, PrState::Merged, CiState::Success, Some(12), Some("0.1.1")),
        ];
        let report = build(&sprint, &versions, scope, &pulls, true, "v", "2026-09-13");
        assert_eq!(report.issues.iter().map(|i| i.number).collect::<Vec<_>>(), [156, 138, 131]);
        assert_eq!(report.pulls.iter().map(|p| p.number).collect::<Vec<_>>(), [86, 90]);
        assert_eq!(report.issues[1].linked_pr, Some(86));
        assert_eq!((report.previous.as_deref(), report.next.as_deref()), (Some("0.1.1"), Some("0.2.0")));
        assert_eq!((report.sprint_days, report.tag.as_deref()), (Some(14), None));
        assert_eq!((report.detail.progress, report.detail.burndown.len()), (33, 5));
        assert!(report.current);
    }

    #[test]
    fn a_shipped_version_has_its_tag_and_a_palier_no_sprint_length() {
        let versions = [
            version("0.1.1", VersionKind::Sprint, VersionState::Closed, Some("2026-09-09T07:00:00Z")),
            version("0.2.0", VersionKind::Palier, VersionState::Open, None),
        ];
        let shipped = build(&versions[0], &versions, vec![], &[], false, "v", "2026-10-15");
        assert_eq!((shipped.tag.as_deref(), shipped.previous, shipped.sprint_days), (Some("v0.1.1"), None, Some(14)));
        let palier = build(&versions[1], &versions, vec![], &[], false, "", "2026-10-15");
        assert_eq!((palier.sprint_days, palier.next, palier.tag), (None, None, None));
    }
}
