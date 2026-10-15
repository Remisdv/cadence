//! Agent CTO : rendre les issues exécutables.
//!
//! Une session par issue : il lit le titre, la description et le code du dépôt (lecture
//! seule), puis rend une description selon le modèle du projet, une estimation, des labels
//! et, quand le titre ne suffit pas à trancher, des questions plutôt qu'une invention.
//! Vos réponses sont renvoyées dans la même session, qui complète la description.

use serde::Deserialize;

use crate::{
    domain::{
        agent::{CtoItem, CtoItemState},
        issue::CtoQuestion,
    },
    error::{AppError, AppResult},
};

/// Consignes de l'agent, placées en tête de la première requête de chaque session.
pub fn system_prompt(template: &str, has_code: bool) -> String {
    let structure = match template {
        "libre" => "Structure libre, claire et concise.",
        _ => "Trois sections markdown, dans cet ordre : `## Contexte` (ce qui existe aujourd'hui, \
              avec les fichiers concernés), `## Attendu` (liste de ce qui doit changer), \
              `## Critères d'acceptation` (cases à cocher `- [ ]` vérifiables).",
    };
    let code = if has_code {
        "Le dossier courant est une copie du dépôt : lis le code (Read, Grep, Glob) pour comprendre \
         de quoi parle l'issue avant d'écrire. Tu ne peux rien modifier."
    } else {
        "Le code du dépôt n'est pas disponible : appuie-toi sur le titre et la description."
    };
    format!(
        "Tu es l'agent CTO de Cadence. Ton rôle : rendre une issue GitHub exécutable par un \
         développeur, sans rien inventer.\n\n\
         - {code}\n\
         - Réécris la description en français. {structure}\n\
         - Garde tout ce que la description actuelle dit de juste ; ne retire pas d'information.\n\
         - Quand un point ne peut pas être tranché à partir du titre, de la description et du code, \
         pose une question (au plus 3) plutôt que de supposer. Rédige quand même la meilleure \
         description possible.\n\
         - Propose une estimation (S ≈ 1 h, M ≈ 4 h, L ≈ 1 j, XL ≈ plusieurs jours), au plus 3 \
         labels de domaine (ex. `area/api`, jamais `type/` ni `agent/`), un risque (faible, moyen, \
         élevé) et les numéros d'issues liées si tu en vois.\n\n\
         Termine ta réponse par un unique bloc ```json, sans rien après, de la forme :\n\
         {{\"body\": \"…markdown…\", \"questions\": [\"…\"], \"estimate\": \"M\", \"estimate_hours\": 4, \
         \"labels\": [\"area/api\"], \"risk\": \"faible\", \"related\": [129]}}"
    )
}

/// Requête de départ pour une issue.
pub fn issue_prompt(repo: &str, item: &CtoItem) -> String {
    let body = if item.before.trim().is_empty() { "(vide)" } else { item.before.trim() };
    format!(
        "Dépôt : {repo}\nIssue #{number} — {title}\nVersion : {version}\nType : {kind}\n\n\
         Description actuelle :\n{body}",
        number = item.number,
        title = item.title,
        version = item.version.as_deref().unwrap_or("aucune"),
        kind = item.issue_type.map(|t| t.slug()).unwrap_or("non précisé"),
    )
}

/// Requête de reprise avec vos réponses.
pub fn answers_prompt(questions: &[CtoQuestion]) -> String {
    let answers: Vec<String> = questions
        .iter()
        .filter_map(|q| q.answer.as_deref().map(|a| format!("- {}\n  Réponse : {}", q.question, a.trim())))
        .collect();
    format!(
        "Voici les réponses à tes questions :\n{}\n\nIntègre-les à la description. Ne repose une \
         question que si une réponse en ouvre une nouvelle. Termine par le même bloc ```json.",
        answers.join("\n")
    )
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct CtoProposal {
    pub body: String,
    pub questions: Vec<String>,
    pub estimate: Option<String>,
    pub estimate_hours: Option<u32>,
    pub labels: Vec<String>,
    pub risk: Option<String>,
    pub related: Vec<u32>,
}

/// Lit le dernier bloc ```json de la réponse. Refuse une réponse sans description.
pub fn parse_proposal(text: &str) -> AppResult<CtoProposal> {
    let block = text
        .rsplit_once("```json")
        .map(|(_, rest)| rest.split("```").next().unwrap_or(rest))
        .or_else(|| {
            // Bloc absent : un objet JSON nu en fin de réponse.
            text.rfind('{').map(|start| &text[start..])
        })
        .ok_or_else(|| AppError::Validation("L'agent n'a pas rendu de proposition lisible.".into()))?;
    let mut proposal: CtoProposal = serde_json::from_str(block.trim())
        .map_err(|e| AppError::Validation(format!("Proposition de l'agent illisible : {e}")))?;
    if proposal.body.trim().is_empty() {
        return Err(AppError::Validation("L'agent n'a pas proposé de description.".into()));
    }
    // Garde-fous sur ce que l'agent peut poser : pas de labels conventionnés, 3 au plus.
    proposal.labels.retain(|l| {
        let l = l.trim();
        !l.is_empty() && !l.starts_with("type/") && !l.starts_with("agent/") && !l.starts_with("status/")
    });
    proposal.labels.truncate(3);
    proposal.questions.retain(|q| !q.trim().is_empty());
    proposal.questions.truncate(3);
    proposal.estimate = proposal.estimate.filter(|e| ["S", "M", "L", "XL"].contains(&e.as_str()));
    Ok(proposal)
}

/// Range une proposition dans l'élément de file.
pub fn apply_proposal(item: &mut CtoItem, proposal: CtoProposal, session_id: Option<String>) {
    item.questions = proposal
        .questions
        .into_iter()
        .enumerate()
        .map(|(i, question)| CtoQuestion { id: format!("q{}", i + 1), question, answer: None })
        .collect();
    item.state = if item.questions.is_empty() { CtoItemState::Ready } else { CtoItemState::Questions };
    item.after = Some(proposal.body.trim().to_string());
    item.estimate = proposal.estimate;
    item.estimate_hours = proposal.estimate_hours;
    item.labels = proposal.labels;
    item.risk = proposal.risk;
    item.related = proposal.related.into_iter().filter(|n| *n != item.number).collect();
    item.error = None;
    if session_id.is_some() {
        item.session_id = session_id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> CtoItem {
        CtoItem {
            number: 156,
            title: "Message d'erreur quota API".into(),
            version: Some("0.1.2".into()),
            issue_type: None,
            state: CtoItemState::Running,
            before: "Erreur brute.".into(),
            after: None,
            questions: vec![],
            estimate: None,
            estimate_hours: None,
            labels: vec![],
            risk: None,
            related: vec![],
            error: None,
            session_id: None,
        }
    }

    #[test]
    fn reads_the_final_json_block() {
        let text = "J'ai lu client.rs.\n\n```json\n{\"body\": \"## Contexte\\n…\", \"questions\": [\"Le bandeau reste-t-il ?\", \" \"], \
                    \"estimate\": \"M\", \"estimate_hours\": 4, \"labels\": [\"area/api\", \"type/fix\", \"agent/cto\"], \
                    \"risk\": \"faible\", \"related\": [129, 156]}\n```";
        let proposal = parse_proposal(text).unwrap();
        assert_eq!(proposal.labels, ["area/api"]);
        assert_eq!(proposal.questions, ["Le bandeau reste-t-il ?"]);

        let mut it = item();
        apply_proposal(&mut it, proposal, Some("sess-1".into()));
        assert_eq!(it.state, CtoItemState::Questions);
        assert_eq!(it.questions[0].id, "q1");
        assert_eq!(it.related, [129], "l'issue elle-même n'est pas « liée »");
        assert_eq!(it.session_id.as_deref(), Some("sess-1"));
    }

    #[test]
    fn rejects_unreadable_or_empty_proposals() {
        assert!(parse_proposal("pas de json").is_err());
        assert!(parse_proposal("```json\n{\"body\": \"  \"}\n```").is_err());
        let bare = parse_proposal("Voilà : {\"body\": \"ok\", \"estimate\": \"XXL\"}").unwrap();
        assert_eq!((bare.body.as_str(), bare.estimate), ("ok", None));
    }

    #[test]
    fn builds_prompts() {
        assert!(system_prompt("contexte-attendu-criteres", true).contains("## Critères d'acceptation"));
        assert!(system_prompt("libre", false).contains("n'est pas disponible"));
        assert!(issue_prompt("me/app", &item()).contains("Issue #156"));
        let questions = [
            CtoQuestion { id: "q1".into(), question: "Visible ?".into(), answer: Some("Oui".into()) },
            CtoQuestion { id: "q2".into(), question: "Seuls ?".into(), answer: None },
        ];
        let prompt = answers_prompt(&questions);
        assert!(prompt.contains("Réponse : Oui") && !prompt.contains("Seuls"));
    }
}
