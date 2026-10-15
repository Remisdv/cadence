//! Détection des agents de code locaux (Claude Code, Codex CLI).
//!
//! Pas d'OAuth ici : ces CLI ont déjà leur propre session sur la machine. Cadence cherche le
//! binaire (réglage manuel, puis `PATH`, puis emplacements usuels — une app graphique macOS
//! n'hérite pas du `PATH` du shell), lit sa version, et peut lancer une courte session de test.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use tokio::process::Command;

use crate::{
    domain::auth::{AuthProvider, CliAgentInfo, CliTest, ProviderStatus},
    error::{AppError, AppResult},
};

const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
const TEST_TIMEOUT: Duration = Duration::from_secs(45);
const TEST_PROMPT: &str = "Réponds uniquement par OK.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliTool {
    ClaudeCode,
    Codex,
}

impl CliTool {
    pub const ALL: [CliTool; 2] = [CliTool::ClaudeCode, CliTool::Codex];

    pub fn from_provider(provider: AuthProvider) -> AppResult<Self> {
        match provider {
            AuthProvider::ClaudeCode => Ok(CliTool::ClaudeCode),
            AuthProvider::Codex => Ok(CliTool::Codex),
            AuthProvider::Github => {
                Err(AppError::Validation("GitHub n'est pas un agent de code.".into()))
            }
        }
    }

    pub fn provider(self) -> AuthProvider {
        match self {
            CliTool::ClaudeCode => AuthProvider::ClaudeCode,
            CliTool::Codex => AuthProvider::Codex,
        }
    }

    pub fn binary(self) -> &'static str {
        match self {
            CliTool::ClaudeCode => super::CLAUDE_CODE_BIN,
            CliTool::Codex => super::CODEX_BIN,
        }
    }

    /// Seul Claude Code a une session de test : son mode headless renvoie un JSON exploitable.
    pub fn supports_test(self) -> bool {
        self == CliTool::ClaudeCode
    }
}

/// Emplacements usuels hors `PATH`.
pub fn usual_locations(tool: CliTool, home: Option<&Path>) -> Vec<PathBuf> {
    let bin = tool.binary();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = home {
        if tool == CliTool::ClaudeCode {
            dirs.push(home.join(".claude").join("local"));
        }
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".npm-global").join("bin"));
    }
    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            dirs.push(PathBuf::from(appdata).join("npm"));
        }
    } else {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }

    let names: Vec<String> = if cfg!(windows) {
        vec![format!("{bin}.exe"), format!("{bin}.cmd")]
    } else {
        vec![bin.to_string()]
    };
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).collect()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Chemin du binaire : réglage manuel, puis `PATH`, puis emplacements usuels.
pub fn locate(tool: CliTool, configured: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = configured.map(PathBuf::from) {
        return path.is_file().then_some(path);
    }
    which::which(tool.binary())
        .ok()
        .or_else(|| usual_locations(tool, home_dir().as_deref()).into_iter().find(|p| p.is_file()))
}

/// Première suite `x.y.z` d'une sortie `--version` (« 2.4.1 (Claude Code) », « codex-cli 0.46.0 »).
pub fn parse_version(output: &str) -> Option<String> {
    let numeric = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    output.split(|c: char| c.is_whitespace() || c == '(' || c == ')').find_map(|token| {
        let token = token.trim_start_matches('v');
        let mut parts = token.splitn(3, '.');
        let (major, minor, rest) = (parts.next()?, parts.next()?, parts.next()?);
        // Le segment patch peut porter un suffixe : « 3-beta.1 ».
        let patch = rest.split(|c: char| !c.is_ascii_digit()).next().unwrap_or("");
        (numeric(major) && numeric(minor) && numeric(patch)).then(|| token.to_string())
    })
}

fn command(path: &Path) -> Command {
    let mut cmd = Command::new(path);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    {
        // Pas de fenêtre de console qui clignote à chaque appel.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub async fn read_version(path: &Path) -> AppResult<String> {
    let output = tokio::time::timeout(VERSION_TIMEOUT, command(path).arg("--version").output())
        .await
        .map_err(|_| AppError::Internal(format!("{} --version ne répond pas", path.display())))??;
    let text = String::from_utf8_lossy(&output.stdout);
    parse_version(&text).ok_or_else(|| {
        AppError::Validation(format!("{} ne ressemble pas au binaire attendu", path.display()))
    })
}

/// Détection sans test : chemin + version.
pub async fn detect(tool: CliTool, configured: Option<&str>, enabled: bool) -> CliAgentInfo {
    let Some(path) = locate(tool, configured) else {
        // Introuvable et écarté par l'utilisateur (« Ignorer ») : désactivé plutôt qu'introuvable.
        return CliAgentInfo {
            status: if enabled { ProviderStatus::NotFound } else { ProviderStatus::Disconnected },
            ..CliAgentInfo::not_found()
        };
    };
    let binary_path = Some(path.display().to_string());
    match read_version(&path).await {
        Ok(version) => CliAgentInfo {
            status: if !enabled {
                ProviderStatus::Disconnected
            } else if tool.supports_test() {
                ProviderStatus::Pending
            } else {
                ProviderStatus::Connected
            },
            binary_path,
            version: Some(version),
            auth_method: None,
            test: None,
        },
        Err(err) => CliAgentInfo {
            status: ProviderStatus::Error,
            binary_path,
            version: None,
            auth_method: None,
            test: Some(CliTest { ok: false, latency_ms: 0, message: Some(err.to_string()) }),
        },
    }
}

/// Ce qu'on retient d'une session de test en `stream-json`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TestOutput {
    pub ok: bool,
    pub message: Option<String>,
    pub api_key_source: Option<String>,
}

/// Lecture tolérante du flux : on ne garde que l'événement `system/init` et le `result` final,
/// en ignorant les lignes et champs inconnus (le format du CLI peut évoluer).
pub fn parse_test_output(stdout: &str) -> TestOutput {
    let mut out = TestOutput::default();
    let mut saw_result = false;
    for line in stdout.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        match event.get("type").and_then(|t| t.as_str()) {
            Some("system") if event.get("subtype").and_then(|s| s.as_str()) == Some("init") => {
                out.api_key_source =
                    event.get("apiKeySource").and_then(|s| s.as_str()).map(str::to_string);
            }
            Some("result") => {
                saw_result = true;
                let is_error = event.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false);
                out.ok = !is_error && event.get("subtype").and_then(|s| s.as_str()) == Some("success");
                if !out.ok {
                    out.message = event.get("result").and_then(|r| r.as_str()).map(str::to_string);
                }
            }
            _ => {}
        }
    }
    if !saw_result {
        out.message = Some("aucun résultat renvoyé par la session".into());
    }
    out
}

/// Libellé du mode d'authentification à partir de `apiKeySource`.
pub fn auth_method_label(api_key_source: Option<&str>) -> Option<String> {
    match api_key_source? {
        "none" => Some("Compte Claude".into()),
        source if source.contains("ANTHROPIC_API_KEY") => Some("Clé API".into()),
        other => Some(other.to_string()),
    }
}

/// Session de test de quelques secondes : vérifie que le CLI répond et qu'il est authentifié.
/// Renvoie le résultat et, s'il est connu, le mode d'authentification.
pub async fn run_test(path: &Path) -> (CliTest, Option<String>) {
    let started = Instant::now();
    let mut cmd = command(path);
    cmd.args(["-p", TEST_PROMPT, "--output-format", "stream-json", "--verbose"])
        .args(["--max-turns", "1", "--model", "haiku"])
        // Hors de tout dépôt : la session ne lit aucun CLAUDE.md de projet.
        .current_dir(std::env::temp_dir());

    let result = tokio::time::timeout(TEST_TIMEOUT, cmd.output()).await;
    let latency_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
    let fail = |message: String| (CliTest { ok: false, latency_ms, message: Some(message) }, None);

    match result {
        Err(_) => fail(format!("pas de réponse en {} s", TEST_TIMEOUT.as_secs())),
        Ok(Err(err)) => fail(format!("lancement impossible : {err}")),
        Ok(Ok(output)) => {
            let parsed = parse_test_output(&String::from_utf8_lossy(&output.stdout));
            let message = parsed.message.or_else(|| {
                (!output.status.success())
                    .then(|| String::from_utf8_lossy(&output.stderr).trim().to_string())
                    .filter(|s| !s.is_empty())
            });
            let auth = auth_method_label(parsed.api_key_source.as_deref());
            (CliTest { ok: parsed.ok, latency_ms, message: if parsed.ok { None } else { message } }, auth)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions_from_cli_output() {
        assert_eq!(parse_version("2.4.1 (Claude Code)\n"), Some("2.4.1".into()));
        assert_eq!(parse_version("codex-cli 0.46.0"), Some("0.46.0".into()));
        assert_eq!(parse_version("v1.2.3-beta.1"), Some("1.2.3-beta.1".into()));
        assert_eq!(parse_version("command not found"), None);
    }

    #[test]
    fn reads_a_successful_test_session() {
        let stdout = r#"{"type":"system","subtype":"init","apiKeySource":"none","model":"claude-haiku"}
{"type":"assistant","message":{"content":[{"type":"text","text":"OK"}]}}
{"type":"result","subtype":"success","is_error":false,"result":"OK","duration_ms":812}"#;
        let out = parse_test_output(stdout);
        assert!(out.ok);
        assert_eq!(out.api_key_source.as_deref(), Some("none"));
        assert_eq!(auth_method_label(out.api_key_source.as_deref()).as_deref(), Some("Compte Claude"));
    }

    #[test]
    fn reads_a_failed_test_session() {
        let stdout = r#"{"type":"result","subtype":"success","is_error":true,"result":"Invalid API key · Please run /login"}"#;
        let out = parse_test_output(stdout);
        assert!(!out.ok);
        assert_eq!(out.message.as_deref(), Some("Invalid API key · Please run /login"));
        assert!(!parse_test_output("garbage\n").ok);
    }

    #[test]
    fn looks_in_usual_places() {
        let home = PathBuf::from("/home/me");
        let places = usual_locations(CliTool::ClaudeCode, Some(&home));
        assert!(places.iter().any(|p| p.starts_with(home.join(".claude").join("local"))));
    }

    /// Détection réelle sur la machine : `cargo test -- --ignored detects_local_agents --nocapture`
    #[test]
    #[ignore]
    fn detects_local_agents() {
        tauri::async_runtime::block_on(async {
            for tool in CliTool::ALL {
                println!("{tool:?} → {:?}", detect(tool, None, true).await);
            }
        });
    }
}
