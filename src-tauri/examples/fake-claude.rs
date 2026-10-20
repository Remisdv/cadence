//! `fake-claude` : imite `claude -p --input-format stream-json --output-format stream-json`
//! pour les tests de l'adaptateur et du chat (aucun appel réseau, aucun abonnement).
//!
//! Selon le message reçu :
//! - par défaut : répond « écho : <message> » (deltas mot à mot, message complet, résultat) ;
//! - « approbation » : demande `can_use_tool` (Write) et répond selon la décision ;
//! - « question » : pose deux questions `AskUserQuestion` (par `can_use_tool`, s'il y a un
//!   `--permission-prompt-tool`) et répond « réponses (2 questions) : Quel format ?=… | … » ou
//!   « question refusée : <message> » ;
//! - « long » : diffuse lentement, jusqu'à une interruption (`control_request interrupt`) ;
//! - « plante » : écrit sur stderr et quitte (code 1) ;
//! - « dossiers » : répond la liste des `--add-dir` reçus (« dossiers : a | b ») ;
//! - « mode » : répond le `--permission-mode` reçu (« mode [reprise] : acceptEdits ») ;
//! - « lis <chemin> » : demande `can_use_tool` (Read) et répond « lu : autorisé [dossiers
//!   ajoutés] » ou « lu : refusé ».
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
    let add_dirs: Vec<String> =
        args.windows(2).filter(|w| w[0] == "--add-dir").map(|w| w[1].clone()).collect();
    let permission_mode =
        args.iter().position(|a| a == "--permission-mode").and_then(|i| args.get(i + 1)).cloned().unwrap_or_default();
    let id = resume.unwrap_or_else(|| "fake-1".into());
    let mut session = Session { id, turn: 0 };

    let (tx, rx) = mpsc::channel::<Value>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if let Ok(value) = serde_json::from_str::<Value>(&line)
                && tx.send(value).is_err()
            {
                break;
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
                if let Some(path) = text.strip_prefix("lis ") {
                    let request = format!("lire-{}", session.turn);
                    emit(json!({ "type": "control_request", "request_id": request, "request": {
                        "subtype": "can_use_tool", "tool_name": "Read", "display_name": "Read", "description": path,
                        "input": { "file_path": path },
                        "permission_suggestions": [],
                        "tool_use_id": "toolu_read" } }));
                    let answer = loop {
                        match rx.recv() {
                            Ok(m) if m["type"] == "control_response" && m["response"]["request_id"] == request.as_str() => break m,
                            Ok(_) => continue,
                            Err(_) => return,
                        }
                    };
                    let response = &answer["response"]["response"];
                    let reply = if response["behavior"] == "allow" {
                        let added: Vec<String> = response["updatedPermissions"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter(|u| u["type"] == "addDirectories")
                            .flat_map(|u| u["directories"].as_array().cloned().unwrap_or_default())
                            .filter_map(|d| d.as_str().map(str::to_string))
                            .collect();
                        format!("lu : autorisé [{}]", added.join(" | "))
                    } else {
                        "lu : refusé".to_string()
                    };
                    session.start_text();
                    session.text(&reply);
                    session.result(false);
                    continue;
                }
                if text.contains("question") {
                    // `AskUserQuestion` passe par `can_use_tool` dans tous les modes, à condition
                    // d'avoir un outil de demande ; sinon la question revient vide.
                    session.start_text();
                    if !args.iter().any(|a| a == "--permission-prompt-tool") {
                        session.text("question : aucune réponse");
                        session.result(false);
                        continue;
                    }
                    let request = format!("question-{}", session.turn);
                    let questions = json!([
                        { "question": "Quel format ?", "header": "Format", "multiSelect": false,
                          "options": [{ "label": "Résumé", "description": "Court" }, { "label": "Détaillé", "description": "Long" }] },
                        { "question": "Quelles sections ?", "header": "Sections", "multiSelect": true,
                          "options": [{ "label": "Intro", "description": "" }, { "label": "Conclusion", "description": "" }] }
                    ]);
                    emit(json!({ "type": "control_request", "request_id": request, "request": {
                        "subtype": "can_use_tool", "tool_name": "AskUserQuestion", "display_name": "AskUserQuestion",
                        "input": { "questions": questions }, "permission_suggestions": [], "tool_use_id": "toolu_q" } }));
                    let answer = loop {
                        match rx.recv() {
                            Ok(m) if m["type"] == "control_response" && m["response"]["request_id"] == request.as_str() => break m,
                            Ok(_) => continue,
                            Err(_) => return,
                        }
                    };
                    let response = &answer["response"]["response"];
                    let reply = if response["behavior"] == "allow" {
                        let input = &response["updatedInput"];
                        let kept = input["questions"].as_array().map(Vec::len).unwrap_or(0);
                        let answers: Vec<String> = input["answers"]
                            .as_object()
                            .into_iter()
                            .flatten()
                            .map(|(q, a)| format!("{q}={}", a.as_str().unwrap_or_default()))
                            .collect();
                        format!("réponses ({kept} questions) : {}", answers.join(" | "))
                    } else {
                        format!("question refusée : {}", response["message"].as_str().unwrap_or_default())
                    };
                    session.text(&reply);
                    session.result(false);
                    continue;
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
                        if let Ok(m) = rx.try_recv()
                            && m["type"] == "control_request"
                            && m["request"]["subtype"] == "interrupt"
                        {
                            emit(json!({ "type": "control_response", "response": { "subtype": "success", "request_id": m["request_id"], "response": {} } }));
                            interrupted = true;
                            break;
                        }
                    }
                    session.result(interrupted);
                    continue;
                }
                let reply = if text == "mode" {
                    format!("mode{} : {permission_mode}", if resumed { " [reprise]" } else { "" })
                } else if text.contains("dossiers") {
                    format!("dossiers{} : {}", if resumed { " [reprise]" } else { "" }, add_dirs.join(" | "))
                } else if resumed {
                    format!("écho [reprise {}] : {text}", session.id)
                } else {
                    format!("écho : {text}")
                };
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
