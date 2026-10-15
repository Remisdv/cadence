//! Opérations Git locales (prévu : `git2`) : branches, worktrees, commits, push.
//!
//! L'agent développeur travaille dans un worktree isolé par issue, sous
//! `~/.cadence/wt/<num>` ; le dépôt principal de l'utilisateur n'est jamais modifié.

pub mod checkout;
pub mod worktree;

use crate::domain::issue::IssueType;

const MAX_SLUG_LEN: usize = 40;

/// Nom de branche conventionné : `<type>/<num>-<slug>`, ex. `feat/142-github-device-flow`.
pub fn branch_name(issue_type: IssueType, number: u32, title: &str) -> String {
    format!("{}/{}-{}", issue_type.slug(), number, slugify(title))
}

/// Minuscules ASCII, accents retirés, séparateurs réduits à un tiret, tronqué sur un mot.
pub fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    for c in text.chars().flat_map(fold_accent) {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.len() <= MAX_SLUG_LEN {
        return slug.to_string();
    }
    let cut = &slug[..MAX_SLUG_LEN];
    cut.rsplit_once('-').map_or(cut, |(head, _)| head).to_string()
}

fn fold_accent(c: char) -> Vec<char> {
    let folded = match c {
        'à' | 'â' | 'ä' | 'á' | 'ã' | 'À' | 'Â' | 'Ä' => "a",
        'ç' | 'Ç' => "c",
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => "e",
        'î' | 'ï' | 'í' | 'Î' | 'Ï' => "i",
        'ô' | 'ö' | 'ó' | 'õ' | 'Ô' | 'Ö' => "o",
        'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' => "u",
        'ÿ' => "y",
        'œ' | 'Œ' => "oe",
        'æ' | 'Æ' => "ae",
        _ => return vec![c],
    };
    folded.chars().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_conventional_branch_names() {
        assert_eq!(
            branch_name(IssueType::Feat, 142, "Device flow OAuth GitHub + stockage trousseau"),
            "feat/142-device-flow-oauth-github-stockage"
        );
        assert_eq!(
            branch_name(IssueType::Fix, 156, "Message d'erreur lisible quand le quota API est atteint"),
            "fix/156-message-d-erreur-lisible-quand-le-quota"
        );
    }

    #[test]
    fn folds_accents_and_trims_separators() {
        assert_eq!(slugify("  Clôture de la version — 0.1.2 !"), "cloture-de-la-version-0-1-2");
        assert_eq!(slugify("Œuvre"), "oeuvre");
    }
}
