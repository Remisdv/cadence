//! Copie du dépôt lue par les agents : `<données de l'app>/repos/<owner>/<nom>`.
//!
//! Clone superficiel (`--depth 1`) de la branche par défaut, remis à jour avant chaque
//! exécution. C'est un dossier de Cadence, jamais le dépôt de travail de l'utilisateur.
//! Le jeton passe par la configuration Git en variables d'environnement, jamais en argument
//! (visible dans `ps`), et aucun assistant d'identifiants n'est sollicité.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD, Engine};
use secrecy::{ExposeSecret, SecretString};
use tokio::process::Command;

use crate::error::{AppError, AppResult};

const GIT_TIMEOUT: Duration = Duration::from_secs(300);

/// Dossier de la copie de `repo` (nom validé : il finit dans un chemin).
pub fn checkout_dir(base: &Path, repo: &str) -> AppResult<PathBuf> {
    Ok(crate::store::db_path(base, repo)?.with_extension(""))
}

/// Clone ou remet à jour la copie sur la dernière version de `branch`. Renvoie son dossier.
pub async fn ensure(base: &Path, repo: &str, branch: &str, token: &SecretString) -> AppResult<PathBuf> {
    let git = which::which("git").map_err(|_| AppError::Config("Git est introuvable sur cette machine.".into()))?;
    let dir = checkout_dir(base, repo)?;
    if dir.join(".git").exists() {
        run(&git, token, Some(&dir), &["fetch", "--depth", "1", "origin", branch]).await?;
        run(&git, token, Some(&dir), &["reset", "--hard", "FETCH_HEAD"]).await?;
        run(&git, token, Some(&dir), &["clean", "-fdq"]).await?;
    } else {
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if dir.exists() {
            // Reste d'un clone interrompu.
            std::fs::remove_dir_all(&dir)?;
        }
        let url = format!("https://github.com/{repo}.git");
        let target = dir.to_string_lossy().to_string();
        run(&git, token, None, &["clone", "--depth", "1", "--single-branch", "--branch", branch, &url, &target]).await?;
    }
    Ok(dir)
}

/// En-tête HTTP d'authentification GitHub pour Git (`x-access-token:<jeton>` en Basic).
fn auth_header(token: &SecretString) -> String {
    let basic = STANDARD.encode(format!("x-access-token:{}", token.expose_secret()));
    format!("AUTHORIZATION: basic {basic}")
}

async fn run(git: &Path, token: &SecretString, cwd: Option<&Path>, args: &[&str]) -> AppResult<()> {
    let mut cmd = Command::new(git);
    cmd.args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_CONFIG_COUNT", "2")
        .env("GIT_CONFIG_KEY_0", "credential.helper")
        .env("GIT_CONFIG_VALUE_0", "")
        .env("GIT_CONFIG_KEY_1", "http.https://github.com/.extraheader")
        .env("GIT_CONFIG_VALUE_1", auth_header(token))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = tokio::time::timeout(GIT_TIMEOUT, cmd.output())
        .await
        .map_err(|_| AppError::Network(format!("git {} ne répond pas", args[0])))??;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first = stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("échec");
    Err(AppError::Validation(format!("Copie du dépôt impossible (git {}) : {first}", args[0])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_checkouts_inside_the_app_directory() {
        let base = Path::new("repos");
        assert_eq!(checkout_dir(base, "Me/App").unwrap(), base.join("me").join("app"));
        assert!(checkout_dir(base, "../x").is_err());
    }

    #[test]
    fn builds_a_basic_auth_header_without_exposing_the_token_elsewhere() {
        let header = auth_header(&SecretString::from("ghp_x"));
        assert_eq!(header, format!("AUTHORIZATION: basic {}", STANDARD.encode("x-access-token:ghp_x")));
    }
}
