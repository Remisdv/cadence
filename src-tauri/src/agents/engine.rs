//! Exécutions d'agents : lancement, file, journal, décisions.
//!
//! Une exécution vit en mémoire (`RunHandle`) et dans `agent_runs` (reprise après
//! redémarrage). Les sessions Claude Code d'une exécution passent une par une (`session_lock`),
//! qu'il s'agisse de la file ou d'une reprise après vos réponses. Rien n'est écrit sur GitHub
//! hors de `apply` : validation explicite, ou autonomie `auto` (garde-fous inchangés).

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::json;
use tauri::{AppHandle, Manager};
use tokio::sync::{watch, RwLock};

use crate::{
    agents::{claude, cto},
    api::github::{issues as gh_issues, writes},
    auth::{self, cli::CliTool},
    domain::{
        agent::{
            AgentAction, AgentInput, AgentKind, AgentLogLine, AgentRunDetail, ApplyFailure, ApplyReport,
            Autonomy, CtoItem, CtoItemState, ProposedAction, RunState,
        },
        issue::CtoAnswer,
        settings::AgentEngine as Engine,
        version,
    },
    error::{AppError, AppResult},
    events::{self, AgentEvent, AgentEventKind, AgentStateEvent},
    state::AppState,
    store::RepoCache,
};

/// Lignes de journal gardées par exécution.
const LOG_LIMIT: usize = 400;
const MAX_TURNS: u32 = 40;

pub struct RunHandle {
    pub detail: RwLock<AgentRunDetail>,
    stop: watch::Sender<bool>,
    /// Une session Claude Code à la fois par exécution.
    session_lock: tokio::sync::Mutex<()>,
}

#[derive(Default)]
pub struct AgentEngine {
    runs: Mutex<HashMap<String, Arc<RunHandle>>>,
}

impl AgentEngine {
    fn get(&self, run: &str) -> Option<Arc<RunHandle>> {
        self.runs.lock().unwrap().get(run).cloned()
    }

    fn insert(&self, detail: AgentRunDetail) -> Arc<RunHandle> {
        let (stop, _) = watch::channel(false);
        let handle = Arc::new(RunHandle { detail: RwLock::new(detail), stop, session_lock: Default::default() });
        let run = handle.detail.try_read().map(|d| d.run.clone()).unwrap_or_default();
        self.runs.lock().unwrap().insert(run, handle.clone());
        handle
    }

    /// Oublie les exécutions en mémoire (changement de dépôt ou de compte) et arrête les sessions.
    pub fn clear(&self) {
        for (_, handle) in self.runs.lock().unwrap().drain() {
            let _ = handle.stop.send(true);
        }
    }
}

/* ---------- Lecture ---------- */

/// Exécution par identifiant : en mémoire, sinon relue dans le cache du dépôt.
pub async fn handle(state: &AppState, run: &str) -> AppResult<Arc<RunHandle>> {
    if let Some(h) = state.agents.get(run) {
        return Ok(h);
    }
    let cache = state.repo_cache().await?;
    let detail = cache
        .load_run(run)
        .await?
        .ok_or_else(|| AppError::Validation(format!("Exécution {run} introuvable.")))?;
    Ok(state.agents.insert(recovered(detail)))
}

/// Dernière exécution d'un agent sur le dépôt ouvert.
pub async fn latest(state: &AppState, kind: AgentKind) -> AppResult<Option<Arc<RunHandle>>> {
    let in_memory = {
        let runs = state.agents.runs.lock().unwrap();
        let mut candidates: Vec<_> = runs.values().cloned().collect();
        candidates.retain(|h| h.detail.try_read().is_ok_and(|d| d.kind == kind));
        candidates
    };
    let mut best: Option<(String, Arc<RunHandle>)> = None;
    for h in in_memory {
        let started = h.detail.read().await.started_at.clone();
        if best.as_ref().is_none_or(|(s, _)| started > *s) {
            best = Some((started, h));
        }
    }
    if let Some((_, h)) = best {
        return Ok(Some(h));
    }
    let cache = state.repo_cache().await?;
    Ok(cache.latest_run(kind).await?.map(|detail| state.agents.insert(recovered(detail))))
}

/// Exécution relue après un redémarrage : ce qui tournait ne tourne plus.
fn recovered(mut detail: AgentRunDetail) -> AgentRunDetail {
    let interrupted = matches!(detail.state, RunState::Prepare | RunState::Running | RunState::Applying);
    for item in &mut detail.items {
        if item.state == CtoItemState::Running {
            item.state = CtoItemState::Failed;
            item.error = Some("Session interrompue (Cadence a été fermé).".into());
        }
    }
    if interrupted {
        let waiting = detail.items.iter().any(|i| i.state.awaits_review());
        detail.state = if waiting { RunState::Review } else { RunState::Discarded };
        detail.ended_at.get_or_insert_with(version::now_iso);
    }
    detail
}

/// Actions en attente de validation, dérivées de la file.
pub fn proposed_actions(detail: &AgentRunDetail) -> Vec<ProposedAction> {
    let mut out = Vec::new();
    for item in &detail.items {
        match item.state {
            CtoItemState::Ready => {
                let after = item.after.clone().unwrap_or_default();
                out.push(ProposedAction {
                    id: format!("update-{}", item.number),
                    preview: format!(
                        "Réécrire la description de #{} ({} → {} lignes)",
                        item.number,
                        item.before.lines().count(),
                        after.lines().count()
                    ),
                    action: AgentAction::UpdateIssueBody { number: item.number, before: item.before.clone(), after },
                });
            }
            CtoItemState::Questions => {
                for q in item.questions.iter().filter(|q| q.answer.is_none()) {
                    out.push(ProposedAction {
                        id: format!("ask-{}-{}", item.number, q.id),
                        preview: format!("Question sur #{} : {}", item.number, q.question),
                        action: AgentAction::AskQuestion { number: item.number, question: q.question.clone() },
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/* ---------- Lancement ---------- */

/// Lance l'agent CTO sur une version (par défaut celle en cours) ou une liste d'issues.
/// Une seule exécution CTO active à la fois : la renvoie si elle existe.
pub async fn start_cto(app: &AppHandle, state: &AppState, input: AgentInput) -> AppResult<String> {
    if let Some(h) = latest(state, AgentKind::Cto).await? {
        let d = h.detail.read().await;
        if matches!(d.state, RunState::Prepare | RunState::Running) {
            return Ok(d.run.clone());
        }
    }
    let project = state.require_project().await?;
    let config = crate::commands::settings::load(state).await?.config;
    let agent = config.agents.cto.clone();
    if agent.engine != Engine::ClaudeCode {
        return Err(AppError::Validation("L'agent CTO ne sait pas encore piloter Codex : choisissez Claude Code dans les réglages.".into()));
    }
    let cli = auth::cli_status(state, CliTool::ClaudeCode, false).await;
    if cli.binary_path.is_none() {
        return Err(AppError::Validation("Claude Code est introuvable sur cette machine (Réglages › Machine locale).".into()));
    }

    let cache = state.repo_cache().await?;
    crate::sync::refresh(state, &cache, crate::domain::sync::SyncResource::Issues).await?;
    let target = input.version.clone().or(project.current_version.clone());
    let mut items = queue(&cache, target.as_deref(), input.issues.as_deref()).await?;
    // Propositions encore à relire dans l'exécution précédente : reportées dans la nouvelle,
    // qui devient la seule à suivre (l'ancienne est close).
    let mut carried = vec![];
    if let Some(previous) = latest(state, AgentKind::Cto).await? {
        let mut d = previous.detail.write().await;
        let pending: Vec<CtoItem> = d.items.iter().filter(|i| i.state.awaits_review()).cloned().collect();
        if !pending.is_empty() {
            for item in d.items.iter_mut().filter(|i| i.state.awaits_review()) {
                item.state = CtoItemState::Skipped;
                item.error = Some("Reportée dans l'exécution suivante.".into());
            }
            d.state = RunState::Done;
            d.ended_at.get_or_insert_with(version::now_iso);
            carried = pending;
        }
        drop(d);
        if !carried.is_empty() {
            persist(state, &previous).await;
        }
    }
    items.retain(|i| !carried.iter().any(|c| c.number == i.number));
    let new_items = items.len();
    items.splice(0..0, carried);
    if new_items == 0 && items.is_empty() {
        return Err(AppError::Validation(match &target {
            Some(v) => format!("Aucune issue ouverte à traiter sur {v} : toutes sont déjà passées par l'agent CTO."),
            None => "Aucune issue à traiter.".into(),
        }));
    }

    let now = version::now_iso();
    let detail = AgentRunDetail {
        run: format!("cto-{}", now.replace([':', '-'], "").trim_end_matches('Z').replace('T', "-")),
        kind: AgentKind::Cto,
        state: RunState::Prepare,
        autonomy: agent.autonomy,
        version: target,
        model: agent.model.clone(),
        started_at: now,
        ended_at: None,
        items,
        log: vec![],
        error: None,
    };
    let run = detail.run.clone();
    cache.save_run(&detail).await?;
    let handle = state.agents.insert(detail);
    emit_state(app, &handle).await;

    let app = app.clone();
    let max = Duration::from_secs(u64::from(agent.max_session_minutes.max(1)) * 60);
    let template = config.issues.template.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(err) = run_queue(&app, &state, &handle, &template, max).await {
            let mut d = handle.detail.write().await;
            d.error = Some(err.to_string());
            d.state = RunState::Failed;
            d.ended_at = Some(version::now_iso());
            drop(d);
            log(&app, &handle, AgentEventKind::Error, format!("✕ {err}")).await;
            persist(&state, &handle).await;
            emit_state(&app, &handle).await;
        }
    });
    Ok(run)
}

/// Envoie des issues précises à l'agent CTO : ajoutées à l'exécution en cours (la file les
/// prend au passage), sinon dans une nouvelle exécution.
pub async fn send_issues(app: &AppHandle, state: &AppState, numbers: Vec<u32>) -> AppResult<String> {
    if let Some(h) = latest(state, AgentKind::Cto).await? {
        let active = matches!(h.detail.read().await.state, RunState::Prepare | RunState::Running);
        if active {
            let cache = state.repo_cache().await?;
            crate::sync::refresh(state, &cache, crate::domain::sync::SyncResource::Issues).await?;
            let fresh = queue(&cache, None, Some(&numbers)).await?;
            let run = {
                let mut d = h.detail.write().await;
                let known: Vec<u32> = d.items.iter().map(|i| i.number).collect();
                let added: Vec<CtoItem> = fresh.into_iter().filter(|i| !known.contains(&i.number)).collect();
                if added.is_empty() {
                    return Ok(d.run.clone());
                }
                d.items.extend(added);
                d.run.clone()
            };
            log(app, &h, AgentEventKind::System, format!("› ajoutée(s) à la file : {}", numbers.iter().map(|n| format!("#{n}")).collect::<Vec<_>>().join(", "))).await;
            persist(state, &h).await;
            emit_state(app, &h).await;
            return Ok(run);
        }
    }
    start_cto(app, state, AgentInput { version: None, issues: Some(numbers) }).await
}

/// Issues à traiter : ouvertes, de la version (ou listées), pas encore passées par le CTO
/// (sauf si listées explicitement). Les descriptions vides d'abord.
async fn queue(cache: &RepoCache, version_title: Option<&str>, only: Option<&[u32]>) -> AppResult<Vec<CtoItem>> {
    let raws = cache.issues().await?;
    let mut items: Vec<CtoItem> = raws
        .iter()
        .filter(|raw| raw.state == "open")
        .filter(|raw| match only {
            Some(list) => list.contains(&raw.number),
            None => {
                let v = raw.milestone.as_ref().and_then(|m| version::parse(&m.title)).map(|v| v.to_string());
                v.as_deref() == version_title && !raw.labels.iter().any(|l| l.name == AgentKind::Cto.label())
            }
        })
        .map(|raw| {
            let row = gh_issues::to_row(raw, None);
            CtoItem {
                number: raw.number,
                title: raw.title.clone(),
                version: row.version,
                issue_type: row.issue_type,
                state: CtoItemState::Queued,
                before: raw.body.clone().unwrap_or_default(),
                after: None,
                questions: vec![],
                estimate: None,
                estimate_hours: None,
                labels: vec![],
                risk: None,
                related: vec![],
                error: None,
                session_id: None,
            }
        })
        .collect();
    items.sort_by_key(|i| (!i.before.trim().is_empty(), i.number));
    Ok(items)
}

async fn run_queue(
    app: &AppHandle,
    state: &AppState,
    handle: &Arc<RunHandle>,
    template: &str,
    max: Duration,
) -> AppResult<()> {
    let project = state.require_project().await?;
    let repo = project.repo.full_name.clone();
    let token = state.github_token()?;
    let cli = auth::cli_status(state, CliTool::ClaudeCode, false)
        .await
        .binary_path
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Validation("Claude Code est introuvable.".into()))?;

    log(app, handle, AgentEventKind::System, format!("› copie de {repo} ({})…", project.repo.default_branch)).await;
    let (cwd, has_code) = match crate::vcs::checkout::ensure(&state.checkout_dir, &repo, &project.repo.default_branch, &token).await {
        Ok(dir) => (dir, true),
        Err(err) => {
            log(app, handle, AgentEventKind::Error, format!("copie impossible, l'agent travaillera sans le code : {err}")).await;
            (std::env::temp_dir(), false)
        }
    };
    drop(token);
    let count = handle.detail.read().await.items.len();
    log(app, handle, AgentEventKind::System, format!("› {count} issue{} à traiter", if count > 1 { "s" } else { "" })).await;
    set_state(app, state, handle, RunState::Running).await;

    let system = cto::system_prompt(template, has_code);
    loop {
        if *handle.stop.borrow() {
            break;
        }
        // Prochaine issue encore en file (vous avez pu en passer entre-temps).
        let next = {
            let d = handle.detail.read().await;
            d.items.iter().position(|i| i.state == CtoItemState::Queued)
        };
        let Some(index) = next else { break };
        let _session = handle.session_lock.lock().await;
        let (item, model) = {
            let mut d = handle.detail.write().await;
            d.items[index].state = CtoItemState::Running;
            (d.items[index].clone(), d.model.clone())
        };
        emit_state(app, handle).await;
        log(app, handle, AgentEventKind::System, format!("› #{} {}", item.number, item.title)).await;

        let prompt = cto::issue_prompt(&repo, &item);
        let outcome = session(app, handle, &cli, &cwd, model.as_deref(), Some(&system), &prompt, None, max).await;
        let auto = finish_item(app, state, handle, index, outcome).await?;
        if auto && let Err(err) = apply_item(app, state, handle, item.number).await {
            log(app, handle, AgentEventKind::Error, format!("✕ #{} non appliquée : {err}", item.number)).await;
        }
    }

    let mut d = handle.detail.write().await;
    if d.state == RunState::Running {
        let waiting = d.items.iter().any(|i| i.state.awaits_review());
        d.state = if waiting { RunState::Review } else { RunState::Done };
        if !waiting {
            d.ended_at = Some(version::now_iso());
        }
    }
    drop(d);
    log(app, handle, AgentEventKind::System, "› file terminée".into()).await;
    persist(state, handle).await;
    emit_state(app, handle).await;
    Ok(())
}

/// Range le résultat d'une session dans l'issue `index`. Renvoie vrai s'il faut appliquer
/// directement (autonomie `auto`, aucune question). Une session arrêtée ou non connectée
/// interrompt toute la file.
async fn finish_item(
    app: &AppHandle,
    state: &AppState,
    handle: &Arc<RunHandle>,
    index: usize,
    outcome: AppResult<claude::SessionOutcome>,
) -> AppResult<bool> {
    let mut d = handle.detail.write().await;
    let autonomy = d.autonomy;
    let item = &mut d.items[index];
    let result = outcome.and_then(|o| cto::parse_proposal(&o.text).map(|p| (p, o.session_id)));
    let stopped = *handle.stop.borrow();
    let (kind, line, fatal) = match result {
        Ok((proposal, session)) => {
            cto::apply_proposal(item, proposal, session);
            let len = item.after.as_deref().map(|a| a.chars().count()).unwrap_or(0);
            if item.questions.is_empty() {
                (AgentEventKind::Success, format!("✓ description rédigée ({len} car.)"), None)
            } else {
                let n = item.questions.len();
                let verb = if n > 1 { "restent" } else { "reste" };
                (AgentEventKind::Question, format!("? {n} point{} {verb} ambigu{}", plural(n), plural(n)), None)
            }
        }
        // Arrêt demandé : l'issue revient en file, sans erreur.
        Err(_) if stopped => {
            item.state = CtoItemState::Queued;
            (AgentEventKind::System, format!("› #{} remise en file", item.number), None)
        }
        Err(err) => {
            let message = err.to_string();
            item.state = CtoItemState::Failed;
            item.error = Some(message.clone());
            // Claude Code non connecté : inutile de continuer la file.
            let fatal = message.contains("/login");
            (AgentEventKind::Error, format!("✕ {message}"), fatal.then_some(err))
        }
    };
    let auto = autonomy == Autonomy::Auto && item.state == CtoItemState::Ready;
    drop(d);
    log(app, handle, kind, line).await;
    persist(state, handle).await;
    emit_state(app, handle).await;
    match fatal {
        Some(err) => Err(err),
        None => Ok(auto),
    }
}

#[allow(clippy::too_many_arguments)]
async fn session(
    app: &AppHandle,
    handle: &Arc<RunHandle>,
    cli: &std::path::Path,
    cwd: &std::path::Path,
    model: Option<&str>,
    system: Option<&str>,
    prompt: &str,
    resume: Option<&str>,
    max: Duration,
) -> AppResult<claude::SessionOutcome> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let opts = claude::SessionOptions { cli, cwd, model, system, prompt, resume, max_turns: MAX_TURNS, timeout: max };
    let run = claude::run(opts, handle.stop.subscribe(), move |kind, text| {
        let _ = tx.send((kind, text));
    });
    tokio::pin!(run);
    loop {
        tokio::select! {
            Some((kind, text)) = rx.recv() => log(app, handle, kind, text).await,
            result = &mut run => {
                while let Ok((kind, text)) = rx.try_recv() {
                    log(app, handle, kind, text).await;
                }
                return result;
            }
        }
    }
}

/* ---------- Réponses, décisions ---------- */

/// Vos réponses aux questions de l'agent sur une issue : reprises dans la même session.
pub async fn answer(app: &AppHandle, state: &AppState, number: u32, answers: Vec<CtoAnswer>) -> AppResult<()> {
    let handle = latest(state, AgentKind::Cto)
        .await?
        .ok_or_else(|| AppError::Validation("Aucune exécution de l'agent CTO en cours.".into()))?;
    let (index, item, model) = {
        let mut d = handle.detail.write().await;
        let index = d
            .items
            .iter()
            .position(|i| i.number == number && i.state == CtoItemState::Questions)
            .ok_or_else(|| AppError::Validation(format!("L'agent n'attend pas de réponse sur #{number}.")))?;
        let item = &mut d.items[index];
        if item.session_id.is_none() {
            return Err(AppError::Validation("Session de l'agent introuvable : relancez l'issue.".into()));
        }
        for a in &answers {
            if let Some(q) = item.questions.iter_mut().find(|q| q.id == a.question_id) {
                q.answer = Some(a.answer.trim().to_string()).filter(|s| !s.is_empty());
            }
        }
        if item.questions.iter().all(|q| q.answer.is_none()) {
            return Err(AppError::Validation("Répondez à au moins une question.".into()));
        }
        item.state = CtoItemState::Running;
        (index, item.clone(), d.model.clone())
    };
    persist(state, &handle).await;
    emit_state(app, &handle).await;

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let _session = handle.session_lock.lock().await;
        log(&app, &handle, AgentEventKind::System, format!("› réponses reçues pour #{number}")).await;
        let cwd = state
            .require_project()
            .await
            .ok()
            .and_then(|p| crate::vcs::checkout::checkout_dir(&state.checkout_dir, &p.repo.full_name).ok())
            .filter(|d| d.exists())
            .unwrap_or_else(std::env::temp_dir);
        let cli = auth::cli_status(&state, CliTool::ClaudeCode, false).await.binary_path.map(PathBuf::from);
        let max = Duration::from_secs(30 * 60);
        let outcome = match cli {
            Some(cli) => {
                let prompt = cto::answers_prompt(&item.questions);
                session(&app, &handle, &cli, &cwd, model.as_deref(), None, &prompt, item.session_id.as_deref(), max).await
            }
            None => Err(AppError::Validation("Claude Code est introuvable.".into())),
        };
        let _ = finish_item(&app, &state, &handle, index, outcome).await;
        settle(&app, &state, &handle).await;
    });
    Ok(())
}

/// Applique les actions retenues : seul chemin d'écriture d'un agent vers GitHub.
pub async fn apply(app: &AppHandle, state: &AppState, run: &str, ids: Vec<String>) -> AppResult<ApplyReport> {
    let handle = handle(state, run).await?;
    let mut report = ApplyReport { applied: vec![], failed: vec![] };
    for id in ids {
        let Some(number) = id.strip_prefix("update-").and_then(|n| n.parse::<u32>().ok()) else {
            report.failed.push(ApplyFailure { id, message: "Action inconnue.".into() });
            continue;
        };
        match apply_item(app, state, &handle, number).await {
            Ok(()) => report.applied.push(id),
            Err(err) => report.failed.push(ApplyFailure { id, message: err.to_string() }),
        }
    }
    settle(app, state, &handle).await;
    Ok(report)
}

/// Passe des issues sans rien écrire.
pub async fn reject(app: &AppHandle, state: &AppState, run: &str, numbers: Vec<u32>) -> AppResult<()> {
    let handle = handle(state, run).await?;
    {
        let mut d = handle.detail.write().await;
        for item in d.items.iter_mut().filter(|i| numbers.contains(&i.number)) {
            if matches!(item.state, CtoItemState::Queued | CtoItemState::Ready | CtoItemState::Questions | CtoItemState::Failed) {
                item.state = CtoItemState::Skipped;
            }
        }
    }
    settle(app, state, &handle).await;
    Ok(())
}

/// Remplace la description proposée (« Éditer avant d'appliquer »).
pub async fn edit(app: &AppHandle, state: &AppState, run: &str, number: u32, body: String) -> AppResult<()> {
    if body.trim().is_empty() {
        return Err(AppError::Validation("La description ne peut pas être vide.".into()));
    }
    let handle = handle(state, run).await?;
    {
        let mut d = handle.detail.write().await;
        let item = d
            .items
            .iter_mut()
            .find(|i| i.number == number && i.state.awaits_review())
            .ok_or_else(|| AppError::Validation(format!("Aucune proposition à éditer sur #{number}.")))?;
        item.after = Some(body.trim().to_string());
    }
    persist(state, &handle).await;
    emit_state(app, &handle).await;
    Ok(())
}

pub async fn stop(app: &AppHandle, state: &AppState, run: &str) -> AppResult<()> {
    let handle = handle(state, run).await?;
    let _ = handle.stop.send(true);
    {
        let mut d = handle.detail.write().await;
        if !d.state.is_terminal() {
            let waiting = d.items.iter().any(|i| i.state.awaits_review());
            d.state = if waiting { RunState::Review } else { RunState::Discarded };
            d.ended_at.get_or_insert_with(version::now_iso);
        }
    }
    log(app, &handle, AgentEventKind::System, "› arrêtée à votre demande".into()).await;
    persist(state, &handle).await;
    emit_state(app, &handle).await;
    Ok(())
}

/// Écrit la description d'une issue prête sur GitHub, avec labels et commentaire signé.
async fn apply_item(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, number: u32) -> AppResult<()> {
    let (item, autonomy) = {
        let d = handle.detail.read().await;
        let item = d
            .items
            .iter()
            .find(|i| i.number == number)
            .cloned()
            .ok_or_else(|| AppError::Validation(format!("#{number} n'est pas dans la file.")))?;
        (item, d.autonomy)
    };
    if autonomy == Autonomy::Propose {
        return Err(AppError::Validation("L'agent CTO est réglé sur « propose seulement » : rien n'est écrit.".into()));
    }
    if item.state != CtoItemState::Ready {
        return Err(AppError::Validation(match item.state {
            CtoItemState::Questions => format!("#{number} attend vos réponses avant d'être appliquée."),
            _ => format!("#{number} n'a pas de proposition prête."),
        }));
    }
    let after = item.after.clone().unwrap_or_default();
    let cache = state.repo_cache().await?;
    let token = state.github_token()?;

    // Labels : ceux de l'issue, plus la trace d'agent, les labels et l'estimation proposés.
    let current = cache.issues().await?.into_iter().find(|i| i.number == number);
    let mut labels: Vec<String> = current.iter().flat_map(|i| i.labels.iter().map(|l| l.name.clone())).collect();
    if current.as_ref().is_some_and(|i| i.state != "open") {
        return Err(AppError::Validation(format!("#{number} a été fermée entre-temps.")));
    }
    labels.retain(|l| !l.starts_with("estimate/"));
    labels.push(AgentKind::Cto.label().to_string());
    labels.extend(item.labels.iter().cloned());
    if let Some(estimate) = &item.estimate {
        labels.push(format!("estimate/{estimate}"));
    }
    labels.sort();
    labels.dedup();

    let (issue, raw) = writes::update_issue(
        &state.http,
        &token,
        &cache.repo,
        number,
        &json!({ "body": after, "labels": labels }),
    )
    .await?;
    cache.upsert_issues(&[(issue, raw)]).await?;
    writes::create_comment(&state.http, &token, &cache.repo, number, &signed_comment(&item)).await?;

    {
        let mut d = handle.detail.write().await;
        if let Some(i) = d.items.iter_mut().find(|i| i.number == number) {
            i.state = CtoItemState::Applied;
            i.before = after;
        }
    }
    log(app, handle, AgentEventKind::Write, format!("write #{number} : description et labels")).await;
    persist(state, handle).await;
    emit_state(app, handle).await;
    Ok(())
}

/// Trace visible sur GitHub pour qui n'a pas Cadence.
fn signed_comment(item: &CtoItem) -> String {
    let mut lines = vec!["**Agent CTO** · [Cadence](https://github.com/cadence-app/desktop) — description réécrite et validée.".to_string()];
    let answered: Vec<String> = item
        .questions
        .iter()
        .filter_map(|q| q.answer.as_ref().map(|a| format!("> {}\n\n{}", q.question, a)))
        .collect();
    if !answered.is_empty() {
        lines.push(format!("Réponses intégrées :\n\n{}", answered.join("\n\n")));
    }
    let mut extra = vec![];
    if let Some(e) = &item.estimate {
        extra.push(format!("estimation {e}{}", item.estimate_hours.map(|h| format!(" (~{h} h)")).unwrap_or_default()));
    }
    if let Some(r) = &item.risk {
        extra.push(format!("risque {r}"));
    }
    if !item.related.is_empty() {
        extra.push(format!("liée à {}", item.related.iter().map(|n| format!("#{n}")).collect::<Vec<_>>().join(", ")));
    }
    if !extra.is_empty() {
        lines.push(format!("_{}_", extra.join(" · ")));
    }
    lines.join("\n\n")
}

/// Après une décision : l'exécution est terminée quand plus rien n'attend.
async fn settle(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>) {
    {
        let mut d = handle.detail.write().await;
        let busy = d.items.iter().any(|i| matches!(i.state, CtoItemState::Queued | CtoItemState::Running));
        let waiting = d.items.iter().any(|i| i.state.awaits_review());
        if d.state == RunState::Review && !busy && !waiting {
            d.state = RunState::Done;
            d.ended_at = Some(version::now_iso());
        }
    }
    persist(state, handle).await;
    emit_state(app, handle).await;
}

/* ---------- Journal, persistance, événements ---------- */

async fn log(app: &AppHandle, handle: &Arc<RunHandle>, kind: AgentEventKind, text: String) {
    let run = {
        let mut d = handle.detail.write().await;
        d.log.push(AgentLogLine { kind, text: text.clone() });
        let excess = d.log.len().saturating_sub(LOG_LIMIT);
        d.log.drain(..excess);
        d.run.clone()
    };
    events::emit(app, events::AGENT_EVENT, AgentEvent { run, kind, text, payload: serde_json::Value::Null });
}

async fn set_state(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, next: RunState) {
    {
        let mut d = handle.detail.write().await;
        if d.state.can_transition_to(next) {
            d.state = next;
        } else {
            tracing::warn!(from = ?d.state, to = ?next, "transition d'agent ignorée");
        }
    }
    persist(state, handle).await;
    emit_state(app, handle).await;
}

async fn persist(state: &AppState, handle: &Arc<RunHandle>) {
    let detail = handle.detail.read().await.clone();
    match state.repo_cache().await {
        Ok(cache) => {
            if let Err(err) = cache.save_run(&detail).await {
                tracing::warn!(%err, "journal d'agent non enregistré");
            }
        }
        Err(err) => tracing::warn!(%err, "journal d'agent non enregistré"),
    }
}

async fn emit_state(app: &AppHandle, handle: &Arc<RunHandle>) {
    let d = handle.detail.read().await;
    let done = d.items.iter().filter(|i| !matches!(i.state, CtoItemState::Queued | CtoItemState::Running)).count();
    let event = AgentStateEvent {
        run: d.run.clone(),
        state: d.state,
        step: Some(done.min(255) as u8),
        steps: Some(d.items.len().min(255) as u8),
    };
    drop(d);
    events::emit(app, events::AGENT_STATE, event);
}

fn plural(n: usize) -> &'static str {
    if n > 1 { "s" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::issue::CtoQuestion;

    fn item(number: u32, state: CtoItemState) -> CtoItem {
        CtoItem {
            number,
            title: format!("Issue {number}"),
            version: Some("0.1.2".into()),
            issue_type: None,
            state,
            before: "avant".into(),
            after: Some("## Contexte\n…\n## Attendu".into()),
            questions: vec![
                CtoQuestion { id: "q1".into(), question: "Visible ?".into(), answer: None },
                CtoQuestion { id: "q2".into(), question: "Seuls ?".into(), answer: Some("Oui".into()) },
            ],
            estimate: Some("M".into()),
            estimate_hours: Some(4),
            labels: vec!["area/api".into()],
            risk: Some("faible".into()),
            related: vec![129],
            error: None,
            session_id: Some("s".into()),
        }
    }

    fn detail(items: Vec<CtoItem>, state: RunState) -> AgentRunDetail {
        AgentRunDetail {
            run: "cto-1".into(),
            kind: AgentKind::Cto,
            state,
            autonomy: Autonomy::ApplyAfterReview,
            version: Some("0.1.2".into()),
            model: Some("opus".into()),
            started_at: "2026-10-15T10:00:00Z".into(),
            ended_at: None,
            items,
            log: vec![],
            error: None,
        }
    }

    #[test]
    fn derives_pending_actions_from_the_queue() {
        let d = detail(
            vec![item(156, CtoItemState::Ready), item(157, CtoItemState::Questions), item(158, CtoItemState::Applied)],
            RunState::Review,
        );
        let ids: Vec<_> = proposed_actions(&d).into_iter().map(|a| a.id).collect();
        assert_eq!(ids, ["update-156", "ask-157-q1"]);
    }

    #[test]
    fn a_run_interrupted_by_a_restart_keeps_what_awaits_review() {
        let d = recovered(detail(vec![item(1, CtoItemState::Ready), item(2, CtoItemState::Running)], RunState::Running));
        assert_eq!(d.state, RunState::Review);
        assert_eq!(d.items[1].state, CtoItemState::Failed);
        assert!(d.ended_at.is_some());
        let idle = recovered(detail(vec![item(2, CtoItemState::Running)], RunState::Running));
        assert_eq!(idle.state, RunState::Discarded);
    }

    #[test]
    fn signs_the_comment_with_answers_and_proposals() {
        let comment = signed_comment(&item(156, CtoItemState::Ready));
        assert!(comment.starts_with("**Agent CTO**"));
        assert!(comment.contains("> Seuls ?\n\nOui"));
        assert!(!comment.contains("Visible ?"), "une question sans réponse n'est pas citée");
        assert!(comment.contains("estimation M (~4 h) · risque faible · liée à #129"));
    }
}
