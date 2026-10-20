//! Lecture du flux d'événements de `opencode serve` (`GET /event`, SSE), plan § 4.4, version de
//! référence 1.19 : événements d'une session traduits en `AiEvent`. Lecture tolérante : les
//! types inconnus (dont les événements « v2 » `session.next.*`) sont ignorés.

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use serde_json::Value;

use super::events::{
    AiErrorKind, AiEvent, AiQuestion, ApprovalKind, FileChange, PlanItem, PlanStatus, ToolSource, question_options, truncate,
};

/// État d'une session pendant un tour.
#[derive(Debug, Default)]
pub struct OpencodeStream {
    session: String,
    user_messages: HashSet<String>,
    /// Type de chaque morceau vu (`text`, `reasoning`, `tool`…).
    parts: HashMap<String, String>,
    finished_parts: HashSet<String>,
    started_tools: HashSet<String>,
    input_tokens: u64,
    output_tokens: u64,
    cost: f64,
    error: Option<String>,
    aborted: bool,
    busy_since: Option<Instant>,
}

fn str_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// Une ligne `data:` du flux SSE, décodée.
pub fn sse_data(line: &str) -> Option<Value> {
    let data = line.strip_prefix("data:")?.trim();
    serde_json::from_str(data).ok()
}

impl OpencodeStream {
    pub fn new(session: &str) -> Self {
        Self { session: session.to_string(), ..Self::default() }
    }

    /// Un tour est en cours côté serveur (`session.status` = `busy`).
    pub fn is_busy(&self) -> bool {
        self.busy_since.is_some()
    }

    /// Traduit un événement du flux (déjà décodé).
    pub fn push(&mut self, event: &Value) -> Vec<AiEvent> {
        let kind = str_of(event, "type").unwrap_or_default();
        let props = event.get("properties").unwrap_or(&Value::Null);
        // Événements d'une autre session : ignorés (ceux sans session, comme file.edited, passent).
        if let Some(session) = str_of(props, "sessionID")
            && session != self.session
        {
            return Vec::new();
        }
        match kind {
            "message.updated" => {
                let info = props.get("info").unwrap_or(&Value::Null);
                if str_of(info, "role") == Some("user")
                    && let Some(id) = str_of(info, "id")
                {
                    self.user_messages.insert(id.to_string());
                }
                Vec::new()
            }
            "message.part.updated" => props.get("part").map(|p| self.part(p)).unwrap_or_default(),
            "message.part.delta" => {
                let part = str_of(props, "partID").unwrap_or_default();
                let delta = str_of(props, "delta").unwrap_or_default();
                let from_user = str_of(props, "messageID").is_some_and(|m| self.user_messages.contains(m));
                if str_of(props, "field") == Some("text")
                    && !delta.is_empty()
                    && !from_user
                    && self.parts.get(part).map(String::as_str) == Some("text")
                {
                    vec![AiEvent::MessageDelta { message: part.into(), text: delta.into() }]
                } else {
                    Vec::new()
                }
            }
            "file.edited" if self.is_busy() => str_of(props, "file")
                .map(|path| vec![AiEvent::FileChanged { path: path.into(), change: FileChange::Modified }])
                .unwrap_or_default(),
            "todo.updated" => {
                let items = props
                    .get("todos")
                    .and_then(Value::as_array)
                    .map(|todos| {
                        todos
                            .iter()
                            .filter_map(|t| {
                                let status = match str_of(t, "status")? {
                                    "in_progress" => PlanStatus::InProgress,
                                    "completed" | "cancelled" => PlanStatus::Completed,
                                    _ => PlanStatus::Pending,
                                };
                                Some(PlanItem { text: str_of(t, "content")?.into(), status })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                vec![AiEvent::Plan { items }]
            }
            "permission.asked" => {
                let Some(id) = str_of(props, "id") else { return Vec::new() };
                let permission = str_of(props, "permission").unwrap_or("outil");
                let kind = match permission {
                    "bash" => ApprovalKind::Command,
                    "edit" | "write" | "patch" => ApprovalKind::FileChange,
                    _ => ApprovalKind::Tool,
                };
                let patterns: Vec<&str> =
                    props.get("patterns").and_then(Value::as_array).map(|p| p.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                let title = if patterns.is_empty() { permission.to_string() } else { format!("{permission} : {}", patterns.join(", ")) };
                vec![AiEvent::ApprovalRequested { request: id.into(), kind, title, detail: props.clone() }]
            }
            "question.asked" => {
                let Some(id) = str_of(props, "id") else { return Vec::new() };
                // Questions dans l'ordre (la réponse est une liste dans le même ordre) ; l'id
                // d'une question est son texte. `custom` (réponse libre) vaut vrai par défaut.
                let questions = props
                    .get("questions")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|q| {
                        let question = str_of(q, "question")?.to_string();
                        Some(AiQuestion {
                            id: question.clone(),
                            header: str_of(q, "header").filter(|h| !h.is_empty()).map(str::to_string),
                            options: question_options(q.get("options")),
                            multi_select: q.get("multiple").and_then(Value::as_bool).unwrap_or(false),
                            other: q.get("custom").and_then(Value::as_bool).unwrap_or(true),
                            question,
                        })
                    })
                    .collect();
                vec![AiEvent::question(id.into(), questions)]
            }
            "session.status" => {
                if props.pointer("/status/type").and_then(Value::as_str) == Some("busy") && self.busy_since.is_none() {
                    self.busy_since = Some(Instant::now());
                }
                Vec::new()
            }
            "session.error" => {
                let error = props.get("error").unwrap_or(&Value::Null);
                let name = str_of(error, "name").unwrap_or_default();
                let message = error.pointer("/data/message").and_then(Value::as_str).unwrap_or("erreur d'OpenCode").to_string();
                if name == "MessageAbortedError" {
                    self.aborted = true;
                    return Vec::new();
                }
                let status = error.pointer("/data/statusCode").and_then(Value::as_u64);
                let kind = match (name, status) {
                    ("ProviderAuthError", _) | (_, Some(401 | 403)) => AiErrorKind::Auth,
                    (_, Some(429)) => AiErrorKind::Quota,
                    _ => AiErrorKind::Other,
                };
                let message = match kind {
                    AiErrorKind::Auth => format!("OpenCode n'est pas connecté à ce fournisseur : lancez « opencode auth login ». ({message})"),
                    _ => message,
                };
                self.error = Some(message.clone());
                vec![AiEvent::Error { kind, message }]
            }
            "session.idle" => self.finish(),
            _ => Vec::new(),
        }
    }

    fn part(&mut self, part: &Value) -> Vec<AiEvent> {
        let id = str_of(part, "id").unwrap_or_default().to_string();
        let kind = str_of(part, "type").unwrap_or_default();
        if str_of(part, "messageID").is_some_and(|m| self.user_messages.contains(m))
            || part.get("synthetic").and_then(Value::as_bool) == Some(true)
        {
            return Vec::new();
        }
        self.parts.insert(id.clone(), kind.to_string());
        let ended = part.pointer("/time/end").is_some();
        match kind {
            "text" if ended && self.finished_parts.insert(id.clone()) => {
                let text = str_of(part, "text").unwrap_or_default();
                if text.is_empty() { Vec::new() } else { vec![AiEvent::MessageCompleted { message: id, text: text.into() }] }
            }
            "reasoning" if ended && self.finished_parts.insert(id.clone()) => {
                let text = str_of(part, "text").unwrap_or_default().trim();
                if text.is_empty() { Vec::new() } else { vec![AiEvent::Reasoning { text: text.into() }] }
            }
            "tool" => self.tool(part),
            "step-finish" => {
                let tokens = part.get("tokens").unwrap_or(&Value::Null);
                self.input_tokens += tokens.get("input").and_then(Value::as_u64).unwrap_or(0);
                self.output_tokens += tokens.get("output").and_then(Value::as_u64).unwrap_or(0);
                self.cost += part.get("cost").and_then(Value::as_f64).unwrap_or(0.0);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn tool(&mut self, part: &Value) -> Vec<AiEvent> {
        let call = str_of(part, "callID").or_else(|| str_of(part, "id")).unwrap_or_default().to_string();
        let name = str_of(part, "tool").unwrap_or("outil").to_string();
        let state = part.get("state").unwrap_or(&Value::Null);
        let status = str_of(state, "status").unwrap_or("pending");
        let input = state.get("input").cloned().unwrap_or(Value::Null);
        let bash = name == "bash";
        let command = || str_of(&input, "command").unwrap_or_default().to_string();
        let mut events = Vec::new();
        if status != "pending" && self.started_tools.insert(call.clone()) {
            events.push(if bash {
                AiEvent::Command { call: call.clone(), command: command(), exit_code: None, output: None }
            } else {
                AiEvent::ToolStarted { call: call.clone(), source: ToolSource::from_opencode_name(&name), name: name.clone(), input: input.clone() }
            });
        }
        match status {
            "completed" if bash => events.push(AiEvent::Command {
                call,
                command: command(),
                exit_code: state.pointer("/metadata/exit").and_then(Value::as_i64).map(|c| c as i32),
                output: str_of(state, "output").map(truncate),
            }),
            "completed" => events.push(AiEvent::ToolCompleted {
                call,
                ok: true,
                summary: str_of(state, "output").map(truncate).or_else(|| str_of(state, "title").map(str::to_string)),
                output: None,
            }),
            "error" if bash => events.push(AiEvent::Command {
                call,
                command: command(),
                exit_code: None,
                output: str_of(state, "error").map(truncate),
            }),
            "error" => events.push(AiEvent::ToolCompleted {
                call,
                ok: false,
                summary: str_of(state, "error").map(str::to_string),
                output: None,
            }),
            _ => {}
        }
        events
    }

    /// Fin du tour (`session.idle`) : consommation, puis `TurnCompleted` ; état remis à zéro.
    fn finish(&mut self) -> Vec<AiEvent> {
        let mut events = Vec::new();
        if self.input_tokens + self.output_tokens > 0 {
            let clamp = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
            events.push(AiEvent::Usage {
                input_tokens: clamp(self.input_tokens),
                output_tokens: clamp(self.output_tokens),
                cost_usd: (self.cost > 0.0).then_some(self.cost),
            });
        }
        let duration_ms = self.busy_since.map(|t| t.elapsed().as_millis().min(u32::MAX as u128) as u32).unwrap_or(0);
        let error = self.error.take();
        events.push(AiEvent::TurnCompleted { ok: error.is_none() && !self.aborted, interrupted: self.aborted, error, duration_ms });
        let session = std::mem::take(&mut self.session);
        let users = std::mem::take(&mut self.user_messages);
        *self = Self { session, user_messages: users, ..Self::default() };
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(fixture: &str, session: &str) -> Vec<AiEvent> {
        let mut stream = OpencodeStream::new(session);
        fixture.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).flat_map(|e| stream.push(&e)).collect()
    }

    #[test]
    fn reads_a_simple_answer_recorded_from_opencode() {
        // Enregistré avec opencode 1.19.3 (modèle gratuit d'OpenCode Zen).
        let got = events(include_str!("../../tests/fixtures/ai/opencode/simple.jsonl"), "ses_1");
        assert!(got.iter().any(|e| matches!(e, AiEvent::Reasoning { text } if text.contains("answer OK"))));
        assert!(got.contains(&AiEvent::MessageDelta { message: "prt_text".into(), text: "OK".into() }));
        assert!(got.contains(&AiEvent::MessageCompleted { message: "prt_text".into(), text: "OK".into() }));
        assert!(!got.iter().any(|e| matches!(e, AiEvent::MessageCompleted { text, .. } if text.contains("Réponds"))), "message de l'utilisateur écarté");
        assert!(got.iter().any(|e| matches!(e, AiEvent::Usage { input_tokens: 8130, output_tokens: 3, .. })));
        assert!(matches!(got.last(), Some(AiEvent::TurnCompleted { ok: true, interrupted: false, error: None, .. })));
    }

    #[test]
    fn reads_tools_commands_and_approvals() {
        let got = events(include_str!("../../tests/fixtures/ai/opencode/tool-call.jsonl"), "ses_1");
        assert!(got.iter().any(|e| matches!(e, AiEvent::ToolStarted { source: ToolSource::Cadence { plugin, tool }, .. }
            if plugin == "exemple" && tool == "echo")));
        assert!(got.iter().any(|e| matches!(e, AiEvent::ToolCompleted { ok: true, summary: Some(s), .. } if s == "bonjour")));
        assert!(got.iter().any(|e| matches!(e, AiEvent::ApprovalRequested { kind: ApprovalKind::Command, request, .. } if request == "per_1")));
        assert!(got.contains(&AiEvent::Command { call: "call_bash".into(), command: "ls".into(), exit_code: Some(0), output: Some("a.txt\n".into()) }));
        assert!(got.contains(&AiEvent::FileChanged { path: "notes.md".into(), change: FileChange::Modified }));
        assert!(got.iter().any(|e| matches!(e, AiEvent::Plan { items } if items.len() == 2)));
        assert_eq!(got.iter().filter(|e| matches!(e, AiEvent::TurnCompleted { .. })).count(), 1);
    }

    #[test]
    fn reads_aborted_and_failed_turns() {
        let aborted = r#"{"type":"session.status","properties":{"sessionID":"s","status":{"type":"busy"}}}
{"type":"session.error","properties":{"sessionID":"s","error":{"name":"MessageAbortedError","data":{"message":"Aborted"}}}}
{"type":"session.idle","properties":{"sessionID":"s"}}"#;
        let got = events(aborted, "s");
        assert!(matches!(got.as_slice(), [AiEvent::TurnCompleted { ok: false, interrupted: true, error: None, .. }]));

        let auth = r#"{"type":"session.error","properties":{"sessionID":"s","error":{"name":"ProviderAuthError","data":{"providerID":"openai","message":"clé absente"}}}}
{"type":"session.idle","properties":{"sessionID":"s"}}
{"type":"session.idle","properties":{"sessionID":"autre"}}"#;
        let got = events(auth, "s");
        assert!(matches!(&got[0], AiEvent::Error { kind: AiErrorKind::Auth, .. }));
        assert!(matches!(&got[1], AiEvent::TurnCompleted { ok: false, error: Some(_), .. }));
        assert_eq!(got.len(), 2, "autre session ignorée");
        assert_eq!(sse_data("data: {\"type\":\"server.connected\"}").unwrap()["type"], "server.connected");
    }

    #[test]
    fn reads_every_question_of_a_question_request() {
        let mut stream = OpencodeStream::new("ses_1");
        let got = stream.push(&serde_json::json!({ "type": "question.asked", "properties": {
            "id": "que_1", "sessionID": "ses_1",
            "questions": [
                { "question": "Quelle base ?", "header": "Base", "options": [{ "label": "SQLite", "description": "Locale" }] },
                { "question": "Quels tests ?", "header": "Tests", "multiple": true, "custom": false,
                  "options": [{ "label": "Unitaires", "description": "" }, { "label": "Intégration", "description": "" }] }
            ] } }));
        let [AiEvent::Question { request, questions, .. }] = got.as_slice() else { panic!("{got:?}") };
        assert_eq!(request, "que_1");
        assert_eq!(questions.len(), 2);
        assert_eq!((questions[0].id.as_str(), questions[0].other, questions[0].multi_select), ("Quelle base ?", true, false));
        assert_eq!((questions[1].other, questions[1].multi_select), (false, true));
        assert_eq!(questions[0].options[0].description.as_deref(), Some("Locale"));
    }
}
