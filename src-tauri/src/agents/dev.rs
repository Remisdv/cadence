//! Agent développeur : implémenter une issue dans un worktree isolé.
//!
//! Six étapes par tâche, une tâche après l'autre :
//! 1. analyse de l'issue et du dépôt (session en lecture seule, dans le worktree) ;
//! 2. branche et PR en brouillon (commit d'ouverture vide, poussé par Cadence) ;
//! 3. implémentation (même session, outils d'écriture, jamais de push) ;
//! 4. tests (même session) puis commit du travail par Cadence ;
//! 5. push — après votre validation, sauf autonomie `auto` ;
//! 6. sortie du brouillon et commentaire signé sur la PR.
//!
//! En autonomie `propose`, rien ne quitte la machine : branche locale, aucun push, aucune PR.

use std::{path::PathBuf, sync::Arc, time::Duration};

use tauri::{AppHandle, Manager};

use super::{
    claude::{SessionOutcome, Tools},
    engine::{self, RunHandle},
};
use crate::{
    api::github::writes,
    auth::{self, cli::CliTool},
    domain::{
        agent::{AgentInput, AgentKind, AgentRunDetail, Autonomy, DevTask, DevTaskState, RunState},
        issue::IssueType,
        settings::AgentEngine,
        version,
    },
    error::{AppError, AppResult},
    events::AgentEventKind,
    state::AppState,
    vcs::{self, worktree},
};

pub const STEPS: u8 = 6;

pub fn system_prompt() -> &'static str {
    "Tu es l'agent développeur de Cadence. Tu implémentes une issue GitHub dans le dossier \
     courant, un worktree Git isolé sur sa propre branche.\n\n\
     - Respecte les conventions du dépôt (CLAUDE.md, README, style du code voisin).\n\
     - Change le minimum nécessaire pour remplir les critères d'acceptation de l'issue.\n\
     - Ne fais jamais de commit, de push ni de configuration Git : Cadence s'en charge.\n\
     - N'accède pas au réseau. Écris en français dans tes messages."
}

pub fn analyse_prompt(repo: &str, task: &DevTask, body: &str) -> String {
    let body = if body.trim().is_empty() { "(vide)" } else { body.trim() };
    format!(
        "Étape 1 — analyse. Dépôt {repo}, issue #{number} : {title}\n\nDescription :\n{body}\n\n\
         Lis le code pour comprendre ce qu'il faut changer. Ne modifie rien à cette étape. \
         Rends un plan court (approche, fichiers à toucher), puis un bloc ```json \
         {{\"plan\": \"…\", \"files\": [\"chemin\"]}}.",
        number = task.number,
        title = task.title,
    )
}

pub fn implement_prompt(branch: &str, notes: &[String]) -> String {
    format!(
        "Étape 3 — implémentation sur la branche {branch}. Implémente ton plan dans le dossier \
         courant.{}\n\nTermine par un bloc ```json {{\"summary\": \"…ce que tu as fait, en markdown court…\"}}.",
        interventions(notes)
    )
}

pub fn tests_prompt(notes: &[String]) -> String {
    format!(
        "Étape 4 — tests. Lance les tests et vérifications pertinents du dépôt (build, tests \
         unitaires des parties touchées) et corrige ce que tes changements ont cassé.{}\n\n\
         Termine par un bloc ```json {{\"summary\": \"…\", \"tests\": \"réussis | échecs | non lancés\"}}.",
        interventions(notes)
    )
}

fn interventions(notes: &[String]) -> String {
    if notes.is_empty() {
        String::new()
    } else {
        format!("\n\nMessages de l'utilisateur pendant l'exécution :\n{}", notes.iter().map(|n| format!("- {n}")).collect::<Vec<_>>().join("\n"))
    }
}

/// Champ texte du dernier bloc ```json de la réponse.
pub fn json_field(text: &str, field: &str) -> Option<String> {
    let block = text.rsplit_once("```json").map(|(_, rest)| rest.split("```").next().unwrap_or(rest))?;
    let value: serde_json::Value = serde_json::from_str(block.trim()).ok()?;
    value.get(field).and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Texte visible d'une réponse, sans le bloc JSON final.
fn visible(text: &str) -> String {
    text.split("```json").next().unwrap_or(text).trim().to_string()
}

/* ---------- Lancement ---------- */

/// Lance l'agent développeur sur des issues précises. Une seule exécution active à la fois.
pub async fn start(app: &AppHandle, state: &AppState, input: AgentInput) -> AppResult<String> {
    if let Some(h) = engine::latest(state, AgentKind::Dev).await? {
        let d = h.detail.read().await;
        if matches!(d.state, RunState::Prepare | RunState::Running) {
            return Err(AppError::Validation("L'agent développeur travaille déjà : attendez la fin ou arrêtez-le.".into()));
        }
    }
    let numbers = input.issues.clone().unwrap_or_default();
    if numbers.is_empty() {
        return Err(AppError::Validation("Choisissez au moins une tâche.".into()));
    }
    let config = crate::commands::settings::load(state).await?.config;
    let agent = config.agents.dev.clone();
    if agent.engine != AgentEngine::ClaudeCode {
        return Err(AppError::Validation("L'agent développeur ne sait pas encore piloter Codex : choisissez Claude Code dans les réglages.".into()));
    }
    if auth::cli_status(state, CliTool::ClaudeCode, false).await.binary_path.is_none() {
        return Err(AppError::Validation("Claude Code est introuvable sur cette machine (Réglages › Machine locale).".into()));
    }
    let cache = state.repo_cache().await?;
    let raws = cache.issues().await?;
    let tasks: Vec<DevTask> = numbers
        .iter()
        .filter_map(|n| raws.iter().find(|r| r.number == *n && r.state == "open"))
        .map(|raw| {
            let row = crate::api::github::issues::to_row(raw, None);
            DevTask {
                number: raw.number,
                title: raw.title.clone(),
                issue_type: row.issue_type,
                estimate: row.estimate,
                state: DevTaskState::Queued,
                step: 0,
                step_notes: vec![],
                branch: vcs::branch_name(row.issue_type.unwrap_or(IssueType::Feat), raw.number, &raw.title),
                worktree: None,
                pr: None,
                pr_url: None,
                files: vec![],
                additions: 0,
                deletions: 0,
                summary: None,
                error: None,
                started_at: None,
                session_id: None,
                pr_node_id: None,
            }
        })
        .collect();
    if tasks.is_empty() {
        return Err(AppError::Validation("Aucune de ces issues n'est ouverte dans le cache.".into()));
    }

    let now = version::now_iso();
    let detail = AgentRunDetail {
        run: format!("dev-{}", now.replace([':', '-'], "").trim_end_matches('Z').replace('T', "-")),
        kind: AgentKind::Dev,
        state: RunState::Prepare,
        autonomy: agent.autonomy,
        version: None,
        model: agent.model.clone(),
        started_at: now,
        ended_at: None,
        items: vec![],
        tasks,
        log: vec![],
        error: None,
        paused: false,
    };
    let run = detail.run.clone();
    cache.save_run(&detail).await?;
    let handle = state.agents.insert(detail);
    engine::emit_state(app, &handle).await;

    let app = app.clone();
    let max = Duration::from_secs(u64::from(agent.max_session_minutes.max(1)) * 60);
    let run_tests = input.run_tests.unwrap_or(true);
    let root = worktree::root(&config.agents.worktree_root);
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(err) = run_queue(&app, &state, &handle, max, run_tests, &root).await {
            let mut d = handle.detail.write().await;
            d.error = Some(err.to_string());
            d.state = RunState::Failed;
            d.ended_at = Some(version::now_iso());
            drop(d);
            engine::log(&app, &handle, AgentEventKind::Error, format!("✕ {err}")).await;
            engine::persist(&state, &handle).await;
            engine::emit_state(&app, &handle).await;
        }
    });
    Ok(run)
}

struct Context {
    repo: String,
    default_branch: String,
    checkout: PathBuf,
    root: PathBuf,
    cli: PathBuf,
    max: Duration,
    run_tests: bool,
}

async fn run_queue(
    app: &AppHandle,
    state: &AppState,
    handle: &Arc<RunHandle>,
    max: Duration,
    run_tests: bool,
    root: &std::path::Path,
) -> AppResult<()> {
    let project = state.require_project().await?;
    let repo = project.repo.full_name.clone();
    let token = state.github_token()?;
    let cli = auth::cli_status(state, CliTool::ClaudeCode, false)
        .await
        .binary_path
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Validation("Claude Code est introuvable.".into()))?;
    engine::log(app, handle, AgentEventKind::System, format!("› copie de {repo} ({})…", project.repo.default_branch)).await;
    let checkout = vcs::checkout::ensure(&state.checkout_dir, &repo, &project.repo.default_branch, &token).await?;
    drop(token);
    engine::set_state(app, state, handle, RunState::Running).await;

    let ctx = Context {
        repo,
        default_branch: project.repo.default_branch.clone(),
        checkout,
        root: root.to_path_buf(),
        cli,
        max,
        run_tests,
    };
    loop {
        if *handle.stop.borrow() {
            break;
        }
        let next = handle.detail.read().await.tasks.iter().position(|t| t.state == DevTaskState::Queued);
        let Some(index) = next else { break };
        if let Err(err) = run_task(app, state, handle, &ctx, index).await {
            let stopped = *handle.stop.borrow();
            let message = err.to_string();
            {
                let mut d = handle.detail.write().await;
                let task = &mut d.tasks[index];
                task.state = if stopped { DevTaskState::Skipped } else { DevTaskState::Failed };
                task.error = Some(if stopped { "Arrêtée à votre demande.".into() } else { message.clone() });
            }
            if !stopped {
                engine::log(app, handle, AgentEventKind::Error, format!("✕ #{} : {message}", handle.detail.read().await.tasks[index].number)).await;
            }
            engine::persist(state, handle).await;
            engine::emit_state(app, handle).await;
            if message.contains("/login") {
                return Err(err);
            }
        }
    }

    let mut d = handle.detail.write().await;
    if d.state == RunState::Running {
        let waiting = d.tasks.iter().any(|t| t.state == DevTaskState::Review);
        d.state = if waiting { RunState::Review } else { RunState::Done };
        if !waiting {
            d.ended_at = Some(version::now_iso());
        }
    }
    drop(d);
    engine::log(app, handle, AgentEventKind::System, "› file terminée".into()).await;
    engine::persist(state, handle).await;
    engine::emit_state(app, handle).await;
    Ok(())
}

/// Attend la fin d'une pause (ou un arrêt) avant l'étape suivante.
async fn wait_if_paused(handle: &Arc<RunHandle>) -> AppResult<()> {
    loop {
        if *handle.stop.borrow() {
            return Err(AppError::Validation("Arrêtée à votre demande.".into()));
        }
        if !handle.detail.read().await.paused {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn step(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, index: usize, n: u8) -> AppResult<()> {
    wait_if_paused(handle).await?;
    {
        let mut d = handle.detail.write().await;
        d.tasks[index].step = n;
    }
    engine::persist(state, handle).await;
    engine::emit_state(app, handle).await;
    Ok(())
}

/// Résumé de l'étape courante (remplace le précédent de la même étape).
async fn note(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, index: usize, text: String) {
    {
        let mut d = handle.detail.write().await;
        let task = &mut d.tasks[index];
        let i = usize::from(task.step.max(1) - 1);
        if task.step_notes.len() <= i {
            task.step_notes.resize(i + 1, String::new());
        }
        task.step_notes[i] = text;
    }
    engine::persist(state, handle).await;
    engine::emit_state(app, handle).await;
}

async fn run_task(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, ctx: &Context, index: usize) -> AppResult<()> {
    let (task, autonomy, model) = {
        let mut d = handle.detail.write().await;
        let t = &mut d.tasks[index];
        t.state = DevTaskState::Running;
        t.started_at = Some(version::now_iso());
        (t.clone(), d.autonomy, d.model.clone())
    };
    let cache = state.repo_cache().await?;
    let body = cache.issues().await?.into_iter().find(|i| i.number == task.number).and_then(|i| i.body).unwrap_or_default();
    engine::log(app, handle, AgentEventKind::System, format!("› #{} {}", task.number, task.title)).await;

    // 1 — Analyse, dans le worktree (la session y sera reprise).
    step(app, state, handle, index, 1).await?;
    let wt = worktree::dir(&ctx.root, &ctx.repo, task.number)?;
    let base = worktree::create(&ctx.checkout, &wt, &task.branch).await?;
    engine::log(app, handle, AgentEventKind::System, format!("› worktree {}", wt.display())).await;
    {
        let mut d = handle.detail.write().await;
        d.tasks[index].worktree = Some(wt.to_string_lossy().to_string());
    }
    let prompt = analyse_prompt(&ctx.repo, &task, &body);
    let outcome = session(app, handle, ctx, &wt, model.as_deref(), Some(system_prompt()), &prompt, None, Tools::ReadOnly).await?;
    let session_id = outcome.session_id.clone();
    let plan = json_field(&outcome.text, "plan").unwrap_or_else(|| visible(&outcome.text));
    {
        let mut d = handle.detail.write().await;
        d.tasks[index].session_id = session_id.clone();
    }
    note(app, state, handle, index, first_line(&plan, "plan établi")).await;

    // 2 — Branche et PR en brouillon.
    step(app, state, handle, index, 2).await?;
    let author = author(state).await?;
    worktree::commit_empty(&wt, &author, &format!("chore(#{}): ouverture de la branche", task.number)).await?;
    if autonomy == Autonomy::Propose {
        note(app, state, handle, index, format!("{} · locale (propose seulement)", task.branch)).await;
    } else {
        let token = state.github_token()?;
        worktree::push(&wt, &ctx.repo, &task.branch, &ctx.default_branch, &token).await?;
        let title = format!("{}(#{}) : {}", task.issue_type.map(|t| t.slug()).unwrap_or("feat"), task.number, task.title);
        let body = format!("Ferme #{}\n\n### Plan\n\n{plan}\n\n_PR ouverte par l'agent développeur de Cadence ; le code arrive au fil des étapes._", task.number);
        let pr = writes::create_pull(&state.http, &token, &ctx.repo, &task.branch, &ctx.default_branch, &title, &body, true).await?;
        let _ = writes::add_labels(&state.http, &token, &ctx.repo, pr.number, &[AgentKind::Dev.label()]).await;
        engine::log(app, handle, AgentEventKind::Success, format!("✓ PR #{} ouverte (brouillon)", pr.number)).await;
        {
            let mut d = handle.detail.write().await;
            let t = &mut d.tasks[index];
            t.pr = Some(pr.number);
            t.pr_url = Some(pr.url);
            t.pr_node_id = Some(pr.node_id);
        }
        note(app, state, handle, index, format!("{} · PR #{}", task.branch, pr.number)).await;
    }

    // 3 — Implémentation.
    step(app, state, handle, index, 3).await?;
    let notes = handle.take_notes();
    let outcome = session(app, handle, ctx, &wt, model.as_deref(), None, &implement_prompt(&task.branch, &notes), session_id.as_deref(), Tools::Write).await?;
    let mut summary = json_field(&outcome.text, "summary").unwrap_or_else(|| visible(&outcome.text));
    let files = worktree::changed_files_uncommitted(&wt).await.unwrap_or_default();
    note(app, state, handle, index, files_note(&files)).await;

    // 4 — Tests, puis commit du travail.
    step(app, state, handle, index, 4).await?;
    if ctx.run_tests {
        let notes = handle.take_notes();
        let outcome = session(app, handle, ctx, &wt, model.as_deref(), None, &tests_prompt(&notes), session_id.as_deref(), Tools::Write).await?;
        let tests = json_field(&outcome.text, "tests").unwrap_or_else(|| "non précisé".into());
        if let Some(s) = json_field(&outcome.text, "summary") {
            summary = format!("{summary}\n\n**Tests** : {s}");
        }
        note(app, state, handle, index, format!("tests : {tests}")).await;
    } else {
        note(app, state, handle, index, "tests non lancés (option désactivée)".into()).await;
    }
    let kind = task.issue_type.map(|t| t.slug()).unwrap_or("feat");
    let message = format!("{kind}(#{}): {}\n\n{summary}\n\nTravail de l'agent développeur de Cadence.", task.number, task.title);
    if !worktree::commit_all(&wt, &author, &message).await? {
        return Err(AppError::Validation("L'agent n'a modifié aucun fichier.".into()));
    }
    let files = worktree::changed_files(&wt, &base).await?;
    {
        let mut d = handle.detail.write().await;
        let t = &mut d.tasks[index];
        t.additions = files.iter().map(|f| f.additions).sum();
        t.deletions = files.iter().map(|f| f.deletions).sum();
        t.files = files;
        t.summary = Some(summary);
    }
    engine::log(app, handle, AgentEventKind::Success, "✓ travail commité dans le worktree".into()).await;

    // 5 — Push : selon l'autonomie.
    step(app, state, handle, index, 5).await?;
    match autonomy {
        Autonomy::Auto => push_steps(app, state, handle, index, ctx).await,
        Autonomy::ApplyAfterReview => {
            {
                let mut d = handle.detail.write().await;
                d.tasks[index].state = DevTaskState::Review;
            }
            note(app, state, handle, index, "en attente de votre validation pour pousser".into()).await;
            engine::log(app, handle, AgentEventKind::Question, format!("? #{} : relisez le diff puis validez le push", task.number)).await;
            Ok(())
        }
        Autonomy::Propose => {
            {
                let mut d = handle.detail.write().await;
                d.tasks[index].state = DevTaskState::Done;
            }
            note(app, state, handle, index, "propose seulement : rien n'a été poussé".into()).await;
            Ok(())
        }
    }
}

/// Étapes 5 et 6 : push, sortie du brouillon, commentaire signé.
async fn push_steps(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, index: usize, ctx: &Context) -> AppResult<()> {
    let task = handle.detail.read().await.tasks[index].clone();
    let wt = PathBuf::from(task.worktree.clone().ok_or_else(|| AppError::Internal("worktree inconnu".into()))?);
    let token = state.github_token()?;
    worktree::push(&wt, &ctx.repo, &task.branch, &ctx.default_branch, &token).await?;
    engine::log(app, handle, AgentEventKind::Write, format!("push {}", task.branch)).await;
    note(app, state, handle, index, format!("{} commit(s) poussé(s)", 1)).await;

    step(app, state, handle, index, 6).await?;
    if let (Some(pr), Some(node)) = (task.pr, task.pr_node_id.as_deref()) {
        writes::mark_ready(&state.http, &token, node).await?;
        let comment = format!(
            "**Agent développeur** · [Cadence](https://github.com/cadence-app/desktop) — prêt pour la revue.\n\n{}\n\n_{} fichier(s), +{} −{}_",
            task.summary.clone().unwrap_or_default(),
            task.files.len(),
            task.additions,
            task.deletions
        );
        writes::create_comment(&state.http, &token, &ctx.repo, pr, &comment).await?;
        engine::log(app, handle, AgentEventKind::Success, format!("✓ PR #{pr} prête pour la revue")).await;
        note(app, state, handle, index, format!("PR #{pr} sortie du brouillon")).await;
    }
    {
        let mut d = handle.detail.write().await;
        d.tasks[index].state = DevTaskState::Done;
    }
    engine::persist(state, handle).await;
    engine::emit_state(app, handle).await;
    Ok(())
}

/// Validation du push d'une tâche en attente (« Pousser et demander la revue »).
pub async fn push_task(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, number: u32) -> AppResult<()> {
    let (index, autonomy) = {
        let d = handle.detail.read().await;
        let index = d
            .tasks
            .iter()
            .position(|t| t.number == number && t.state == DevTaskState::Review)
            .ok_or_else(|| AppError::Validation(format!("#{number} n'attend pas de validation.")))?;
        (index, d.autonomy)
    };
    if autonomy == Autonomy::Propose {
        return Err(AppError::Validation("L'agent développeur est réglé sur « propose seulement » : rien n'est poussé.".into()));
    }
    let project = state.require_project().await?;
    let ctx = Context {
        repo: project.repo.full_name.clone(),
        default_branch: project.repo.default_branch.clone(),
        checkout: PathBuf::new(),
        root: PathBuf::new(),
        cli: PathBuf::new(),
        max: Duration::from_secs(60),
        run_tests: false,
    };
    {
        let mut d = handle.detail.write().await;
        d.tasks[index].state = DevTaskState::Running;
    }
    let result = push_steps(app, state, handle, index, &ctx).await;
    if let Err(err) = &result {
        let mut d = handle.detail.write().await;
        d.tasks[index].state = DevTaskState::Review;
        d.tasks[index].error = Some(err.to_string());
    }
    engine::persist(state, handle).await;
    engine::emit_state(app, handle).await;
    result
}

#[allow(clippy::too_many_arguments)]
async fn session(
    app: &AppHandle,
    handle: &Arc<RunHandle>,
    ctx: &Context,
    cwd: &std::path::Path,
    model: Option<&str>,
    system: Option<&str>,
    prompt: &str,
    resume: Option<&str>,
    tools: Tools,
) -> AppResult<SessionOutcome> {
    engine::session(app, handle, &ctx.cli, cwd, model, system, prompt, resume, ctx.max, tools).await
}

async fn author(state: &AppState) -> AppResult<worktree::Author> {
    let github = state.github.read().await.clone().ok_or(AppError::Unauthenticated("GitHub"))?;
    let login = github.login.clone().ok_or(AppError::Unauthenticated("GitHub"))?;
    Ok(worktree::Author::github(&login, github.name.as_deref()))
}

fn first_line(text: &str, fallback: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or(fallback);
    let line = line.trim_start_matches(['#', '-', '*', ' ']);
    if line.chars().count() > 90 {
        format!("{}…", line.chars().take(89).collect::<String>())
    } else {
        line.to_string()
    }
}

fn files_note(files: &[crate::domain::agent::DevFile]) -> String {
    match files.len() {
        0 => "aucun fichier modifié".into(),
        n => {
            let names: Vec<&str> = files.iter().take(3).map(|f| f.path.rsplit('/').next().unwrap_or(&f.path)).collect();
            format!("{n} fichier(s) : {}{}", names.join(", "), if n > 3 { "…" } else { "" })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_json_fields_from_the_final_block() {
        let text = "Plan prêt.\n```json\n{\"plan\": \"Ajouter device_flow.rs\", \"files\": [\"a\"]}\n```";
        assert_eq!(json_field(text, "plan").as_deref(), Some("Ajouter device_flow.rs"));
        assert_eq!(json_field(text, "summary"), None);
        assert_eq!(json_field("rien", "plan"), None);
        assert_eq!(visible(text), "Plan prêt.");
    }

    #[test]
    fn prompts_forbid_git_and_carry_interventions() {
        assert!(system_prompt().contains("Ne fais jamais de commit, de push"));
        let p = implement_prompt("feat/1-x", &["Garde l'API existante".into()]);
        assert!(p.contains("feat/1-x") && p.contains("- Garde l'API existante"));
        assert!(!tests_prompt(&[]).contains("Messages de l'utilisateur"));
    }

    #[test]
    fn summarises_lines_and_files() {
        assert_eq!(first_line("\n## Approche : un module séparé\n…", "x"), "Approche : un module séparé");
        let files = vec![
            crate::domain::agent::DevFile { path: "src/a/b.rs".into(), additions: 1, deletions: 0, added: true },
        ];
        assert_eq!(files_note(&files), "1 fichier(s) : b.rs");
        assert_eq!(files_note(&[]), "aucun fichier modifié");
    }
}
