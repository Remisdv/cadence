//! Prompt d'une action de plugin (plan § 3.5) : le même pour toutes les IA, seule l'invocation
//! du skill change (§ 4) — commande `/cadence:<plugin>-<skill>` pour Claude Code, mention
//! `$<plugin>-<skill>` pour Codex, outil `skill` pour OpenCode.

use serde_json::Value;

use crate::app_settings::AiId;

/// Dernière ligne d'une exécution de fond : personne ne peut répondre.
pub const BACKGROUND_LINE: &str = "Mode : tâche de fond — personne ne peut te répondre ; ne pose pas de question, \
appelle `cadence__run_report` pour la progression et le résultat final (`final: true`).";

pub fn action_prompt(ai: AiId, plugin: &str, skill: &str, label: &str, context: Option<&Value>, background: bool) -> String {
    let name = format!("{plugin}-{skill}");
    let mut lines = vec![match ai {
        AiId::ClaudeCode => format!("/cadence:{name}"),
        AiId::Codex => format!("${name} — suis le skill `{name}` pour cette tâche."),
        AiId::Opencode => format!("Charge le skill `{name}` avec l'outil `skill`, puis suis-le pour cette tâche."),
    }];
    lines.push(format!("Action : {label}"));
    if let Some(context) = context.filter(|c| !c.is_null()) {
        let pretty = serde_json::to_string_pretty(context).unwrap_or_else(|_| context.to_string());
        lines.push(format!("Contexte :\n```json\n{pretty}\n```"));
    }
    if background {
        lines.push(BACKGROUND_LINE.to_string());
    }
    lines.join("\n\n")
}

/// Titre d'une exécution : le libellé, suivi d'un repère court du contexte (« #142 »…).
pub fn action_title(label: &str, context: Option<&Value>) -> String {
    let hint = context.and_then(|c| {
        ["number", "title", "name", "id"].iter().find_map(|k| match c.get(k)? {
            Value::Number(n) => Some(format!("#{n}")),
            Value::String(s) if !s.is_empty() => Some(s.chars().take(40).collect()),
            _ => None,
        })
    });
    match hint {
        Some(hint) => format!("{label} · {hint}"),
        None => label.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn invokes_the_skill_the_way_each_ai_expects() {
        let context = json!({ "type": "issue", "number": 142 });
        let claude = action_prompt(AiId::ClaudeCode, "gestion-projet", "preparer-issue", "Préparer l'issue", Some(&context), false);
        assert!(claude.starts_with("/cadence:gestion-projet-preparer-issue\n\nAction : Préparer l'issue"));
        assert!(claude.contains("\"number\": 142"));
        assert!(!claude.contains("tâche de fond"));
        let codex = action_prompt(AiId::Codex, "documents", "qcm", "Générer un QCM", None, true);
        assert!(codex.starts_with("$documents-qcm"));
        assert!(codex.ends_with(BACKGROUND_LINE));
        assert!(action_prompt(AiId::Opencode, "a", "b", "C", None, false).contains("outil `skill`"));
        assert_eq!(action_title("Préparer l'issue", Some(&context)), "Préparer l'issue · #142");
        assert_eq!(action_title("Flashcards", None), "Flashcards");
    }
}
