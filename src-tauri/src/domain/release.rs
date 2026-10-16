//! Livraison d'une version : critères d'acceptation, notes de version, version suivante.
//!
//! Logique pure reprise de l'ancien agent déploiement ; elle deviendra les outils
//! `acceptance_criteria` et `release_notes_draft` du plugin `gestion-projet` (plan § 7.1).

use super::{
    issue::IssueType,
    version::{self, Version, VersionState},
};

/// Critères d'acceptation d'une issue : cases à cocher (`- [ ]`, `- [x]`) de la section
/// « Critères d'acceptation » (titre markdown ou ligne en gras) ; sans cette section, toutes
/// les cases de la description.
pub fn acceptance_criteria(body: &str) -> Vec<String> {
    let (mut section, mut all) = (Vec::new(), Vec::new());
    let (mut inside, mut found) = (false, false);
    for line in body.lines().map(str::trim) {
        if is_heading(line) {
            inside = is_criteria_heading(line);
            found |= inside;
            continue;
        }
        if let Some(c) = checkbox(line) {
            if inside {
                section.push(c.clone());
            }
            all.push(c);
        }
    }
    if found { section } else { all }
}

fn is_heading(line: &str) -> bool {
    line.starts_with('#') || (line.len() > 4 && line.starts_with("**") && line.ends_with("**"))
}

fn is_criteria_heading(line: &str) -> bool {
    let l = line.to_lowercase().replace('’', "'");
    ["critères d'acceptation", "criteres d'acceptation", "critère d'acceptation", "acceptance criteria"]
        .iter()
        .any(|k| l.contains(k))
}

fn checkbox(line: &str) -> Option<String> {
    let rest = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")).or_else(|| line.strip_prefix("+ "))?;
    let rest = rest.trim_start();
    let text = ["[ ]", "[x]", "[X]"].iter().find_map(|b| rest.strip_prefix(b))?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Issue livrée, telle qu'elle figure dans les notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteItem {
    pub number: u32,
    pub title: String,
    pub issue_type: Option<IssueType>,
}

const SECTIONS: [(&str, &[Option<IssueType>]); 5] = [
    ("Nouveautés", &[Some(IssueType::Feat)]),
    ("Corrections", &[Some(IssueType::Fix)]),
    ("Performances", &[Some(IssueType::Perf)]),
    ("Documentation", &[Some(IssueType::Docs)]),
    ("Maintenance", &[Some(IssueType::Chore), None]),
];

/// Notes de version en markdown, regroupées par type d'issue.
pub fn release_notes(tag: &str, name: Option<&str>, items: &[NoteItem]) -> String {
    let mut out = match name.filter(|n| !n.trim().is_empty()) {
        Some(name) => format!("## {tag} — {}\n", name.trim()),
        None => format!("## {tag}\n"),
    };
    let mut any = false;
    for (title, types) in SECTIONS {
        let lines: Vec<String> = items
            .iter()
            .filter(|i| types.contains(&i.issue_type))
            .map(|i| format!("- {} (#{})", i.title.trim(), i.number))
            .collect();
        if !lines.is_empty() {
            any = true;
            out.push_str(&format!("\n### {title}\n{}\n", lines.join("\n")));
        }
    }
    if !any {
        out.push_str("\nAucune issue livrée sur cette version.\n");
    }
    out
}

/// Version où rouvrir une issue en échec : la plus petite version ouverte au-delà de `closed`.
pub fn next_version(versions: &[Version], closed: &str) -> Option<String> {
    let after = version::parse(closed)?;
    versions
        .iter()
        .filter(|v| v.state == VersionState::Open)
        .filter_map(|v| version::parse(&v.title).filter(|s| *s > after).map(|s| (s, v.title.clone())))
        .min_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, title)| title)
}

/// Dernière version close (version à livrer par défaut).
pub fn latest_closed(versions: &[Version]) -> Option<String> {
    versions
        .iter()
        .filter(|v| v.state == VersionState::Closed)
        .filter_map(|v| version::parse(&v.title).map(|s| (s, v.title.clone())))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, title)| title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::version::VersionKind;

    fn version(title: &str, state: VersionState) -> Version {
        Version {
            title: title.into(),
            kind: VersionKind::Sprint,
            state,
            milestone_number: 1,
            name: None,
            start_on: None,
            due_on: None,
            open_issues: 0,
            closed_issues: 0,
            description: None,
        }
    }

    fn item(number: u32, issue_type: Option<IssueType>) -> NoteItem {
        NoteItem { number, title: format!("Issue {number}"), issue_type }
    }

    #[test]
    fn extracts_acceptance_criteria_from_their_section() {
        let body = "## Contexte\n- [ ] pas un critère\n\n## Critères d’acceptation\n- [ ] Le raccourci ouvre la palette\n* [x] Échap la ferme\n- [ ]   \n- texte libre\n\n## Notes\n- [ ] hors section";
        assert_eq!(acceptance_criteria(body), ["Le raccourci ouvre la palette", "Échap la ferme"]);
        let bold = "**Acceptance criteria**\n- [ ] works";
        assert_eq!(acceptance_criteria(bold), ["works"]);
    }

    #[test]
    fn falls_back_to_every_checkbox_without_a_section() {
        assert_eq!(acceptance_criteria("Faire :\n- [ ] a\n- [X] b"), ["a", "b"]);
        assert!(acceptance_criteria("## Critères d'acceptation\nÀ définir.").is_empty());
        assert!(acceptance_criteria("").is_empty());
    }

    #[test]
    fn release_notes_group_by_type() {
        let items = vec![
            item(127, Some(IssueType::Feat)),
            item(120, Some(IssueType::Fix)),
            item(110, None),
        ];
        assert_eq!(
            release_notes("v0.1.1", Some("Lecture du projet"), &items),
            "## v0.1.1 — Lecture du projet\n\n### Nouveautés\n- Issue 127 (#127)\n\n### Corrections\n- Issue 120 (#120)\n\n### Maintenance\n- Issue 110 (#110)\n"
        );
        assert!(release_notes("v0.1.1", None, &[]).contains("Aucune issue livrée"));
    }

    #[test]
    fn picks_the_next_open_version_and_the_latest_closed_one() {
        let versions = vec![
            version("0.1.0", VersionState::Closed),
            version("0.1.1", VersionState::Closed),
            version("0.2.0", VersionState::Open),
            version("0.1.2", VersionState::Open),
        ];
        assert_eq!(next_version(&versions, "0.1.1").as_deref(), Some("0.1.2"));
        assert_eq!(next_version(&versions, "0.2.0"), None);
        assert_eq!(latest_closed(&versions).as_deref(), Some("0.1.1"));
    }
}
