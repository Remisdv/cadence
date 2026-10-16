//! Lecture de la roadmap : thèmes d'un palier et palier visé par défaut.
//!
//! Logique pure reprise de l'ancien agent CEO ; elle deviendra l'outil `roadmap_context` du
//! plugin `gestion-projet` (plan Cadence 2, § 7.1).

use super::version::{self, Version, VersionKind, VersionState};

/// Thèmes d'un palier : les éléments de liste de la description du milestone.
pub fn themes_of(description: &str) -> Vec<String> {
    description
        .lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let rest = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")).or_else(|| l.strip_prefix("+ "))?;
            let rest = rest.trim_start_matches("[ ] ").trim_start_matches("[x] ").trim();
            (!rest.is_empty()).then(|| rest.to_string())
        })
        .collect()
}

/// Palier visé par défaut : le premier palier ouvert après la version en cours ; à défaut,
/// la première version ouverte après elle. Les versions sont supposées triées.
pub fn next_target(versions: &[Version], current: Option<&str>) -> Option<String> {
    let after = |v: &&Version| {
        v.state == VersionState::Open
            && current.is_none_or(|c| version::compare(&v.title, c) == std::cmp::Ordering::Greater)
    };
    versions
        .iter()
        .filter(after)
        .find(|v| v.kind == VersionKind::Palier)
        .or_else(|| versions.iter().find(after))
        .map(|v| v.title.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(title: &str, kind: VersionKind, state: VersionState, description: Option<&str>) -> Version {
        Version {
            title: title.into(),
            kind,
            state,
            milestone_number: 1,
            name: Some(format!("Nom {title}")),
            start_on: None,
            due_on: (kind == VersionKind::Sprint).then(|| "2026-10-07T07:00:00Z".into()),
            open_issues: 0,
            closed_issues: 4,
            description: description.map(str::to_string),
        }
    }

    fn versions() -> Vec<Version> {
        vec![
            version("0.1.1", VersionKind::Sprint, VersionState::Closed, None),
            version("0.1.2", VersionKind::Sprint, VersionState::Open, None),
            version("0.1.3", VersionKind::Sprint, VersionState::Open, None),
            version("0.2.0", VersionKind::Palier, VersionState::Open, Some("# Travail à plusieurs\n- Rôles et permissions\n- Revue assistée\n* Multi-dépôts")),
            version("0.2.1", VersionKind::Palier, VersionState::Open, Some("- Notifications")),
        ]
    }

    #[test]
    fn reads_themes_and_picks_the_next_palier() {
        assert_eq!(themes_of("# Titre\n- Rôles\n  * Revue\n+ Multi\ntexte\n- [ ] Coché"), ["Rôles", "Revue", "Multi", "Coché"]);
        let v = versions();
        assert_eq!(next_target(&v, Some("0.1.2")).as_deref(), Some("0.2.0"));
        assert_eq!(next_target(&v, Some("0.2.0")).as_deref(), Some("0.2.1"));
        assert_eq!(next_target(&v, None).as_deref(), Some("0.2.0"));
        // Aucun palier après : la prochaine version ouverte.
        assert_eq!(next_target(&v[..3], Some("0.1.2")).as_deref(), Some("0.1.3"));
        assert_eq!(next_target(&v[..3], Some("0.1.3")), None);
    }
}
