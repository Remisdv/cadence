//! Rythme d'un sprint : burndown jour par jour, vélocité, retard sur la droite idéale.
//!
//! Fonctions pures sur des jours civils `YYYY-MM-DD` (les instants ISO sont tronqués au
//! jour). La portée est celle d'aujourd'hui : une issue ajoutée en cours de sprint compte
//! dès le premier jour (GitHub ne date pas l'entrée d'une issue dans un milestone).

use super::version::{add_days, days_between, BurndownPoint, CloseRules, Version, VersionDetail, VersionKind};

/// Garde-fou : un sprint de plus d'un an est une donnée aberrante, pas un burndown.
const MAX_POINTS: i64 = 366;

fn day(iso: &str) -> &str {
    iso.get(..10).unwrap_or(iso)
}

/// Bornes effectives d'un sprint : (début, échéance, dernier jour observé).
/// `None` si les dates sont illisibles, l'échéance avant le début, ou le sprint pas commencé.
fn window<'a>(start: &'a str, end: &'a str, today: &'a str) -> Option<(&'a str, &'a str, &'a str)> {
    let (start, end, today) = (day(start), day(end), day(today));
    if days_between(start, end)? < 0 || days_between(start, today)? < 0 {
        return None;
    }
    Some((start, end, if today < end { today } else { end }))
}

/// Issues encore ouvertes à la fin du jour `on`. `closed_on` : jour de fermeture de chaque
/// issue de la portée, `None` si elle est ouverte.
pub fn remaining_on(closed_on: &[Option<&str>], on: &str) -> u32 {
    let on = day(on);
    closed_on.iter().filter(|c| c.is_none_or(|c| day(c) > on)).count() as u32
}

/// Burndown du début du sprint jusqu'à aujourd'hui (ou l'échéance si elle est passée),
/// un point par jour. Vide si le sprint n'a pas commencé.
pub fn burndown(closed_on: &[Option<&str>], start: &str, end: &str, today: &str) -> Vec<BurndownPoint> {
    let Some((start, _, last)) = window(start, end, today) else { return vec![] };
    let days = days_between(start, last).unwrap_or(0).min(MAX_POINTS);
    (0..=days)
        .filter_map(|i| add_days(start, i))
        .map(|date| BurndownPoint { remaining: remaining_on(closed_on, &date), date })
        .collect()
}

/// Issues fermées par jour écoulé depuis le début du sprint (le jour même compte pour un).
pub fn velocity(closed_on: &[Option<&str>], start: &str, end: &str, today: &str) -> f32 {
    let Some((start, _, last)) = window(start, end, today) else { return 0.0 };
    let closed = closed_on.iter().flatten().map(|c| day(c)).filter(|c| *c >= start && *c <= last).count();
    let elapsed = days_between(start, last).unwrap_or(0).max(1);
    round1(closed as f32 / elapsed as f32)
}

/// Retard (positif) ou avance (négatif) en jours sur la droite idéale, qui va de `scope`
/// issues au début à zéro à l'échéance : écart entre aujourd'hui et le jour où la droite
/// passe par le reste actuel.
pub fn delay_days(scope: u32, remaining: u32, start: &str, end: &str, today: &str) -> f32 {
    let Some((start, end, last)) = window(start, end, today) else { return 0.0 };
    let length = days_between(start, end).unwrap_or(0);
    if scope == 0 || length == 0 {
        return 0.0;
    }
    let elapsed = days_between(start, last).unwrap_or(0) as f32;
    let on_track_at = length as f32 * (1.0 - remaining.min(scope) as f32 / scope as f32);
    round1(elapsed - on_track_at)
}

/// Jours restants avant l'échéance (négatif si elle est dépassée).
pub fn days_left(due: Option<&str>, today: &str) -> Option<i32> {
    days_between(today, due?).map(|d| d as i32)
}

/// Avancement 0 → 100, arrondi.
pub fn progress(closed: u32, total: u32) -> u8 {
    if total == 0 {
        return 0;
    }
    ((closed.min(total) as f32 / total as f32) * 100.0).round() as u8
}

/// Début d'un sprint : l'échéance du précédent ; à défaut (premier sprint), l'échéance
/// moins la durée par défaut d'un sprint.
pub fn sprint_start(version: &Version) -> Option<String> {
    version.start_on.clone().or_else(|| {
        let due = version.due_on.as_deref()?;
        add_days(due, -CloseRules::default().sprint_days)
    })
}

/// Rythme d'une version. `closed_on` couvre les issues en cache de la version ; s'il n'y en
/// a aucune, les compteurs du milestone donnent l'avancement. Un palier n'a ni échéance ni
/// burndown.
pub fn version_detail(version: &Version, closed_on: &[Option<&str>], today: &str) -> VersionDetail {
    let (total, closed) = if closed_on.is_empty() {
        (version.open_issues + version.closed_issues, version.closed_issues)
    } else {
        (closed_on.len() as u32, closed_on.iter().filter(|c| c.is_some()).count() as u32)
    };
    let mut version = version.clone();
    let mut detail = VersionDetail {
        days_left: None,
        progress: progress(closed, total),
        velocity: 0.0,
        delay_days: 0.0,
        burndown: vec![],
        version: version.clone(),
    };
    let (VersionKind::Sprint, Some(due)) = (version.kind, version.due_on.clone()) else { return detail };
    let Some(start) = sprint_start(&version) else { return detail };
    version.start_on = Some(start.clone());
    detail.days_left = days_left(Some(&due), today);
    detail.burndown = burndown(closed_on, &start, &due, today);
    detail.velocity = velocity(closed_on, &start, &due, today);
    let last = if day(today) < day(&due) { today } else { &due };
    detail.delay_days = delay_days(total, remaining_on(closed_on, last), &start, &due, today);
    detail.version = version;
    detail
}

fn round1(x: f32) -> f32 {
    (x * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::version::VersionState;

    /// 6 issues : 2 fermées le 10, 1 le 12, 1 avant le sprint, 2 ouvertes.
    const SCOPE: [Option<&str>; 6] = [
        Some("2026-09-10T15:00:00Z"),
        Some("2026-09-10T09:00:00Z"),
        Some("2026-09-12T23:59:00Z"),
        Some("2026-09-01T10:00:00Z"),
        None,
        None,
    ];

    #[test]
    fn burns_down_day_by_day_until_today() {
        let points = burndown(&SCOPE, "2026-09-09T07:00:00Z", "2026-09-23T07:00:00Z", "2026-09-13T10:00:00Z");
        let remaining: Vec<(String, u32)> = points.into_iter().map(|p| (p.date, p.remaining)).collect();
        assert_eq!(
            remaining,
            [
                ("2026-09-09".to_string(), 5),
                ("2026-09-10".to_string(), 3),
                ("2026-09-11".to_string(), 3),
                ("2026-09-12".to_string(), 2),
                ("2026-09-13".to_string(), 2),
            ]
        );
    }

    #[test]
    fn stops_at_the_due_date_and_waits_for_the_start() {
        assert_eq!(burndown(&SCOPE, "2026-09-09", "2026-09-23", "2026-10-15").len(), 15);
        assert!(burndown(&SCOPE, "2026-09-09", "2026-09-23", "2026-09-01").is_empty());
        assert!(burndown(&SCOPE, "2026-09-23", "2026-09-09", "2026-09-15").is_empty());
        assert!(burndown(&SCOPE, "nope", "2026-09-23", "2026-09-15").is_empty());
    }

    #[test]
    fn velocity_counts_closures_inside_the_sprint_per_elapsed_day() {
        // 3 fermées dans le sprint (celle du 1er septembre est hors sprint), 4 jours écoulés.
        assert_eq!(velocity(&SCOPE, "2026-09-09", "2026-09-23", "2026-09-13"), 0.8);
        // Premier jour : compte pour un jour.
        assert_eq!(velocity(&[Some("2026-09-09")], "2026-09-09", "2026-09-23", "2026-09-09"), 1.0);
        assert_eq!(velocity(&SCOPE, "2026-09-09", "2026-09-23", "2026-09-01"), 0.0);
    }

    #[test]
    fn delay_is_measured_against_the_ideal_line() {
        // 14 jours, 14 issues : la droite perd une issue par jour.
        assert_eq!(delay_days(14, 7, "2026-09-09", "2026-09-23", "2026-09-16"), 0.0);
        assert_eq!(delay_days(14, 9, "2026-09-09", "2026-09-23", "2026-09-16"), 2.0);
        assert_eq!(delay_days(14, 4, "2026-09-09", "2026-09-23", "2026-09-16"), -3.0);
        // Échéance passée : le retard est figé à l'échéance.
        assert_eq!(delay_days(14, 2, "2026-09-09", "2026-09-23", "2026-10-15"), 2.0);
        assert_eq!(delay_days(0, 0, "2026-09-09", "2026-09-23", "2026-09-16"), 0.0);
    }

    #[test]
    fn days_left_and_progress() {
        assert_eq!(days_left(Some("2026-09-23T07:00:00Z"), "2026-09-17T22:00:00Z"), Some(6));
        assert_eq!(days_left(Some("2026-09-23"), "2026-09-25"), Some(-2));
        assert_eq!(days_left(None, "2026-09-25"), None);
        assert_eq!(progress(11, 18), 61);
        assert_eq!(progress(0, 0), 0);
        assert_eq!(progress(5, 3), 100);
    }

    fn version(kind: VersionKind, start: Option<&str>, due: Option<&str>) -> Version {
        Version {
            title: "0.1.2".into(),
            kind,
            state: VersionState::Open,
            milestone_number: 4,
            name: None,
            start_on: start.map(str::to_string),
            due_on: due.map(str::to_string),
            open_issues: 7,
            closed_issues: 11,
            description: None,
        }
    }

    #[test]
    fn assembles_a_sprint_detail() {
        let v = version(VersionKind::Sprint, Some("2026-09-09T07:00:00Z"), Some("2026-09-23T07:00:00Z"));
        let d = version_detail(&v, &SCOPE, "2026-09-13");
        assert_eq!((d.days_left, d.progress, d.velocity), (Some(10), 67, 0.8));
        assert_eq!(d.burndown.len(), 5);
        // 14 j, 6 issues, 2 restantes au jour 4 : la droite y passe au jour 9,3.
        assert_eq!(d.delay_days, -5.3);
    }

    #[test]
    fn first_sprint_starts_one_default_sprint_before_its_due_date() {
        let v = version(VersionKind::Sprint, None, Some("2026-09-23T07:00:00Z"));
        let d = version_detail(&v, &SCOPE, "2026-09-10");
        assert_eq!(d.version.start_on.as_deref(), Some("2026-09-09"));
        assert_eq!(d.burndown.len(), 2);
    }

    #[test]
    fn a_palier_has_only_its_progress() {
        let v = version(VersionKind::Palier, None, None);
        let d = version_detail(&v, &[], "2026-09-13");
        assert_eq!((d.progress, d.days_left, d.burndown.len()), (61, None, 0));
        assert_eq!((d.velocity, d.delay_days), (0.0, 0.0));
    }
}
