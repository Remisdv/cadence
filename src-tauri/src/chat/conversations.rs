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
    ai::events::{AiEvent, ApprovalDecision, FileChange, ToolSource},
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

/// Question posée par l'IA dans la chronologie (sans les descriptions des choix).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConversationQuestion {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub header: Option<String>,
    pub question: String,
    pub options: Vec<String>,
}

/// Élément de la chronologie d'une conversation (`conversations.get`, champ `timeline`) : ce
/// qui s'est passé, dans l'ordre, pour un compte rendu exact. Les sorties d'outils et de
/// commandes n'y sont jamais ; les entrées d'outils sont abrégées (`compact_input`).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum ConversationEntry {
    Message { role: ConversationRole, text: String, at: String },
    /// Outil appelé : `plugin` pour un outil servi par Cadence (`cadence` pour ceux du cœur),
    /// `ok` absent tant qu'il n'a pas rendu son résultat.
    Tool {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        plugin: Option<String>,
        /// Entrée abrégée ; absente pour l'outil d'un autre plugin.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional, type = "unknown")]
        input: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        ok: Option<bool>,
        at: String,
    },
    Command {
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        exit_code: Option<i32>,
        at: String,
    },
    File { path: String, change: FileChange, at: String },
    /// Questions à choix de l'IA et la réponse donnée (absente sans réponse).
    Question {
        questions: Vec<ConversationQuestion>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        answer: Option<String>,
        at: String,
    },
    /// Demande d'accord de la CLI et la décision (absente sans réponse).
    Approval {
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        decision: Option<ApprovalDecision>,
        at: String,
    },
    /// Rapport d'une exécution de fond.
    Report { text: String, is_final: bool, at: String },
    Error { message: String, at: String },
    /// Fin d'un tour.
    Turn {
        ok: bool,
        interrupted: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        error: Option<String>,
        duration_ms: u32,
        at: String,
    },
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
    /// Chronologie complète (messages, questions et réponses, accords, outils et leurs entrées
    /// abrégées, commandes, fichiers, rapports, tours), sans aucune sortie.
    pub timeline: Vec<ConversationEntry>,
}

/// Longueur gardée d'un texte dans l'entrée abrégée d'un outil.
const INPUT_TEXT_MAX: usize = 300;
/// Éléments gardés d'une liste dans l'entrée abrégée d'un outil.
const INPUT_LIST_MAX: usize = 20;

/// Entrée d'outil abrégée pour la chronologie : textes de plus de 300 caractères coupés,
/// listes de plus de 20 éléments tronquées (le contenu d'un fichier écrit ne sort pas entier).
pub fn compact_input(value: &Value) -> Value {
    match value {
        Value::String(text) if text.chars().count() > INPUT_TEXT_MAX => {
            Value::String(format!("{}…", text.chars().take(INPUT_TEXT_MAX).collect::<String>()))
        }
        Value::Array(items) => {
            let mut out: Vec<Value> = items.iter().take(INPUT_LIST_MAX).map(compact_input).collect();
            if items.len() > INPUT_LIST_MAX {
                out.push(Value::String(format!("… {} de plus", items.len() - INPUT_LIST_MAX)));
            }
            Value::Array(out)
        }
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), compact_input(v))).collect()),
        other => other.clone(),
    }
}

/// Nom donné aux commandes shell de la CLI dans la liste des outils.
pub const SHELL_TOOL: &str = "shell";

impl ConversationDetail {
    /// Messages complets, outils (nom, issue) et chronologie vue par `plugin` : les entrées des
    /// outils d'un autre plugin n'y sont pas ; jamais aucune sortie d'outil ni de commande.
    pub fn of(plugin: &str, info: ConversationInfo, events: &[ChatEventRow]) -> Self {
        let mut messages = Vec::new();
        let mut tools = Vec::new();
        let mut timeline: Vec<ConversationEntry> = Vec::new();
        let mut names: HashMap<&str, &str> = HashMap::new();
        // Position dans la chronologie des entrées complétées plus tard (outil, commande,
        // question, accord).
        let mut open: HashMap<&str, usize> = HashMap::new();
        let mut usage: Option<ConversationUsage> = None;
        for row in events {
            let at = || row.at.clone();
            match &row.event {
                AiEvent::UserMessage { text } => {
                    messages.push(ConversationMessage { role: ConversationRole::User, text: text.clone(), at: at() });
                    timeline.push(ConversationEntry::Message { role: ConversationRole::User, text: text.clone(), at: at() });
                }
                AiEvent::MessageCompleted { text, .. } if !text.trim().is_empty() => {
                    messages.push(ConversationMessage { role: ConversationRole::Assistant, text: text.clone(), at: at() });
                    timeline.push(ConversationEntry::Message { role: ConversationRole::Assistant, text: text.clone(), at: at() });
                }
                AiEvent::ToolStarted { call, name, source, input } => {
                    names.insert(call, name);
                    let (owner, shown) = match source {
                        ToolSource::Cadence { plugin: owner, .. } => {
                            let mine = owner == plugin || owner == "cadence";
                            (Some(owner.clone()), mine.then(|| compact_input(input)))
                        }
                        ToolSource::OtherMcp { .. } => (None, None),
                        ToolSource::Builtin => (None, Some(compact_input(input))),
                    };
                    open.insert(call, timeline.len());
                    timeline.push(ConversationEntry::Tool { name: name.clone(), plugin: owner, input: shown, ok: None, at: at() });
                }
                AiEvent::ToolCompleted { call, ok, .. } => {
                    tools.push(ConversationTool {
                        name: names.get(call.as_str()).map(|n| n.to_string()).unwrap_or_else(|| "outil".into()),
                        ok: *ok,
                        at: at(),
                    });
                    if let Some(ConversationEntry::Tool { ok: done, .. }) = open.get(call.as_str()).and_then(|i| timeline.get_mut(*i)) {
                        *done = Some(*ok);
                    }
                }
                // Une commande est réémise à sa fin, avec son code de sortie.
                AiEvent::Command { call, command, exit_code, .. } => {
                    if let Some(code) = exit_code {
                        tools.push(ConversationTool { name: SHELL_TOOL.into(), ok: *code == 0, at: at() });
                    }
                    match open.get(call.as_str()).and_then(|i| timeline.get_mut(*i)) {
                        Some(ConversationEntry::Command { exit_code: done, .. }) => *done = *exit_code,
                        _ => {
                            open.insert(call, timeline.len());
                            timeline.push(ConversationEntry::Command { command: command.clone(), exit_code: *exit_code, at: at() });
                        }
                    }
                }
                AiEvent::FileChanged { path, change } => {
                    timeline.push(ConversationEntry::File { path: path.clone(), change: *change, at: at() })
                }
                AiEvent::Question { request, text, options, questions } => {
                    let questions = if questions.is_empty() {
                        vec![ConversationQuestion { header: None, question: text.clone(), options: options.clone() }]
                    } else {
                        questions
                            .iter()
                            .map(|q| ConversationQuestion {
                                header: q.header.clone(),
                                question: q.question.clone(),
                                options: q.options.iter().map(|o| o.label.clone()).collect(),
                            })
                            .collect()
                    };
                    open.insert(request, timeline.len());
                    timeline.push(ConversationEntry::Question { questions, answer: None, at: at() });
                }
                AiEvent::QuestionAnswered { request, answer, .. } => {
                    if let Some(ConversationEntry::Question { answer: given, .. }) = open.get(request.as_str()).and_then(|i| timeline.get_mut(*i)) {
                        *given = Some(answer.clone());
                    }
                }
                AiEvent::ApprovalRequested { request, title, .. } => {
                    open.insert(request, timeline.len());
                    timeline.push(ConversationEntry::Approval { title: title.clone(), decision: None, at: at() });
                }
                AiEvent::ApprovalAnswered { request, decision } => {
                    if let Some(ConversationEntry::Approval { decision: given, .. }) = open.get(request.as_str()).and_then(|i| timeline.get_mut(*i)) {
                        *given = Some(*decision);
                    }
                }
                AiEvent::Report { text, is_final, .. } => {
                    timeline.push(ConversationEntry::Report { text: text.clone(), is_final: *is_final, at: at() })
                }
                AiEvent::Error { message, .. } => timeline.push(ConversationEntry::Error { message: message.clone(), at: at() }),
                AiEvent::TurnCompleted { ok, interrupted, error, duration_ms } => timeline.push(ConversationEntry::Turn {
                    ok: *ok,
                    interrupted: *interrupted,
                    error: error.clone(),
                    duration_ms: *duration_ms,
                    at: at(),
                }),
                AiEvent::Usage { input_tokens, output_tokens, .. } => {
                    let total = usage.get_or_insert(ConversationUsage { input_tokens: 0, output_tokens: 0 });
                    total.input_tokens = total.input_tokens.saturating_add(*input_tokens).min(SDK_MAX);
                    total.output_tokens = total.output_tokens.saturating_add(*output_tokens).min(SDK_MAX);
                }
                _ => {}
            }
        }
        Self { info, messages, tools, usage, timeline }
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
        let detail = ConversationDetail::of("gestion-projet", info, &events);
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
        assert!(!json.contains("sortie privée") && !json.contains("résumé") && !json.contains("FAIL"), "pas de sortie d'outil");
        // Chronologie : l'entrée des outils (abrégée), la commande et son code, sans sortie.
        assert_eq!(detail.timeline.len(), 4, "{:#?}", detail.timeline);
        assert!(matches!(&detail.timeline[1], ConversationEntry::Tool { name, input: Some(input), ok: Some(true), .. }
            if name == "Read" && input["file_path"] == "secret.rs"));
        assert!(matches!(&detail.timeline[2], ConversationEntry::Command { command, exit_code: Some(1), at } if command == "cargo test" && at == "t4"));
        assert!(matches!(&detail.timeline[3], ConversationEntry::Message { role: ConversationRole::Assistant, .. }));
        assert!(json.contains("\"inputTokens\"") && json.contains("\"createdAt\"") && json.contains("\"role\":\"user\""));
        assert!(!json.contains("gestion-projet:sprint"), "le préfixe de l'hôte ne sort pas");
    }

    #[test]
    fn timeline_tells_what_happened_without_outputs() {
        let session = ChatSessionRow {
            id: "c2".into(),
            title: "Bilan du sprint".into(),
            ai: AiId::Codex,
            model: None,
            origin: "user".into(),
            mode: ChatMode::Chat,
            permissions: Permissions::Ask,
            state: ChatState::Idle,
            created_at: "t0".into(),
            updated_at: "t9".into(),
            group: None,
            hidden: false,
            context: None,
        };
        let info = ConversationInfo::of("gestion-projet", &session, 1);
        assert_eq!(info.action, None, "conversation du chat global");
        let plugin_tool = |call: &str, plugin: &str, input: Value| AiEvent::ToolStarted {
            call: call.into(),
            source: ToolSource::Cadence { plugin: plugin.into(), tool: "x".into() },
            name: format!("{plugin}__x"),
            input,
        };
        let question = crate::ai::events::AiQuestion {
            id: "Fusionner ?".into(),
            header: Some("Fusion".into()),
            question: "Fusionner ?".into(),
            options: vec![crate::ai::events::QuestionOption { label: "Oui".into(), description: Some("détail".into()) }],
            multi_select: false,
            other: true,
        };
        let events = vec![
            row("t1", plugin_tool("a", "gestion-projet", json!({ "repo": "o/n", "body": "y".repeat(1000), "labels": (0..30).collect::<Vec<_>>() }))),
            row("t2", plugin_tool("b", "documents", json!({ "note": "privée" }))),
            row("t3", AiEvent::question("q1".into(), vec![question])),
            row("t4", AiEvent::QuestionAnswered { request: "q1".into(), answer: "Oui".into(), answers: vec![] }),
            row("t5", AiEvent::ApprovalRequested {
                request: "r1".into(),
                kind: crate::ai::events::ApprovalKind::Command,
                title: "Bash : git push".into(),
                detail: json!({}),
                tool: None,
                scope: None,
            }),
            row("t6", AiEvent::ApprovalAnswered { request: "r1".into(), decision: ApprovalDecision::Deny }),
            row("t7", AiEvent::FileChanged { path: "src/a.rs".into(), change: FileChange::Modified }),
            row("t8", AiEvent::Error { kind: crate::ai::events::AiErrorKind::Quota, message: "Limite atteinte".into() }),
            row("t9", AiEvent::TurnCompleted { ok: false, interrupted: false, error: Some("Limite".into()), duration_ms: 1200 }),
        ];
        let detail = ConversationDetail::of("gestion-projet", info, &events);
        let ConversationEntry::Tool { input: Some(input), plugin, ok: None, .. } = &detail.timeline[0] else { panic!("{:?}", detail.timeline[0]) };
        assert_eq!(plugin.as_deref(), Some("gestion-projet"));
        assert_eq!(input["body"].as_str().unwrap().chars().count(), 301, "texte abrégé");
        assert_eq!(input["labels"].as_array().unwrap().len(), 21, "liste abrégée");
        assert!(matches!(&detail.timeline[1], ConversationEntry::Tool { input: None, plugin: Some(p), .. } if p == "documents"), "entrée d'un autre plugin cachée");
        assert!(matches!(&detail.timeline[2], ConversationEntry::Question { questions, answer: Some(a), at }
            if questions[0].options == ["Oui"] && questions[0].header.as_deref() == Some("Fusion") && a == "Oui" && at == "t3"));
        assert!(matches!(&detail.timeline[3], ConversationEntry::Approval { decision: Some(ApprovalDecision::Deny), .. }));
        assert!(matches!(&detail.timeline[6], ConversationEntry::Turn { ok: false, duration_ms: 1200, .. }));
        let json = serde_json::to_string(&detail).unwrap();
        assert!(!json.contains("privée") && !json.contains("détail"), "{json}");
        assert!(json.contains("\"kind\":\"question\"") && json.contains("\"durationMs\":1200"), "{json}");
    }
}
