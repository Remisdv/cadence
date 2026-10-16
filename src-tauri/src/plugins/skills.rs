//! Skills d'un plugin : `skills/<skill>/SKILL.md` au format Agent Skills (plan § 3.1).
//!
//! Le frontmatter YAML doit donner `name` (égal au nom du dossier, `[a-z0-9-]{1,64}`) et
//! `description` (1 à 1024 caractères). Seul ce sous-ensemble du YAML est lu : clés de premier
//! niveau `clé: valeur`, valeurs entre guillemets, blocs `|` et `>` ; les autres clés sont
//! ignorées. Les fichiers annexes du dossier sont libres.

use std::{fs, path::Path};

use crate::error::{AppError, AppResult};

pub const FILE_NAME: &str = "SKILL.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
}

/// `[a-z0-9-]{1,64}`.
pub fn is_skill_name(name: &str) -> bool {
    (1..=64).contains(&name.len()) && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::Validation(message.into())
}

/// Tous les skills de `dir` (`skills/`), triés par nom ; dossier absent = aucun skill.
pub fn load_all(dir: &Path) -> AppResult<Vec<SkillInfo>> {
    let Ok(entries) = fs::read_dir(dir) else { return Ok(Vec::new()) };
    let mut skills = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let folder = entry.file_name().to_string_lossy().into_owned();
        if !is_skill_name(&folder) {
            return Err(invalid(format!("Skill « {folder} » : nom de dossier invalide ([a-z0-9-], 64 caractères au plus).")));
        }
        let text = fs::read_to_string(entry.path().join(FILE_NAME))
            .map_err(|_| invalid(format!("Skill « {folder} » : `{FILE_NAME}` absent ou illisible.")))?;
        let skill = parse(&text).map_err(|why| invalid(format!("Skill « {folder} » : {why}")))?;
        if skill.name != folder {
            return Err(invalid(format!("Skill « {folder} » : `name` (« {} ») doit être le nom du dossier.", skill.name)));
        }
        skills.push(skill);
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

/// Lit le frontmatter d'un `SKILL.md`.
pub fn parse(text: &str) -> Result<SkillInfo, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Err("frontmatter YAML absent (le fichier doit commencer par `---`).".into());
    }
    let mut front = Vec::new();
    let mut closed = false;
    for line in lines {
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
        front.push(line);
    }
    if !closed {
        return Err("frontmatter non refermé par `---`.".into());
    }

    let name = value(&front, "name").ok_or("`name` absent du frontmatter.")?;
    if !is_skill_name(&name) {
        return Err(format!("`name` « {name} » invalide ([a-z0-9-], 64 caractères au plus)."));
    }
    let description = value(&front, "description").filter(|d| !d.is_empty()).ok_or("`description` absente du frontmatter.")?;
    if description.chars().count() > 1024 {
        return Err("`description` dépasse 1024 caractères.".into());
    }
    Ok(SkillInfo { name, description })
}

/// Valeur d'une clé de premier niveau.
fn value(front: &[&str], key: &str) -> Option<String> {
    let index = front.iter().position(|line| {
        line.strip_prefix(key).is_some_and(|rest| rest.starts_with(':')) && !line.starts_with(char::is_whitespace)
    })?;
    let raw = front[index][key.len() + 1..].trim();
    // Lignes indentées qui suivent : bloc `|` / `>` ou valeur simple sur plusieurs lignes.
    let continuation: Vec<&str> = front[index + 1..]
        .iter()
        .take_while(|l| l.trim().is_empty() || l.starts_with(char::is_whitespace))
        .map(|l| l.trim())
        .collect();
    let joined = |sep: &str| continuation.join(sep).trim().to_string();
    Some(match raw.chars().next() {
        Some('|') => joined("\n"),
        Some('>') => joined(" "),
        Some(q @ ('"' | '\'')) if raw.len() >= 2 && raw.ends_with(q) => {
            let inner = &raw[1..raw.len() - 1];
            if q == '"' { inner.replace("\\\"", "\"") } else { inner.replace("''", "'") }
        }
        _ if raw.is_empty() => joined(" "),
        _ => {
            let mut parts = vec![raw];
            parts.extend(continuation.iter().copied().filter(|l| !l.is_empty()));
            parts.join(" ")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_frontmatter_subset() {
        let simple = "---\nname: preparer-issue\ndescription: Prépare une issue avant le développement.\nallowed-tools: []\n---\n# Corps\n";
        assert_eq!(
            parse(simple).unwrap(),
            SkillInfo { name: "preparer-issue".into(), description: "Prépare une issue avant le développement.".into() }
        );
        let quoted = "\u{feff}---\r\nname: \"qcm\"\r\ndescription: 'Génère un QCM : l''IA cite ses sources.'\r\n---\r\n";
        assert_eq!(parse(quoted).unwrap().description, "Génère un QCM : l'IA cite ses sources.");
        let folded = "---\nname: revue\ndescription: >\n  Relit une pull request\n  et propose des commentaires.\nmetadata:\n  version: 1\n---\n";
        assert_eq!(parse(folded).unwrap().description, "Relit une pull request et propose des commentaires.");
        let literal = "---\nname: revue\ndescription: |\n  Ligne 1\n  Ligne 2\n---\n";
        assert_eq!(parse(literal).unwrap().description, "Ligne 1\nLigne 2");
    }

    #[test]
    fn rejects_broken_frontmatters() {
        for (text, why) in [
            ("# Pas de frontmatter\n", "absent"),
            ("---\nname: a\ndescription: b\n", "non refermé"),
            ("---\ndescription: b\n---\n", "sans nom"),
            ("---\nname: A_b\ndescription: b\n---\n", "nom invalide"),
            ("---\nname: a\n---\n", "sans description"),
            ("---\nname: a\ndescription: \"\"\n---\n", "description vide"),
        ] {
            assert!(parse(text).is_err(), "{why}");
        }
        let long = format!("---\nname: a\ndescription: {}\n---\n", "x".repeat(1025));
        assert!(parse(&long).is_err());
        // Une clé indentée n'est pas une clé de premier niveau.
        assert!(parse("---\nmetadata:\n  name: a\ndescription: b\n---\n").is_err());
    }

    #[test]
    fn loads_skill_folders() {
        let dir = crate::app_settings::temp_dir("plugin-skills");
        let write = |folder: &str, text: &str| {
            fs::create_dir_all(dir.join(folder)).unwrap();
            fs::write(dir.join(folder).join(FILE_NAME), text).unwrap();
        };
        assert!(load_all(&dir).unwrap().is_empty(), "dossier absent : aucun skill");
        write("resumer", "---\nname: resumer\ndescription: Résume un chapitre.\n---\n");
        write("cartes", "---\nname: cartes\ndescription: Flashcards.\n---\n");
        fs::write(dir.join("LISEZMOI.md"), "fichier libre").unwrap();
        let names: Vec<String> = load_all(&dir).unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["cartes", "resumer"]);

        write("autre", "---\nname: pas-le-dossier\ndescription: x\n---\n");
        assert!(load_all(&dir).is_err(), "name différent du dossier");
        fs::remove_dir_all(dir.join("autre")).unwrap();
        fs::create_dir_all(dir.join("vide")).unwrap();
        assert!(load_all(&dir).is_err(), "SKILL.md absent");
        let _ = fs::remove_dir_all(&dir);
    }
}
