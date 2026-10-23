//! Détection : chemin du binaire, version, état de connexion. Reprend `auth/cli.rs`
//! (même lecture de version) et l'étend aux outils du registre.

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use super::registry::{Manager, Os, ToolId};
use crate::{
    auth::{cli, external::parse_gh_login},
    domain::auth::CliTest,
};

const QUICK_TIMEOUT: Duration = Duration::from_secs(15);
const TEST_TIMEOUT: Duration = Duration::from_secs(60);
const TEST_PROMPT: &str = "Réponds uniquement par OK.";

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Emplacements usuels hors `PATH` : un outil installé pendant que Cadence tourne n'est pas
/// encore dans le `PATH` du processus (winget, installeurs officiels).
pub fn usual_locations(tool: ToolId, home: Option<&Path>) -> Vec<PathBuf> {
    let bin = tool.binary();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = home {
        match tool {
            ToolId::Claude => dirs.push(home.join(".claude").join("local")),
            ToolId::Opencode => dirs.push(home.join(".opencode").join("bin")),
            _ => {}
        }
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".npm-global").join("bin"));
    }
    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            dirs.push(PathBuf::from(appdata).join("npm"));
        }
        let program_files =
            std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
        let program_files = PathBuf::from(program_files);
        match tool {
            ToolId::Git => dirs.push(program_files.join("Git").join("cmd")),
            ToolId::Gh => dirs.push(program_files.join("GitHub CLI")),
            ToolId::Node => dirs.push(program_files.join("nodejs")),
            _ => {}
        }
    } else {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/usr/bin"));
    }

    let names: Vec<String> = if cfg!(windows) {
        vec![format!("{bin}.exe"), format!("{bin}.cmd")]
    } else {
        vec![bin.to_string()]
    };
    dirs.iter()
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .collect()
}

/// Chemin choisi à la main, puis `PATH`, puis emplacements usuels.
pub fn locate(tool: ToolId, configured: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = configured.map(PathBuf::from) {
        return path.is_file().then_some(path);
    }
    which::which(tool.binary()).ok().or_else(|| {
        usual_locations(tool, home_dir().as_deref())
            .into_iter()
            .find(|p| p.is_file())
    })
}

pub async fn version(path: &Path) -> Result<String, String> {
    cli::read_version(path).await.map_err(|e| e.to_string())
}

/// `true` si `version` est au moins `min` (versions illisibles : acceptées).
pub fn meets(version: &str, min: &str) -> bool {
    // Trois premiers nombres seulement : « 2.52.0.windows.1 », « 1.2.3-beta.1 ».
    let triple = |v: &str| -> Option<[u64; 3]> {
        let mut parts = v.split('.').map(|p| {
            p.split(|c: char| !c.is_ascii_digit())
                .next()?
                .parse::<u64>()
                .ok()
        });
        Some([
            parts.next()??,
            parts.next()??,
            parts.next().flatten().unwrap_or(0),
        ])
    };
    match (triple(version), triple(min)) {
        (Some(v), Some(m)) => v >= m,
        _ => true,
    }
}

/// État de connexion d'un outil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Login {
    /// Rien à connecter, ou état inconnu (ancienne version sans commande d'état).
    Unknown,
    Connected(Option<String>),
    NotConnected,
}

struct Output {
    ok: bool,
    text: String,
}

async fn run(path: &Path, args: &[&str], timeout: Duration) -> Option<Output> {
    let mut cmd = cli::command(path);
    cmd.args(args).current_dir(std::env::temp_dir());
    let output = tokio::time::timeout(timeout, cmd.output())
        .await
        .ok()?
        .ok()?;
    Some(Output {
        ok: output.status.success(),
        text: format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    })
}

pub async fn login_state(tool: ToolId, path: &Path) -> Login {
    match tool {
        ToolId::Gh => match run(
            path,
            &["auth", "status", "--hostname", "github.com", "--active"],
            QUICK_TIMEOUT,
        )
        .await
        {
            Some(out) if out.ok => {
                Login::Connected(parse_gh_login(&out.text).map(|l| format!("@{l}")))
            }
            Some(_) => Login::NotConnected,
            None => Login::Unknown,
        },
        ToolId::Claude => match run(path, &["auth", "status"], QUICK_TIMEOUT).await {
            Some(out) => parse_claude_auth(&out.text).unwrap_or(Login::Unknown),
            None => Login::Unknown,
        },
        ToolId::Codex => match run(path, &["login", "status"], QUICK_TIMEOUT).await {
            Some(out) if out.ok => Login::Connected(parse_codex_login(&out.text)),
            Some(_) => Login::NotConnected,
            None => Login::Unknown,
        },
        // Aucun modèle listé tant qu'aucun fournisseur n'est connecté.
        ToolId::Opencode => match run(path, &["models"], QUICK_TIMEOUT).await {
            Some(out) if out.ok && out.text.lines().any(|l| l.contains('/')) => {
                Login::Connected(None)
            }
            Some(_) => Login::NotConnected,
            None => Login::Unknown,
        },
        ToolId::Git | ToolId::Node => Login::Unknown,
    }
}

/// `claude auth status` (JSON) : `loggedIn`, `authMethod`, `subscriptionType`, `email`.
pub fn parse_claude_auth(text: &str) -> Option<Login> {
    let start = text.find('{')?;
    let json: serde_json::Value = serde_json::from_str(text[start..].trim()).ok()?;
    if !json.get("loggedIn")?.as_bool()? {
        return Some(Login::NotConnected);
    }
    let field = |k: &str| {
        json.get(k)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
    };
    let label = match (field("subscriptionType"), field("authMethod")) {
        (Some(plan), _) => {
            let mut chars = plan.chars();
            let plan: String = chars
                .next()
                .map(|c| c.to_uppercase().chain(chars).collect())
                .unwrap_or_default();
            Some(format!("Abonnement {plan}"))
        }
        (None, Some(method)) if method.contains("api") || method.contains("key") => {
            Some("Clé API".into())
        }
        (None, Some(_)) => Some("Compte Claude".into()),
        (None, None) => None,
    };
    let account = match (label, field("email")) {
        (Some(l), Some(e)) => Some(format!("{l} · {e}")),
        (l, e) => l.or(e.map(str::to_string)),
    };
    Some(Login::Connected(account))
}

/// « Logged in using ChatGPT » → « Compte ChatGPT » ; « … an API key … » → « Clé API ».
pub fn parse_codex_login(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if lower.contains("api key") {
        Some("Clé API".into())
    } else if lower.contains("chatgpt") {
        Some("Compte ChatGPT".into())
    } else {
        None
    }
}

/// Système affiché dans l'en-tête (« Windows 11 », « macOS 15.1 », « Ubuntu 24.04 LTS »).
pub async fn platform_label() -> String {
    match Os::current() {
        Os::Windows => {
            let build = match run(Path::new("cmd"), &["/C", "ver"], QUICK_TIMEOUT).await {
                Some(out) => parse_windows_build(&out.text),
                None => None,
            };
            match build {
                Some(b) if b >= 22000 => "Windows 11".into(),
                Some(_) => "Windows 10".into(),
                None => "Windows".into(),
            }
        }
        Os::Macos => match run(Path::new("sw_vers"), &["-productVersion"], QUICK_TIMEOUT).await {
            Some(out) if out.ok => format!("macOS {}", out.text.trim()),
            _ => "macOS".into(),
        },
        Os::Linux => std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|s| {
                s.lines().find_map(|l| {
                    l.strip_prefix("PRETTY_NAME=")
                        .map(|v| v.trim_matches('"').to_string())
                })
            })
            .unwrap_or_else(|| "Linux".into()),
    }
}

/// « Microsoft Windows [version 10.0.26200.6899] » → 26200
pub fn parse_windows_build(text: &str) -> Option<u32> {
    let start = text.find("10.0.")? + 5;
    text[start..]
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// Gestionnaires présents, avec leur version (« winget 1.29.380 »).
pub async fn managers(os: Os) -> Vec<(Manager, String)> {
    let candidates: &[Manager] = match os {
        Os::Windows => &[Manager::Winget, Manager::Npm],
        Os::Macos => &[Manager::Brew, Manager::Npm],
        Os::Linux => &[Manager::Apt, Manager::Dnf, Manager::Pacman, Manager::Npm],
    };
    let mut found = Vec::new();
    for &manager in candidates {
        let Ok(path) = which::which(manager.probe()) else {
            continue;
        };
        let version = match run(&path, &["--version"], QUICK_TIMEOUT).await {
            Some(out) => cli::parse_version(&out.text).or_else(|| first_number(&out.text)),
            None => None,
        };
        let name = match manager {
            Manager::Apt => "apt",
            other => other.probe(),
        };
        found.push((
            manager,
            version
                .map(|v| format!("{name} {v}"))
                .unwrap_or_else(|| name.to_string()),
        ));
    }
    found
}

/// Première suite de chiffres et de points (« v1.29.380 » → « 1.29.380 »).
fn first_number(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|t| t.trim_start_matches('v'))
        .find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(str::to_string)
}

/// Gestionnaire disponible sur la machine (pour griser une recette).
pub fn manager_available(manager: Manager) -> bool {
    which::which(manager.probe()).is_ok()
}

/// Session de test de quelques secondes : l'IA répond-elle, avec le compte connecté ?
pub async fn ai_test(tool: ToolId, path: &Path) -> CliTest {
    if tool == ToolId::Claude {
        return cli::run_test(path).await.0;
    }
    let started = Instant::now();
    let args: &[&str] = match tool {
        ToolId::Codex => &["exec", "--skip-git-repo-check", TEST_PROMPT],
        _ => &["run", TEST_PROMPT],
    };
    let result = run(path, args, TEST_TIMEOUT).await;
    let latency_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
    match result {
        None => CliTest {
            ok: false,
            latency_ms,
            message: Some(format!("pas de réponse en {} s", TEST_TIMEOUT.as_secs())),
        },
        Some(out) if out.ok => CliTest {
            ok: true,
            latency_ms,
            message: None,
        },
        Some(out) => CliTest {
            ok: false,
            latency_ms,
            message: out
                .text
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_claude_auth_status() {
        let json = r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","email":"a@b.fr","subscriptionType":"max"}"#;
        assert_eq!(
            parse_claude_auth(json),
            Some(Login::Connected(Some("Abonnement Max · a@b.fr".into())))
        );
        assert_eq!(
            parse_claude_auth(r#"{"loggedIn":false}"#),
            Some(Login::NotConnected)
        );
        assert_eq!(
            parse_claude_auth(r#"{"loggedIn":true,"authMethod":"api-key"}"#),
            Some(Login::Connected(Some("Clé API".into())))
        );
        assert_eq!(parse_claude_auth("error: unknown command 'auth'"), None);
    }

    #[test]
    fn reads_codex_login_status() {
        assert_eq!(
            parse_codex_login("Logged in using ChatGPT\n").as_deref(),
            Some("Compte ChatGPT")
        );
        assert_eq!(
            parse_codex_login("Logged in using an API key - sk-***").as_deref(),
            Some("Clé API")
        );
    }

    #[test]
    fn compares_versions() {
        assert!(meets("2.52.0.windows.1", "2.30.0"));
        assert!(!meets("2.29.9", "2.30.0"));
        assert!(!meets("1.9.0", "2.0.0"));
        assert!(meets("22.11.0", "18.0.0"));
        assert!(meets("inconnue", "1.0.0"));
    }

    #[test]
    fn reads_windows_build() {
        assert_eq!(
            parse_windows_build("\r\nMicrosoft Windows [version 10.0.26200.6899]\r\n"),
            Some(26200)
        );
        assert_eq!(parse_windows_build("rien"), None);
    }

    #[test]
    fn looks_in_install_folders() {
        let home = PathBuf::from("/home/me");
        assert!(
            usual_locations(ToolId::Opencode, Some(&home))
                .iter()
                .any(|p| p.starts_with(home.join(".opencode")))
        );
    }
}
