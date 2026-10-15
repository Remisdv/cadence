//! Comptes GitHub déjà présents sur la machine : pas d'OAuth App à créer pour Cadence.
//!
//! - **CLI GitHub** (`gh auth token`) : reprise du jeton de `gh`, sans navigateur.
//! - **Git Credential Manager** (`git credential fill`) : GCM a sa propre application OAuth
//!   enregistrée chez GitHub ; s'il n'a pas encore de session, il ouvre le navigateur et
//!   récupère la main tout seul.
//!
//! Les jetons lus ici ne sont jamais affichés : ils sont validés auprès de GitHub puis copiés
//! dans le trousseau, sous l'entrée de Cadence.

use std::{process::Stdio, time::Duration};

use secrecy::SecretString;
use tokio::{io::AsyncWriteExt, process::Command};

use crate::error::{AppError, AppResult};

const QUICK_TIMEOUT: Duration = Duration::from_secs(10);
/// Laisse le temps de se connecter dans le navigateur.
const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const GITHUB_CREDENTIAL_QUERY: &str = "protocol=https\nhost=github.com\n\n";

fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    {
        // Pas de console qui clignote ; les fenêtres de GCM restent visibles.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/* ---------- CLI GitHub ---------- */

/// Compte actif de `gh` sur github.com, sans lire le jeton.
pub async fn gh_active_login() -> Option<String> {
    let mut cmd = command("gh");
    cmd.args(["auth", "status", "--hostname", "github.com", "--active"]).stdin(Stdio::null());
    let output = tokio::time::timeout(QUICK_TIMEOUT, cmd.output()).await.ok()?.ok()?;
    // Selon la version, le texte sort sur stdout ou stderr.
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    output.status.success().then(|| parse_gh_login(&text)).flatten()
}

/// « ✓ Logged in to github.com account Remisdv (keyring) » → `Remisdv`
pub fn parse_gh_login(status: &str) -> Option<String> {
    status.lines().find_map(|line| {
        let rest = line.split("Logged in to github.com account ").nth(1)?;
        rest.split_whitespace().next().map(str::to_string)
    })
}

pub async fn gh_token() -> AppResult<SecretString> {
    let mut cmd = command("gh");
    cmd.args(["auth", "token", "--hostname", "github.com"]).stdin(Stdio::null());
    let output = tokio::time::timeout(QUICK_TIMEOUT, cmd.output())
        .await
        .map_err(|_| AppError::Internal("gh auth token ne répond pas".into()))?
        .map_err(|e| AppError::Validation(format!("CLI GitHub introuvable : {e}")))?;
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() || token.is_empty() {
        return Err(AppError::Validation(
            "La CLI GitHub n'est pas connectée : lancez `gh auth login`.".into(),
        ));
    }
    Ok(SecretString::from(token))
}

/* ---------- Git Credential Manager ---------- */

pub async fn credential_manager_available() -> bool {
    let mut cmd = command("git");
    cmd.args(["credential-manager", "--version"]).stdin(Stdio::null());
    matches!(tokio::time::timeout(QUICK_TIMEOUT, cmd.output()).await, Ok(Ok(o)) if o.status.success())
}

/// Identifiants renvoyés par `git credential fill`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCredential {
    pub username: Option<String>,
    pub password: String,
}

pub fn parse_credential(output: &str) -> Option<GitCredential> {
    let field = |key: &str| {
        output.lines().find_map(|l| l.strip_prefix(key).and_then(|v| v.strip_prefix('=')).map(str::to_string))
    };
    Some(GitCredential { username: field("username"), password: field("password").filter(|p| !p.is_empty())? })
}

async fn run_credential(action: &str, input: &str, timeout: Duration) -> AppResult<String> {
    let mut cmd = command("git");
    cmd.args(["credential", action])
        .stdin(Stdio::piped())
        // Jamais d'invite dans un terminal : seule l'interface de GCM peut demander quelque chose.
        .env("GIT_TERMINAL_PROMPT", "0");
    let mut child = cmd.spawn().map_err(|e| AppError::Validation(format!("git introuvable : {e}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).await?;
    }
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| AppError::Validation("Aucune connexion terminée dans le navigateur.".into()))??;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AppError::Validation(if stderr.is_empty() {
            "Connexion annulée.".into()
        } else {
            format!("Git Credential Manager : {stderr}")
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Jeton GitHub de GCM : immédiat s'il a déjà une session, sinon connexion dans le navigateur.
pub async fn credential_manager_token() -> AppResult<GitCredential> {
    let output = run_credential("fill", GITHUB_CREDENTIAL_QUERY, INTERACTIVE_TIMEOUT).await?;
    parse_credential(&output)
        .ok_or_else(|| AppError::Validation("Git Credential Manager n'a renvoyé aucun jeton.".into()))
}

/// Oublie une session GCM refusée par GitHub, pour que le prochain `fill` rouvre le navigateur.
pub async fn credential_manager_reject(credential: &GitCredential) -> AppResult<()> {
    let mut input = String::from("protocol=https\nhost=github.com\n");
    if let Some(user) = &credential.username {
        input.push_str(&format!("username={user}\n"));
    }
    input.push_str(&format!("password={}\n\n", credential.password));
    run_credential("reject", &input, QUICK_TIMEOUT).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_active_gh_account() {
        let status = "github.com\n  ✓ Logged in to github.com account Remisdv (keyring)\n  - Active account: true\n";
        assert_eq!(parse_gh_login(status).as_deref(), Some("Remisdv"));
        assert_eq!(parse_gh_login("You are not logged into any GitHub hosts."), None);
    }

    #[test]
    fn reads_git_credential_output() {
        let out = "protocol=https\nhost=github.com\nusername=Remisdv\npassword=gho_secret\n";
        assert_eq!(
            parse_credential(out),
            Some(GitCredential { username: Some("Remisdv".into()), password: "gho_secret".into() })
        );
        assert_eq!(parse_credential("protocol=https\nhost=github.com\n"), None);
    }

    /// Détection réelle : `cargo test --lib -- --ignored detects_local_github_sources --nocapture`
    #[test]
    #[ignore]
    fn detects_local_github_sources() {
        tauri::async_runtime::block_on(async {
            println!("gh → {:?}", gh_active_login().await);
            println!("GCM → {}", credential_manager_available().await);
        });
    }
}
