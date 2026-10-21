//! Conversations vues par un plugin (SDK `cadence.conversations`) : celles que ses actions ont
//! ouvertes, regroupées au besoin (un sprint…). Le champ réservé `group` du contexte d'action
//! (`{ id, label, hidden }`) rattache la conversation ; l'hôte préfixe son id par celui du plugin,
//! qui ne voit donc que ses groupes. Une conversation de groupe est cachée du chat global par
//! défaut (`hidden`), sans cesser de s'ouvrir dans la vue Chat.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use super::store::{ChatEventRow, ChatGroup, ChatMode, ChatSessionRow, ChatState, origin_action};
use crate::{
    ai::events::AiEvent,
    app_settings::AiId,
    error::{AppError, AppResult},
};

/// Longueur maximale de l'id et du libellé d'un groupe.
const GROUP_MAX: usize = 120;

/// Plafond des nombres rendus au SDK (`< 2^31`).
const SDK_MAX: u32 = i32::MAX as u32;

/// Ce qu'attend une conversation en `waiting-approval`, pour le SDK des plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ConversationWaiting {
    /// Une demande d'accord ou une question à choix, en plein tour.
    Approval,
    /// Le tour s'est terminé sur une question posée en texte (« Réponds oui… ») : un nouveau
    /// message de l'utilisateur y répond.
    Reply,
}

/// État vu par le SDK des plugins (API 1) : ses états restent `idle`, `running`,
/// `waiting-approval`, `failed`, `interrupted`. « En attente de votre réponse » (`WaitingReply`
/// côté Cadence, recette du 21/10, N10) y devient `waiting-approval` avec `waiting: "reply"`,
/// pour qu'un plugin écrit avant ne la montre pas comme terminée.
pub fn sdk_state(state: ChatState) -> (ChatState, Option<ConversationWaiting>) {
    match state {
        ChatState::WaitingReply => (ChatState::WaitingApproval, Some(ConversationWaiting::Reply)),
        ChatState::WaitingApproval => (ChatState::WaitingApproval, Some(ConversationWaiting::Approval)),
        other => (other, None),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConversationInfo {
    pub id: String,
    pub title: String,
    pub ai: AiId,
    pub model: Option<String>,
    /// État au sens du SDK (voir `sdk_state`) : jamais `waiting-reply`.
    pub state: ChatState,
    /// Avec `waiting-approval` : accord ou question à choix (`approval`), ou réponse à une
    /// question posée en fin de tour (`reply`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub waiting: Option<ConversationWaiting>,
    pub mode: ChatMode,
    /// Action du plugin qui l'a ouverte.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "Record<string, unknown>")]
    pub context: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub group: Option<ChatGroup>,
    pub created_at: String,
    pub updated_at: String,
    pub turns: u32,
}

impl ConversationInfo {
    pub fn of(plugin: &str, row: &ChatSessionRow, turns: u32) -> Self {
        let (state, waiting) = sdk_state(row.state);
        Self {
            id: row.id.clone(),
            title: row.title.clone(),
            ai: row.ai,
            model: row.model.clone(),
            state,
            waiting,
            mode: row.mode,
            action: origin_action(&row.origin).map(|(_, action)| action.to_string()),
            context: row.context.clone(),
            group: row.group.as_ref().and_then(|g| g.local_to(plugin)),
            created_at: row.created_at.clone(),
            updated_at: row.updated_at.clone(),
            turns: turns.min(SDK_MAX),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ConversationRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConversationMessage {
    pub role: ConversationRole,
    pub text: String,
    pub at: String,
}

/// Outil appelé pendant la conversation, sans son entrée ni sa sortie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConversationTool {
    pub name: String,
    pub ok: bool,
    pub at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConversationUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Réponse de `conversations.get` : de quoi exporter la conversation plus tard.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConversationDetail {
    pub info: ConversationInfo,
    pub messages: Vec<ConversationMessage>,
    pub tools: Vec<ConversationTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub usage: Option<ConversationUsage>,
}

/// Nom donné aux commandes shell de la CLI dans la liste des outils.
pub const SHELL_TOOL: &str = "shell";

impl ConversationDetail {
    /// Messages complets et outils (nom, issue) ; jamais les entrées ni les sorties d'outils.
    pub fn of(info: ConversationInfo, events: &[ChatEventRow]) -> Self {
        let mut messages = Vec::new();
        let mut tools = Vec::new();
        let mut names: HashMap<&str, &str> = HashMap::new();
        let mut usage: Option<ConversationUsage> = None;
        for row in events {
            match &row.event {
                AiEvent::UserMessage { text } => {
                    messages.push(ConversationMessage { role: ConversationRole::User, text: text.clone(), at: row.at.clone() })
                }
                AiEvent::MessageCompleted { text, .. } if !text.trim().is_empty() => messages.push(ConversationMessage {
                    role: ConversationRole::Assistant,
                    text: text.clone(),
                    at: row.at.clone(),
                }),
                AiEvent::ToolStarted { call, name, .. } => {
                    names.insert(call, name);
                }
                AiEvent::ToolCompleted { call, ok, .. } => tools.push(ConversationTool {
                    name: names.get(call.as_str()).map(|n| n.to_string()).unwrap_or_else(|| "outil".into()),
                    ok: *ok,
                    at: row.at.clone(),
                }),
                // Une commande est réémise à sa fin, avec son code de sortie.
                AiEvent::Command { exit_code: Some(code), .. } => {
                    tools.push(ConversationTool { name: SHELL_TOOL.into(), ok: *code == 0, at: row.at.clone() })
                }
                AiEvent::Usage { input_tokens, output_tokens, .. } => {
                    let total = usage.get_or_insert(ConversationUsage { input_tokens: 0, output_tokens: 0 });
                    total.input_tokens = total.input_tokens.saturating_add(*input_tokens).min(SDK_MAX);
                    total.output_tokens = total.output_tokens.saturating_add(*output_tokens).min(SDK_MAX);
                }
                _ => {}
            }
        }
        Self { info, messages, tools, usage }
    }
}

fn valid_group_id(id: &str) -> bool {
    (1..=GROUP_MAX).contains(&id.len())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '/' | '.' | '_' | '-'))
}

/// Champ réservé `group` d'un contexte d'action : groupe (id préfixé par le plugin) et
/// visibilité (`hidden`, vrai par défaut quand le groupe est présent).
pub fn action_group(plugin: &str, context: Option<&Value>) -> AppResult<(Option<ChatGroup>, bool)> {
    let Some(raw) = context.and_then(|c| c.get("group")).filter(|g| !g.is_null()) else { return Ok((None, false)) };
    let invalid = |why: &str| AppError::Validation(format!("Contexte d'action : « group » {why}."));
    let group = raw.as_object().ok_or_else(|| invalid("doit être un objet { id, label, hidden }"))?;
    let id = group.get("id").and_then(Value::as_str).ok_or_else(|| invalid("demande un « id »"))?;
    if !valid_group_id(id) {
        return Err(invalid("a un id invalide (1 à 120 caractères parmi A-Z a-z 0-9 : / . _ -)"));
    }
    let label = match group.get("label") {
        None | Some(Value::Null) => id.to_string(),
        Some(Value::String(label)) if !label.trim().is_empty() => label.trim().chars().take(GROUP_MAX).collect(),
        Some(Value::String(_)) => id.to_string(),
        Some(_) => return Err(invalid("a un libellé qui n'est pas du texte")),
    };
    let hidden = match group.get("hidden") {
        None | Some(Value::Null) => true,
        Some(Value::Bool(hidden)) => *hidden,
        Some(_) => return Err(invalid("a un « hidden » qui n'est pas un booléen")),
    };
    Ok((Some(ChatGroup { id: ChatGroup::scoped_id(plugin, id), label }), hidden))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ai::{claude_code::Permissions, events::ToolSource};

    #[test]
    fn reads_the_reserved_group_field() {
        let context = json!({ "type": "issue", "repo": "o/n", "issue": 42,
            "group": { "id": "sprint:o/n:3", "label": "Sprint 3 · o/n" } });
        let (group, hidden) = action_group("gestion-projet", Some(&context)).unwrap();
        let group = group.unwrap();
        assert_eq!(group.id, "gestion-projet:sprint:o/n:3", "préfixé par l'id du plugin");
        assert_eq!(group.label, "Sprint 3 · o/n");
        assert!(hidden, "caché par défaut");

        let visible = json!({ "group": { "id": "sprint:1", "hidden": false } });
        let (group, hidden) = action_group("gp", Some(&visible)).unwrap();
        assert_eq!((group.unwrap().label, hidden), ("sprint:1".to_string(), false), "libellé par défaut : l'id");

        assert_eq!(action_group("gp", Some(&json!({ "type": "issue" }))).unwrap(), (None, false));
        assert_eq!(action_group("gp", Some(&json!({ "group": null }))).unwrap(), (None, false));
        assert_eq!(action_group("gp", None).unwrap(), (None, false));
        for bad in [
            json!({ "group": "sprint:1" }),
            json!({ "group": { "label": "Sans id" } }),
            json!({ "group": { "id": "" } }),
            json!({ "group": { "id": "a b" } }),
            json!({ "group": { "id": "x".repeat(121) } }),
            json!({ "group": { "id": "sprint", "hidden": "oui" } }),
            json!({ "group": { "id": "sprint", "label": 3 } }),
        ] {
            assert!(action_group("gp", Some(&bad)).is_err(), "{bad}");
        }
        assert!(action_group("gp", Some(&json!({ "group": { "id": "x".repeat(120) } }))).is_ok());
    }

    fn row(at: &str, event: AiEvent) -> ChatEventRow {
        ChatEventRow { seq: 0, turn: 1, at: at.into(), event }
    }

    #[test]
    fn details_keep_messages_and_drop_tool_outputs() {
        let session = ChatSessionRow {
            id: "c1".into(),
            title: "Développer l'issue 42".into(),
            ai: AiId::ClaudeCode,
            model: Some("opus".into()),
            origin: "action:gestion-projet/developper-issue".into(),
            mode: ChatMode::Background,
            permissions: Permissions::AutoEdit,
            state: ChatState::Idle,
            created_at: "2026-10-20T08:00:00Z".into(),
            updated_at: "2026-10-20T08:05:00Z".into(),
            group: Some(ChatGroup { id: "gestion-projet:sprint:o/n:3".into(), label: "Sprint 3".into() }),
            hidden: true,
            context: Some(json!({ "type": "issue", "issue": 42 })),
        };
        let info = ConversationInfo::of("gestion-projet", &session, 2);
        assert_eq!((info.state, info.waiting), (ChatState::Idle, None));
        let waiting = ConversationInfo::of("gestion-projet", &ChatSessionRow { state: ChatState::WaitingReply, ..session.clone() }, 2);
        assert_eq!((waiting.state, waiting.waiting), (ChatState::WaitingApproval, Some(ConversationWaiting::Reply)), "compatible SDK");
        let json = serde_json::to_string(&waiting).unwrap();
        assert!(json.contains("\"state\":\"waiting-approval\"") && json.contains("\"waiting\":\"reply\""), "{json}");
        assert_eq!(sdk_state(ChatState::WaitingApproval), (ChatState::WaitingApproval, Some(ConversationWaiting::Approval)));
        assert_eq!(info.action.as_deref(), Some("developper-issue"));
        assert_eq!(info.group.as_ref().unwrap().id, "sprint:o/n:3", "id vu par le plugin");
        let long = "x".repeat(40_000);
        let events = vec![
            row("t1", AiEvent::UserMessage { text: "Développe l'issue 42".into() }),
            row("t2", AiEvent::ToolStarted {
                call: "a".into(),
                source: ToolSource::Builtin,
                name: "Read".into(),
                input: json!({ "file_path": "secret.rs" }),
            }),
            row("t3", AiEvent::ToolCompleted { call: "a".into(), ok: true, summary: Some("résumé".into()), output: Some(json!("sortie privée")) }),
            row("t4", AiEvent::Command { call: "b".into(), command: "cargo test".into(), exit_code: None, output: None }),
            row("t5", AiEvent::Command { call: "b".into(), command: "cargo test".into(), exit_code: Some(1), output: Some("FAIL".into()) }),
            row("t6", AiEvent::MessageCompleted { message: "m".into(), text: long.clone() }),
            row("t7", AiEvent::MessageCompleted { message: "vide".into(), text: " ".into() }),
            row("t8", AiEvent::Usage { input_tokens: u32::MAX, output_tokens: 10, cost_usd: None }),
            row("t9", AiEvent::Usage { input_tokens: 5, output_tokens: 20, cost_usd: Some(0.1) }),
        ];
        let detail = ConversationDetail::of(info, &events);
        assert_eq!(detail.messages.len(), 2);
        assert_eq!(detail.messages[1].text, long, "texte complet");
        assert_eq!(detail.messages[1].role, ConversationRole::Assistant);
        assert_eq!(
            detail.tools,
            [
                ConversationTool { name: "Read".into(), ok: true, at: "t3".into() },
                ConversationTool { name: SHELL_TOOL.into(), ok: false, at: "t5".into() },
            ]
        );
        assert_eq!(detail.usage, Some(ConversationUsage { input_tokens: i32::MAX as u32, output_tokens: 30 }));
        let json = serde_json::to_string(&detail).unwrap();
        assert!(!json.contains("sortie privée") && !json.contains("secret.rs") && !json.contains("FAIL"), "pas de sortie d'outil");
        assert!(json.contains("\"inputTokens\"") && json.contains("\"createdAt\"") && json.contains("\"role\":\"user\""));
        assert!(!json.contains("gestion-projet:sprint"), "le préfixe de l'hôte ne sort pas");
    }
}
