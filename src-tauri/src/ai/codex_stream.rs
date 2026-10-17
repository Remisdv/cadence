//! Lecture du protocole `codex app-server` (JSON-RPC 2.0, une ligne par message), plan § 4.3,
//! version de référence 0.163.0 : réponses aux requêtes de Cadence, notifications traduites en
//! `AiEvent`, requêtes du serveur (approbations) à relayer. Lecture tolérante : champs et
//! méthodes inconnus ignorés.

use serde_json::{Value, json};

use super::events::{
    AiErrorKind, AiEvent, ApprovalDecision, ApprovalKind, FileChange, PlanItem, PlanStatus, ToolSource, truncate,
};

/// Ce qu'une ligne du serveur demande à la session.
#[derive(Debug, Clone, PartialEq)]
pub enum CodexItem {
    /// Réponse à une requête de Cadence.
    Response { id: u64, result: Result<Value, String> },
    Event(AiEvent),
    /// Demande d'approbation relayée au chat (l'événement `ApprovalRequested` suit).
    Approval(PendingCodex),
    /// Requête du serveur que Cadence ne sait pas traiter : réponse immédiate.
    Reply(Value),
}

/// Approbation en attente de la réponse de l'utilisateur.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingCodex {
    /// Identifiant de la requête JSON-RPC, renvoyé tel quel.
    pub id: Value,
    pub request: String,
}

/// Réponse JSON-RPC à une approbation.
pub fn approval_reply(pending: &PendingCodex, decision: ApprovalDecision) -> Value {
    let decision = match decision {
        ApprovalDecision::Allow => "accept",
        ApprovalDecision::AllowSession => "acceptForSession",
        ApprovalDecision::Deny => "decline",
    };
    json!({ "jsonrpc": "2.0", "id": pending.id, "result": { "decision": decision } })
}

/// Requête de Cadence.
pub fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

/// Notification de Cadence (sans réponse).
pub fn notification(method: &str) -> Value {
    json!({ "jsonrpc": "2.0", "method": method })
}

/// Entrée texte d'un tour (`turn/start`).
pub fn text_input(text: &str) -> Value {
    json!([{ "type": "text", "text": text, "text_elements": [] }])
}

/// État d'un fil : jetons du dernier tour, erreur définitive vue avant `turn/completed`.
#[derive(Debug, Default)]
pub struct CodexStream {
    usage: Option<(u32, u32)>,
    error_sent: bool,
}

fn str_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// Erreur d'un tour : 401/403 → connexion, quota → crédit épuisé.
pub fn error_kind(error: &Value) -> AiErrorKind {
    let info = error.get("codexErrorInfo").map(Value::to_string).unwrap_or_default();
    let message = str_of(error, "message").unwrap_or_default().to_lowercase();
    if info.contains("\"httpStatusCode\":401") || info.contains("\"httpStatusCode\":403") || info.contains("unauthorized")
        || message.contains("401 unauthorized")
    {
        AiErrorKind::Auth
    } else if info.contains("usageLimit") || info.contains("quota") || message.contains("usage limit") {
        AiErrorKind::Quota
    } else {
        AiErrorKind::Other
    }
}

/// Message lisible d'une erreur de tour.
fn error_message(error: &Value) -> String {
    match error_kind(error) {
        AiErrorKind::Auth => "Codex CLI n'est pas connecté : lancez « codex login », puis relancez le message.".into(),
        AiErrorKind::Quota => {
            format!("Quota de Codex atteint : {}", str_of(error, "message").unwrap_or("réessayez plus tard"))
        }
        _ => str_of(error, "message").unwrap_or("erreur de Codex").to_string(),
    }
}

impl CodexStream {
    /// Traduit une ligne du serveur.
    pub fn push_line(&mut self, line: &str) -> Vec<CodexItem> {
        let Ok(message) = serde_json::from_str::<Value>(line.trim()) else { return Vec::new() };
        let method = str_of(&message, "method");
        let id = message.get("id");
        match (method, id) {
            (None, Some(id)) => {
                let Some(id) = id.as_u64() else { return Vec::new() };
                let result = match message.get("error") {
                    Some(error) => Err(str_of(error, "message").unwrap_or("erreur JSON-RPC").to_string()),
                    None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                };
                vec![CodexItem::Response { id, result }]
            }
            (Some(method), Some(id)) => self.server_request(method, id.clone(), message.get("params").unwrap_or(&Value::Null)),
            (Some(method), None) => self
                .notification(method, message.get("params").unwrap_or(&Value::Null))
                .into_iter()
                .map(CodexItem::Event)
                .collect(),
            (None, None) => Vec::new(),
        }
    }

    fn server_request(&mut self, method: &str, id: Value, params: &Value) -> Vec<CodexItem> {
        let request = match &id {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let approval = |kind: ApprovalKind, title: String| {
            vec![
                CodexItem::Approval(PendingCodex { id: id.clone(), request: request.clone() }),
                CodexItem::Event(AiEvent::ApprovalRequested { request: request.clone(), kind, title, detail: params.clone() }),
            ]
        };
        match method {
            "item/commandExecution/requestApproval" => {
                let title = str_of(params, "command")
                    .or_else(|| str_of(params, "reason"))
                    .unwrap_or("Exécuter une commande")
                    .to_string();
                approval(ApprovalKind::Command, title)
            }
            "item/fileChange/requestApproval" => {
                let title = str_of(params, "reason").unwrap_or("Modifier des fichiers").to_string();
                approval(ApprovalKind::FileChange, title)
            }
            // Ancien protocole : même sens, autre forme de réponse (refus par défaut).
            "applyPatchApproval" | "execCommandApproval" => {
                vec![CodexItem::Reply(json!({ "jsonrpc": "2.0", "id": id, "result": { "decision": "denied" } }))]
            }
            _ => vec![CodexItem::Reply(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("{method} n'est pas pris en charge par Cadence") }
            }))],
        }
    }

    fn notification(&mut self, method: &str, params: &Value) -> Vec<AiEvent> {
        match method {
            "item/agentMessage/delta" => match (str_of(params, "itemId"), str_of(params, "delta")) {
                (Some(item), Some(delta)) if !delta.is_empty() => {
                    vec![AiEvent::MessageDelta { message: item.into(), text: delta.into() }]
                }
                _ => Vec::new(),
            },
            "item/started" => params.get("item").map(started).unwrap_or_default(),
            "item/completed" => params.get("item").map(completed).unwrap_or_default(),
            "turn/plan/updated" => {
                let items = params
                    .get("plan")
                    .and_then(Value::as_array)
                    .map(|steps| {
                        steps
                            .iter()
                            .filter_map(|s| {
                                let status = match str_of(s, "status")? {
                                    "completed" => PlanStatus::Completed,
                                    "inProgress" => PlanStatus::InProgress,
                                    _ => PlanStatus::Pending,
                                };
                                Some(PlanItem { text: str_of(s, "step")?.into(), status })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                vec![AiEvent::Plan { items }]
            }
            "thread/tokenUsage/updated" => {
                let last = params.pointer("/tokenUsage/last");
                if let Some(last) = last {
                    let n = |k: &str| super::events::saturating_u32(last.get(k));
                    self.usage = Some((n("inputTokens"), n("outputTokens")));
                }
                Vec::new()
            }
            "error" => {
                if params.get("willRetry").and_then(Value::as_bool).unwrap_or(false) {
                    return Vec::new();
                }
                let error = params.get("error").cloned().unwrap_or(Value::Null);
                self.error_sent = true;
                vec![AiEvent::Error { kind: error_kind(&error), message: error_message(&error) }]
            }
            "mcpServer/startupStatus/updated"
                if str_of(params, "name") == Some("cadence") && str_of(params, "status") == Some("failed") =>
            {
                vec![AiEvent::Error {
                    kind: AiErrorKind::Other,
                    message: format!(
                        "Les outils des plugins sont indisponibles : la passerelle de Cadence n'a pas répondu ({}).",
                        str_of(params, "error").unwrap_or("raison inconnue")
                    ),
                }]
            }
            "turn/completed" => {
                let turn = params.get("turn").unwrap_or(&Value::Null);
                let status = str_of(turn, "status").unwrap_or("completed");
                let error = turn.get("error").filter(|e| !e.is_null());
                let mut events = Vec::new();
                if let Some((input_tokens, output_tokens)) = self.usage.take() {
                    events.push(AiEvent::Usage { input_tokens, output_tokens, cost_usd: None });
                }
                if status == "failed" && !self.error_sent {
                    let error = error.cloned().unwrap_or(json!({ "message": "Le tour a échoué." }));
                    events.push(AiEvent::Error { kind: error_kind(&error), message: error_message(&error) });
                }
                self.error_sent = false;
                events.push(AiEvent::TurnCompleted {
                    ok: status == "completed",
                    interrupted: status == "interrupted",
                    error: error.map(error_message).filter(|_| status == "failed"),
                    duration_ms: super::events::saturating_u32(turn.get("durationMs")),
                });
                events
            }
            _ => Vec::new(),
        }
    }
}

/// Début d'un élément du tour.
fn started(item: &Value) -> Vec<AiEvent> {
    let id = str_of(item, "id").unwrap_or_default().to_string();
    match str_of(item, "type") {
        Some("commandExecution") => vec![AiEvent::Command {
            call: id,
            command: str_of(item, "command").unwrap_or_default().into(),
            exit_code: None,
            output: None,
        }],
        Some("mcpToolCall") => {
            let server = str_of(item, "server").unwrap_or_default();
            let tool = str_of(item, "tool").unwrap_or_default();
            vec![AiEvent::ToolStarted {
                call: id,
                source: ToolSource::from_mcp(server, tool),
                name: format!("mcp__{server}__{tool}"),
                input: item.get("arguments").cloned().unwrap_or(Value::Null),
            }]
        }
        Some("webSearch") => vec![AiEvent::ToolStarted {
            call: id,
            source: ToolSource::Builtin,
            name: "web_search".into(),
            input: item.get("query").cloned().unwrap_or(Value::Null),
        }],
        Some("dynamicToolCall") => vec![AiEvent::ToolStarted {
            call: id,
            source: ToolSource::Builtin,
            name: str_of(item, "tool").unwrap_or("outil").into(),
            input: item.get("arguments").cloned().unwrap_or(Value::Null),
        }],
        _ => Vec::new(),
    }
}

/// Fin d'un élément du tour.
fn completed(item: &Value) -> Vec<AiEvent> {
    let id = str_of(item, "id").unwrap_or_default().to_string();
    match str_of(item, "type") {
        Some("agentMessage") => {
            let text = str_of(item, "text").unwrap_or_default();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![AiEvent::MessageCompleted { message: id, text: text.into() }]
            }
        }
        Some("reasoning") => {
            let join = |key: &str| {
                item.get(key)
                    .and_then(Value::as_array)
                    .map(|parts| parts.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"))
                    .unwrap_or_default()
            };
            let text = [join("summary"), join("content")].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n");
            if text.is_empty() { Vec::new() } else { vec![AiEvent::Reasoning { text }] }
        }
        Some("commandExecution") => vec![AiEvent::Command {
            call: id,
            command: str_of(item, "command").unwrap_or_default().into(),
            exit_code: item.get("exitCode").and_then(Value::as_i64).map(|c| c as i32),
            output: str_of(item, "aggregatedOutput").map(truncate),
        }],
        Some("fileChange") => item
            .get("changes")
            .and_then(Value::as_array)
            .map(|changes| {
                changes
                    .iter()
                    .filter_map(|c| {
                        let change = match c.pointer("/kind/type").and_then(Value::as_str) {
                            Some("add") => FileChange::Added,
                            Some("delete") => FileChange::Deleted,
                            _ => FileChange::Modified,
                        };
                        Some(AiEvent::FileChanged { path: str_of(c, "path")?.into(), change })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Some("mcpToolCall") => {
            let error = item.get("error").filter(|e| !e.is_null());
            let result = item.get("result").filter(|r| !r.is_null());
            let ok = str_of(item, "status") == Some("completed") && error.is_none();
            let text = result
                .and_then(|r| r.get("content"))
                .and_then(Value::as_array)
                .and_then(|parts| parts.iter().find_map(|p| str_of(p, "text")))
                .map(|t| truncate(t));
            vec![AiEvent::ToolCompleted {
                call: id,
                ok,
                summary: error.and_then(|e| str_of(e, "message")).map(str::to_string).or(text),
                output: result.and_then(|r| r.get("structuredContent").filter(|s| !s.is_null()).cloned()),
            }]
        }
        Some("webSearch") | Some("dynamicToolCall") => vec![AiEvent::ToolCompleted {
            call: id,
            ok: item.get("success").and_then(Value::as_bool).unwrap_or(true),
            summary: None,
            output: None,
        }],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(lines: &str) -> Vec<AiEvent> {
        let mut stream = CodexStream::default();
        lines
            .lines()
            .flat_map(|l| stream.push_line(l))
            .filter_map(|i| match i {
                CodexItem::Event(e) => Some(e),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn reads_a_turn_with_a_tool_call() {
        let fixture = include_str!("../../tests/fixtures/ai/codex/tool-call.jsonl");
        let got = events(fixture);
        assert!(got.contains(&AiEvent::MessageDelta { message: "msg_1".into(), text: "J'appelle ".into() }));
        assert!(got.iter().any(|e| matches!(e, AiEvent::ToolStarted { source: ToolSource::Cadence { plugin, tool }, .. }
            if plugin == "exemple" && tool == "echo")));
        assert!(got.iter().any(|e| matches!(e, AiEvent::ToolCompleted { ok: true, summary: Some(s), .. } if s == "bonjour")));
        assert!(got.contains(&AiEvent::Command {
            call: "cmd_1".into(),
            command: "ls".into(),
            exit_code: Some(0),
            output: Some("a.txt\n".into())
        }));
        assert!(got.contains(&AiEvent::FileChanged { path: "notes.md".into(), change: FileChange::Added }));
        assert!(got.contains(&AiEvent::MessageCompleted { message: "msg_1".into(), text: "J'appelle l'outil : bonjour.".into() }));
        assert!(matches!(got[got.len() - 2], AiEvent::Usage { input_tokens: 1200, output_tokens: 40, .. }));
        assert_eq!(
            got.last(),
            Some(&AiEvent::TurnCompleted { ok: true, interrupted: false, error: None, duration_ms: 2300 })
        );
    }

    #[test]
    fn reads_an_authentication_failure_recorded_from_codex() {
        // Enregistré avec codex-cli 0.163.0 sans compte connecté.
        let fixture = include_str!("../../tests/fixtures/ai/codex/auth-error.jsonl");
        let got = events(fixture);
        let errors: Vec<_> = got.iter().filter(|e| matches!(e, AiEvent::Error { .. })).collect();
        assert_eq!(errors.len(), 2, "passerelle injoignable + connexion, sans les tentatives : {errors:?}");
        assert!(matches!(errors[1], AiEvent::Error { kind: AiErrorKind::Auth, .. }));
        assert!(matches!(got.last(), Some(AiEvent::TurnCompleted { ok: false, interrupted: false, error: Some(_), .. })));
    }

    #[test]
    fn relays_approvals_and_refuses_the_rest() {
        let mut stream = CodexStream::default();
        let items = stream.push_line(
            r#"{"jsonrpc":"2.0","id":7,"method":"item/commandExecution/requestApproval","params":{"threadId":"t","turnId":"u","itemId":"i","startedAtMs":1,"environmentId":null,"command":"rm -rf build"}}"#,
        );
        let CodexItem::Approval(pending) = &items[0] else { panic!("{items:?}") };
        assert_eq!(pending.request, "7");
        assert!(matches!(&items[1], CodexItem::Event(AiEvent::ApprovalRequested { kind: ApprovalKind::Command, title, .. }) if title == "rm -rf build"));
        assert_eq!(approval_reply(pending, ApprovalDecision::AllowSession)["result"]["decision"], "acceptForSession");
        assert_eq!(approval_reply(pending, ApprovalDecision::Deny)["id"], 7);

        let other = stream.push_line(r#"{"jsonrpc":"2.0","id":"x","method":"attestation/generate","params":{}}"#);
        assert!(matches!(&other[0], CodexItem::Reply(v) if v["error"]["code"] == -32601));
        let response = stream.push_line(r#"{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"u"}}}"#);
        assert!(matches!(&response[0], CodexItem::Response { id: 3, result: Ok(v) } if v["turn"]["id"] == "u"));
    }

    #[test]
    fn reads_an_interrupted_turn() {
        let got = events(r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"t","turn":{"id":"u","status":"interrupted","error":null,"durationMs":900}}}"#);
        assert_eq!(got, vec![AiEvent::TurnCompleted { ok: false, interrupted: true, error: None, duration_ms: 900 }]);
    }
}
