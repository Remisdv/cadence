//! Agent déploiement : livrer une version close et vérifier ce qu'elle promet.
//!
//! Cinq étapes, sur le tag de la version :
//! 1. pré-vol : tag et brouillon de release présents, issues closes de la version et leurs
//!    critères d'acceptation (lus dans le cache) ;
//! 2. build : worktree détaché sur le tag, session Claude Code qui construit selon les
//!    conventions du dépôt ;
//! 3. tests de vérification : un test par issue, dérivé de ses critères d'acceptation et
//!    rejoué sur ce qui a été construit (même session, reprise) ;
//! 4. documentation : notes de version regroupées par type d'issue, sans les issues en échec ;
//! 5. tag et release : `PublishRelease` attend votre validation, avec un `ReopenIssue` par
//!    test en échec. Un échec ne bloque pas la release : vous tranchez.
//!
//! La publication n'est jamais automatique (même en autonomie `auto`) ni mise en file
//! hors-ligne. L'agent ne pousse rien : le worktree est détaché, seul Cadence écrit sur GitHub.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::{
    claude::{SessionOutcome, Tools},
    engine::{self, RunHandle},
};
use crate::{
    api::github::{repos, writes},
    auth::{self, cli::CliTool},
    domain::{
        agent::{
            AgentAction, AgentInput, AgentKind, AgentRunDetail, Autonomy, DeployArtifact, DeployDecision, DeployStage,
            DeployStageState, DeployState, DeployTest, DeployTestStatus, ProposedAction, RunState,
        },
        issue::IssueType,
        settings::AgentEngine,
        sync::SyncResource,
        version::{self, Version, VersionState},
    },
    error::{AppError, AppResult},
    events::AgentEventKind,
    state::AppState,
    vcs::{self, checkout, worktree},
};

pub const STEPS: u8 = 5;
/// Identifiant de l'action `PublishRelease`.
pub const PUBLISH_ID: &str = "publish-release";

/* ---------- Prompts ---------- */

pub fn system_prompt(tag: &str) -> String {
    format!(
        "Tu es l'agent déploiement de Cadence. Le dossier courant est un worktree Git détaché sur le \
         tag {tag} d'une version close. Tu construis le projet, puis tu vérifies une à une les \
         promesses de la version sur ce que tu as construit.\n\n\
         - Respecte les conventions du dépôt (CLAUDE.md, README, scripts de build).\n\
         - Ne modifie pas le code source : construis et vérifie seulement.\n\
         - Ne fais jamais de commit, de push, de tag ni de release : Cadence s'en charge après validation.\n\
         - N'utilise pas le réseau, sauf pour installer les dépendances du build. Écris en français."
    )
}

pub fn build_prompt(repo: &str, tag: &str) -> String {
    format!(
        "Étape 2 — build. Dépôt {repo}, tag {tag}. Construis le livrable selon les conventions du \
         dépôt (README, CLAUDE.md, scripts du gestionnaire de paquets). S'il ne produit pas de \
         binaire, lance son build de production habituel.\n\n\
         Termine par un bloc ```json {{\"status\": \"réussi | échec\", \"summary\": \"…plateformes, \
         commandes lancées…\", \"artifacts\": [\"chemin relatif de chaque fichier produit\"]}}."
    )
}

pub fn verify_prompt(test: &DeployTest, notes: &[String]) -> String {
    let criteria = test.criteria.iter().map(|c| format!("- {c}")).collect::<Vec<_>>().join("\n");
    let interventions = if notes.is_empty() {
        String::new()
    } else {
        format!("\n\nMessages de l'utilisateur :\n{}", notes.iter().map(|n| format!("- {n}")).collect::<Vec<_>>().join("\n"))
    };
    format!(
        "Étape 3 — test de vérification de l'issue #{number} : {title}\n\nCritères d'acceptation :\n{criteria}\n\n\
         Vérifie chaque critère sur ce que tu viens de construire (lance le binaire, les tests ou les \
         commandes utiles ; lis le code seulement si rien d'autre n'est possible). Ne corrige rien.{interventions}\n\n\
         Termine par un bloc ```json {{\"status\": \"réussi | échec | non vérifiable\", \"detail\": \"…ce que \
         tu as constaté, en une ou deux phrases…\"}}.",
        number = test.number,
        title = test.title,
    )
}

/* ---------- Logique pure ---------- */

/// Critères d'acceptation d'une issue : cases à cocher (`- [ ]`, `- [x]`) de la section
/// « Critères d'acceptation » (titre markdown ou ligne en gras) ; sans cette section, toutes
/// les cases de la description.
pub fn acceptance_criteria(body: &str) -> Vec<String> {
    let (mut section, mut all) = (Vec::new(), Vec::new());
    let (mut inside, mut found) = (false, false);
    for line in body.lines().map(str::trim) {
        if is_heading(line) {
            inside = is_criteria_heading(line);
            found |= inside;
            continue;
        }
        if let Some(c) = checkbox(line) {
            if inside {
                section.push(c.clone());
            }
            all.push(c);
        }
    }
    if found { section } else { all }
}

fn is_heading(line: &str) -> bool {
    line.starts_with('#') || (line.len() > 4 && line.starts_with("**") && line.ends_with("**"))
}

fn is_criteria_heading(line: &str) -> bool {
    let l = line.to_lowercase().replace('’', "'");
    ["critères d'acceptation", "criteres d'acceptation", "critère d'acceptation", "acceptance criteria"]
        .iter()
        .any(|k| l.contains(k))
}

fn checkbox(line: &str) -> Option<String> {
    let rest = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")).or_else(|| line.strip_prefix("+ "))?;
    let rest = rest.trim_start();
    let text = ["[ ]", "[x]", "[X]"].iter().find_map(|b| rest.strip_prefix(b))?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Dernier bloc ```json d'une réponse.
fn last_json(text: &str) -> Option<Value> {
    let block = text.rsplit_once("```json").map(|(_, rest)| rest.split("```").next().unwrap_or(rest))?;
    serde_json::from_str(block.trim()).ok()
}

fn str_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Texte visible d'une réponse, sans le bloc JSON final.
fn visible(text: &str) -> String {
    text.split("```json").next().unwrap_or(text).trim().to_string()
}

fn fold(word: &str) -> String {
    word.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' => 'e',
            '-' | '_' => ' ',
            c => c,
        })
        .collect()
}

/// Verdict d'une session de vérification : statut et détail.
pub fn parse_verdict(text: &str) -> (DeployTestStatus, Option<String>) {
    let json = last_json(text);
    let detail = json.as_ref().and_then(|v| str_field(v, "detail")).or_else(|| Some(visible(text)).filter(|s| !s.is_empty()));
    let status = json.as_ref().and_then(|v| str_field(v, "status")).map(|s| fold(&s));
    match status.as_deref() {
        Some("reussi" | "passed" | "pass" | "ok" | "succes") => (DeployTestStatus::Passed, detail),
        Some("echec" | "failed" | "fail" | "ko") => (DeployTestStatus::Failed, detail),
        Some("non verifiable" | "unverifiable") => (DeployTestStatus::Unverifiable, detail),
        _ => (DeployTestStatus::Unverifiable, Some(detail.unwrap_or_else(|| "Verdict illisible.".into()))),
    }
}

/// Résultat du build annoncé par l'agent.
#[derive(Debug, PartialEq)]
pub struct BuildReport {
    pub ok: bool,
    pub summary: Option<String>,
    pub artifacts: Vec<String>,
}

pub fn parse_build(text: &str) -> BuildReport {
    let json = last_json(text).unwrap_or(Value::Null);
    let status = str_field(&json, "status").map(|s| fold(&s));
    BuildReport {
        ok: matches!(status.as_deref(), Some("reussi" | "passed" | "ok" | "succes")),
        summary: str_field(&json, "summary").or_else(|| Some(visible(text)).filter(|s| !s.is_empty())),
        artifacts: json["artifacts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|p| p.trim().replace('\\', "/"))
            .filter(|p| artifact_path_ok(p))
            .collect(),
    }
}

/// Chemin d'artefact relatif, qui reste dans le worktree.
pub fn artifact_path_ok(path: &str) -> bool {
    !path.is_empty()
        && !Path::new(path).is_absolute()
        && !path.starts_with('/')
        && !path.contains(':')
        && !path.split('/').any(|part| part == "..")
}

/// Une issue figure dans les notes si son test passe, n'est pas vérifiable, ou si vous avez
/// ignoré son échec.
pub fn in_notes(test: &DeployTest) -> bool {
    match test.status {
        DeployTestStatus::Passed | DeployTestStatus::Unverifiable => true,
        DeployTestStatus::Failed => test.decision == Some(DeployDecision::Ignored),
        DeployTestStatus::Queued | DeployTestStatus::Running => false,
    }
}

const SECTIONS: [(&str, &[Option<IssueType>]); 5] = [
    ("Nouveautés", &[Some(IssueType::Feat)]),
    ("Corrections", &[Some(IssueType::Fix)]),
    ("Performances", &[Some(IssueType::Perf)]),
    ("Documentation", &[Some(IssueType::Docs)]),
    ("Maintenance", &[Some(IssueType::Chore), None]),
];

/// Notes de version en markdown, regroupées par type d'issue, sans les issues en échec.
pub fn release_notes(tag: &str, name: Option<&str>, tests: &[DeployTest]) -> String {
    let mut out = match name.filter(|n| !n.trim().is_empty()) {
        Some(name) => format!("## {tag} — {}\n", name.trim()),
        None => format!("## {tag}\n"),
    };
    let mut any = false;
    for (title, types) in SECTIONS {
        let lines: Vec<String> = tests
            .iter()
            .filter(|t| in_notes(t) && types.contains(&t.issue_type))
            .map(|t| format!("- {} (#{})", t.title.trim(), t.number))
            .collect();
        if !lines.is_empty() {
            any = true;
            out.push_str(&format!("\n### {title}\n{}\n", lines.join("\n")));
        }
    }
    if !any {
        out.push_str("\nAucune issue livrée sur cette version.\n");
    }
    out
}

/// État et résumé de l'étape 3 : « 13 / 14 · 1 échec sur #128 ».
pub fn tests_summary(tests: &[DeployTest]) -> (DeployStageState, String) {
    let total = tests.len();
    let passed = tests.iter().filter(|t| t.status == DeployTestStatus::Passed).count();
    let failed: Vec<String> =
        tests.iter().filter(|t| t.status == DeployTestStatus::Failed).map(|t| format!("#{}", t.number)).collect();
    let unverifiable = tests.iter().filter(|t| t.status == DeployTestStatus::Unverifiable).count();
    let mut parts = vec![format!("{passed} / {total}")];
    if !failed.is_empty() {
        let n = failed.len();
        parts.push(format!("{n} échec{} sur {}", if n > 1 { "s" } else { "" }, failed.join(", ")));
    }
    if unverifiable > 0 {
        parts.push(format!("{unverifiable} non vérifiable{}", if unverifiable > 1 { "s" } else { "" }));
    }
    let state = if failed.is_empty() { DeployStageState::Success } else { DeployStageState::Failure };
    (state, parts.join(" · "))
}

/// Version où rouvrir une issue en échec : la plus petite version ouverte au-delà de `closed`.
pub fn next_version(versions: &[Version], closed: &str) -> Option<String> {
    let after = version::parse(closed)?;
    versions
        .iter()
        .filter(|v| v.state == VersionState::Open)
        .filter_map(|v| version::parse(&v.title).filter(|s| *s > after).map(|s| (s, v.title.clone())))
        .min_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, title)| title)
}

/// Dernière version close (cible par défaut de l'agent).
pub fn latest_closed(versions: &[Version]) -> Option<String> {
    versions
        .iter()
        .filter(|v| v.state == VersionState::Closed)
        .filter_map(|v| version::parse(&v.title).map(|s| (s, v.title.clone())))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, title)| title)
}

/// Actions en attente : publication une fois les notes rédigées, réouverture par test en échec.
pub fn proposed_actions(detail: &AgentRunDetail) -> Vec<ProposedAction> {
    let Some(dp) = detail.deploy.as_ref() else { return vec![] };
    let mut out = Vec::new();
    if dp.step >= STEPS
        && !dp.published
        && let Some(notes) = dp.notes.clone()
    {
        out.push(ProposedAction {
            id: PUBLISH_ID.into(),
            preview: format!(
                "Publier la release {} ({} issue(s) dans les notes)",
                dp.tag,
                dp.tests.iter().filter(|t| in_notes(t)).count()
            ),
            action: AgentAction::PublishRelease { tag: dp.tag.clone(), notes },
        });
    }
    if dp.step >= 4
        && let Some(next) = &dp.next_version
    {
        for t in dp.tests.iter().filter(|t| t.status == DeployTestStatus::Failed && t.decision.is_none()) {
            out.push(ProposedAction {
                id: format!("reopen-{}", t.number),
                preview: format!("Rouvrir #{} sur {next} (test de vérification en échec)", t.number),
                action: AgentAction::ReopenIssue { number: t.number, milestone: next.clone() },
            });
        }
    }
    out
}

/// Des actions de l'agent déploiement attendent votre validation.
pub fn awaits_review(detail: &AgentRunDetail) -> bool {
    !proposed_actions(detail).is_empty()
}

/// Ignore l'échec des tests `numbers` : l'issue reste close et revient dans les notes.
pub fn ignore(dp: &mut DeployState, numbers: &[u32]) {
    for t in dp.tests.iter_mut().filter(|t| numbers.contains(&t.number)) {
        if t.status == DeployTestStatus::Failed && t.decision.is_none() {
            t.decision = Some(DeployDecision::Ignored);
        }
    }
    regenerate_notes(dp);
}

fn regenerate_notes(dp: &mut DeployState) {
    if dp.notes.is_some() && !dp.notes_edited && !dp.published {
        dp.notes = Some(release_notes(&dp.tag, dp.name.as_deref(), &dp.tests));
    }
}

/// Après une interruption : ce qui tournait est marqué, rien n'est perdu de ce qui est fait.
pub fn fail_running(dp: &mut DeployState, reason: &str, stopped: bool) {
    for (i, stage) in dp.stages.iter_mut().enumerate() {
        // L'étape 5 « en cours » attend seulement votre validation.
        if stage.state == DeployStageState::Running && i + 1 < usize::from(STEPS) {
            stage.state = DeployStageState::Failure;
            stage.detail = Some(reason.to_string());
        }
    }
    for t in dp.tests.iter_mut().filter(|t| t.status == DeployTestStatus::Running) {
        if stopped {
            t.status = DeployTestStatus::Queued;
        } else {
            t.status = DeployTestStatus::Unverifiable;
            t.detail = Some(reason.to_string());
        }
    }
}

fn pending_stages() -> Vec<DeployStage> {
    (0..STEPS).map(|_| DeployStage { state: DeployStageState::Pending, detail: None }).collect()
}

fn plural(n: usize) -> &'static str {
    if n > 1 { "s" } else { "" }
}

/* ---------- Lancement ---------- */

/// Lance l'agent déploiement sur une version close (par défaut la dernière). Une seule
/// exécution active à la fois.
pub async fn start(app: &AppHandle, state: &AppState, input: AgentInput) -> AppResult<String> {
    if let Some(h) = engine::latest(state, AgentKind::Deploy).await? {
        let d = h.detail.read().await;
        if matches!(d.state, RunState::Prepare | RunState::Running) {
            return Err(AppError::Validation("L'agent déploiement travaille déjà : attendez la fin ou arrêtez-le.".into()));
        }
    }
    let config = crate::commands::settings::load(state).await?.config;
    let agent = config.agents.deploy.clone();
    if agent.engine != AgentEngine::ClaudeCode {
        return Err(AppError::Validation("L'agent déploiement ne sait pas encore piloter Codex : choisissez Claude Code dans les réglages.".into()));
    }
    if auth::cli_status(state, CliTool::ClaudeCode, false).await.binary_path.is_none() {
        return Err(AppError::Validation("Claude Code est introuvable sur cette machine (Réglages › Machine locale).".into()));
    }
    let cache = state.repo_cache().await?;
    crate::sync::refresh(state, &cache, SyncResource::Milestones).await?;
    crate::sync::refresh(state, &cache, SyncResource::Issues).await?;
    let (versions, _) = repos::to_versions(&cache.milestones().await?);
    let target = match input.version.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => version::parse(v).map(|s| s.to_string()).ok_or_else(|| AppError::Validation(format!("Version invalide : {v}")))?,
        None => latest_closed(&versions).ok_or_else(|| AppError::Validation("Aucune version close à déployer.".into()))?,
    };
    let Some(milestone) = versions.iter().find(|v| v.title == target) else {
        return Err(AppError::Validation(format!("La version {target} n'existe pas sur ce dépôt.")));
    };
    if milestone.state != VersionState::Closed {
        return Err(AppError::Validation(format!("La version {target} n'est pas close : clôturez-la avant de la déployer.")));
    }

    let mut tests: Vec<DeployTest> = cache
        .issues()
        .await?
        .iter()
        .filter(|raw| raw.pull_request.is_none() && raw.state == "closed")
        .filter(|raw| raw.state_reason.as_deref() != Some("not_planned"))
        .filter(|raw| {
            raw.milestone.as_ref().and_then(|m| version::parse(&m.title)).map(|v| v.to_string()).as_deref() == Some(target.as_str())
        })
        .map(|raw| {
            let row = crate::api::github::issues::to_row(raw, None);
            DeployTest {
                number: raw.number,
                title: raw.title.clone(),
                issue_type: row.issue_type,
                criteria: acceptance_criteria(raw.body.as_deref().unwrap_or_default()),
                status: DeployTestStatus::Queued,
                detail: None,
                duration_ms: None,
                decision: None,
            }
        })
        .collect();
    if tests.is_empty() {
        return Err(AppError::Validation(format!("Aucune issue close sur {target} : rien à vérifier.")));
    }
    tests.sort_by_key(|t| std::cmp::Reverse(t.number));

    let now = version::now_iso();
    let detail = AgentRunDetail {
        run: format!("deploy-{}", now.replace([':', '-'], "").trim_end_matches('Z').replace('T', "-")),
        kind: AgentKind::Deploy,
        state: RunState::Prepare,
        autonomy: agent.autonomy,
        version: Some(target.clone()),
        model: agent.model.clone(),
        started_at: now,
        ended_at: None,
        items: vec![],
        tasks: vec![],
        log: vec![],
        error: None,
        paused: false,
        deploy: Some(DeployState {
            tag: format!("{}{target}", config.project.tag_prefix),
            name: milestone.name.clone(),
            next_version: next_version(&versions, &target),
            stages: pending_stages(),
            step: 0,
            tests,
            artifacts: vec![],
            notes: None,
            notes_edited: false,
            release_url: None,
            published: false,
            worktree: None,
            session_id: None,
        }),
        proposals: vec![],
    };
    let run = detail.run.clone();
    cache.save_run(&detail).await?;
    let handle = state.agents.insert(detail);
    engine::emit_state(app, &handle).await;

    let app = app.clone();
    let max = Duration::from_secs(u64::from(agent.max_session_minutes.max(1)) * 60);
    let root = worktree::root(&config.agents.worktree_root);
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(err) = run_pipeline(&app, &state, &handle, max, &root).await {
            let stopped = *handle.stop.borrow();
            let message = err.to_string();
            let mut d = handle.detail.write().await;
            if let Some(dp) = d.deploy.as_mut() {
                fail_running(dp, if stopped { "Arrêté à votre demande." } else { &message }, stopped);
            }
            if !stopped {
                d.error = Some(message.clone());
                d.state = RunState::Failed;
                d.ended_at = Some(version::now_iso());
            }
            drop(d);
            if !stopped {
                engine::log(&app, &handle, AgentEventKind::Error, format!("✕ {message}")).await;
            }
            engine::persist(&state, &handle).await;
            engine::emit_state(&app, &handle).await;
        }
    });
    Ok(run)
}

struct Context {
    tag: String,
    cli: PathBuf,
    max: Duration,
}

async fn wait_if_paused(handle: &Arc<RunHandle>) -> AppResult<()> {
    loop {
        if *handle.stop.borrow() {
            return Err(AppError::Validation("Arrêté à votre demande.".into()));
        }
        if !handle.detail.read().await.paused {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Modifie l'état du pipeline, puis enregistre et notifie.
async fn update(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, f: impl FnOnce(&mut DeployState)) {
    {
        let mut d = handle.detail.write().await;
        if let Some(dp) = d.deploy.as_mut() {
            f(dp);
        }
    }
    engine::persist(state, handle).await;
    engine::emit_state(app, handle).await;
}

/// Passe à l'étape `n` (après une éventuelle pause).
async fn begin(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, n: u8, detail: &str) -> AppResult<()> {
    wait_if_paused(handle).await?;
    let detail = detail.to_string();
    update(app, state, handle, |dp| {
        dp.step = n;
        dp.stages[usize::from(n - 1)] = DeployStage { state: DeployStageState::Running, detail: Some(detail) };
    })
    .await;
    Ok(())
}

async fn finish(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, n: u8, stage: DeployStageState, detail: String) {
    update(app, state, handle, |dp| dp.stages[usize::from(n - 1)] = DeployStage { state: stage, detail: Some(detail) }).await;
}

#[allow(clippy::too_many_arguments)]
async fn session(
    app: &AppHandle,
    handle: &Arc<RunHandle>,
    ctx: &Context,
    cwd: &Path,
    model: Option<&str>,
    system: Option<&str>,
    prompt: &str,
    resume: Option<&str>,
) -> AppResult<SessionOutcome> {
    engine::session(app, handle, &ctx.cli, cwd, model, system, prompt, resume, ctx.max, Tools::Write).await
}

async fn run_pipeline(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, max: Duration, root: &Path) -> AppResult<()> {
    let project = state.require_project().await?;
    let repo = project.repo.full_name.clone();
    let cli = auth::cli_status(state, CliTool::ClaudeCode, false)
        .await
        .binary_path
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Validation("Claude Code est introuvable.".into()))?;
    let (dp, model, autonomy) = {
        let d = handle.detail.read().await;
        (d.deploy.clone().ok_or_else(|| AppError::Internal("pipeline de déploiement absent".into()))?, d.model.clone(), d.autonomy)
    };
    let ctx = Context { tag: dp.tag.clone(), cli, max };
    engine::set_state(app, state, handle, RunState::Running).await;

    // 1 — Pré-vol : tag, brouillon de release, issues et critères.
    begin(app, state, handle, 1, "lecture du tag et de la release…").await?;
    let token = state.github_token()?;
    if !writes::tag_exists(&state.http, &token, &repo, &ctx.tag).await? {
        return Err(AppError::Validation(format!("Le tag {} n'existe pas sur GitHub : clôturez la version d'abord.", ctx.tag)));
    }
    let release = writes::find_release(&state.http, &token, &repo, &ctx.tag).await?;
    let with_criteria = dp.tests.iter().filter(|t| !t.criteria.is_empty()).count();
    let release_line = match &release {
        Some(r) if r.draft => "brouillon présent",
        Some(_) => "release déjà publiée",
        None => "aucun brouillon (créé à la publication)",
    };
    let n = dp.tests.len();
    let preflight = format!("{n} issue{} fermée{} · {with_criteria} avec critères · {release_line}", plural(n), plural(n));
    engine::log(app, handle, AgentEventKind::System, format!("› pré-vol {} : {preflight}", ctx.tag)).await;
    update(app, state, handle, |dp| dp.release_url = release.as_ref().map(|r| r.url.clone()).filter(|u| !u.is_empty())).await;
    finish(app, state, handle, 1, DeployStageState::Success, preflight).await;

    // 2 — Build, dans un worktree détaché sur le tag.
    begin(app, state, handle, 2, format!("worktree sur {}…", ctx.tag).as_str()).await?;
    engine::log(app, handle, AgentEventKind::System, format!("› copie de {repo} et tag {}…", ctx.tag)).await;
    let copy = checkout::ensure(&state.checkout_dir, &repo, &project.repo.default_branch, &token).await?;
    checkout::fetch_tag(&copy, &ctx.tag, &token).await?;
    drop(token);
    let wt = checkout::checkout_dir(root, &repo)?.join(format!("deploy-{}", vcs::slugify(&ctx.tag)));
    worktree::create_detached(&copy, &wt, &format!("refs/tags/{}", ctx.tag)).await?;
    engine::log(app, handle, AgentEventKind::System, format!("› worktree {}", wt.display())).await;
    let wt_path = wt.to_string_lossy().to_string();
    update(app, state, handle, |dp| dp.worktree = Some(wt_path)).await;

    let system = system_prompt(&ctx.tag);
    let outcome = session(app, handle, &ctx, &wt, model.as_deref(), Some(&system), &build_prompt(&repo, &ctx.tag), None).await?;
    let mut session_id = outcome.session_id.clone();
    let build = parse_build(&outcome.text);
    if !build.ok {
        let why = build.summary.unwrap_or_else(|| "l'agent n'a pas rendu de verdict".into());
        return Err(AppError::Validation(format!("Build en échec : {}", first_line(&why))));
    }
    let artifacts: Vec<DeployArtifact> = build
        .artifacts
        .iter()
        .map(|p| DeployArtifact {
            path: p.clone(),
            size_kb: std::fs::metadata(wt.join(p)).ok().map(|m| (m.len() / 1024).min(u64::from(u32::MAX)) as u32),
        })
        .collect();
    let build_line = build
        .summary
        .as_deref()
        .map(first_line)
        .unwrap_or_else(|| format!("{} artefact{}", artifacts.len(), plural(artifacts.len())));
    engine::log(app, handle, AgentEventKind::Success, format!("✓ build réussi ({} artefact{})", artifacts.len(), plural(artifacts.len()))).await;
    let sid = session_id.clone();
    update(app, state, handle, |dp| {
        dp.artifacts = artifacts;
        dp.session_id = sid;
    })
    .await;
    finish(app, state, handle, 2, DeployStageState::Success, build_line).await;

    // 3 — Tests de vérification : un par issue, dans la même session.
    begin(app, state, handle, 3, format!("0 / {n}").as_str()).await?;
    for index in 0..n {
        wait_if_paused(handle).await?;
        update(app, state, handle, |dp| dp.tests[index].status = DeployTestStatus::Running).await;
        let test = handle.detail.read().await.deploy.as_ref().map(|dp| dp.tests[index].clone()).ok_or_else(|| AppError::Internal("test absent".into()))?;
        engine::log(app, handle, AgentEventKind::System, format!("› test #{} {}", test.number, test.title)).await;
        let started = Instant::now();
        let (status, detail) = if test.criteria.is_empty() {
            (DeployTestStatus::Unverifiable, Some("Aucun critère d'acceptation (cases « - [ ] ») dans l'issue.".to_string()))
        } else {
            let prompt = verify_prompt(&test, &handle.take_notes());
            match session(app, handle, &ctx, &wt, model.as_deref(), None, &prompt, session_id.as_deref()).await {
                Ok(o) => {
                    if o.session_id.is_some() {
                        session_id = o.session_id.clone();
                    }
                    parse_verdict(&o.text)
                }
                Err(err) if *handle.stop.borrow() || err.to_string().contains("/login") => return Err(err),
                Err(err) => (DeployTestStatus::Unverifiable, Some(err.to_string())),
            }
        };
        let elapsed = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let (kind, mark) = match status {
            DeployTestStatus::Passed => (AgentEventKind::Success, "✓"),
            DeployTestStatus::Failed => (AgentEventKind::Error, "✕"),
            _ => (AgentEventKind::Question, "?"),
        };
        engine::log(app, handle, kind, format!("{mark} #{} {}", test.number, detail.as_deref().map(first_line).unwrap_or_default())).await;
        let sid = session_id.clone();
        update(app, state, handle, |dp| {
            let t = &mut dp.tests[index];
            t.status = status;
            t.detail = detail;
            t.duration_ms = Some(elapsed);
            dp.session_id = sid;
            let done = dp.tests.iter().filter(|t| !matches!(t.status, DeployTestStatus::Queued | DeployTestStatus::Running)).count();
            dp.stages[2].detail = Some(format!("{done} / {}", dp.tests.len()));
        })
        .await;
    }
    let (tests_state, tests_line) = {
        let d = handle.detail.read().await;
        tests_summary(&d.deploy.as_ref().map(|dp| dp.tests.clone()).unwrap_or_default())
    };
    finish(app, state, handle, 3, tests_state, tests_line).await;

    // 4 — Documentation : notes de version sans les issues en échec.
    begin(app, state, handle, 4, "rédaction des notes…").await?;
    let mut docs_line = String::new();
    update(app, state, handle, |dp| {
        dp.notes = Some(release_notes(&dp.tag, dp.name.as_deref(), &dp.tests));
        let kept = dp.tests.iter().filter(|t| in_notes(t)).count();
        let removed = dp.tests.len() - kept;
        docs_line = format!("Notes rédigées · {kept} issue{}", plural(kept));
        if removed > 0 {
            docs_line.push_str(&format!(" · {removed} retirée{}", plural(removed)));
        }
    })
    .await;
    finish(app, state, handle, 4, DeployStageState::Success, docs_line).await;

    // 5 — Tag et release : en attente de votre validation, jamais automatique.
    let waiting = if autonomy == Autonomy::Propose {
        format!("{} · propose seulement : rien n'est publié", ctx.tag)
    } else {
        format!("{} · en attente de votre validation", ctx.tag)
    };
    begin(app, state, handle, 5, &waiting).await?;
    {
        let mut d = handle.detail.write().await;
        if d.state == RunState::Running {
            d.state = RunState::Review;
        }
    }
    engine::log(app, handle, AgentEventKind::Question, format!("? relisez les notes et les tests, puis publiez {}", ctx.tag)).await;
    engine::persist(state, handle).await;
    engine::emit_state(app, handle).await;
    Ok(())
}

fn first_line(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    let line = line.trim_start_matches(['#', '-', '*', ' ']);
    if line.chars().count() > 90 {
        format!("{}…", line.chars().take(89).collect::<String>())
    } else {
        line.to_string()
    }
}

/* ---------- Décisions ---------- */

/// Applique `publish-release` ou `reopen-<n>` ; `None` si l'identifiant n'est pas à cet agent.
pub async fn apply_id(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, id: &str) -> Option<AppResult<()>> {
    handle.detail.read().await.deploy.as_ref()?;
    if id == PUBLISH_ID {
        return Some(publish(app, state, handle).await);
    }
    let number = id.strip_prefix("reopen-").and_then(|n| n.parse::<u32>().ok())?;
    Some(reopen(app, state, handle, number).await)
}

fn refuse_propose(autonomy: Autonomy) -> AppResult<()> {
    if autonomy == Autonomy::Propose {
        return Err(AppError::Validation("L'agent déploiement est réglé sur « propose seulement » : rien n'est écrit.".into()));
    }
    Ok(())
}

/// Publie la release : met à jour le brouillon avec les notes et le publie (ou la crée).
async fn publish(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>) -> AppResult<()> {
    let (dp, autonomy) = {
        let d = handle.detail.read().await;
        (d.deploy.clone().ok_or_else(|| AppError::Internal("pipeline absent".into()))?, d.autonomy)
    };
    refuse_propose(autonomy)?;
    if dp.published {
        return Err(AppError::Validation(format!("La release {} est déjà publiée.", dp.tag)));
    }
    let notes = dp
        .notes
        .clone()
        .filter(|n| !n.trim().is_empty() && dp.step >= STEPS)
        .ok_or_else(|| AppError::Validation("Les notes de version ne sont pas encore prêtes.".into()))?;
    let project = state.require_project().await?;
    let token = state.github_token()?;
    let repo = &project.repo.full_name;
    if !writes::tag_exists(&state.http, &token, repo, &dp.tag).await? {
        return Err(AppError::Validation(format!("Le tag {} a disparu de GitHub : rien n'est publié.", dp.tag)));
    }
    let url = writes::publish_release(&state.http, &token, repo, &dp.tag, &notes).await?;
    let tag = dp.tag.clone();
    update(app, state, handle, |dp| {
        dp.published = true;
        dp.release_url = Some(url).filter(|u| !u.is_empty()).or(dp.release_url.take());
        dp.stages[4] = DeployStage { state: DeployStageState::Success, detail: Some(format!("{tag} publiée")) };
    })
    .await;
    engine::log(app, handle, AgentEventKind::Write, format!("release {} publiée", dp.tag)).await;
    Ok(())
}

/// Rouvre l'issue d'un test en échec et la rattache au milestone de la version suivante.
async fn reopen(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, number: u32) -> AppResult<()> {
    let (dp, autonomy) = {
        let d = handle.detail.read().await;
        (d.deploy.clone().ok_or_else(|| AppError::Internal("pipeline absent".into()))?, d.autonomy)
    };
    refuse_propose(autonomy)?;
    let test = dp
        .tests
        .iter()
        .find(|t| t.number == number && t.status == DeployTestStatus::Failed && t.decision.is_none())
        .cloned()
        .ok_or_else(|| AppError::Validation(format!("#{number} n'a pas de test en échec à trancher.")))?;
    let next = dp
        .next_version
        .clone()
        .ok_or_else(|| AppError::Validation("Aucune version ouverte où rouvrir l'issue.".into()))?;
    let cache = state.repo_cache().await?;
    let (versions, _) = repos::to_versions(&cache.milestones().await?);
    let milestone = versions
        .iter()
        .find(|v| v.title == next && v.state == VersionState::Open)
        .map(|v| v.milestone_number)
        .ok_or_else(|| AppError::Validation(format!("Le milestone {next} n'est plus ouvert sur GitHub.")))?;
    let token = state.github_token()?;
    let updated =
        writes::update_issue(&state.http, &token, &cache.repo, number, &json!({ "state": "open", "milestone": milestone })).await?;
    cache.upsert_issues(&[updated]).await?;
    let comment = format!(
        "**Agent déploiement** · [Cadence](https://github.com/cadence-app/desktop) — test de vérification en échec sur {}.\n\n{}\n\n_Issue rouverte sur {next} et retirée des notes de version._",
        dp.tag,
        test.detail.clone().unwrap_or_else(|| "Un critère d'acceptation n'est pas rempli.".into())
    );
    writes::create_comment(&state.http, &token, &cache.repo, number, &comment).await?;
    update(app, state, handle, |dp| {
        if let Some(t) = dp.tests.iter_mut().find(|t| t.number == number) {
            t.decision = Some(DeployDecision::Reopened);
        }
        regenerate_notes(dp);
    })
    .await;
    engine::log(app, handle, AgentEventKind::Write, format!("write #{number} : rouverte sur {next}")).await;
    Ok(())
}

/// Remplace les notes de version (« Éditer ») : elles ne sont plus régénérées ensuite.
pub async fn edit_notes(app: &AppHandle, state: &AppState, run: &str, notes: String) -> AppResult<()> {
    let notes = notes.trim().to_string();
    if notes.is_empty() {
        return Err(AppError::Validation("Les notes de version ne peuvent pas être vides.".into()));
    }
    let handle = engine::handle(state, run).await?;
    {
        let mut d = handle.detail.write().await;
        let dp = d.deploy.as_mut().ok_or_else(|| AppError::Validation("Cette exécution n'a pas de notes de version.".into()))?;
        if dp.published {
            return Err(AppError::Validation("La release est déjà publiée : éditez-la sur GitHub.".into()));
        }
        if dp.notes.is_none() {
            return Err(AppError::Validation("Les notes ne sont pas encore rédigées.".into()));
        }
        dp.notes = Some(notes);
        dp.notes_edited = true;
    }
    engine::persist(state, &handle).await;
    engine::emit_state(app, &handle).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::version::VersionKind;

    fn test(number: u32, issue_type: Option<IssueType>, status: DeployTestStatus) -> DeployTest {
        DeployTest {
            number,
            title: format!("Issue {number}"),
            issue_type,
            criteria: vec!["ça marche".into()],
            status,
            detail: None,
            duration_ms: None,
            decision: None,
        }
    }

    fn state(tests: Vec<DeployTest>, step: u8) -> DeployState {
        DeployState {
            tag: "v0.1.1".into(),
            name: Some("Lecture du projet".into()),
            next_version: Some("0.1.2".into()),
            stages: pending_stages(),
            step,
            tests,
            artifacts: vec![],
            notes: Some("## v0.1.1".into()),
            notes_edited: false,
            release_url: None,
            published: false,
            worktree: None,
            session_id: None,
        }
    }

    fn detail(dp: DeployState) -> AgentRunDetail {
        AgentRunDetail {
            run: "deploy-1".into(),
            kind: AgentKind::Deploy,
            state: RunState::Review,
            autonomy: Autonomy::ApplyAfterReview,
            version: Some("0.1.1".into()),
            model: None,
            started_at: "2026-10-15T10:00:00Z".into(),
            ended_at: None,
            items: vec![],
            tasks: vec![],
            log: vec![],
            error: None,
            paused: false,
            deploy: Some(dp),
            proposals: vec![],
        }
    }

    fn version(title: &str, state: VersionState) -> Version {
        Version {
            title: title.into(),
            kind: VersionKind::Sprint,
            state,
            milestone_number: 1,
            name: None,
            start_on: None,
            due_on: None,
            open_issues: 0,
            closed_issues: 0,
            description: None,
        }
    }

    #[test]
    fn extracts_acceptance_criteria_from_their_section() {
        let body = "## Contexte\n- [ ] pas un critère\n\n## Critères d’acceptation\n- [ ] Le raccourci ouvre la palette\n* [x] Échap la ferme\n- [ ]   \n- texte libre\n\n## Notes\n- [ ] hors section";
        assert_eq!(acceptance_criteria(body), ["Le raccourci ouvre la palette", "Échap la ferme"]);
        let bold = "**Acceptance criteria**\n- [ ] works";
        assert_eq!(acceptance_criteria(bold), ["works"]);
    }

    #[test]
    fn falls_back_to_every_checkbox_without_a_section() {
        assert_eq!(acceptance_criteria("Faire :\n- [ ] a\n- [X] b"), ["a", "b"]);
        assert!(acceptance_criteria("## Critères d'acceptation\nÀ définir.").is_empty());
        assert!(acceptance_criteria("").is_empty());
    }

    #[test]
    fn reads_verdicts_in_french_or_english() {
        let ok = "Tout est bon.\n```json\n{\"status\": \"réussi\", \"detail\": \"La palette s'ouvre.\"}\n```";
        assert_eq!(parse_verdict(ok), (DeployTestStatus::Passed, Some("La palette s'ouvre.".into())));
        let ko = "```json\n{\"status\": \"Échec\", \"detail\": \"Pas après la veille.\"}\n```";
        assert_eq!(parse_verdict(ko).0, DeployTestStatus::Failed);
        let nv = "```json\n{\"status\": \"non vérifiable\"}\n```";
        assert_eq!(parse_verdict(nv).0, DeployTestStatus::Unverifiable);
        let garbled = parse_verdict("Je pense que ça marche.");
        assert_eq!(garbled, (DeployTestStatus::Unverifiable, Some("Je pense que ça marche.".into())));
        assert_eq!(parse_verdict("").1.as_deref(), Some("Verdict illisible."));
    }

    #[test]
    fn reads_the_build_report_and_keeps_artifacts_inside_the_worktree() {
        let text = "Build fait.\n```json\n{\"status\": \"réussi\", \"summary\": \"Windows x64\", \"artifacts\": [\"target\\\\release\\\\app.exe\", \"../../etc/passwd\", \"C:/x.exe\", \"/abs\"]}\n```";
        assert_eq!(
            parse_build(text),
            BuildReport { ok: true, summary: Some("Windows x64".into()), artifacts: vec!["target/release/app.exe".into()] }
        );
        assert!(!parse_build("```json\n{\"status\": \"échec\"}\n```").ok);
        assert!(!parse_build("rien").ok);
    }

    #[test]
    fn release_notes_group_by_type_and_drop_failures() {
        let mut ignored = test(128, Some(IssueType::Feat), DeployTestStatus::Failed);
        ignored.decision = Some(DeployDecision::Ignored);
        let tests = vec![
            test(127, Some(IssueType::Feat), DeployTestStatus::Passed),
            test(126, Some(IssueType::Feat), DeployTestStatus::Failed),
            test(120, Some(IssueType::Fix), DeployTestStatus::Unverifiable),
            test(110, None, DeployTestStatus::Passed),
        ];
        let notes = release_notes("v0.1.1", Some("Lecture du projet"), &tests);
        assert_eq!(
            notes,
            "## v0.1.1 — Lecture du projet\n\n### Nouveautés\n- Issue 127 (#127)\n\n### Corrections\n- Issue 120 (#120)\n\n### Maintenance\n- Issue 110 (#110)\n"
        );
        assert!(!notes.contains("#126"));
        assert!(release_notes("v0.1.1", None, &[ignored]).contains("- Issue 128 (#128)"));
        assert!(release_notes("v0.1.1", None, &[]).contains("Aucune issue livrée"));
    }

    #[test]
    fn summarises_the_verification_step() {
        let tests = vec![
            test(1, None, DeployTestStatus::Passed),
            test(2, None, DeployTestStatus::Failed),
            test(3, None, DeployTestStatus::Unverifiable),
        ];
        assert_eq!(tests_summary(&tests), (DeployStageState::Failure, "1 / 3 · 1 échec sur #2 · 1 non vérifiable".into()));
        assert_eq!(tests_summary(&tests[..1]), (DeployStageState::Success, "1 / 1".into()));
    }

    #[test]
    fn picks_the_next_open_version_and_the_latest_closed_one() {
        let versions = vec![
            version("0.1.0", VersionState::Closed),
            version("0.1.1", VersionState::Closed),
            version("0.2.0", VersionState::Open),
            version("0.1.2", VersionState::Open),
        ];
        assert_eq!(next_version(&versions, "0.1.1").as_deref(), Some("0.1.2"));
        assert_eq!(next_version(&versions, "0.2.0"), None);
        assert_eq!(latest_closed(&versions).as_deref(), Some("0.1.1"));
    }

    #[test]
    fn proposes_publication_and_one_reopen_per_undecided_failure() {
        let tests = vec![test(128, None, DeployTestStatus::Failed), test(127, None, DeployTestStatus::Passed)];
        let ids: Vec<String> = proposed_actions(&detail(state(tests.clone(), 5))).into_iter().map(|a| a.id).collect();
        assert_eq!(ids, ["publish-release", "reopen-128"]);
        // Avant la fin des tests : rien à valider.
        assert!(!awaits_review(&detail(state(tests.clone(), 3))));

        let mut dp = state(tests, 5);
        ignore(&mut dp, &[128, 127]);
        assert_eq!(dp.tests[0].decision, Some(DeployDecision::Ignored));
        assert_eq!(dp.tests[1].decision, None, "un test réussi n'a rien à trancher");
        assert!(dp.notes.as_deref().unwrap().contains("#128"), "les notes régénérées reprennent l'issue ignorée");
        dp.published = true;
        assert!(proposed_actions(&detail(dp)).is_empty());
    }

    #[test]
    fn edited_notes_are_not_regenerated() {
        let mut dp = state(vec![test(128, None, DeployTestStatus::Failed)], 5);
        dp.notes = Some("à la main".into());
        dp.notes_edited = true;
        ignore(&mut dp, &[128]);
        assert_eq!(dp.notes.as_deref(), Some("à la main"));
    }

    #[test]
    fn an_interruption_marks_what_was_running() {
        let mut dp = state(vec![test(1, None, DeployTestStatus::Running)], 3);
        dp.stages[2].state = DeployStageState::Running;
        dp.stages[4].state = DeployStageState::Running;
        fail_running(&mut dp, "Session interrompue.", false);
        assert_eq!(dp.stages[2].state, DeployStageState::Failure);
        assert_eq!(dp.stages[4].state, DeployStageState::Running, "l'étape 5 attend seulement votre validation");
        assert_eq!(dp.tests[0].status, DeployTestStatus::Unverifiable);

        let mut stopped = state(vec![test(1, None, DeployTestStatus::Running)], 3);
        fail_running(&mut stopped, "Arrêté.", true);
        assert_eq!(stopped.tests[0].status, DeployTestStatus::Queued);
    }

    #[test]
    fn prompts_carry_criteria_and_forbid_publishing() {
        assert!(system_prompt("v0.1.1").contains("Ne fais jamais de commit, de push, de tag ni de release"));
        let p = verify_prompt(&test(128, None, DeployTestStatus::Queued), &["Teste sous Windows".into()]);
        assert!(p.contains("#128") && p.contains("- ça marche") && p.contains("- Teste sous Windows"));
        assert!(build_prompt("me/app", "v0.1.1").contains("\"artifacts\""));
    }
}
