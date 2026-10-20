//! Lecture du flux stream-json de Claude Code (plan § 4.2) : chaque ligne devient zéro, un ou
//! plusieurs `AiEvent`, plus les demandes de contrôle à traiter (approbations).
//!
//! Vérifié sur Claude Code 2.1.263 (fixtures `tests/fixtures/ai/claude/*.jsonl`) :
//! - `system/init` est réémis à chaque tour : `SessionStarted` seulement si l'id change ;
//! - texte : `stream_event` `content_block_delta/text_delta` (deltas) puis un message
//!   `assistant` par bloc (texte complet) ;
//! - `can_use_tool` arrive en `control_request` ; la réponse part en `control_response` ;
//!   `AskUserQuestion` y passe aussi (même en `bypassPermissions`) : ses réponses repartent dans
//!   `updatedInput.answers` ;
//! - erreur d'authentification : `assistant.error` puis `result` `is_error` (jamais le seul
//!   `subtype`) ; interruption : `result` `terminal_reason = "aborted_streaming"`.
//!
//! Les types inconnus sont ignorés.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::events::{
    AiErrorKind, AiEvent, AiQuestion, ApprovalDecision, ApprovalKind, FileChange, PlanItem, PlanStatus, QuestionAnswer,
    BACKGROUND_ANSWER, ToolSource, question_options, saturating_u32, truncate,
};

/// Ce qu'une ligne du flux demande.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamItem {
    Event(AiEvent),
    /// Demande d'approbation à garder pour y répondre (`answer`).
    Approval(PendingApproval),
}

/// Demande `can_use_tool` en attente de réponse.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingApproval {
    pub request_id: String,
    /// Outil demandé (`Read`, `Edit`, `Bash`…).
    pub tool: String,
    pub input: Value,
    pub suggestions: Value,
}

/// Réponse `control_response` à une demande d'approbation.
pub fn approval_response(pending: &PendingApproval, decision: ApprovalDecision) -> Value {
    let response = match decision {
        ApprovalDecision::Deny => json!({ "behavior": "deny", "message": "Refusé par l'utilisateur dans Cadence." }),
        ApprovalDecision::Allow => json!({ "behavior": "allow", "updatedInput": pending.input }),
        ApprovalDecision::AllowSession => {
            json!({ "behavior": "allow", "updatedInput": pending.input, "updatedPermissions": pending.suggestions })
        }
    };
    json!({ "type": "control_response", "response": { "subtype": "success", "request_id": pending.request_id, "response": response } })
}

/// Accord d'office d'un accès à un worktree ouvert à la session : le dossier rejoint les
/// dossiers de travail de la session (plus de demande pour lui jusqu'à la fin du processus).
pub fn grant_response(pending: &PendingApproval, dir: &std::path::Path) -> Value {
    let update = json!({ "type": "addDirectories", "directories": [dir.display().to_string()], "destination": "session" });
    let response = json!({ "behavior": "allow", "updatedInput": pending.input, "updatedPermissions": [update] });
    json!({ "type": "control_response", "response": { "subtype": "success", "request_id": pending.request_id, "response": response } })
}

/// Refus d'office (exécution de fond : personne pour répondre).
pub fn background_denial(pending: &PendingApproval) -> Value {
    let response = json!({
        "behavior": "deny",
        "message": "Refusé : une tâche de fond ne peut pas demander d'autorisation. Seul le worktree rendu par cadence__repo_checkout est accessible."
    });
    json!({ "type": "control_response", "response": { "subtype": "success", "request_id": pending.request_id, "response": response } })
}

/// Outil des questions à choix de Claude Code. Il passe par `can_use_tool` dans tous les modes,
/// `bypassPermissions` compris.
pub const ASK_USER_QUESTION: &str = "AskUserQuestion";

/// Questions d'un `AskUserQuestion` (`input.questions` : question, header, options
/// { label, description }, multiSelect). L'id d'une question est son texte : c'est la clé de
/// `answers`. Une réponse libre est toujours acceptée.
pub fn questions_of(input: &Value) -> Vec<AiQuestion> {
    input
        .get("questions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|q| {
            let question = str_at(q, "question")?.to_string();
            Some(AiQuestion {
                id: question.clone(),
                header: str_at(q, "header").filter(|h| !h.is_empty()).map(str::to_string),
                options: question_options(q.get("options")),
                multi_select: q.get("multiSelect").and_then(Value::as_bool).unwrap_or(false),
                other: true,
                question,
            })
        })
        .collect()
}

/// Réponse à un `AskUserQuestion` : l'entrée d'origine plus `answers` (texte de la question →
/// libellés choisis séparés par « , », ou texte libre).
pub fn question_response(pending: &PendingApproval, answers: &[QuestionAnswer]) -> Value {
    let map: serde_json::Map<String, Value> =
        answers.iter().map(|a| (a.id.clone(), Value::String(a.answers.join(", ")))).collect();
    let mut input = match &pending.input {
        Value::Object(object) => object.clone(),
        _ => serde_json::Map::new(),
    };
    input.insert("answers".into(), Value::Object(map));
    let response = json!({ "behavior": "allow", "updatedInput": input });
    json!({ "type": "control_response", "response": { "subtype": "success", "request_id": pending.request_id, "response": response } })
}

/// Question posée pendant une exécution de fond : personne pour répondre.
pub fn background_question_denial(pending: &PendingApproval) -> Value {
    let response = json!({
        "behavior": "deny",
        "message": BACKGROUND_ANSWER
    });
    json!({ "type": "control_response", "response": { "subtype": "success", "request_id": pending.request_id, "response": response } })
}

/// Message utilisateur d'un tour.
pub fn user_message(text: &str) -> Value {
    json!({ "type": "user", "message": { "role": "user", "content": text }, "parent_tool_use_id": null })
}

/// Demande d'interruption du tour en cours.
pub fn interrupt_request(id: &str) -> Value {
    json!({ "type": "control_request", "request_id": id, "request": { "subtype": "interrupt" } })
}

/// Outil en cours, pour interpréter son résultat.
#[derive(Debug, Clone)]
struct OpenTool {
    name: String,
    input: Value,
}

/// État de lecture d'une session.
#[derive(Debug, Default)]
pub struct ClaudeStream {
    native_id: Option<String>,
    model: Option<String>,
    /// Message en cours de diffusion (`message_start`).
    message: Option<String>,
    /// Index de bloc → n° du bloc texte dans le message (deltas).
    text_blocks: HashMap<u64, usize>,
    started_texts: usize,
    /// Blocs texte complets déjà reçus, par message.
    completed_texts: HashMap<String, usize>,
    tools: HashMap<String, OpenTool>,
    /// Une erreur d'API (`assistant.error`) a marqué le tour.
    turn_error: Option<String>,
}

fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// Texte d'un `tool_result` (chaîne ou blocs `text`).
fn result_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => {
            blocks.iter().filter_map(|b| str_at(b, "text")).collect::<Vec<_>>().join("\n")
        }
        _ => String::new(),
    }
}

fn error_kind(code: &str) -> AiErrorKind {
    match code {
        "authentication_failed" | "oauth_token_expired" | "invalid_api_key" => AiErrorKind::Auth,
        "billing_error" | "rate_limit" | "usage_limit" | "overloaded" => AiErrorKind::Quota,
        _ => AiErrorKind::Other,
    }
}

impl ClaudeStream {
    pub fn native_id(&self) -> Option<&str> {
        self.native_id.as_deref()
    }

    /// Une ligne de stdout ; une ligne qui n'est pas du JSON est ignorée.
    pub fn push_line(&mut self, line: &str) -> Vec<StreamItem> {
        match serde_json::from_str::<Value>(line.trim()) {
            Ok(value) => self.push(&value),
            Err(_) => Vec::new(),
        }
    }

    pub fn push(&mut self, line: &Value) -> Vec<StreamItem> {
        let mut out = Vec::new();
        let event = |out: &mut Vec<StreamItem>, e: AiEvent| out.push(StreamItem::Event(e));
        match str_at(line, "type") {
            Some("system") if str_at(line, "subtype") == Some("init") => {
                let id = str_at(line, "session_id").map(str::to_string);
                let model = str_at(line, "model").map(str::to_string);
                if id.is_some() && (id != self.native_id || model != self.model) {
                    self.native_id.clone_from(&id);
                    self.model.clone_from(&model);
                    event(&mut out, AiEvent::SessionStarted { native_id: id.unwrap_or_default(), model });
                }
            }
            Some("stream_event") => self.push_stream_event(line.get("event").unwrap_or(&Value::Null), &mut out),
            Some("assistant") => self.push_assistant(line, &mut out),
            Some("user") => self.push_user(line, &mut out),
            Some("control_request") => {
                let request = line.get("request").unwrap_or(&Value::Null);
                if str_at(request, "subtype") == Some("can_use_tool") {
                    let request_id = str_at(line, "request_id").unwrap_or_default().to_string();
                    let tool = str_at(request, "tool_name").unwrap_or_default();
                    let input = request.get("input").cloned().unwrap_or(Value::Null);
                    // Question à choix : une carte de question, pas d'approbation ; la réponse
                    // repart dans `updatedInput.answers` (voir `question_response`).
                    if tool == ASK_USER_QUESTION {
                        event(&mut out, AiEvent::question(request_id.clone(), questions_of(&input)));
                        out.push(StreamItem::Approval(PendingApproval {
                            request_id,
                            tool: tool.to_string(),
                            input,
                            suggestions: json!([]),
                        }));
                        return out;
                    }
                    let kind = match tool {
                        "Bash" | "PowerShell" => ApprovalKind::Command,
                        "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => ApprovalKind::FileChange,
                        _ => ApprovalKind::Tool,
                    };
                    let shown = str_at(request, "display_name").unwrap_or(tool);
                    let title = match (kind, str_at(&input, "command"), str_at(request, "description")) {
                        (ApprovalKind::Command, Some(command), _) => format!("{shown} : {command}"),
                        (_, _, Some(description)) if !description.is_empty() => format!("{shown} {description}"),
                        _ => shown.to_string(),
                    };
                    event(&mut out, AiEvent::ApprovalRequested { request: request_id.clone(), kind, title, detail: input.clone() });
                    out.push(StreamItem::Approval(PendingApproval {
                        request_id,
                        tool: tool.to_string(),
                        input,
                        suggestions: request.get("permission_suggestions").cloned().unwrap_or(json!([])),
                    }));
                }
            }
            Some("rate_limit_event") => {
                let info = line.get("rate_limit_info").unwrap_or(&Value::Null);
                if str_at(info, "status") == Some("rejected") {
                    event(&mut out, AiEvent::Error { kind: AiErrorKind::Quota, message: "Limite d'utilisation de l'abonnement atteinte.".into() });
                }
            }
            Some("result") => self.push_result(line, &mut out),
            _ => {}
        }
        out
    }

    fn push_stream_event(&mut self, event: &Value, out: &mut Vec<StreamItem>) {
        match str_at(event, "type") {
            Some("message_start") => {
                self.message = event.pointer("/message/id").and_then(Value::as_str).map(str::to_string);
                self.text_blocks.clear();
                self.started_texts = 0;
            }
            Some("content_block_start") => {
                if event.pointer("/content_block/type").and_then(Value::as_str) == Some("text")
                    && let Some(index) = event.get("index").and_then(Value::as_u64)
                {
                    self.text_blocks.insert(index, self.started_texts);
                    self.started_texts += 1;
                }
            }
            Some("content_block_delta") => {
                let delta = event.get("delta").unwrap_or(&Value::Null);
                if str_at(delta, "type") == Some("text_delta")
                    && let (Some(message), Some(index)) = (&self.message, event.get("index").and_then(Value::as_u64))
                    && let Some(block) = self.text_blocks.get(&index)
                {
                    let text = str_at(delta, "text").unwrap_or_default();
                    if !text.is_empty() {
                        out.push(StreamItem::Event(AiEvent::MessageDelta { message: format!("{message}:{block}"), text: text.into() }));
                    }
                }
            }
            _ => {}
        }
    }

    fn push_assistant(&mut self, line: &Value, out: &mut Vec<StreamItem>) {
        if let Some(code) = str_at(line, "error") {
            let message = line
                .pointer("/message/content/0/text")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("Claude Code : {code}"));
            self.turn_error = Some(message.clone());
            out.push(StreamItem::Event(AiEvent::Error { kind: error_kind(code), message }));
            return;
        }
        let message = line.get("message").unwrap_or(&Value::Null);
        let id = str_at(message, "id").unwrap_or_default().to_string();
        for block in message.get("content").and_then(Value::as_array).into_iter().flatten() {
            match str_at(block, "type") {
                Some("text") => {
                    let n = self.completed_texts.entry(id.clone()).or_default();
                    let text = str_at(block, "text").unwrap_or_default().to_string();
                    let key = format!("{id}:{n}");
                    *n += 1;
                    out.push(StreamItem::Event(AiEvent::MessageCompleted { message: key, text }));
                }
                Some("thinking") => {
                    let text = str_at(block, "thinking").unwrap_or_default();
                    if !text.trim().is_empty() {
                        out.push(StreamItem::Event(AiEvent::Reasoning { text: text.into() }));
                    }
                }
                Some("tool_use") => {
                    let call = str_at(block, "id").unwrap_or_default().to_string();
                    let name = str_at(block, "name").unwrap_or_default().to_string();
                    let input = block.get("input").cloned().unwrap_or(Value::Null);
                    self.tools.insert(call.clone(), OpenTool { name: name.clone(), input: input.clone() });
                    match name.as_str() {
                        "Bash" | "PowerShell" => out.push(StreamItem::Event(AiEvent::Command {
                            call,
                            command: str_at(&input, "command").unwrap_or_default().into(),
                            exit_code: None,
                            output: None,
                        })),
                        "TodoWrite" => out.push(StreamItem::Event(AiEvent::Plan { items: plan_items(&input) })),
                        _ => out.push(StreamItem::Event(AiEvent::ToolStarted {
                            call,
                            source: ToolSource::from_claude_name(&name),
                            name,
                            input,
                        })),
                    }
                }
                _ => {}
            }
        }
    }

    fn push_user(&mut self, line: &Value, out: &mut Vec<StreamItem>) {
        let content = line.pointer("/message/content").and_then(Value::as_array);
        for block in content.into_iter().flatten().filter(|b| str_at(b, "type") == Some("tool_result")) {
            let call = str_at(block, "tool_use_id").unwrap_or_default().to_string();
            let ok = !block.get("is_error").and_then(Value::as_bool).unwrap_or(false);
            let text = result_text(block.get("content"));
            let Some(tool) = self.tools.remove(&call) else { continue };
            match tool.name.as_str() {
                "Bash" | "PowerShell" => out.push(StreamItem::Event(AiEvent::Command {
                    call,
                    command: str_at(&tool.input, "command").unwrap_or_default().into(),
                    exit_code: (!ok).then_some(1),
                    output: Some(truncate(&text)),
                })),
                "TodoWrite" => {}
                name => {
                    if ok
                        && matches!(name, "Write" | "Edit" | "MultiEdit" | "NotebookEdit")
                        && let Some(path) = str_at(&tool.input, "file_path").or_else(|| str_at(&tool.input, "notebook_path"))
                    {
                        let change = if name == "Write" && text.contains("created") { FileChange::Added } else { FileChange::Modified };
                        out.push(StreamItem::Event(AiEvent::FileChanged { path: path.into(), change }));
                    }
                    let summary = text.lines().find(|l| !l.trim().is_empty()).map(|l| truncate(l.trim()));
                    let output = (!text.is_empty()).then(|| Value::String(truncate(&text)));
                    out.push(StreamItem::Event(AiEvent::ToolCompleted { call, ok, summary, output }));
                }
            }
        }
    }

    fn push_result(&mut self, line: &Value, out: &mut Vec<StreamItem>) {
        let usage = line.get("usage").unwrap_or(&Value::Null);
        let input_tokens = saturating_u32(usage.get("input_tokens"))
            .saturating_add(saturating_u32(usage.get("cache_creation_input_tokens")))
            .saturating_add(saturating_u32(usage.get("cache_read_input_tokens")));
        let output_tokens = saturating_u32(usage.get("output_tokens"));
        let cost_usd = line.get("total_cost_usd").and_then(Value::as_f64);
        if input_tokens > 0 || output_tokens > 0 {
            out.push(StreamItem::Event(AiEvent::Usage { input_tokens, output_tokens, cost_usd }));
        }
        let interrupted = str_at(line, "terminal_reason") == Some("aborted_streaming");
        let is_error = line.get("is_error").and_then(Value::as_bool).unwrap_or(false) || self.turn_error.is_some();
        let error = match (is_error, interrupted) {
            (true, false) => Some(
                self.turn_error
                    .clone()
                    .or_else(|| str_at(line, "result").filter(|r| !r.is_empty()).map(str::to_string))
                    .unwrap_or_else(|| format!("Tour en échec ({}).", str_at(line, "subtype").unwrap_or("erreur"))),
            ),
            _ => None,
        };
        out.push(StreamItem::Event(AiEvent::TurnCompleted {
            ok: !is_error,
            interrupted,
            error,
            duration_ms: saturating_u32(line.get("duration_ms")),
        }));
        self.turn_error = None;
        self.tools.clear();
    }
}

fn plan_items(input: &Value) -> Vec<PlanItem> {
    input
        .get("todos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|todo| PlanItem {
            text: str_at(todo, "content").unwrap_or_default().into(),
            status: match str_at(todo, "status") {
                Some("completed") => PlanStatus::Completed,
                Some("in_progress") => PlanStatus::InProgress,
                _ => PlanStatus::Pending,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replay(name: &str) -> (ClaudeStream, Vec<StreamItem>) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ai/claude").join(format!("{name}.jsonl"));
        let text = std::fs::read_to_string(path).unwrap();
        let mut stream = ClaudeStream::default();
        let items = text.lines().flat_map(|line| stream.push_line(line)).collect();
        (stream, items)
    }

    fn events(items: &[StreamItem]) -> Vec<&AiEvent> {
        items.iter().filter_map(|i| if let StreamItem::Event(e) = i { Some(e) } else { None }).collect()
    }

    #[test]
    fn normalizes_a_two_turn_session_with_tools_and_an_approval() {
        let (stream, items) = replay("multi-turn");
        assert_eq!(stream.native_id(), Some("c9605cfd-b89e-4850-ac6a-53e823307e59"));
        let events = events(&items);

        let started: Vec<_> = events.iter().filter(|e| matches!(e, AiEvent::SessionStarted { .. })).collect();
        assert_eq!(started.len(), 1, "init réémis au 2ᵉ tour : un seul SessionStarted");

        // Les deltas d'un message reconstituent le texte complet.
        let completed: Vec<(&String, &String)> = events
            .iter()
            .filter_map(|e| if let AiEvent::MessageCompleted { message, text } = e { Some((message, text)) } else { None })
            .collect();
        assert_eq!(completed.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(), ["pamplemousse", "fait"]);
        let streamed: String = events
            .iter()
            .filter_map(|e| match e {
                AiEvent::MessageDelta { message, text } if message == completed[0].0 => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(streamed, "pamplemousse");

        let tools: Vec<&str> = events
            .iter()
            .filter_map(|e| if let AiEvent::ToolStarted { name, source, .. } = e { assert_eq!(source, &ToolSource::Builtin); Some(name.as_str()) } else { None })
            .collect();
        assert_eq!(tools, ["Read", "Write"]);
        assert!(events.iter().any(|e| matches!(e, AiEvent::ToolCompleted { ok: true, summary: Some(s), .. } if s.contains("pamplemousse"))));
        assert!(events.iter().any(|e| matches!(e, AiEvent::FileChanged { path, change: FileChange::Added } if path.ends_with("ok.txt"))));

        let approval = items.iter().find_map(|i| if let StreamItem::Approval(p) = i { Some(p) } else { None }).unwrap();
        assert_eq!(approval.input["content"], "ok");
        assert!(events.iter().any(|e| matches!(e, AiEvent::ApprovalRequested { kind: ApprovalKind::FileChange, title, .. } if title == "Write ok.txt")));
        let response = approval_response(approval, ApprovalDecision::Allow);
        assert_eq!(response["response"]["request_id"], approval.request_id.as_str());
        assert_eq!(response["response"]["response"]["behavior"], "allow");
        assert_eq!(approval_response(approval, ApprovalDecision::AllowSession)["response"]["response"]["updatedPermissions"][0]["mode"], "acceptEdits");

        let turns: Vec<_> = events.iter().filter(|e| matches!(e, AiEvent::TurnCompleted { ok: true, interrupted: false, .. })).collect();
        assert_eq!(turns.len(), 2);
        assert!(events.iter().any(|e| matches!(e, AiEvent::Usage { output_tokens, cost_usd: Some(_), .. } if *output_tokens > 0)));
    }

    #[test]
    fn reports_an_interrupted_turn() {
        let (_, items) = replay("interrupt");
        let events = events(&items);
        assert!(matches!(events.last(), Some(AiEvent::TurnCompleted { ok: false, interrupted: true, error: None, .. })), "{:?}", events.last());
        assert!(events.iter().any(|e| matches!(e, AiEvent::MessageDelta { .. })));
    }

    #[test]
    fn reports_authentication_errors_even_with_a_success_subtype() {
        let mut stream = ClaudeStream::default();
        let mut items = stream.push(&json!({ "type": "assistant", "error": "authentication_failed",
            "message": { "id": "m1", "content": [{ "type": "text", "text": "Invalid API key · Please run /login" }] } }));
        items.extend(stream.push(&json!({ "type": "result", "subtype": "success", "is_error": true, "result": "Invalid API key", "duration_ms": 5 })));
        let events = events(&items);
        assert!(matches!(events[0], AiEvent::Error { kind: AiErrorKind::Auth, .. }));
        assert!(matches!(events[1], AiEvent::TurnCompleted { ok: false, error: Some(e), .. } if e.contains("/login")));
    }

    #[test]
    fn maps_shell_commands_and_todo_lists() {
        let mut stream = ClaudeStream::default();
        let mut items = stream.push(&json!({ "type": "assistant", "message": { "id": "m1", "content": [
            { "type": "tool_use", "id": "t1", "name": "Bash", "input": { "command": "cargo test" } },
            { "type": "tool_use", "id": "t2", "name": "TodoWrite", "input": { "todos": [
                { "content": "Lire", "status": "completed" }, { "content": "Écrire", "status": "in_progress" } ] } },
            { "type": "tool_use", "id": "t3", "name": "mcp__cadence__gestion-projet__issue_get", "input": { "number": 1 } } ] } }));
        items.extend(stream.push(&json!({ "type": "user", "message": { "content": [
            { "type": "tool_result", "tool_use_id": "t1", "is_error": true, "content": [{ "type": "text", "text": "échec" }] },
            { "type": "tool_result", "tool_use_id": "t3", "content": "{\"title\":\"Bug\"}" } ] } })));
        let events = events(&items);
        assert!(matches!(events[0], AiEvent::Command { output: None, .. }));
        assert!(matches!(events[1], AiEvent::Plan { items } if items[1].status == PlanStatus::InProgress));
        assert!(matches!(events[2], AiEvent::ToolStarted { source: ToolSource::Cadence { plugin, .. }, .. } if plugin == "gestion-projet"));
        assert!(matches!(events[3], AiEvent::Command { exit_code: Some(1), output: Some(o), .. } if o == "échec"));
        assert!(matches!(events[4], AiEvent::ToolCompleted { ok: true, .. }));
    }

    #[test]
    fn turns_ask_user_question_into_a_question_card_and_answers_it() {
        let mut stream = ClaudeStream::default();
        let line = json!({ "type": "control_request", "request_id": "q-1", "request": {
            "subtype": "can_use_tool", "tool_name": "AskUserQuestion", "tool_use_id": "toolu_q",
            "input": { "questions": [
                { "question": "Quel format ?", "header": "Format", "multiSelect": false,
                  "options": [{ "label": "Résumé", "description": "Court" }, { "label": "Détaillé", "description": "Long" }] },
                { "question": "Quelles sections ?", "header": "Sections", "multiSelect": true,
                  "options": [{ "label": "Intro", "description": "" }, { "label": "Conclusion", "description": "" }] }
            ] } } });
        let items = stream.push(&line);
        let [StreamItem::Event(AiEvent::Question { request, text, options, questions }), StreamItem::Approval(pending)] = items.as_slice() else {
            panic!("{items:?}")
        };
        assert_eq!(request, "q-1");
        assert_eq!(text, "Quel format ?\nQuelles sections ?");
        assert_eq!(options, &["Résumé", "Détaillé"]);
        assert_eq!(questions[0].id, "Quel format ?", "la clé de `answers` est le texte de la question");
        assert_eq!(questions[0].options[0].description.as_deref(), Some("Court"));
        assert_eq!(questions[1].options[0].description, None);
        assert!(questions[1].multi_select && questions[0].other);
        assert_eq!(pending.tool, ASK_USER_QUESTION);

        let answers = [
            QuestionAnswer { id: "Quel format ?".into(), answers: vec!["Résumé".into()] },
            QuestionAnswer { id: "Quelles sections ?".into(), answers: vec!["Intro".into(), "Conclusion".into()] },
        ];
        let response = question_response(pending, &answers);
        assert_eq!(response["response"]["request_id"], "q-1");
        let body = &response["response"]["response"];
        assert_eq!(body["behavior"], "allow");
        assert_eq!(body["updatedInput"]["questions"].as_array().unwrap().len(), 2, "entrée d'origine gardée");
        assert_eq!(body["updatedInput"]["answers"]["Quel format ?"], "Résumé");
        assert_eq!(body["updatedInput"]["answers"]["Quelles sections ?"], "Intro, Conclusion");
        let denied = background_question_denial(pending);
        assert_eq!(denied["response"]["response"]["behavior"], "deny");
        assert_eq!(denied["response"]["response"]["message"], BACKGROUND_ANSWER);
    }
}
