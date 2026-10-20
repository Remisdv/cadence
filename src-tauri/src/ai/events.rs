//! Format d'événements normalisé des IA (plan § 4.5) : chaque adaptateur traduit le flux de sa
//! CLI en `AiEvent`, que le chat affiche et enregistre. Les `MessageDelta` ne sont jamais
//! enregistrés : le `MessageCompleted` qui suit porte le texte entier.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Taille au-delà de laquelle une sortie d'outil est tronquée.
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024;

/// Provenance d'un outil appelé par l'IA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[ts(export)]
pub enum ToolSource {
    /// Outil d'un plugin (ou du cœur) servi par la passerelle.
    Cadence { plugin: String, tool: String },
    /// Outil natif de la CLI (lecture de fichier, recherche…).
    Builtin,
    /// Autre serveur MCP (l'utilisateur en a configuré un).
    OtherMcp { server: String },
}

impl ToolSource {
    /// Déduit du nom d'outil vu par Claude Code (`mcp__<serveur>__<outil>`).
    pub fn from_claude_name(name: &str) -> Self {
        let Some(rest) = name.strip_prefix("mcp__") else { return ToolSource::Builtin };
        match rest.split_once("__") {
            Some(("cadence", exposed)) => match exposed.split_once("__") {
                Some((plugin, tool)) => ToolSource::Cadence { plugin: plugin.into(), tool: tool.into() },
                None => ToolSource::OtherMcp { server: "cadence".into() },
            },
            Some((server, _)) => ToolSource::OtherMcp { server: server.into() },
            None => ToolSource::OtherMcp { server: rest.into() },
        }
    }

    /// Outil MCP désigné par son serveur et son nom (Codex : `server`, `tool`).
    pub fn from_mcp(server: &str, tool: &str) -> Self {
        match (server, tool.split_once("__")) {
            ("cadence", Some((plugin, tool))) => ToolSource::Cadence { plugin: plugin.into(), tool: tool.into() },
            _ => ToolSource::OtherMcp { server: server.into() },
        }
    }

    /// Déduit du nom d'outil vu par OpenCode (`<serveur>_<outil>`, `cadence_<plugin>__<outil>`).
    pub fn from_opencode_name(name: &str) -> Self {
        match name.strip_prefix("cadence_").and_then(|rest| rest.split_once("__")) {
            Some((plugin, tool)) => ToolSource::Cadence { plugin: plugin.into(), tool: tool.into() },
            None if name.starts_with("cadence_") => ToolSource::OtherMcp { server: "cadence".into() },
            None => ToolSource::Builtin,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum FileChange {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ApprovalKind {
    /// Commande shell.
    Command,
    /// Écriture ou modification de fichier.
    FileChange,
    /// Autre outil.
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ApprovalDecision {
    Allow,
    /// Autoriser pour le reste de la session.
    AllowSession,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AiErrorKind {
    /// Connexion de la CLI expirée ou absente.
    Auth,
    /// Quota de l'abonnement ou crédit épuisé.
    Quota,
    /// La CLI s'est arrêtée.
    Crash,
    /// Flux illisible.
    Protocol,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum PlanStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanItem {
    pub text: String,
    pub status: PlanStatus,
}

/// Choix proposé par une question de l'IA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuestionOption {
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Question à choix posée par l'IA (`AskUserQuestion` de Claude Code, `requestUserInput` de
/// Codex, `question.asked` d'OpenCode).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AiQuestion {
    /// Identifiant rendu avec la réponse : celui de la CLI (Codex), sinon le texte de la question.
    pub id: String,
    /// Titre court (« Format »).
    #[serde(default)]
    pub header: Option<String>,
    pub question: String,
    #[serde(default)]
    pub options: Vec<QuestionOption>,
    /// Plusieurs choix possibles.
    #[serde(default)]
    pub multi_select: bool,
    /// Réponse libre acceptée (« Autre »).
    #[serde(default)]
    pub other: bool,
}

/// Réponse de l'utilisateur à une question : les libellés choisis, ou un texte libre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuestionAnswer {
    /// `id` de la question.
    pub id: String,
    pub answers: Vec<String>,
}

/// Choix `[{ label, description }]` d'une question, tels que les trois CLI les décrivent.
pub fn question_options(value: Option<&Value>) -> Vec<QuestionOption> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|o| {
            let label = o.get("label").and_then(Value::as_str)?.to_string();
            let description = o.get("description").and_then(Value::as_str).filter(|d| !d.is_empty()).map(str::to_string);
            Some(QuestionOption { label, description })
        })
        .collect()
}

/// Réponse donnée d'office à une question posée pendant une exécution de fond.
pub const BACKGROUND_ANSWER: &str =
    "Personne ne peut répondre pendant une tâche de fond : choisis l'option la plus raisonnable, poursuis, et signale ce choix dans ton rapport.";

/// Réponses d'office (fond) aux questions `ids`.
pub fn background_answers(ids: &[String]) -> Vec<QuestionAnswer> {
    ids.iter().map(|id| QuestionAnswer { id: id.clone(), answers: vec![BACKGROUND_ANSWER.into()] }).collect()
}

/// Vérifie une réponse : une entrée non vide par question posée, rien d'autre.
pub fn check_answers(questions: &[AiQuestion], answers: &[QuestionAnswer]) -> Result<(), String> {
    for question in questions {
        let given = answers.iter().find(|a| a.id == question.id);
        if !given.is_some_and(|a| a.answers.iter().any(|s| !s.trim().is_empty())) {
            return Err(format!("Répondez à « {} ».", question.question));
        }
    }
    if let Some(extra) = answers.iter().find(|a| !questions.iter().any(|q| q.id == a.id)) {
        return Err(format!("« {} » n'est pas une question posée.", extra.id));
    }
    Ok(())
}

/// Réponse lisible (« Format : Résumé · Sections : Intro, Conclusion »), pour l'historique.
pub fn answers_summary(questions: &[AiQuestion], answers: &[QuestionAnswer]) -> String {
    answers
        .iter()
        .map(|a| {
            let joined = a.answers.join(", ");
            let question = questions.iter().find(|q| q.id == a.id);
            match question.and_then(|q| q.header.as_deref()).filter(|h| !h.is_empty()) {
                Some(header) if answers.len() > 1 => format!("{header} : {joined}"),
                _ => joined,
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Événement d'une session d'IA.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum AiEvent {
    /// Message de l'utilisateur (écrit par Cadence, pas par la CLI).
    UserMessage { text: String },
    SessionStarted { native_id: String, model: Option<String> },
    /// Morceau de texte en cours ; `message` identifie le message à compléter.
    MessageDelta { message: String, text: String },
    MessageCompleted { message: String, text: String },
    Reasoning { text: String },
    ToolStarted {
        call: String,
        source: ToolSource,
        name: String,
        #[ts(type = "unknown")]
        input: Value,
    },
    ToolCompleted {
        call: String,
        ok: bool,
        summary: Option<String>,
        #[ts(type = "unknown")]
        output: Option<Value>,
    },
    /// Commande shell de la CLI ; réémise à la fin avec sa sortie (même `call`).
    Command { call: String, command: String, exit_code: Option<i32>, output: Option<String> },
    FileChanged { path: String, change: FileChange },
    Plan { items: Vec<PlanItem> },
    ApprovalRequested {
        request: String,
        kind: ApprovalKind,
        title: String,
        #[ts(type = "unknown")]
        detail: Value,
    },
    /// Réponse donnée à une demande d'approbation (écrit par Cadence).
    ApprovalAnswered { request: String, decision: ApprovalDecision },
    /// Questions à choix de l'IA ; `text` et `options` résument la première (historique d'avant
    /// les questions multiples, où `questions` est absent).
    Question {
        request: String,
        text: String,
        options: Vec<String>,
        #[serde(default)]
        questions: Vec<AiQuestion>,
    },
    /// Réponse donnée à une question (écrit par Cadence) : `answer` lisible, `answers` détaillées.
    QuestionAnswered {
        request: String,
        answer: String,
        #[serde(default)]
        answers: Vec<QuestionAnswer>,
    },
    /// Rapport d'une exécution de fond (`cadence__run_report`), écrit par Cadence.
    Report {
        text: String,
        progress: Option<u32>,
        is_final: bool,
        #[ts(type = "unknown")]
        data: Option<Value>,
    },
    Usage { input_tokens: u32, output_tokens: u32, cost_usd: Option<f64> },
    TurnCompleted { ok: bool, interrupted: bool, error: Option<String>, duration_ms: u32 },
    Error { kind: AiErrorKind, message: String },
}

impl AiEvent {
    /// Question(s) de l'IA, avec le résumé `text` / `options` (anciens lecteurs).
    pub fn question(request: String, questions: Vec<AiQuestion>) -> Self {
        let text = questions.iter().map(|q| q.question.as_str()).collect::<Vec<_>>().join("\n");
        let options = questions.first().map(|q| q.options.iter().map(|o| o.label.clone()).collect()).unwrap_or_default();
        AiEvent::Question { request, text, options, questions }
    }

    /// Enregistré dans l'historique (les deltas ne le sont pas).
    pub fn is_persisted(&self) -> bool {
        !matches!(self, AiEvent::MessageDelta { .. })
    }
}

/// Tronque un texte à `MAX_OUTPUT_BYTES` sur une frontière de caractère.
pub fn truncate(text: &str) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        return text.to_string();
    }
    let mut end = MAX_OUTPUT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Entier JSON vers `u32` saturé (pas d'entier 64 bits dans l'IPC).
pub fn saturating_u32(value: Option<&Value>) -> u32 {
    value.and_then(Value::as_u64).map(|n| u32::try_from(n).unwrap_or(u32::MAX)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn deduces_the_tool_source() {
        assert_eq!(
            ToolSource::from_claude_name("mcp__cadence__gestion-projet__issue_get"),
            ToolSource::Cadence { plugin: "gestion-projet".into(), tool: "issue_get".into() }
        );
        assert_eq!(ToolSource::from_claude_name("mcp__github__search"), ToolSource::OtherMcp { server: "github".into() });
        assert_eq!(ToolSource::from_claude_name("Read"), ToolSource::Builtin);
        assert_eq!(
            ToolSource::from_mcp("cadence", "exemple__echo"),
            ToolSource::Cadence { plugin: "exemple".into(), tool: "echo".into() }
        );
        assert_eq!(ToolSource::from_mcp("github", "search"), ToolSource::OtherMcp { server: "github".into() });
        assert_eq!(
            ToolSource::from_opencode_name("cadence_exemple__echo"),
            ToolSource::Cadence { plugin: "exemple".into(), tool: "echo".into() }
        );
        assert_eq!(ToolSource::from_opencode_name("bash"), ToolSource::Builtin);
    }

    #[test]
    fn checks_and_summarizes_question_answers() {
        let question = |id: &str, header: Option<&str>| AiQuestion {
            id: id.into(),
            header: header.map(str::to_string),
            question: format!("{id} ?"),
            options: Vec::new(),
            multi_select: false,
            other: true,
        };
        let questions = [question("format", Some("Format")), question("sections", Some("Sections"))];
        let answer = |id: &str, answers: &[&str]| QuestionAnswer { id: id.into(), answers: answers.iter().map(|s| s.to_string()).collect() };
        let full = [answer("format", &["Résumé"]), answer("sections", &["Intro", "Conclusion"])];
        assert!(check_answers(&questions, &full).is_ok());
        assert_eq!(answers_summary(&questions, &full), "Format : Résumé · Sections : Intro, Conclusion");
        assert_eq!(answers_summary(&questions[..1], &full[..1]), "Résumé", "une seule question : sans titre");
        assert!(check_answers(&questions, &full[..1]).unwrap_err().contains("sections ?"));
        assert!(check_answers(&questions, &[answer("format", &["Résumé"]), answer("sections", &[" "])]).is_err());
        assert!(check_answers(&questions[..1], &full).is_err(), "question inconnue");
        assert_eq!(background_answers(&["q1".into()])[0].answers, [BACKGROUND_ANSWER]);

        // Historique d'avant les questions multiples : `questions` et `answers` absents.
        let old: AiEvent = serde_json::from_value(json!({ "type": "question", "request": "q", "text": "Langue ?", "options": ["Fr"] })).unwrap();
        assert!(matches!(old, AiEvent::Question { questions, .. } if questions.is_empty()));
        let old: AiEvent = serde_json::from_value(json!({ "type": "question-answered", "request": "q", "answer": "Fr" })).unwrap();
        assert!(matches!(old, AiEvent::QuestionAnswered { answers, .. } if answers.is_empty()));
        let event = AiEvent::question("q".into(), vec![question("format", None)]);
        assert_eq!(serde_json::to_value(&event).unwrap()["questions"][0]["multiSelect"], false);
    }

    #[test]
    fn serializes_with_a_type_tag_and_camel_case_fields() {
        let event = AiEvent::TurnCompleted { ok: true, interrupted: false, error: None, duration_ms: 12 };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            json!({ "type": "turn-completed", "ok": true, "interrupted": false, "error": null, "durationMs": 12 })
        );
        let back: AiEvent = serde_json::from_value(serde_json::to_value(&event).unwrap()).unwrap();
        assert_eq!(back, event);
        assert_eq!(truncate(&"é".repeat(MAX_OUTPUT_BYTES)).len(), MAX_OUTPUT_BYTES + "…".len());
        assert_eq!(saturating_u32(Some(&json!(u64::MAX))), u32::MAX);
    }
}
