//! Session Claude Code en headless : `claude -p --output-format stream-json`.
//!
//! Reprise de l'ancien moteur d'agents, en attente de l'adaptateur Claude Code (plan
//! Cadence 2, § 4.2) qui normalisera ces lignes en `AiEvent`.
//!
//! Consignes et prompt passent par l'entrée standard (jamais en argument : un wrapper `.cmd`
//! sous Windows casserait les retours à la ligne), les outils sont restreints à
//! la lecture du dépôt (garde-fou « portée limitée au dépôt »), la durée est plafonnée.
//! Chaque ligne du flux est traduite en ligne de console pour l'écran d'agent.

use std::{path::Path, process::Stdio, time::Duration};

use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::watch,
};

use crate::error::{AppError, AppResult};

/// Nature d'une ligne de console (couleur de la ligne dans le terminal du chat).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleKind {
    System,
    Read,
    Write,
    Message,
    Success,
    Question,
    Error,
}

/// Outils autorisés : lire, chercher, lister. Rien qui écrive ni n'exécute.
pub const READ_ONLY_TOOLS: &str = "Read,Grep,Glob";
/// Interdits explicitement, en plus de l'absence d'autorisation.
const DENIED_READ_ONLY: &str = "Bash,Edit,Write,MultiEdit,NotebookEdit,WebFetch,WebSearch,Task";

/// Agent développeur : écrire dans le worktree, construire et tester. Jamais de push, de
/// remote, de config Git ni d'accès réseau : seul Cadence pousse, après garde-fous.
pub const WRITE_TOOLS: &str = "Read,Grep,Glob,Edit,Write,MultiEdit,Bash(cargo:*),Bash(pnpm:*),Bash(npm:*),Bash(npx:*),Bash(yarn:*),Bash(node:*),Bash(bun:*),Bash(python:*),Bash(python3:*),Bash(pytest:*),Bash(go:*),Bash(make:*),Bash(dotnet:*),Bash(git status:*),Bash(git diff:*),Bash(git log:*),Bash(ls:*)";
const DENIED_WRITE: &str = "Bash(git push:*),Bash(git remote:*),Bash(git config:*),Bash(git commit:*),Bash(gh:*),Bash(curl:*),Bash(wget:*),WebFetch,WebSearch,Task";

/// Jeu d'outils d'une session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tools {
    ReadOnly,
    Write,
}

pub struct SessionOptions<'a> {
    pub cli: &'a Path,
    pub cwd: &'a Path,
    pub model: Option<&'a str>,
    /// Consignes de l'agent, placées en tête du prompt (absentes d'une reprise : la session
    /// les a déjà).
    pub system: Option<&'a str>,
    pub prompt: &'a str,
    /// Session à reprendre (réponses aux questions).
    pub resume: Option<&'a str>,
    pub max_turns: u32,
    pub timeout: Duration,
    pub tools: Tools,
}

#[derive(Debug, Default)]
pub struct SessionOutcome {
    pub session_id: Option<String>,
    /// Texte final de la session (`result`), ou à défaut le dernier message.
    pub text: String,
}

/// Lance la session et appelle `on_line` pour chaque ligne de console. `stop` passe à vrai
/// pour l'interrompre (le processus est tué).
pub async fn run(
    opts: SessionOptions<'_>,
    mut stop: watch::Receiver<bool>,
    mut on_line: impl FnMut(ConsoleKind, String),
) -> AppResult<SessionOutcome> {
    let mut cmd = crate::auth::cli::command(opts.cli);
    cmd.args(["-p", "--output-format", "stream-json", "--verbose"])
        .args(["--max-turns", &opts.max_turns.to_string()])
        .current_dir(opts.cwd)
        // Aucun identifiant Git à portée de l'agent : même un push qui passerait le filtre
        // d'outils échouerait.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "credential.helper")
        .env("GIT_CONFIG_VALUE_0", "")
        .stdin(Stdio::piped());
    match opts.tools {
        Tools::ReadOnly => cmd.args(["--allowedTools", READ_ONLY_TOOLS, "--disallowedTools", DENIED_READ_ONLY]),
        Tools::Write => cmd.args(["--allowedTools", WRITE_TOOLS, "--disallowedTools", DENIED_WRITE]),
    };
    if let Some(model) = opts.model {
        cmd.args(["--model", model]);
    }
    if let Some(session) = opts.resume {
        cmd.args(["--resume", session]);
    }
    let mut child = cmd.spawn().map_err(|e| AppError::Internal(format!("Claude Code ne démarre pas : {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        let input = match opts.system {
            Some(system) => format!("{system}

---

{}", opts.prompt),
            None => opts.prompt.to_string(),
        };
        stdin.write_all(input.as_bytes()).await?;
        // Fermer l'entrée : le CLI commence quand le prompt est complet.
        drop(stdin);
    }
    let stdout = child.stdout.take().ok_or_else(|| AppError::Internal("sortie de Claude Code absente".into()))?;
    let mut lines = BufReader::new(stdout).lines();
    let mut outcome = SessionOutcome::default();
    let mut last_text = String::new();
    let mut failure: Option<String> = None;

    let deadline = tokio::time::sleep(opts.timeout);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line? else { break };
                let Ok(event) = serde_json::from_str::<Value>(&line) else { continue };
                if let Some(id) = event.get("session_id").and_then(Value::as_str) {
                    outcome.session_id.get_or_insert_with(|| id.to_string());
                }
                match event.get("type").and_then(Value::as_str) {
                    Some("assistant") => {
                        if let Some(text) = assistant_text(&event) {
                            last_text = text;
                        }
                    }
                    Some("result") => {
                        let text = event.get("result").and_then(Value::as_str).unwrap_or_default();
                        if event.get("is_error").and_then(Value::as_bool).unwrap_or(false)
                            || event.get("subtype").and_then(Value::as_str) != Some("success")
                        {
                            failure = Some(if text.is_empty() { "session interrompue".into() } else { text.to_string() });
                        } else {
                            outcome.text = text.to_string();
                        }
                    }
                    _ => {}
                }
                for (kind, text) in console_lines(&event) {
                    on_line(kind, text);
                }
            }
            _ = &mut deadline => {
                let _ = child.kill().await;
                return Err(AppError::Validation(format!(
                    "Session arrêtée : plafond de {} min atteint.", opts.timeout.as_secs() / 60
                )));
            }
            _ = stop.changed() => {
                if *stop.borrow() {
                    let _ = child.kill().await;
                    return Err(AppError::Validation("Session arrêtée à votre demande.".into()));
                }
            }
        }
    }

    let status = child.wait().await?;
    if let Some(message) = failure {
        return Err(AppError::Validation(explain_failure(&message)));
    }
    if outcome.text.is_empty() {
        outcome.text = last_text;
    }
    if outcome.text.is_empty() && !status.success() {
        let mut stderr = String::new();
        if let Some(mut err) = child.stderr.take() {
            use tokio::io::AsyncReadExt;
            let _ = err.read_to_string(&mut stderr).await;
        }
        return Err(AppError::Validation(explain_failure(stderr.trim())));
    }
    Ok(outcome)
}

/// Message d'échec lisible (session expirée, quota du modèle…).
fn explain_failure(message: &str) -> String {
    let lower = message.to_lowercase();
    if lower.contains("login") || lower.contains("expired") || lower.contains("api key") {
        format!("Claude Code n'est pas connecté ({message}). Lancez « claude » puis « /login ».")
    } else if message.is_empty() {
        "Claude Code s'est arrêté sans réponse.".into()
    } else {
        format!("Claude Code : {message}")
    }
}

fn assistant_text(event: &Value) -> Option<String> {
    let blocks = event["message"]["content"].as_array()?;
    let text: Vec<&str> = blocks
        .iter()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect();
    (!text.is_empty()).then(|| text.join("\n"))
}

/// Traduit un événement stream-json en lignes de console (`read  src/x.rs`, texte de
/// l'agent…). Le bloc JSON final de l'agent n'est pas affiché : l'écran le montre en clair.
pub fn console_lines(event: &Value) -> Vec<(ConsoleKind, String)> {
    match event.get("type").and_then(Value::as_str) {
        Some("system") if event["subtype"] == "init" => {
            let model = event["model"].as_str().unwrap_or("modèle par défaut");
            vec![(ConsoleKind::System, format!("› session démarrée ({model})"))]
        }
        Some("assistant") => event["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|block| match block["type"].as_str() {
                Some("text") => {
                    let text = block["text"].as_str().unwrap_or_default();
                    let visible = text.split("```json").next().unwrap_or_default();
                    visible
                        .lines()
                        .map(str::trim_end)
                        .filter(|l| !l.trim().is_empty())
                        .map(|l| (ConsoleKind::Message, l.to_string()))
                        .collect::<Vec<_>>()
                }
                Some("tool_use") => vec![tool_line(block)],
                _ => vec![],
            })
            .collect(),
        _ => vec![],
    }
}

fn tool_line(block: &Value) -> (ConsoleKind, String) {
    let input = &block["input"];
    let path = || short_path(input["file_path"].as_str().unwrap_or("?"));
    match block["name"].as_str().unwrap_or_default() {
        "Read" => (ConsoleKind::Read, format!("read  {}", path())),
        "Grep" => (ConsoleKind::Read, format!("grep  \"{}\"", input["pattern"].as_str().unwrap_or("?"))),
        "Glob" => (ConsoleKind::Read, format!("glob  {}", input["pattern"].as_str().unwrap_or("?"))),
        "Write" => (ConsoleKind::Write, format!("write {}", path())),
        "Edit" | "MultiEdit" => (ConsoleKind::Write, format!("edit  {}", path())),
        "Bash" => {
            let command = input["command"].as_str().unwrap_or("?").lines().next().unwrap_or("?");
            (ConsoleKind::Read, format!("bash  {command}"))
        }
        other => (ConsoleKind::System, format!("outil {other}")),
    }
}

/// Chemin relatif au dépôt quand c'est possible (le clone vit dans un dossier de l'app).
fn short_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    for marker in ["/repos/", "/wt/"] {
        if let Some((_, rest)) = normalized.split_once(marker) {
            // repos/<owner>/<name>/<chemin>
            if let Some(rel) = rest.splitn(3, '/').nth(2) {
                return rel.to_string();
            }
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn translates_stream_events_into_console_lines() {
        let init = json!({"type": "system", "subtype": "init", "model": "claude-opus", "session_id": "s1"});
        assert_eq!(console_lines(&init), [(ConsoleKind::System, "› session démarrée (claude-opus)".into())]);

        let tools = json!({"type": "assistant", "message": {"content": [
            {"type": "tool_use", "name": "Read", "input": {"file_path": "C:\\Users\\me\\AppData\\cadence\\repos\\me\\app\\src\\api\\client.rs"}},
            {"type": "tool_use", "name": "Grep", "input": {"pattern": "ratelimit"}}
        ]}});
        assert_eq!(
            console_lines(&tools),
            [(ConsoleKind::Read, "read  src/api/client.rs".into()), (ConsoleKind::Read, "grep  \"ratelimit\"".into())]
        );

        let text = json!({"type": "assistant", "message": {"content": [
            {"type": "text", "text": "Le client ne distingue pas les 403.\n\n```json\n{\"body\": \"…\"}\n```"}
        ]}});
        assert_eq!(console_lines(&text), [(ConsoleKind::Message, "Le client ne distingue pas les 403.".into())]);
        assert!(console_lines(&json!({"type": "user"})).is_empty());
    }

    #[test]
    fn explains_authentication_failures() {
        assert!(explain_failure("OAuth session expired").contains("/login"));
        assert_eq!(explain_failure(""), "Claude Code s'est arrêté sans réponse.");
    }
}
