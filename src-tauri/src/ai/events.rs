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
    Question { request: String, text: String, options: Vec<String> },
    Usage { input_tokens: u32, output_tokens: u32, cost_usd: Option<f64> },
    TurnCompleted { ok: bool, interrupted: bool, error: Option<String>, duration_ms: u32 },
    Error { kind: AiErrorKind, message: String },
}

impl AiEvent {
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
