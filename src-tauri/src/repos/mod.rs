//! Service Dépôts GitHub (plan § 2.2, révision 1.1) : clones et worktrees des dépôts suivis.
//!
//! ```text
//! <config de l'app>/repos/
//!   <owner>/<nom>/                      clone fait par Cadence, à la demande
//!   .worktrees/<owner>/<nom>/<tâche>/   un worktree par tâche (détaché, l'IA crée sa branche)
//! ```
//!
//! Un dépôt peut aussi pointer vers un clone de l'utilisateur (« Désigner un clone
//! existant… », réglage `repoClones`) : Cadence n'y fait rien d'autre qu'ajouter les worktrees
//! des tâches (rangés, eux, sous `.worktrees`), et ne le met jamais à jour.
//!
//! Le clone géré par Cadence est remis à jour (`fetch`) à chaque demande ; hors ligne, la copie
//! existante sert telle quelle. Un appel répété pour la même tâche renvoie le même worktree,
//! avec le travail en cours.
//!
//! Nettoyage (`prune_merged`, au lancement puis toutes les 6 h) : un worktree dont la branche a
//! des PR GitHub toutes fusionnées ou fermées est supprimé, sauf s'il garde du travail non
//! commité ou non poussé. Un worktree resté détaché (aucune branche créée) n'est pas touché.

pub mod git;

use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use self::git::Git;
use crate::{
    app_settings::{AppSettings, AppSettingsStore, is_repo_name},
    error::{AppError, AppResult},
    plugins::env::GITHUB_ACCOUNT,
    secrets::SecretStore,
};

/// Notification au serveur d'un plugin `services = ["repos"]` : `{ "repos": ["owner/name", …] }`.
pub const REPOS_NOTIFICATION: &str = "notifications/cadence/repos";

/// Dossier des worktrees, à côté des clones (un owner GitHub ne commence jamais par un point).
const WORKTREES: &str = ".worktrees";

/// Où en est la copie locale d'un dépôt suivi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum CloneState {
    /// Cloné par Cadence sous `<config>/repos`.
    Cloned,
    /// Clone de l'utilisateur, désigné dans les réglages.
    Linked,
    /// Pas encore cloné : le premier skill qui touche au code le clonera.
    Missing,
}

/// Copie locale d'un dépôt suivi (Réglages › Dépôts GitHub).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepoCloneView {
    /// `owner/name`, tel que suivi.
    pub repo: String,
    pub state: CloneState,
    /// Dossier du clone (`null` tant qu'il n'existe pas).
    pub path: Option<String>,
    /// Tâches qui ont un worktree.
    pub worktrees: Vec<String>,
}

/// Résultat de `repos_checkout` et de `cadence__repo_checkout`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepoCheckout {
    /// Worktree de la tâche, ou le clone lui-même sans tâche.
    pub path: String,
}

/// Délai entre deux nettoyages des worktrees.
pub const PRUNE_EVERY: Duration = Duration::from_secs(6 * 3600);

/// État d'une pull request GitHub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    Open,
    Merged,
    Closed,
}

/// Worktree supprimé par le nettoyage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrunedWorktree {
    pub repo: String,
    pub task: String,
    pub branch: String,
}

/// PR d'une branche poussée sur le dépôt (`GET /repos/{repo}/pulls?head=owner:branch&state=all`).
pub async fn branch_prs(http: &reqwest::Client, token: &SecretString, repo: &str, branch: &str) -> AppResult<Vec<PrState>> {
    #[derive(Deserialize)]
    struct Raw {
        state: String,
        merged_at: Option<String>,
    }
    let owner = repo.split('/').next().unwrap_or_default();
    let resp = http
        .get(format!(
            "{}/repos/{repo}/pulls?state=all&per_page=20&head={}",
            crate::api::github::API_URL,
            percent_encode(&format!("{owner}:{branch}"))
        ))
        .bearer_auth(token.expose_secret())
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", crate::api::github::API_VERSION)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::GitHub(format!("PR de {repo}:{branch} illisibles ({})", resp.status())));
    }
    let raw: Vec<Raw> = resp.json().await?;
    Ok(raw
        .into_iter()
        .map(|pr| match (pr.state.as_str(), pr.merged_at) {
            ("open", _) => PrState::Open,
            (_, Some(_)) => PrState::Merged,
            _ => PrState::Closed,
        })
        .collect())
}

/// Encodage d'un paramètre d'URL (tout sauf `A-Z a-z 0-9 - . _ ~`).
fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `owner/name` en minuscules, validé (il finit dans un chemin).
pub fn repo_key(repo: &str) -> AppResult<String> {
    let repo = repo.trim().trim_end_matches(".git");
    let safe = is_repo_name(repo) && repo.split('/').all(|part| !part.starts_with('.'));
    if !safe {
        return Err(AppError::Validation(format!("« {repo} » n'est pas un dépôt owner/nom.")));
    }
    Ok(repo.to_ascii_lowercase())
}

/// Nom de dossier d'une tâche (`issue-42`, `Implémenter #42` → `implementer-42`).
pub fn task_slug(task: &str) -> AppResult<String> {
    let slug = crate::vcs::slugify(task);
    if slug.is_empty() {
        return Err(AppError::Validation(format!("« {task} » n'est pas un nom de tâche utilisable.")));
    }
    Ok(slug)
}

/// Le dépôt est suivi par `plugin`, ou par au moins un plugin si `plugin` est `None`.
pub fn is_tracked(settings: &AppSettings, repo: &str, plugin: Option<&str>) -> bool {
    settings
        .tracked_repos
        .iter()
        .filter(|(id, _)| plugin.is_none_or(|p| p == id.as_str()))
        .flat_map(|(_, repos)| repos)
        .any(|r| r.eq_ignore_ascii_case(repo.trim()))
}

/// Un remote (`git remote -v`) pointe vers `github.com/<key>`, en HTTPS ou en SSH.
fn points_to(remotes: &str, key: &str) -> bool {
    remotes.lines().filter_map(|line| line.split_whitespace().nth(1)).any(|url| {
        let url = url.to_ascii_lowercase();
        let url = url.trim_end_matches('/').trim_end_matches(".git");
        url.ends_with(&format!("github.com/{key}")) || url.ends_with(&format!("github.com:{key}"))
    })
}

pub struct RepoService {
    /// `<config de l'app>/repos`.
    root: PathBuf,
    settings: Arc<AppSettingsStore>,
    secrets: Arc<dyn SecretStore>,
    /// Préfixe des URL de clone (`https://github.com/` ; un dossier local dans les tests).
    remote_base: String,
    /// Une opération Git à la fois par dépôt.
    locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl RepoService {
    pub fn new(root: PathBuf, settings: Arc<AppSettingsStore>, secrets: Arc<dyn SecretStore>) -> Self {
        Self { root, settings, secrets, remote_base: "https://github.com/".into(), locks: Mutex::default() }
    }

    /// Clone depuis `<base><owner>/<nom>.git` au lieu de GitHub (tests).
    pub fn with_remote_base(mut self, base: impl Into<String>) -> Self {
        self.remote_base = base.into();
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Le dépôt est suivi par `plugin` (par au moins un plugin si `None`).
    pub fn is_tracked(&self, repo: &str, plugin: Option<&str>) -> bool {
        is_tracked(&self.settings.get(), repo, plugin)
    }

    /// Dépôts suivis par au moins un plugin.
    pub fn tracked(&self) -> Vec<String> {
        self.settings.get().tracked_repos.into_values().flatten().collect()
    }

    fn clone_dir(&self, key: &str) -> PathBuf {
        key.split('/').fold(self.root.clone(), |dir, part| dir.join(part))
    }

    fn worktrees_dir(&self, key: &str) -> PathBuf {
        key.split('/').fold(self.root.join(WORKTREES), |dir, part| dir.join(part))
    }

    fn lock(&self, key: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.locks.lock().unwrap().entry(key.to_string()).or_default().clone()
    }

    /// Git du registre d'outils, avec le jeton GitHub du trousseau.
    fn git(&self) -> AppResult<Git> {
        let bin = crate::tools::located_paths(&self.settings.get())
            .remove("git")
            .ok_or_else(|| AppError::Config("Git est introuvable : installez-le dans Réglages › Outils externes.".into()))?;
        Ok(Git { bin: PathBuf::from(bin), token: self.secrets.get(GITHUB_ACCOUNT)? })
    }

    /// Clone désigné par l'utilisateur, s'il y en a un.
    fn linked(&self, key: &str) -> Option<PathBuf> {
        self.settings.get().repo_clones.get(key).map(PathBuf::from)
    }

    /// État de la copie locale de chaque dépôt (doublons sans casse retirés).
    pub fn status(&self, repos: &[String]) -> Vec<RepoCloneView> {
        let mut seen = Vec::new();
        repos
            .iter()
            .filter_map(|repo| {
                let key = repo_key(repo).ok()?;
                (!seen.contains(&key)).then(|| {
                    seen.push(key.clone());
                    self.view(repo.trim(), &key)
                })
            })
            .collect()
    }

    fn view(&self, repo: &str, key: &str) -> RepoCloneView {
        let cloned = self.clone_dir(key);
        let (state, path) = match self.linked(key) {
            Some(path) => (CloneState::Linked, Some(path)),
            None if cloned.join(".git").exists() => (CloneState::Cloned, Some(cloned)),
            None => (CloneState::Missing, None),
        };
        let mut worktrees: Vec<String> = std::fs::read_dir(self.worktrees_dir(key))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().join(".git").exists())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        worktrees.sort();
        RepoCloneView { repo: repo.to_string(), state, path: path.map(|p| p.display().to_string()), worktrees }
    }

    /// Désigne le clone de l'utilisateur pour `repo` (`None` : revenir au clone de Cadence).
    /// Le dossier doit être un dépôt Git dont un remote pointe vers `github.com/<repo>`.
    pub async fn link(&self, repo: &str, path: Option<&Path>) -> AppResult<RepoCloneView> {
        let key = repo_key(repo)?;
        let lock = self.lock(&key);
        let _guard = lock.lock().await;
        let Some(path) = path else {
            self.settings.update(|s| {
                s.repo_clones.remove(&key);
            })?;
            return Ok(self.view(repo.trim(), &key));
        };
        if !path.is_dir() {
            return Err(AppError::Validation(format!("Dossier introuvable : {}", path.display())));
        }
        let git = self.git()?;
        let top = git
            .run(path, &["rev-parse", "--show-toplevel"])
            .await
            .map_err(|_| AppError::Validation(format!("{} n'est pas un dépôt Git.", path.display())))?;
        let top = std::path::absolute(PathBuf::from(top.trim()))?;
        if top.starts_with(&self.root) {
            return Err(AppError::Validation("Ce dossier est déjà géré par Cadence.".into()));
        }
        let remotes = git.run(&top, &["remote", "-v"]).await?;
        if !points_to(&remotes, &key) {
            return Err(AppError::Validation(format!(
                "Aucun remote de {} ne pointe vers github.com/{}.",
                top.display(),
                repo.trim()
            )));
        }
        let saved = top.display().to_string();
        self.settings.update(|s| {
            s.repo_clones.insert(key.clone(), saved);
        })?;
        Ok(self.view(repo.trim(), &key))
    }

    /// Nettoyage avec l'API GitHub (jeton du trousseau) ; rien sans connexion GitHub.
    pub async fn prune_merged(&self, http: &reqwest::Client) -> Vec<PrunedWorktree> {
        let token = match self.secrets.get(GITHUB_ACCOUNT) {
            Ok(Some(token)) => token,
            _ => return Vec::new(),
        };
        let repos = self.tracked();
        self.prune(&repos, |repo, branch| {
            let token = token.clone();
            async move { branch_prs(http, &token, &repo, &branch).await }
        })
        .await
    }

    /// Supprime les worktrees des dépôts `repos` dont la branche n'a plus que des PR fusionnées
    /// ou fermées (`prs` : PR d'une branche). Renvoie ceux qui ont été supprimés.
    pub async fn prune<F, Fut>(&self, repos: &[String], prs: F) -> Vec<PrunedWorktree>
    where
        F: Fn(String, String) -> Fut,
        Fut: Future<Output = AppResult<Vec<PrState>>>,
    {
        let Ok(git) = self.git() else { return Vec::new() };
        let mut pruned = Vec::new();
        let mut seen = Vec::new();
        for repo in repos {
            let Ok(key) = repo_key(repo) else { continue };
            if seen.contains(&key) {
                continue;
            }
            seen.push(key.clone());
            let lock = self.lock(&key);
            let _guard = lock.lock().await;
            let entries: Vec<PathBuf> = std::fs::read_dir(self.worktrees_dir(&key))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join(".git").exists())
                .collect();
            for wt in entries {
                let Some(branch) = git.run(&wt, &["symbolic-ref", "--short", "-q", "HEAD"]).await.ok() else {
                    continue; // détaché : l'IA n'y a pas créé de branche
                };
                let branch = branch.trim().to_string();
                let dirty = git.run(&wt, &["status", "--porcelain"]).await.map_or(true, |s| !s.trim().is_empty());
                let ahead = git
                    .run(&wt, &["rev-list", "--count", "@{upstream}..HEAD"])
                    .await
                    .map_or(false, |n| n.trim() != "0");
                if dirty || ahead {
                    continue;
                }
                let states = match prs(repo.trim().to_string(), branch.clone()).await {
                    Ok(states) => states,
                    Err(err) => {
                        tracing::info!(repo = key, branch, %err, "PR de la branche illisibles, worktree gardé");
                        continue;
                    }
                };
                if states.is_empty() || states.contains(&PrState::Open) {
                    continue;
                }
                let target = wt.to_string_lossy().into_owned();
                let base = self.linked(&key).unwrap_or_else(|| self.clone_dir(&key));
                if git.run(&base, &["worktree", "remove", &target]).await.is_err() {
                    let _ = std::fs::remove_dir_all(&wt);
                    let _ = git.run(&base, &["worktree", "prune"]).await;
                }
                if wt.exists() {
                    tracing::warn!(repo = key, branch, "worktree de PR terminée non supprimé");
                    continue;
                }
                let task = wt.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                tracing::info!(repo = key, task, branch, "worktree supprimé : PR fusionnée ou fermée");
                pruned.push(PrunedWorktree { repo: repo.trim().to_string(), task, branch });
            }
        }
        pruned
    }

    /// Copie locale de `repo`, clonée au besoin, puis le worktree de `task` s'il y en a une.
    /// Renvoie le dossier à ouvrir.
    pub async fn checkout(&self, repo: &str, task: Option<&str>) -> AppResult<PathBuf> {
        let key = repo_key(repo)?;
        let task = task.map(str::trim).filter(|t| !t.is_empty()).map(task_slug).transpose()?;
        let lock = self.lock(&key);
        let _guard = lock.lock().await;
        let git = self.git()?;
        let base = self.ensure_clone(&git, &key).await?;
        let Some(task) = task else { return Ok(base) };

        let wt = self.worktrees_dir(&key).join(&task);
        if wt.join(".git").exists() {
            return Ok(wt);
        }
        if wt.exists() {
            // Reste d'un worktree interrompu.
            std::fs::remove_dir_all(&wt)?;
        }
        let _ = git.run(&base, &["worktree", "prune"]).await;
        let mut rev = None;
        for candidate in ["origin/HEAD", "HEAD"] {
            let spec = format!("{candidate}^{{commit}}");
            if git.ok(&base, &["rev-parse", "--verify", "--quiet", &spec]).await {
                rev = Some(candidate);
                break;
            }
        }
        let rev = rev.ok_or_else(|| AppError::Validation(format!("Le dépôt {repo} n'a encore aucun commit.")))?;
        if let Some(parent) = wt.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let target = wt.to_string_lossy().into_owned();
        git.run(&base, &["worktree", "add", "--detach", &target, rev])
            .await
            .map_err(|e| AppError::Validation(format!("Worktree de la tâche {task} impossible ({e})")))?;
        Ok(wt)
    }

    /// Clone désigné, ou clone de Cadence (créé au besoin, sinon remis à jour).
    async fn ensure_clone(&self, git: &Git, key: &str) -> AppResult<PathBuf> {
        if let Some(linked) = self.linked(key) {
            if !linked.join(".git").exists() {
                return Err(AppError::Validation(format!(
                    "Le clone désigné pour {key} est introuvable ({}). Désignez-le à nouveau dans Réglages › Dépôts GitHub.",
                    linked.display()
                )));
            }
            return Ok(linked);
        }
        let dir = self.clone_dir(key);
        if dir.join(".git").exists() {
            if let Err(err) = git.remote(&dir, &["fetch", "--prune", "origin"]).await {
                tracing::warn!(repo = key, %err, "dépôt non remis à jour, copie locale utilisée");
            }
            return Ok(dir);
        }
        let parent = dir.parent().ok_or_else(|| AppError::Internal("dossier de clone invalide".into()))?;
        std::fs::create_dir_all(parent)?;
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // Clone dans un dossier temporaire, renommé à la fin : jamais de clone à moitié fait.
        let partial = parent.join(format!(".{name}.partial"));
        if partial.exists() {
            std::fs::remove_dir_all(&partial)?;
        }
        let url = format!("{}{key}.git", self.remote_base);
        let target = partial.to_string_lossy().into_owned();
        git.remote(parent, &["clone", "--origin", "origin", &url, &target])
            .await
            .map_err(|e| AppError::Validation(format!("Clone de {key} impossible ({e})")))?;
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        std::fs::rename(&partial, &dir)?;
        Ok(dir)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::process::Command;

    use super::*;
    use crate::{app_settings::temp_dir, secrets::MemorySecrets};

    pub(crate) fn git(cwd: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(["-c", "user.name=Cadence", "-c", "user.email=cadence@example.invalid", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("git introuvable");
        assert!(out.status.success(), "git {args:?} : {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Dépôt « distant » local `<root>/remotes/me/app.git`, un commit sur `main`.
    pub(crate) struct Fixture {
        pub root: PathBuf,
        pub remotes: PathBuf,
        pub work: PathBuf,
        pub settings: Arc<AppSettingsStore>,
        pub secrets: Arc<MemorySecrets>,
    }

    impl Fixture {
        pub(crate) fn new(name: &str) -> Self {
            let root = temp_dir(&format!("repos-{name}"));
            let remotes = root.join("remotes");
            let bare = remotes.join("me").join("app.git");
            std::fs::create_dir_all(&bare).unwrap();
            git(&bare, &["init", "--bare", "-q", "-b", "main"]);
            let work = root.join("work");
            std::fs::create_dir_all(&work).unwrap();
            git(&work, &["init", "-q", "-b", "main"]);
            std::fs::write(work.join("README.md"), "# app\n").unwrap();
            git(&work, &["add", "."]);
            git(&work, &["commit", "-q", "-m", "init"]);
            git(&work, &["remote", "add", "origin", &bare.to_string_lossy()]);
            git(&work, &["push", "-q", "origin", "main"]);
            let settings = Arc::new(AppSettingsStore::load(&root.join("config")));
            Self { root, remotes, work, settings, secrets: Arc::new(MemorySecrets::default()) }
        }

        pub(crate) fn service(&self) -> RepoService {
            let base = format!("{}{}", self.remotes.display(), std::path::MAIN_SEPARATOR);
            RepoService::new(self.root.join("repos"), self.settings.clone(), self.secrets.clone()).with_remote_base(base)
        }

        /// Nouveau commit poussé sur le dépôt « distant ».
        pub(crate) fn push_file(&self, name: &str) {
            std::fs::write(self.work.join(name), name).unwrap();
            git(&self.work, &["add", "."]);
            git(&self.work, &["commit", "-q", "-m", name]);
            git(&self.work, &["push", "-q", "origin", "main"]);
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn validates_repo_and_task_names() {
        assert_eq!(repo_key(" Me/App.git ").unwrap(), "me/app");
        for bad in ["../x", "me/..", "me/.git", "me", "me/app/x", "me/a b"] {
            assert!(repo_key(bad).is_err(), "{bad} accepté");
        }
        assert_eq!(task_slug("Issue #42").unwrap(), "issue-42");
        assert!(task_slug("!!!").is_err());
        assert_eq!(percent_encode("me:feat/1-a b#é"), "me%3Afeat%2F1-a%20b%23%C3%A9");
        assert!(points_to("origin\tgit@github.com:Me/App.git (fetch)\n", "me/app"));
        assert!(points_to("up\thttps://github.com/me/app (push)\n", "me/app"));
        assert!(!points_to("origin\thttps://github.com/me/app-fork.git (fetch)\n", "me/app"));
    }

    #[test]
    fn tracked_repos_are_checked_per_plugin() {
        let mut settings = AppSettings::default();
        settings.tracked_repos.insert("gestion-projet".into(), vec!["Me/App".into()]);
        assert!(is_tracked(&settings, "me/app", None));
        assert!(is_tracked(&settings, "me/app", Some("gestion-projet")));
        assert!(!is_tracked(&settings, "me/app", Some("documents")));
        assert!(!is_tracked(&settings, "me/autre", None));
    }

    #[tokio::test]
    async fn clones_on_demand_then_prepares_one_worktree_per_task() {
        let fx = Fixture::new("checkout");
        fx.secrets.set(GITHUB_ACCOUNT, "ghp_jamais_sur_disque").unwrap();
        let repos = fx.service();
        let tracked = vec!["me/app".to_string(), "Me/App".to_string()];
        assert_eq!(repos.status(&tracked)[0].state, CloneState::Missing);
        assert_eq!(repos.status(&tracked).len(), 1, "doublons sans casse retirés");

        let clone = repos.checkout("Me/App", None).await.unwrap();
        assert_eq!(clone, fx.root.join("repos").join("me").join("app"));
        assert!(clone.join("README.md").is_file());
        let config = std::fs::read_to_string(clone.join(".git").join("config")).unwrap();
        assert!(!config.contains("ghp_jamais_sur_disque"), "jeton jamais écrit dans le clone");

        let wt = repos.checkout("me/app", Some("issue-42")).await.unwrap();
        assert_eq!(wt, fx.root.join("repos").join(WORKTREES).join("me").join("app").join("issue-42"));
        assert!(wt.join("README.md").is_file());
        assert_eq!(git(&wt, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "HEAD", "worktree détaché");

        // Reprise : même worktree, travail en cours conservé.
        std::fs::write(wt.join("en-cours.txt"), "wip").unwrap();
        assert_eq!(repos.checkout("me/app", Some("issue-42")).await.unwrap(), wt);
        assert!(wt.join("en-cours.txt").is_file());

        // Nouvelle tâche : le clone est remis à jour avant.
        fx.push_file("nouveau.txt");
        let (a, b) = tokio::join!(repos.checkout("me/app", Some("issue-43")), repos.checkout("me/app", Some("issue-44")));
        assert!(a.unwrap().join("nouveau.txt").is_file());
        assert!(b.unwrap().join("nouveau.txt").is_file());

        let view = &repos.status(&tracked)[0];
        assert_eq!(view.state, CloneState::Cloned);
        assert_eq!(view.path.as_deref(), Some(clone.display().to_string().as_str()));
        assert_eq!(view.worktrees, ["issue-42", "issue-43", "issue-44"]);
        assert_eq!(git(&clone, &["status", "--porcelain"]).trim(), "", "le clone reste propre");
        assert!(repos.checkout("../x", None).await.is_err());
    }

    #[tokio::test]
    async fn prunes_worktrees_whose_pull_requests_are_done() {
        let fx = Fixture::new("prune");
        let repos = fx.service();
        let tracked = vec!["me/app".to_string()];
        // Une tâche par cas : branche fusionnée, PR ouverte, travail non commité, détachée,
        // commit non poussé.
        let mut wts = Vec::new();
        for (task, branch) in [("issue-1", "feat/1-fini"), ("issue-2", "feat/2-en-cours"), ("issue-3", "feat/3-sale"), ("issue-4", ""), ("issue-5", "feat/5-local")] {
            let wt = repos.checkout("me/app", Some(task)).await.unwrap();
            if !branch.is_empty() {
                git(&wt, &["switch", "-q", "-c", branch]);
                std::fs::write(wt.join(format!("{task}.txt")), task).unwrap();
                git(&wt, &["add", "."]);
                git(&wt, &["commit", "-q", "-m", task]);
                if task != "issue-5" {
                    git(&wt, &["push", "-q", "-u", "origin", branch]);
                } else {
                    git(&wt, &["push", "-q", "-u", "origin", branch]);
                    std::fs::write(wt.join("encore.txt"), "x").unwrap();
                    git(&wt, &["add", "."]);
                    git(&wt, &["commit", "-q", "-m", "pas encore poussé"]);
                }
            }
            wts.push(wt);
        }
        std::fs::write(wts[2].join("brouillon.txt"), "pas commité").unwrap();

        let asked = Mutex::new(Vec::new());
        let pruned = repos
            .prune(&tracked, |repo, branch| {
                asked.lock().unwrap().push(branch.clone());
                async move {
                    assert_eq!(repo, "me/app");
                    Ok(match branch.as_str() {
                        "feat/2-en-cours" => vec![PrState::Closed, PrState::Open],
                        _ => vec![PrState::Merged],
                    })
                }
            })
            .await;
        assert_eq!(pruned, [PrunedWorktree { repo: "me/app".into(), task: "issue-1".into(), branch: "feat/1-fini".into() }]);
        assert!(!wts[0].exists(), "PR fusionnée : worktree supprimé");
        assert!(wts[1].exists() && wts[2].exists() && wts[3].exists() && wts[4].exists());
        assert_eq!(*asked.lock().unwrap(), ["feat/1-fini", "feat/2-en-cours"], "GitHub interrogé seulement pour les worktrees propres et poussés");
        assert_eq!(repos.status(&tracked)[0].worktrees, ["issue-2", "issue-3", "issue-4", "issue-5"]);
        assert!(!git(&repos.clone_dir("me/app"), &["worktree", "list"]).contains("issue-1"));

        // Aucune PR (branche jamais proposée) : gardé.
        let none = repos.prune(&tracked, |_, _| async { Ok(Vec::new()) }).await;
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn uses_a_designated_clone_without_touching_it() {
        let fx = Fixture::new("link");
        let repos = fx.service();
        let mine = fx.root.join("mon-clone");
        git(&fx.root, &["clone", "-q", &fx.remotes.join("me").join("app.git").to_string_lossy(), &mine.to_string_lossy()]);

        // Aucun remote vers github.com/me/app : refusé.
        assert!(repos.link("me/app", Some(&mine)).await.is_err());
        assert!(repos.link("me/app", Some(&fx.root.join("absent"))).await.is_err());
        git(&mine, &["remote", "set-url", "origin", "https://github.com/me/app.git"]);

        let view = repos.link("me/app", Some(&mine.join("."))).await.unwrap();
        assert_eq!(view.state, CloneState::Linked);
        let linked = PathBuf::from(view.path.unwrap());
        assert_eq!(std::fs::canonicalize(&linked).unwrap(), std::fs::canonicalize(&mine).unwrap());
        assert_eq!(repos.checkout("me/app", None).await.unwrap(), linked);
        assert!(!fx.root.join("repos").join("me").join("app").exists(), "pas de second clone");

        let wt = repos.checkout("me/app", Some("issue-7")).await.unwrap();
        assert!(wt.starts_with(fx.root.join("repos").join(WORKTREES)));
        assert!(wt.join("README.md").is_file());
        assert!(git(&mine, &["worktree", "list"]).contains("issue-7"));

        let view = repos.link("me/app", None).await.unwrap();
        assert_eq!((view.state, view.worktrees.len()), (CloneState::Missing, 1));
        assert!(fx.settings.get().repo_clones.is_empty());
    }
}
