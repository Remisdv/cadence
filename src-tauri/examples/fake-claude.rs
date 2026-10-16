//! `fake-claude` : imite `claude -p --input-format stream-json --output-format stream-json`
//! pour les tests de l'adaptateur et du chat (aucun appel réseau, aucun abonnement).
//!
//! Selon le message reçu :
//! - par défaut : répond « écho : <message> » (deltas mot à mot, message complet, résultat) ;
//! - « approbation » : demande `can_use_tool` (Write) et répond selon la décision ;
//! - « long » : diffuse lentement, jusqu'à une interruption (`control_request interrupt`) ;
//! - « plante » : écrit sur stderr et quitte (code 1).
//!
//! L'id de session vaut l'argument de `--resume` s'il y en a un, sinon `fake-1`.

use std::{
    io::{BufRead, Write},
    sync::mpsc,
    time::Duration,
};

use serde_json::{Value, json};

fn emit(value: Value) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{value}").unwrap();
    out.flush().unwrap();
}

struct Session {
    id: String,
    turn: u32,
}

impl Session {
    fn init(&self) {
        emit(json!({ "type": "system", "subtype": "init", "session_id": self.id, "model": "fake-model", "cwd": std::env::current_dir().unwrap() }));
    }

    fn message_id(&self) -> String {
        format!("msg_{}", self.turn)
    }

    fn start_text(&self) {
        emit(json!({ "type": "stream_event", "event": { "type": "message_start", "message": { "id": self.message_id() } } }));
        emit(json!({ "type": "stream_event", "event": { "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } } }));
    }

    fn delta(&self, text: &str) {
        emit(json!({ "type": "stream_event", "event": { "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": text } } }));
    }

    fn text(&self, text: &str) {
        emit(json!({ "type": "assistant", "message": { "id": self.message_id(), "content": [{ "type": "text", "text": text }] } }));
    }

    fn result(&self, interrupted: bool) {
        emit(json!({
            "type": "result", "subtype": if interrupted { "error_during_execution" } else { "success" },
            "is_error": interrupted, "duration_ms": 42, "total_cost_usd": 0.001,
            "terminal_reason": if interrupted { "aborted_streaming" } else { "completed" },
            "usage": { "input_tokens": 10, "output_tokens": 5 }
        }));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let resume = args.iter().position(|a| a == "--resume").and_then(|i| args.get(i + 1)).cloned();
    let resumed = resume.is_some();
    let id = resume.unwrap_or_else(|| "fake-1".into());
    let mut session = Session { id, turn: 0 };

    let (tx, rx) = mpsc::channel::<Value>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                if tx.send(value).is_err() {
                    break;
                }
            }
        }
    });

    while let Ok(message) = rx.recv() {
        match message["type"].as_str() {
            Some("control_request") => {
                emit(json!({ "type": "control_response", "response": { "subtype": "success", "request_id": message["request_id"], "response": {} } }));
            }
            Some("user") => {
                session.turn += 1;
                let text = message["message"]["content"].as_str().unwrap_or_default().to_string();
                session.init();
                if text.contains("plante") {
                    eprintln!("panique simulée");
                    std::process::exit(1);
                }
                if text.contains("approbation") {
                    let request = format!("perm-{}", session.turn);
                    emit(json!({ "type": "control_request", "request_id": request, "request": {
                        "subtype": "can_use_tool", "tool_name": "Write", "display_name": "Write", "description": "ok.txt",
                        "input": { "file_path": "ok.txt", "content": "ok" },
                        "permission_suggestions": [{ "type": "setMode", "mode": "acceptEdits", "destination": "session" }],
                        "tool_use_id": "toolu_1" } }));
                    let answer = loop {
                        match rx.recv() {
                            Ok(m) if m["type"] == "control_response" && m["response"]["request_id"] == request.as_str() => break m,
                            Ok(_) => continue,
                            Err(_) => return,
                        }
                    };
                    let allowed = answer["response"]["response"]["behavior"] == "allow";
                    session.start_text();
                    session.text(if allowed { "autorisé" } else { "refusé" });
                    session.result(false);
                    continue;
                }
                session.start_text();
                if text.contains("long") {
                    let mut interrupted = false;
                    for n in 0..200 {
                        session.delta(&format!("vers {n}\n"));
                        std::thread::sleep(Duration::from_millis(50));
                        if let Ok(m) = rx.try_recv() {
                            if m["type"] == "control_request" && m["request"]["subtype"] == "interrupt" {
                                emit(json!({ "type": "control_response", "response": { "subtype": "success", "request_id": m["request_id"], "response": {} } }));
                                interrupted = true;
                                break;
                            }
                        }
                    }
                    session.result(interrupted);
                    continue;
                }
                let reply = if resumed { format!("écho [reprise {}] : {text}", session.id) } else { format!("écho : {text}") };
                for word in reply.split_inclusive(' ') {
                    session.delta(word);
                }
                session.text(&reply);
                session.result(false);
            }
            _ => {}
        }
    }
}
