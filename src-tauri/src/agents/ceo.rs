//! Agent CEO : alimenter la roadmap.
//!
//! Une session Claude Code en lecture seule sur la copie du dépôt : il lit les thèmes du
//! palier visé (éléments de liste de la description du milestone), les autres versions
//! ouvertes, l'historique des versions livrées, les titres des issues existantes et le
//! README, puis dérive les issues qui manquent. Chaque proposition porte sa justification :
//! le thème dont elle découle, pourquoi rien ne la couvre déjà, le recouvrement éventuel.
//!
//! C'est le seul agent verrouillé en `propose` : rien n'est créé sans un clic. Créer une
//! proposition (`propose-<index>` dans `agent_apply`) ouvre l'issue dans le milestone visé,
//! avec le label `agent/ceo` et un pied signé.

use std::{path::PathBuf, sync::Arc, time::Duration};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::{
    claude::Tools,
    engine::{self, RunHandle},
};
use crate::{
    api::github::{issues::RawIssue, repos, writes},
    auth::{self, cli::CliTool},
    domain::{
        agent::{AgentInput, AgentKind, AgentRunDetail, Autonomy, CeoProposal, CeoProposalState, RunState},
        issue::IssueType,
        settings::AgentEngine,
        sync::SyncResource,
        version::{self, Version, VersionKind, VersionState},
    },
    error::{AppError, AppResult},
    events::AgentEventKind,
    state::AppState,
};

/// Propositions gardées au plus par exécution.
pub const MAX_PROPOSALS: usize = 8;
const MAX_TITLE: usize = 120;
const MAX_LABELS: usize = 3;
/// Part du README recopiée dans la requête (le reste se lit avec les outils).
const README_CHARS: usize = 8_000;
const OPEN_ISSUES_LIMIT: usize = 200;
const CLOSED_ISSUES_LIMIT: usize = 100;

/// Pied ajouté au corps de chaque issue créée (trace visible pour qui n'a pas Cadence).
const SIGNATURE: &str = "Proposée par l'agent CEO ([Cadence](https://github.com/cadence-app/desktop))";

/* ---------- Contexte ---------- */

/// Thèmes d'un palier : les éléments de liste de la description du milestone.
pub fn themes_of(description: &str) -> Vec<String> {
    description
        .lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let rest = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")).or_else(|| l.strip_prefix("+ "))?;
            let rest = rest.trim_start_matches("[ ] ").trim_start_matches("[x] ").trim();
            (!rest.is_empty()).then(|| rest.to_string())
        })
        .collect()
}

/// Palier visé par défaut : le premier palier ouvert après la version en cours ; à défaut,
/// la première version ouverte après elle. Les versions sont supposées triées.
pub fn next_target(versions: &[Version], current: Option<&str>) -> Option<String> {
    let after = |v: &&Version| {
        v.state == VersionState::Open
            && current.is_none_or(|c| version::compare(&v.title, c) == std::cmp::Ordering::Greater)
    };
    versions
        .iter()
        .filter(after)
        .find(|v| v.kind == VersionKind::Palier)
        .or_else(|| versions.iter().find(after))
        .map(|v| v.title.clone())
}

/// Tout ce que la requête de départ décrit à l'agent.
pub struct RoadmapContext {
    pub repo: String,
    pub target: Version,
    /// Versions triées (ouvertes et fermées).
    pub versions: Vec<Version>,
    /// Issues du dépôt (pull requests exclues).
    pub issues: Vec<IssueBrief>,
    pub readme: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IssueBrief {
    pub number: u32,
    pub title: String,
    pub open: bool,
    pub version: Option<String>,
}

impl IssueBrief {
    pub fn from_raw(raw: &RawIssue) -> Self {
        IssueBrief {
            number: raw.number,
            title: raw.title.clone(),
            open: raw.state == "open",
            version: raw.milestone.as_ref().and_then(|m| version::parse(&m.title)).map(|v| v.to_string()),
        }
    }
}

/// Consignes de l'agent, en tête de la requête.
pub fn system_prompt(has_code: bool) -> String {
    let code = if has_code {
        "Le dossier courant est une copie du dépôt : lis la documentation (README, docs/, CHANGELOG) \
         et le code (Read, Grep, Glob) pour savoir ce qui existe déjà. Tu ne peux rien modifier."
    } else {
        "Le code du dépôt n'est pas disponible : appuie-toi sur la roadmap et les issues existantes."
    };
    format!(
        "Tu es l'agent CEO de Cadence. Ton rôle : alimenter la roadmap d'un projet GitHub en \
         dérivant les issues qui manquent pour atteindre un palier, à partir de ses thèmes.\n\n\
         - {code}\n\
         - Pour chaque thème du palier visé, demande-toi ce qu'il faut construire et vérifie que \
         ni une issue existante (ouverte ou fermée) ni le code ne le couvre déjà.\n\
         - Ne propose que ce qui manque, au plus {MAX_PROPOSALS} issues, chacune assez petite pour \
         un sprint. Pas de doublon : signale tout recouvrement partiel avec une issue existante \
         (avec son numéro, ex. « Recouvre partiellement #159 »).\n\
         - Chaque proposition porte sa justification : le thème dont elle découle (son libellé \
         exact), et pourquoi rien ne la couvre déjà, en une ou deux phrases.\n\
         - Rédige en français. Corps en trois sections markdown : `## Contexte`, `## Attendu` \
         (liste), `## Critères d'acceptation` (cases `- [ ]`).\n\
         - Milestone : la version visée ; une autre version ouverte seulement si la roadmap le \
         justifie (thème marqué non prioritaire, prérequis d'une version plus proche).\n\
         - Type parmi feat, fix, chore, perf, docs ; au plus {MAX_LABELS} labels de domaine \
         (ex. `area/auth`, jamais `type/`, `agent/` ni `status/`).\n\
         - Tu ne crées rien : un humain relit chaque proposition avant création.\n\n\
         Termine ta réponse par un unique bloc ```json, sans rien après, de la forme :\n\
         {{\"proposals\": [{{\"title\": \"…\", \"body\": \"…markdown…\", \"milestone\": \"0.2.0\", \
         \"type\": \"feat\", \"labels\": [\"area/auth\"], \"theme\": \"Rôles et permissions\", \
         \"rationale\": \"…\", \"overlap\": null}}]}}\n\
         Si rien ne manque, rends {{\"proposals\": []}}."
    )
}

/// Requête de départ : la roadmap, l'historique et les issues existantes.
pub fn context_prompt(ctx: &RoadmapContext) -> String {
    let mut out = format!("Dépôt : {}\n\n", ctx.repo);
    let t = &ctx.target;
    out.push_str(&format!("## Version visée : {}{}\n", t.title, describe(t)));
    let themes = themes_of(t.description.as_deref().unwrap_or_default());
    if themes.is_empty() {
        out.push_str("Aucun thème listé : déduis-les de la description.\n");
    } else {
        out.push_str("Thèmes :\n");
        for theme in &themes {
            out.push_str(&format!("- {theme}\n"));
        }
    }
    if let Some(d) = t.description.as_deref().filter(|d| !d.trim().is_empty()) {
        out.push_str(&format!("\nDescription du milestone :\n{}\n", d.trim()));
    }

    let open: Vec<&Version> =
        ctx.versions.iter().filter(|v| v.state == VersionState::Open && v.title != t.title).collect();
    if !open.is_empty() {
        out.push_str("\n## Autres versions ouvertes (milestones possibles)\n");
        for v in open {
            let themes = themes_of(v.description.as_deref().unwrap_or_default());
            let themes = if themes.is_empty() { String::new() } else { format!(" — thèmes : {}", themes.join(", ")) };
            out.push_str(&format!("- {}{}{themes}\n", v.title, describe(v)));
        }
    }

    let shipped: Vec<&Version> = ctx.versions.iter().filter(|v| v.state == VersionState::Closed).collect();
    out.push_str("\n## Versions livrées\n");
    if shipped.is_empty() {
        out.push_str("Aucune pour l'instant.\n");
    }
    for v in shipped {
        out.push_str(&format!("- {}{} — {} issue(s) fermée(s)\n", v.title, describe(v), v.closed_issues));
    }

    out.push_str("\n## Issues existantes (ne propose rien qu'elles couvrent déjà)\n");
    let mut open: Vec<&IssueBrief> = ctx.issues.iter().filter(|i| i.open).collect();
    let mut closed: Vec<&IssueBrief> = ctx.issues.iter().filter(|i| !i.open).collect();
    open.sort_by_key(|i| std::cmp::Reverse(i.number));
    closed.sort_by_key(|i| std::cmp::Reverse(i.number));
    if open.is_empty() && closed.is_empty() {
        out.push_str("Aucune.\n");
    }
    for (list, limit, label) in [(open, OPEN_ISSUES_LIMIT, "ouverte"), (closed, CLOSED_ISSUES_LIMIT, "fermée")] {
        let hidden = list.len().saturating_sub(limit);
        for i in list.into_iter().take(limit) {
            let v = i.version.as_deref().unwrap_or("sans version");
            out.push_str(&format!("- #{} [{label} · {v}] {}\n", i.number, i.title));
        }
        if hidden > 0 {
            out.push_str(&format!("- … et {hidden} autre(s) {label}(s), plus ancienne(s)\n"));
        }
    }

    if let Some(readme) = ctx.readme.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
        let excerpt: String = readme.chars().take(README_CHARS).collect();
        let cut = if excerpt.len() < readme.len() { "\n[…tronqué : lis README.md pour la suite]" } else { "" };
        out.push_str(&format!("\n## README.md du dépôt\n{excerpt}{cut}\n"));
    }
    out
}

/// « « Travail à plusieurs » (palier, sans date) ».
fn describe(v: &Version) -> String {
    let name = v.name.as_deref().map(|n| format!(" « {n} »")).unwrap_or_default();
    let kind = match (v.kind, v.due_on.as_deref()) {
        (VersionKind::Palier, _) => "palier, sans date".to_string(),
        (VersionKind::Sprint, Some(due)) => format!("sprint, échéance {}", due.get(..10).unwrap_or(due)),
        (VersionKind::Sprint, None) => "sprint".to_string(),
    };
    format!("{name} ({kind})")
}

/* ---------- Lecture des propositions ---------- */

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawProposal {
    title: String,
    body: String,
    milestone: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    labels: Vec<String>,
    theme: Option<String>,
    rationale: String,
    overlap: Value,
}

/// Ce qui borne les propositions : versions ouvertes, issues existantes.
pub struct Bounds<'a> {
    pub target: &'a str,
    pub open_versions: &'a [String],
    pub issues: &'a [IssueBrief],
}

/// Dernier bloc ```json de la réponse (ou objet JSON nu), en `Value`.
fn final_json(text: &str) -> Option<Value> {
    if let Some((_, rest)) = text.rsplit_once("```json") {
        let block = rest.split("```").next().unwrap_or(rest);
        return serde_json::from_str(block.trim()).ok();
    }
    // Bloc absent : le premier objet ou tableau JSON lisible.
    text.char_indices().filter(|(_, c)| *c == '{' || *c == '[').find_map(|(i, _)| {
        let value: Value = serde_json::Deserializer::from_str(&text[i..]).into_iter::<Value>().next()?.ok()?;
        (value.get("proposals").is_some() || value.is_array()).then_some(value)
    })
}

/// Lit, valide et borne les propositions de l'agent.
pub fn parse_proposals(text: &str, bounds: &Bounds) -> AppResult<Vec<CeoProposal>> {
    let value = final_json(text).ok_or_else(|| AppError::Validation("L'agent n'a pas rendu de propositions lisibles.".into()))?;
    let list = match value {
        Value::Array(list) => list,
        Value::Object(mut map) => match map.remove("proposals") {
            Some(Value::Array(list)) => list,
            _ => return Err(AppError::Validation("Propositions de l'agent illisibles : liste `proposals` absente.".into())),
        },
        _ => return Err(AppError::Validation("Propositions de l'agent illisibles.".into())),
    };
    let norm = |s: &str| s.trim().to_lowercase();
    let mut out: Vec<CeoProposal> = Vec::new();
    for raw in list {
        let Ok(raw) = serde_json::from_value::<RawProposal>(raw) else { continue };
        let title = clip(raw.title.trim().trim_start_matches('#').trim(), MAX_TITLE);
        if title.is_empty() {
            continue;
        }
        // Doublon exact d'une issue existante ou d'une proposition précédente : écarté.
        if bounds.issues.iter().any(|i| norm(&i.title) == norm(&title)) || out.iter().any(|p| norm(&p.title) == norm(&title)) {
            continue;
        }
        let milestone = raw
            .milestone
            .as_deref()
            .and_then(version::parse)
            .map(|v| v.to_string())
            .filter(|v| bounds.open_versions.contains(v))
            .unwrap_or_else(|| bounds.target.to_string());
        let issue_type = raw
            .kind
            .as_deref()
            .and_then(|k| IssueType::ALL.into_iter().find(|t| t.slug() == k.trim().trim_start_matches("type/")))
            .or_else(|| IssueType::from_labels(raw.labels.iter().map(|l| l.trim())));
        let mut labels: Vec<String> = Vec::new();
        for label in raw.labels.iter().map(|l| l.trim()) {
            let conventional = ["type/", "agent/", "status/", "estimate/"].iter().any(|p| label.starts_with(p));
            if !label.is_empty() && !conventional && !labels.iter().any(|l| l == label) {
                labels.push(label.to_string());
            }
        }
        labels.truncate(MAX_LABELS);
        let rationale = raw.rationale.trim().to_string();
        let body = match raw.body.trim() {
            "" if rationale.is_empty() => continue,
            "" => format!("## Contexte\n\n{rationale}"),
            body => body.to_string(),
        };
        let (overlap, mut overlap_issues) = match &raw.overlap {
            Value::String(s) => (Some(s.trim().to_string()).filter(|s| !s.is_empty()), vec![]),
            Value::Object(o) => (
                o.get("note").and_then(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
                o.get("issues")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|n| n.as_u64()).filter_map(|n| u32::try_from(n).ok()).collect())
                    .unwrap_or_default(),
            ),
            _ => (None, vec![]),
        };
        overlap_issues.extend(overlap.as_deref().map(issue_refs).unwrap_or_default());
        overlap_issues.retain(|n| bounds.issues.iter().any(|i| i.number == *n));
        overlap_issues.sort_unstable();
        overlap_issues.dedup();
        out.push(CeoProposal {
            index: out.len() as u32 + 1,
            title,
            body,
            milestone,
            issue_type,
            labels,
            theme: raw.theme.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()),
            rationale,
            overlap,
            overlap_issues,
            state: CeoProposalState::Ready,
            issue: None,
            issue_url: None,
            error: None,
        });
        if out.len() == MAX_PROPOSALS {
            break;
        }
    }
    Ok(out)
}

/// Numéros d'issues cités (`#159`).
fn issue_refs(text: &str) -> Vec<u32> {
    text.split('#')
        .skip(1)
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect()
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() > max {
        format!("{}…", text.chars().take(max - 1).collect::<String>().trim_end())
    } else {
        text.to_string()
    }
}

/* ---------- Création ---------- */

/// Corps de l'issue créée : la proposition, puis le pied signé.
pub fn issue_body(p: &CeoProposal) -> String {
    let mut foot = SIGNATURE.to_string();
    if let Some(theme) = &p.theme {
        foot.push_str(&format!(" — thème : « {theme} »"));
    }
    foot.push('.');
    let mut lines = vec![p.body.trim().to_string(), "---".into(), format!("_{foot}_")];
    if !p.rationale.is_empty() {
        lines.push(format!("> {}", p.rationale.replace('\n', "\n> ")));
    }
    if let Some(overlap) = &p.overlap {
        lines.push(format!("Recouvrement signalé : {overlap}"));
    }
    lines.join("\n\n")
}

/// Labels posés à la création : la trace d'agent, le type, puis les labels proposés.
pub fn issue_labels(p: &CeoProposal) -> Vec<String> {
    let mut labels = vec![AgentKind::Ceo.label().to_string()];
    labels.extend(p.issue_type.map(|t| t.label()));
    for l in &p.labels {
        if !labels.contains(l) {
            labels.push(l.clone());
        }
    }
    labels
}

/* ---------- Lancement ---------- */

/// Lance l'agent CEO sur un palier (par défaut le prochain). Une seule exécution active à la
/// fois : la renvoie si elle existe.
pub async fn start(app: &AppHandle, state: &AppState, input: AgentInput) -> AppResult<String> {
    if let Some(h) = engine::latest(state, AgentKind::Ceo).await? {
        let d = h.detail.read().await;
        if matches!(d.state, RunState::Prepare | RunState::Running) {
            return Ok(d.run.clone());
        }
    }
    let project = state.require_project().await?;
    let config = crate::commands::settings::load(state).await?.config;
    let agent = config.agents.ceo.clone();
    if agent.engine != AgentEngine::ClaudeCode {
        return Err(AppError::Validation("L'agent CEO ne sait pas encore piloter Codex : choisissez Claude Code dans les réglages.".into()));
    }
    if auth::cli_status(state, CliTool::ClaudeCode, false).await.binary_path.is_none() {
        return Err(AppError::Validation("Claude Code est introuvable sur cette machine (Réglages › Machine locale).".into()));
    }
    let cache = state.repo_cache().await?;
    crate::sync::refresh(state, &cache, SyncResource::Milestones).await?;
    crate::sync::refresh(state, &cache, SyncResource::Issues).await?;
    let (versions, _) = repos::to_versions(&cache.milestones().await?);
    let target = match input.version.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        Some(wanted) => {
            let wanted = version::parse(wanted).map(|v| v.to_string()).unwrap_or_else(|| wanted.to_string());
            versions
                .iter()
                .find(|v| v.title == wanted && v.state == VersionState::Open)
                .map(|v| v.title.clone())
                .ok_or_else(|| AppError::Validation(format!("La version {wanted} n'est pas un milestone ouvert du dépôt.")))?
        }
        None => next_target(&versions, project.current_version.as_deref()).ok_or_else(|| {
            AppError::Validation("Aucun palier à venir : créez un milestone sans date dont la description liste les thèmes.".into())
        })?,
    };
    let described = versions.iter().find(|v| v.title == target).and_then(|v| v.description.as_deref()).is_some_and(|d| !d.trim().is_empty());
    if !described {
        return Err(AppError::Validation(format!(
            "Le milestone {target} n'a pas de description : listez-y ses thèmes (« - Rôles et permissions ») pour que l'agent en dérive des issues."
        )));
    }

    // Propositions encore à relire dans l'exécution précédente : remplacées par la nouvelle.
    if let Some(previous) = engine::latest(state, AgentKind::Ceo).await? {
        let changed = {
            let mut d = previous.detail.write().await;
            let mut changed = false;
            for p in d.proposals.iter_mut().filter(|p| matches!(p.state, CeoProposalState::Ready | CeoProposalState::Failed)) {
                p.state = CeoProposalState::Skipped;
                p.error = Some("Remplacée par l'exécution suivante.".into());
                changed = true;
            }
            if !d.state.is_terminal() {
                d.state = RunState::Done;
                d.ended_at.get_or_insert_with(version::now_iso);
                changed = true;
            }
            changed
        };
        if changed {
            engine::persist(state, &previous).await;
        }
    }

    let now = version::now_iso();
    let detail = AgentRunDetail {
        run: format!("ceo-{}", now.replace([':', '-'], "").trim_end_matches('Z').replace('T', "-")),
        kind: AgentKind::Ceo,
        state: RunState::Prepare,
        // Verrouillé : rien n'est créé sans un clic, quel que soit le réglage.
        autonomy: Autonomy::Propose,
        version: Some(target),
        model: agent.model.clone(),
        started_at: now,
        ended_at: None,
        items: vec![],
        tasks: vec![],
        log: vec![],
        error: None,
        paused: false,
        deploy: None,
        proposals: vec![],
    };
    let run = detail.run.clone();
    cache.save_run(&detail).await?;
    let handle = state.agents.insert(detail);
    engine::emit_state(app, &handle).await;

    let app = app.clone();
    let max = Duration::from_secs(u64::from(agent.max_session_minutes.max(1)) * 60);
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(err) = run_session(&app, &state, &handle, max).await {
            let stopped = *handle.stop.borrow();
            let mut d = handle.detail.write().await;
            if stopped {
                d.state = RunState::Discarded;
            } else {
                d.error = Some(err.to_string());
                d.state = RunState::Failed;
            }
            d.ended_at.get_or_insert_with(version::now_iso);
            drop(d);
            if !stopped {
                engine::log(&app, &handle, AgentEventKind::Error, format!("✕ {err}")).await;
            }
            engine::persist(&state, &handle).await;
            engine::emit_state(&app, &handle).await;
        }
    });
    Ok(run)
}

async fn run_session(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, max: Duration) -> AppResult<()> {
    let project = state.require_project().await?;
    let repo = project.repo.full_name.clone();
    let token = state.github_token()?;
    let cli = auth::cli_status(state, CliTool::ClaudeCode, false)
        .await
        .binary_path
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Validation("Claude Code est introuvable.".into()))?;

    engine::log(app, handle, AgentEventKind::System, format!("› copie de {repo} ({})…", project.repo.default_branch)).await;
    let (cwd, has_code) = match crate::vcs::checkout::ensure(&state.checkout_dir, &repo, &project.repo.default_branch, &token).await {
        Ok(dir) => (dir, true),
        Err(err) => {
            engine::log(app, handle, AgentEventKind::Error, format!("copie impossible, l'agent travaillera sans le code : {err}")).await;
            (std::env::temp_dir(), false)
        }
    };
    drop(token);

    let cache = state.repo_cache().await?;
    let (versions, _) = repos::to_versions(&cache.milestones().await?);
    let (target_title, model) = {
        let d = handle.detail.read().await;
        (d.version.clone().unwrap_or_default(), d.model.clone())
    };
    let target = versions
        .iter()
        .find(|v| v.title == target_title)
        .cloned()
        .ok_or_else(|| AppError::Validation(format!("La version {target_title} n'est plus dans le cache.")))?;
    let issues: Vec<IssueBrief> = cache.issues().await?.iter().map(IssueBrief::from_raw).collect();
    let readme = if has_code { tokio::fs::read_to_string(cwd.join("README.md")).await.ok() } else { None };
    let themes = themes_of(target.description.as_deref().unwrap_or_default());
    engine::log(
        app,
        handle,
        AgentEventKind::System,
        format!(
            "› {} : {} thème{}, {} issue{} lue{}{}",
            target.title,
            themes.len(),
            plural(themes.len()),
            issues.len(),
            plural(issues.len()),
            plural(issues.len()),
            if readme.is_some() { ", README.md" } else { "" }
        ),
    )
    .await;
    let open_versions: Vec<String> = versions.iter().filter(|v| v.state == VersionState::Open).map(|v| v.title.clone()).collect();
    let ctx = RoadmapContext { repo, target, versions, issues, readme };
    engine::set_state(app, state, handle, RunState::Running).await;

    let system = system_prompt(has_code);
    let prompt = context_prompt(&ctx);
    let outcome = engine::session(app, handle, &cli, &cwd, model.as_deref(), Some(&system), &prompt, None, max, Tools::ReadOnly).await?;
    let bounds = Bounds { target: &ctx.target.title, open_versions: &open_versions, issues: &ctx.issues };
    let proposals = parse_proposals(&outcome.text, &bounds)?;

    let count = proposals.len();
    {
        let mut d = handle.detail.write().await;
        d.proposals = proposals;
        if d.state == RunState::Running {
            d.state = if count > 0 { RunState::Review } else { RunState::Done };
            if count == 0 {
                d.ended_at = Some(version::now_iso());
            }
        }
    }
    let line = match count {
        0 => (AgentEventKind::Success, "✓ rien ne manque : chaque thème est déjà couvert".to_string()),
        n => (AgentEventKind::Success, format!("✓ {n} issue{} proposée{}, à relire", plural(n), plural(n))),
    };
    engine::log(app, handle, line.0, line.1).await;
    engine::persist(state, handle).await;
    engine::emit_state(app, handle).await;
    Ok(())
}

/* ---------- Décisions ---------- */

/// Index d'une action `propose-<index>`.
pub fn proposal_index(id: &str) -> Option<u32> {
    id.strip_prefix("propose-").and_then(|n| n.parse().ok())
}

/// Crée l'issue proposée sur GitHub (après votre clic : seul chemin d'écriture de l'agent).
pub async fn apply(app: &AppHandle, state: &AppState, handle: &Arc<RunHandle>, index: u32) -> AppResult<()> {
    let proposal = {
        let mut d = handle.detail.write().await;
        if d.kind != AgentKind::Ceo {
            return Err(AppError::Validation("Cette exécution n'est pas celle de l'agent CEO.".into()));
        }
        let p = d
            .proposals
            .iter_mut()
            .find(|p| p.index == index)
            .ok_or_else(|| AppError::Validation(format!("Proposition {index} introuvable.")))?;
        match p.state {
            CeoProposalState::Ready | CeoProposalState::Failed => {}
            CeoProposalState::Queued => return Err(AppError::Validation("Création déjà en cours.".into())),
            CeoProposalState::Applied => return Err(AppError::Validation(format!("Déjà créée (#{}).", p.issue.unwrap_or_default()))),
            CeoProposalState::Skipped => return Err(AppError::Validation("Proposition rejetée.".into())),
        }
        p.state = CeoProposalState::Queued;
        p.error = None;
        p.clone()
    };
    let result = create(state, &proposal).await;
    let line = {
        let mut d = handle.detail.write().await;
        let p = d.proposals.iter_mut().find(|p| p.index == index).expect("proposition présente");
        match &result {
            Ok(issue) => {
                p.state = CeoProposalState::Applied;
                p.issue = Some(issue.number);
                p.issue_url = issue.html_url.clone();
                (AgentEventKind::Write, format!("write #{} : issue créée dans {} (« {} »)", issue.number, p.milestone, p.title))
            }
            Err(err) => {
                p.state = CeoProposalState::Failed;
                p.error = Some(err.to_string());
                (AgentEventKind::Error, format!("✕ « {} » non créée : {err}", p.title))
            }
        }
    };
    engine::log(app, handle, line.0, line.1).await;
    engine::persist(state, handle).await;
    engine::emit_state(app, handle).await;
    result.map(|_| ())
}

async fn create(state: &AppState, p: &CeoProposal) -> AppResult<RawIssue> {
    let cache = state.repo_cache().await?;
    crate::sync::refresh(state, &cache, SyncResource::Milestones).await?;
    let (versions, _) = repos::to_versions(&cache.milestones().await?);
    let target = versions
        .iter()
        .find(|v| v.title == p.milestone && v.state == VersionState::Open)
        .ok_or_else(|| AppError::Validation(format!("La version {} n'est plus un milestone ouvert.", p.milestone)))?;
    let payload = json!({
        "title": p.title,
        "body": issue_body(p),
        "milestone": target.milestone_number,
        "labels": issue_labels(p),
    });
    let token = state.github_token()?;
    let (issue, raw) = writes::create_issue(&state.http, &token, &cache.repo, &payload).await?;
    cache.upsert_issues(&[(issue.clone(), raw)]).await?;
    // Compteurs du milestone changés : relus à la prochaine demande.
    cache.invalidate(SyncResource::Milestones).await?;
    Ok(issue)
}

/// Remplace le titre et le corps d'une proposition avant sa création.
pub async fn edit(app: &AppHandle, state: &AppState, run: &str, index: u32, title: String, body: String) -> AppResult<()> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::Validation("Le titre ne peut pas être vide.".into()));
    }
    if title.chars().count() > 256 {
        return Err(AppError::Validation("Titre trop long (256 caractères au plus).".into()));
    }
    if body.trim().is_empty() {
        return Err(AppError::Validation("La description ne peut pas être vide.".into()));
    }
    let handle = engine::handle(state, run).await?;
    {
        let mut d = handle.detail.write().await;
        let p = d
            .proposals
            .iter_mut()
            .find(|p| p.index == index && matches!(p.state, CeoProposalState::Ready | CeoProposalState::Failed))
            .ok_or_else(|| AppError::Validation(format!("Aucune proposition {index} à modifier.")))?;
        p.title = title;
        p.body = body.trim().to_string();
    }
    engine::persist(state, &handle).await;
    engine::emit_state(app, &handle).await;
    Ok(())
}

fn plural(n: usize) -> &'static str {
    if n > 1 { "s" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(title: &str, kind: VersionKind, state: VersionState, description: Option<&str>) -> Version {
        Version {
            title: title.into(),
            kind,
            state,
            milestone_number: 1,
            name: Some(format!("Nom {title}")),
            start_on: None,
            due_on: (kind == VersionKind::Sprint).then(|| "2026-10-07T07:00:00Z".into()),
            open_issues: 0,
            closed_issues: 4,
            description: description.map(str::to_string),
        }
    }

    fn versions() -> Vec<Version> {
        vec![
            version("0.1.1", VersionKind::Sprint, VersionState::Closed, None),
            version("0.1.2", VersionKind::Sprint, VersionState::Open, None),
            version("0.1.3", VersionKind::Sprint, VersionState::Open, None),
            version("0.2.0", VersionKind::Palier, VersionState::Open, Some("# Travail à plusieurs\n- Rôles et permissions\n- Revue assistée\n* Multi-dépôts")),
            version("0.2.1", VersionKind::Palier, VersionState::Open, Some("- Notifications")),
        ]
    }

    fn issues() -> Vec<IssueBrief> {
        vec![
            IssueBrief { number: 129, title: "Stockage du jeton".into(), open: true, version: Some("0.1.2".into()) },
            IssueBrief { number: 159, title: "Changelog".into(), open: true, version: None },
            IssueBrief { number: 12, title: "Ancienne".into(), open: false, version: Some("0.1.1".into()) },
        ]
    }

    #[test]
    fn reads_themes_and_picks_the_next_palier() {
        assert_eq!(themes_of("# Titre\n- Rôles\n  * Revue\n+ Multi\ntexte\n- [ ] Coché"), ["Rôles", "Revue", "Multi", "Coché"]);
        let v = versions();
        assert_eq!(next_target(&v, Some("0.1.2")).as_deref(), Some("0.2.0"));
        assert_eq!(next_target(&v, Some("0.2.0")).as_deref(), Some("0.2.1"));
        assert_eq!(next_target(&v, None).as_deref(), Some("0.2.0"));
        // Aucun palier après : la prochaine version ouverte.
        assert_eq!(next_target(&v[..3], Some("0.1.2")).as_deref(), Some("0.1.3"));
        assert_eq!(next_target(&v[..3], Some("0.1.3")), None);
    }

    #[test]
    fn builds_the_prompts_from_the_roadmap() {
        assert!(system_prompt(true).contains("Tu ne peux rien modifier"));
        assert!(system_prompt(false).contains("n'est pas disponible"));
        let v = versions();
        let ctx = RoadmapContext {
            repo: "me/app".into(),
            target: v[3].clone(),
            versions: v.clone(),
            issues: issues(),
            readme: Some("# App\nUn cockpit.".into()),
        };
        let prompt = context_prompt(&ctx);
        assert!(prompt.contains("## Version visée : 0.2.0 « Nom 0.2.0 » (palier, sans date)"));
        assert!(prompt.contains("- Rôles et permissions\n- Revue assistée\n- Multi-dépôts\n"));
        assert!(prompt.contains("- 0.2.1 « Nom 0.2.1 » (palier, sans date) — thèmes : Notifications"));
        assert!(prompt.contains("- 0.1.3 « Nom 0.1.3 » (sprint, échéance 2026-10-07)"));
        assert!(prompt.contains("- 0.1.1 « Nom 0.1.1 » (sprint, échéance 2026-10-07) — 4 issue(s) fermée(s)"));
        assert!(prompt.contains("- #159 [ouverte · sans version] Changelog"));
        assert!(prompt.contains("- #12 [fermée · 0.1.1] Ancienne"));
        assert!(prompt.contains("## README.md du dépôt\n# App\nUn cockpit."));
    }

    fn bounds<'a>(open: &'a [String], issues: &'a [IssueBrief]) -> Bounds<'a> {
        Bounds { target: "0.2.0", open_versions: open, issues }
    }

    #[test]
    fn reads_validates_and_bounds_proposals() {
        let open = vec!["0.1.2".to_string(), "0.2.0".to_string(), "0.2.1".to_string()];
        let issues = issues();
        let text = r###"J'ai lu le README.

```json
{"proposals": [
  {"title": "Inviter des collaborateurs avec un rôle", "body": "## Contexte\n…", "milestone": "0.2.0",
   "type": "feat", "labels": ["area/auth", "type/fix", "agent/ceo", "area/auth", " "], "theme": "Rôles et permissions",
   "rationale": "Aucune issue ne couvre l'invitation.", "overlap": "Le stockage du jeton (#129) suppose un utilisateur unique ; #999 inconnu."},
  {"title": "Notifications de fin de sprint", "body": "", "milestone": "v0.2.1", "labels": ["type/chore"],
   "theme": " ", "rationale": "Thème non prioritaire.", "overlap": {"note": "Proche de #159", "issues": [159, 4000000000]}},
  {"title": "Pré-revue", "body": "x", "milestone": "9.9.9", "rationale": "r", "overlap": null},
  {"title": "changelog ", "body": "doublon d'une issue existante", "rationale": "r"},
  {"title": "Pré-revue", "body": "doublon interne", "rationale": "r"},
  {"title": "  ", "body": "sans titre"},
  {"title": "Sans corps ni justification"},
  "pas un objet"
]}
```"###;
        let proposals = parse_proposals(text, &bounds(&open, &issues)).unwrap();
        let titles: Vec<_> = proposals.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(titles, ["Inviter des collaborateurs avec un rôle", "Notifications de fin de sprint", "Pré-revue"]);
        let [a, b, c] = &proposals[..] else { unreachable!() };
        assert_eq!((a.index, a.issue_type, a.labels.as_slice()), (1, Some(IssueType::Feat), ["area/auth".to_string()].as_slice()));
        assert_eq!(a.overlap_issues, [129], "seules les issues connues sont retenues");
        assert_eq!(a.state, CeoProposalState::Ready);
        assert_eq!((b.milestone.as_str(), b.issue_type, b.theme.as_deref()), ("0.2.1", Some(IssueType::Chore), None));
        assert_eq!(b.body, "## Contexte\n\nThème non prioritaire.");
        assert_eq!((b.overlap.as_deref(), b.overlap_issues.as_slice()), (Some("Proche de #159"), [159].as_slice()));
        assert_eq!(c.milestone, "0.2.0", "une version inconnue retombe sur le palier visé");
        assert_eq!(c.index, 3);
    }

    #[test]
    fn accepts_bare_json_and_empty_lists_but_rejects_garbage() {
        let open = vec!["0.2.0".to_string()];
        let none: Vec<IssueBrief> = vec![];
        let b = bounds(&open, &none);
        assert!(parse_proposals("```json\n{\"proposals\": []}\n```", &b).unwrap().is_empty());
        let bare = parse_proposals("Voilà : [{\"title\": \"A\", \"body\": \"b\", \"rationale\": \"r\"}] fin", &b).unwrap();
        assert_eq!(bare[0].title, "A");
        assert!(parse_proposals("rien de lisible", &b).is_err());
        assert!(parse_proposals("```json\n{\"autre\": 1}\n```", &b).is_err());
        // Au plus MAX_PROPOSALS, titres bornés.
        let many: Vec<String> =
            (0..12).map(|i| format!("{{\"title\": \"T{i} {}\", \"body\": \"b\", \"rationale\": \"r\"}}", "x".repeat(200))).collect();
        let parsed = parse_proposals(&format!("```json\n{{\"proposals\": [{}]}}\n```", many.join(",")), &b).unwrap();
        assert_eq!(parsed.len(), MAX_PROPOSALS);
        assert_eq!(parsed[0].title.chars().count(), MAX_TITLE);
    }

    #[test]
    fn signs_the_created_issue() {
        let open = vec!["0.2.0".to_string()];
        let none: Vec<IssueBrief> = vec![];
        let text = "```json\n[{\"title\": \"Inviter\", \"body\": \"## Contexte\\nx\", \"type\": \"feat\", \"labels\": [\"area/auth\"], \
                    \"theme\": \"Rôles et permissions\", \"rationale\": \"Rien ne couvre l'invitation.\", \"overlap\": \"#129\"}]\n```";
        let p = &parse_proposals(text, &bounds(&open, &none)).unwrap()[0];
        let body = issue_body(p);
        assert!(body.starts_with("## Contexte\nx\n\n---\n\n_Proposée par l'agent CEO"));
        assert!(body.contains("— thème : « Rôles et permissions »._"));
        assert!(body.contains("> Rien ne couvre l'invitation."));
        assert!(body.contains("Recouvrement signalé : #129"));
        assert_eq!(issue_labels(p), ["agent/ceo", "type/feat", "area/auth"]);
        assert_eq!(proposal_index("propose-3"), Some(3));
        assert_eq!(proposal_index("update-3"), None);
    }
}
