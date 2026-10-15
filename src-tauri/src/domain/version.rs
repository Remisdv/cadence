//! Versions : un sprint = un milestone daté, un palier = un milestone sans `due_on`.
//! Numérotation semver stricte, un incrément du 3e segment par sprint.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

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
    /// Brouillon de release avec les notes générées, si demandé et créé.
    pub release_url: Option<String>,
    /// Exécution de l'agent déploiement déclenchée, le cas échéant.
    pub deploy_run: Option<String>,
}

/// Règles de clôture tirées de `.cadence/config.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct CloseRules {
    pub sprint_days: i64,
    pub tag_prefix: String,
}

impl Default for CloseRules {
    fn default() -> Self {
        Self { sprint_days: 14, tag_prefix: "v".into() }
    }
}

/// Ce que la clôture va écrire, décidé avant toute écriture.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosePlan {
    pub current: String,
    pub current_milestone: u32,
    pub next: String,
    /// Milestone de la version suivante s'il existe déjà (palier planifié, sprint créé d'avance).
    pub next_milestone: Option<u32>,
    /// Échéance à poser sur la version suivante (création, ou palier qui devient sprint).
    pub next_due: Option<String>,
    /// Début affiché du sprint suivant (échéance de la version close, ou aujourd'hui).
    pub next_start: String,
    /// Issues encore ouvertes de la version close.
    pub carry: Vec<u32>,
    pub tag: String,
}

/// Prépare la clôture de `current` vers `input.next`. Refuse ce qui ne peut pas aboutir :
/// version suivante non semver, pas plus grande, déjà close ; version en cours introuvable.
pub fn plan_close(
    versions: &[Version],
    current: &str,
    input: &VersionCloseInput,
    open_issues: Vec<u32>,
    today: &str,
    rules: &CloseRules,
) -> AppResult<ClosePlan> {
    let next = parse(&input.next)
        .ok_or_else(|| AppError::Validation(format!("« {} » n'est pas une version semver.", input.next)))?
        .to_string();
    let cur = versions
        .iter()
        .find(|v| v.title == current && v.state == VersionState::Open)
        .ok_or_else(|| AppError::Validation(format!("La version {current} n'est pas ouverte sur GitHub.")))?;
    if compare(&next, current) != Ordering::Greater {
        return Err(AppError::Validation(format!("La version suivante doit être plus grande que {current}.")));
    }
    let existing = versions.iter().find(|v| v.title == next);
    if existing.is_some_and(|v| v.state == VersionState::Closed) {
        return Err(AppError::Validation(format!("La version {next} est déjà close sur GitHub.")));
    }

    let today_day = date_part(today);
    let start = match cur.due_on.as_deref().map(date_part) {
        Some(due) if due > today_day => due.to_string(),
        _ => today_day.to_string(),
    };
    let next_due = match existing {
        Some(v) if v.due_on.is_some() => None,
        _ => add_days(&start, rules.sprint_days).map(|d| format!("{d}T12:00:00Z")),
    };
    Ok(ClosePlan {
        current: current.to_string(),
        current_milestone: cur.milestone_number,
        tag: format!("{}{current}", rules.tag_prefix),
        next_milestone: existing.map(|v| v.milestone_number),
        next_due,
        next_start: start,
        next,
        carry: open_issues,
    })
}

/* ---------- Dates (jours civils, sans fuseau) ---------- */

fn date_part(iso: &str) -> &str {
    iso.get(..10).unwrap_or(iso)
}

/// Jours depuis le 1970-01-01 (algorithme de H. Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// `2026-09-23` + 14 → `2026-10-07`. `None` si la date est illisible.
pub fn add_days(date: &str, days: i64) -> Option<String> {
    let mut parts = date_part(date).split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    let (y, m, d) = civil_from_days(days_from_civil(y, m, d) + days);
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// Instant présent (UTC), `YYYY-MM-DDTHH:MM:SSZ`.
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", s / 3600, s % 3600 / 60, s % 60)
}

/// Date du jour (UTC), `YYYY-MM-DD`.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
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

    fn version(title: &str, number: u32, state: VersionState, due: Option<&str>) -> Version {
        Version {
            title: title.into(),
            kind: if due.is_some() { VersionKind::Sprint } else { VersionKind::Palier },
            state,
            milestone_number: number,
            name: None,
            start_on: None,
            due_on: due.map(str::to_string),
            open_issues: 0,
            closed_issues: 0,
            description: None,
        }
    }

    fn input(next: &str) -> VersionCloseInput {
        VersionCloseInput { next: next.into(), carry_over: CarryOver::MoveToNext, generate_notes: true }
    }

    #[test]
    fn plans_the_next_sprint_from_the_closing_due_date() {
        let versions = [version("0.1.2", 4, VersionState::Open, Some("2026-09-23T07:00:00Z"))];
        let plan = plan_close(&versions, "0.1.2", &input("v0.1.3"), vec![156, 160], "2026-09-20", &CloseRules::default()).unwrap();
        assert_eq!(plan.next, "0.1.3");
        assert_eq!((plan.current_milestone, plan.next_milestone), (4, None));
        assert_eq!(plan.next_start, "2026-09-23");
        assert_eq!(plan.next_due.as_deref(), Some("2026-10-07T12:00:00Z"));
        assert_eq!(plan.tag, "v0.1.2");
        assert_eq!(plan.carry, [156, 160]);
    }

    #[test]
    fn a_late_sprint_starts_today_and_a_planned_palier_gets_a_due_date() {
        let versions = [
            version("0.1.2", 4, VersionState::Open, Some("2026-09-23T07:00:00Z")),
            version("0.2.0", 6, VersionState::Open, None),
            version("0.1.3", 5, VersionState::Open, Some("2026-10-07T07:00:00Z")),
        ];
        let rules = CloseRules { sprint_days: 7, tag_prefix: String::new() };
        let palier = plan_close(&versions, "0.1.2", &input("0.2.0"), vec![], "2026-10-15T09:00:00Z", &rules).unwrap();
        assert_eq!((palier.next_milestone, palier.next_start.as_str()), (Some(6), "2026-10-15"));
        assert_eq!(palier.next_due.as_deref(), Some("2026-10-22T12:00:00Z"));
        assert_eq!(palier.tag, "0.1.2");
        let sprint = plan_close(&versions, "0.1.2", &input("0.1.3"), vec![], "2026-10-15", &CloseRules::default()).unwrap();
        assert_eq!((sprint.next_milestone, sprint.next_due), (Some(5), None));
    }

    #[test]
    fn refuses_what_cannot_be_closed() {
        let versions = [
            version("0.1.2", 4, VersionState::Open, Some("2026-09-23T07:00:00Z")),
            version("0.1.1", 3, VersionState::Closed, Some("2026-09-09T07:00:00Z")),
            version("0.1.5", 8, VersionState::Closed, None),
        ];
        for (current, next) in [("0.1.2", "0.1.1"), ("0.1.2", "0.1.2"), ("0.1.2", "abc"), ("0.1.1", "0.1.3"), ("0.1.2", "0.1.5")] {
            assert!(plan_close(&versions, current, &input(next), vec![], "2026-09-20", &CloseRules::default()).is_err(), "{current} → {next}");
        }
    }

    #[test]
    fn adds_days_across_months_and_leap_years() {
        assert_eq!(add_days("2026-09-23", 14).as_deref(), Some("2026-10-07"));
        assert_eq!(add_days("2028-02-20T07:00:00Z", 14).as_deref(), Some("2028-03-05"));
        assert_eq!(add_days("2026-12-25", 14).as_deref(), Some("2027-01-08"));
        assert_eq!(add_days("nope", 1), None);
        assert_eq!(today().len(), 10);
        assert!(now_iso().starts_with(&today()) && now_iso().ends_with('Z'));
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
