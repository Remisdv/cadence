//! Worktrees de l'agent développeur : `<racine>/<owner>/<nom>/<issue>`.
//!
//! Chaque tâche travaille dans un worktree de la copie du dépôt gardée par Cadence, sur sa
//! propre branche ; le dépôt de travail de l'utilisateur n'est jamais touché. Les commits
//! portent l'identité GitHub de l'utilisateur (adresse noreply) et la mention de l'agent.
//! Seul Cadence pousse, jamais sur la branche par défaut.

use std::path::{Path, PathBuf};

use secrecy::SecretString;

use super::{
    checkout::{git, git_out},
    is_protected_branch,
};
use crate::error::{AppError, AppResult};

/// Fichier touché dans un worktree (`git diff --numstat`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevFile {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
    /// Fichier créé par la tâche.
    pub added: bool,
}

/// Identité des commits : l'utilisateur, avec son adresse noreply GitHub.
#[derive(Debug, Clone)]
pub struct Author {
    pub name: String,
    pub email: String,
}

impl Author {
    pub fn github(login: &str, name: Option<&str>) -> Self {
        Self {
            name: name.filter(|n| !n.trim().is_empty()).unwrap_or(login).to_string(),
            email: format!("{login}@users.noreply.github.com"),
        }
    }

    fn env(&self) -> [(&'static str, String); 4] {
        [
            ("GIT_AUTHOR_NAME", self.name.clone()),
            ("GIT_AUTHOR_EMAIL", self.email.clone()),
            ("GIT_COMMITTER_NAME", self.name.clone()),
            ("GIT_COMMITTER_EMAIL", self.email.clone()),
        ]
    }
}

/// Racine des worktrees (`worktree_root` de la config, `~` développé).
pub fn root(configured: &str) -> PathBuf {
    match configured.strip_prefix("~/").or_else(|| configured.strip_prefix("~\\")) {
        Some(rest) => home().join(rest),
        None if configured == "~" => home(),
        None => PathBuf::from(configured),
    }
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

/// Dossier du worktree d'une issue (nom du dépôt validé : il finit dans un chemin).
pub fn dir(root: &Path, repo: &str, number: u32) -> AppResult<PathBuf> {
    Ok(crate::vcs::checkout::checkout_dir(root, repo)?.join(number.to_string()))
}

/// Crée (ou recrée) le worktree de `branch` sur le dernier commit de la copie.
/// Renvoie le SHA de base, pour mesurer ce que la tâche a changé.
pub async fn create(checkout: &Path, wt: &Path, branch: &str) -> AppResult<String> {
    let base = git_out(checkout, None, &["rev-parse", "HEAD"]).await?.trim().to_string();
    if wt.exists() {
        let path = wt.to_string_lossy().to_string();
        let _ = git(checkout, None, &["worktree", "remove", "--force", &path]).await;
        if wt.exists() {
            std::fs::remove_dir_all(wt)?;
        }
    }
    let _ = git(checkout, None, &["worktree", "prune"]).await;
    if let Some(parent) = wt.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let path = wt.to_string_lossy().to_string();
    git(checkout, None, &["worktree", "add", "-B", branch, &path, &base]).await?;
    Ok(base)
}

/// Crée (ou recrée) un worktree détaché sur `rev` (agent déploiement : le tag d'une version).
pub async fn create_detached(checkout: &Path, wt: &Path, rev: &str) -> AppResult<()> {
    if wt.exists() {
        let path = wt.to_string_lossy().to_string();
        let _ = git(checkout, None, &["worktree", "remove", "--force", &path]).await;
        if wt.exists() {
            std::fs::remove_dir_all(wt)?;
        }
    }
    let _ = git(checkout, None, &["worktree", "prune"]).await;
    if let Some(parent) = wt.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let path = wt.to_string_lossy().to_string();
    git(checkout, None, &["worktree", "add", "--detach", &path, rev]).await
}

/// Commit d'ouverture (vide) : il permet d'ouvrir la PR en brouillon dès l'étape 2.
pub async fn commit_empty(wt: &Path, author: &Author, message: &str) -> AppResult<()> {
    commit(wt, author, &["commit", "--allow-empty", "-m", message]).await
}

/// Commite tout le travail du worktree. Renvoie faux s'il n'y avait rien à commiter.
pub async fn commit_all(wt: &Path, author: &Author, message: &str) -> AppResult<bool> {
    git(wt, None, &["add", "-A"]).await?;
    let staged = git_out(wt, None, &["diff", "--cached", "--name-only"]).await?;
    if staged.trim().is_empty() {
        return Ok(false);
    }
    commit(wt, author, &["commit", "-m", message]).await?;
    Ok(true)
}

async fn commit(wt: &Path, author: &Author, args: &[&str]) -> AppResult<()> {
    let git_bin = which::which("git").map_err(|_| AppError::Config("Git est introuvable sur cette machine.".into()))?;
    let output = tokio::process::Command::new(git_bin)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .envs(author.env())
        .current_dir(wt)
        .output()
        .await?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(AppError::Validation(format!("Commit impossible : {}", stderr.lines().next().unwrap_or("échec"))))
    }
}

/// Fichiers modifiés et pas encore commités (le travail en cours de l'agent).
pub async fn changed_files_uncommitted(wt: &Path) -> AppResult<Vec<DevFile>> {
    git(wt, None, &["add", "-A"]).await?;
    let numstat = git_out(wt, None, &["diff", "--cached", "--numstat"]).await?;
    let added = git_out(wt, None, &["diff", "--cached", "--name-only", "--diff-filter=A"]).await?;
    Ok(parse_numstat(&numstat, &added))
}

/// Fichiers touchés depuis `base` : `git diff --numstat` et fichiers ajoutés.
pub async fn changed_files(wt: &Path, base: &str) -> AppResult<Vec<DevFile>> {
    let range = format!("{base}..HEAD");
    let numstat = git_out(wt, None, &["diff", "--numstat", &range]).await?;
    let added = git_out(wt, None, &["diff", "--name-only", "--diff-filter=A", &range]).await?;
    Ok(parse_numstat(&numstat, &added))
}

pub fn parse_numstat(numstat: &str, added: &str) -> Vec<DevFile> {
    let added: Vec<&str> = added.lines().map(str::trim).collect();
    numstat
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let (a, d, path) = (parts.next()?, parts.next()?, parts.next()?.trim());
            Some(DevFile {
                path: path.to_string(),
                // Fichier binaire : « - ».
                additions: a.parse().unwrap_or(0),
                deletions: d.parse().unwrap_or(0),
                added: added.contains(&path),
            })
        })
        .collect()
}

/// Pousse la branche du worktree. Refuse la branche par défaut, même en autonomie `auto`.
pub async fn push(wt: &Path, repo: &str, branch: &str, default_branch: &str, token: &SecretString) -> AppResult<()> {
    if is_protected_branch(branch, default_branch) {
        return Err(AppError::Validation(format!("Garde-fou : aucun agent ne pousse sur {branch}.")));
    }
    let url = format!("https://github.com/{repo}.git");
    let refspec = format!("HEAD:refs/heads/{branch}");
    git(wt, Some(token), &["push", &url, &refspec]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_numstat_with_binaries_and_new_files() {
        let files = parse_numstat("142\t0\tsrc/auth/device_flow.rs\n68\t12\tsrc/commands/auth.rs\n-\t-\tassets/logo.png\n", "src/auth/device_flow.rs\n");
        assert_eq!(files.len(), 3);
        assert!(files[0].added && !files[1].added);
        assert_eq!((files[1].additions, files[1].deletions), (68, 12));
        assert_eq!((files[2].additions, files[2].deletions), (0, 0));
    }

    #[test]
    fn expands_the_worktree_root_and_keeps_paths_inside_it() {
        assert_eq!(root("/srv/wt"), PathBuf::from("/srv/wt"));
        assert!(root("~/.cadence/wt").ends_with(".cadence/wt") || root("~/.cadence/wt").ends_with(".cadence\\wt"));
        let r = Path::new("wt");
        assert_eq!(dir(r, "Me/App", 142).unwrap(), r.join("me").join("app").join("142"));
        assert!(dir(r, "../x", 1).is_err());
    }

    #[test]
    fn commits_carry_the_github_noreply_identity() {
        let a = Author::github("remisdv", Some(""));
        assert_eq!((a.name.as_str(), a.email.as_str()), ("remisdv", "remisdv@users.noreply.github.com"));
    }
}
