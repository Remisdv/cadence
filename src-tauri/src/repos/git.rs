//! Lancement de Git pour le service Dépôts.
//!
//! Le binaire est celui du registre d'outils (`tools::located_paths`, chemin réglé compris).
//! Le jeton GitHub du trousseau ne sert qu'aux échanges avec GitHub (clone, fetch) : il passe
//! par la configuration Git en variables d'environnement (`http.https://github.com/.extraheader`),
//! jamais en argument (visible dans `ps`), jamais dans une URL ni dans un fichier.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use secrecy::{ExposeSecret, SecretString};
use tokio::process::Command;

use crate::error::{AppError, AppResult};

/// Un clone de gros dépôt peut être long ; au-delà, Git est arrêté.
const GIT_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Git, avec le jeton GitHub s'il est connecté.
pub struct Git {
    pub bin: PathBuf,
    pub token: Option<SecretString>,
}

/// En-tête HTTP d'authentification GitHub (`x-access-token:<jeton>` en Basic).
fn auth_header(token: &SecretString) -> String {
    let basic = STANDARD.encode(format!("x-access-token:{}", token.expose_secret()));
    format!("AUTHORIZATION: basic {basic}")
}

impl Git {
    /// Commande Git dans `cwd`, sans invite (ni terminal ni fenêtre du gestionnaire
    /// d'identifiants). `remote` : échange avec GitHub, le jeton s'il existe y est joint.
    fn command(&self, cwd: &Path, args: &[&str], remote: bool) -> Command {
        let mut cmd = Command::new(&self.bin);
        cmd.args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let (true, Some(token)) = (remote, &self.token) {
            // Le jeton remplace l'assistant d'identifiants de l'utilisateur pour cet appel.
            cmd.env("GIT_CONFIG_COUNT", "2")
                .env("GIT_CONFIG_KEY_0", "credential.helper")
                .env("GIT_CONFIG_VALUE_0", "")
                .env("GIT_CONFIG_KEY_1", "http.https://github.com/.extraheader")
                .env("GIT_CONFIG_VALUE_1", auth_header(token));
        }
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd
    }

    /// Lance Git ; renvoie la sortie standard, ou la première ligne d'erreur.
    pub async fn run(&self, cwd: &Path, args: &[&str]) -> AppResult<String> {
        self.exec(cwd, args, false).await
    }

    /// Comme `run`, pour un échange avec GitHub (clone, fetch).
    pub async fn remote(&self, cwd: &Path, args: &[&str]) -> AppResult<String> {
        self.exec(cwd, args, true).await
    }

    async fn exec(&self, cwd: &Path, args: &[&str], remote: bool) -> AppResult<String> {
        let output = tokio::time::timeout(GIT_TIMEOUT, self.command(cwd, args, remote).output())
            .await
            .map_err(|_| AppError::Network(format!("git {} ne répond pas", args[0])))??;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first = stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("échec");
        Err(AppError::Validation(format!("git {} : {first}", args[0])))
    }

    /// Vrai si la commande réussit (sortie ignorée).
    pub async fn ok(&self, cwd: &Path, args: &[&str]) -> bool {
        self.run(cwd, args).await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_token_only_travels_in_the_environment() {
        let git = Git { bin: "git".into(), token: Some(SecretString::from("ghp_secret")) };
        let cmd = git.command(Path::new("."), &["fetch", "origin"], true);
        let std = cmd.as_std();
        assert!(std.get_args().all(|a| !a.to_string_lossy().contains("ghp_secret")));
        let header = std
            .get_envs()
            .find(|(k, _)| *k == "GIT_CONFIG_VALUE_1")
            .and_then(|(_, v)| v)
            .map(|v| v.to_string_lossy().into_owned());
        assert_eq!(header, Some(format!("AUTHORIZATION: basic {}", STANDARD.encode("x-access-token:ghp_secret"))));

        let local = git.command(Path::new("."), &["worktree", "list"], false);
        assert!(local.as_std().get_envs().all(|(k, _)| k != "GIT_CONFIG_VALUE_1"), "pas de jeton hors échanges avec GitHub");
    }
}
