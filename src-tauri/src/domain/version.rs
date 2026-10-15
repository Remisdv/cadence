//! Versions : un sprint = un milestone daté, un palier = un milestone sans `due_on`.
//! Numérotation semver stricte, un incrément du 3e segment par sprint.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum VersionKind {
    /// Milestone daté.
    Sprint,
    /// Milestone ouvert sans date : un jalon de roadmap (0.2.0, 1.0.0).
    Palier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum VersionState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Version {
    /// Titre du milestone, semver sans préfixe : `0.1.2`.
    pub title: String,
    pub kind: VersionKind,
    pub state: VersionState,
    pub milestone_number: u32,
    /// Libellé du sprint, ex. « Cockpit Tauri ».
    pub name: Option<String>,
    pub start_on: Option<String>,
    pub due_on: Option<String>,
    pub open_issues: u32,
    pub closed_issues: u32,
    /// Thèmes du palier (markdown), lus par l'agent CEO.
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BurndownPoint {
    pub date: String,
    pub remaining: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VersionDetail {
    pub version: Version,
    pub days_left: Option<i32>,
    /// Avancement 0 → 100.
    pub progress: u8,
    /// Issues fermées par jour sur le sprint.
    pub velocity: f32,
    /// Retard (positif) ou avance (négatif) en jours par rapport à la droite idéale.
    pub delay_days: f32,
    pub burndown: Vec<BurndownPoint>,
}

/// Que faire des issues encore ouvertes à la clôture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum CarryOver {
    MoveToNext,
    Unversioned,
    CloseAsNotPlanned,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VersionCloseInput {
    /// Version suivante (semver), ex. `0.1.3`.
    pub next: String,
    pub carry_over: CarryOver,
    pub generate_notes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CloseReport {
    pub closed: String,
    pub opened: String,
    pub moved_issues: u32,
    pub tag: String,
    /// Exécution de l'agent déploiement déclenchée, le cas échéant.
    pub deploy_run: Option<String>,
}

/* ---------- Logique semver ---------- */

/// Parse un titre de milestone (`0.1.2` ou `v0.1.2`).
pub fn parse(title: &str) -> Option<semver::Version> {
    semver::Version::parse(title.trim().trim_start_matches('v')).ok()
}

/// Ordre des versions : semver croissant ; les titres non semver passent en dernier, par ordre alpha.
pub fn compare(a: &str, b: &str) -> Ordering {
    match (parse(a), parse(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

/// Candidats proposés à la clôture : sprint suivant, nouveau palier, version majeure.
pub fn next_candidates(current: &str) -> Option<[String; 3]> {
    let v = parse(current)?;
    Some([
        semver::Version::new(v.major, v.minor, v.patch + 1).to_string(),
        semver::Version::new(v.major, v.minor + 1, 0).to_string(),
        semver::Version::new(v.major + 1, 0, 0).to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_semver_not_lexically() {
        let mut v = vec!["0.11.0", "0.2.0", "0.1.10", "0.1.2", "1.0.0", "roadmap"];
        v.sort_by(|a, b| compare(a, b));
        assert_eq!(v, ["0.1.2", "0.1.10", "0.2.0", "0.11.0", "1.0.0", "roadmap"]);
    }

    #[test]
    fn accepts_tag_prefix() {
        assert_eq!(parse("v0.1.2"), parse("0.1.2"));
    }

    #[test]
    fn proposes_patch_minor_major() {
        assert_eq!(
            next_candidates("0.1.2").unwrap(),
            ["0.1.3".to_string(), "0.2.0".to_string(), "1.0.0".to_string()]
        );
        assert!(next_candidates("sprint-12").is_none());
    }
}
