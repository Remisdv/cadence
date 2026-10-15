use tauri::{AppHandle, State};

use crate::{
    agents::engine,
    domain::{
        agent::{
            AgentActivity, AgentControl, AgentInput, AgentKind, AgentOverview, AgentRunDetail, ApplyReport,
            CtoItemState, ProposedAction, RunState, RunSummary,
        },
        settings::AgentEngines,
    },
    error::{AppError, AppResult},
    state::AppState,
};

/// Affectation des rôles (moteur et modèle par agent), préférence de la machine.
#[tauri::command]
pub async fn agent_engines_get(state: State<'_, AppState>) -> AppResult<AgentEngines> {
    Ok(state.settings.get().agent_engines)
}

#[tauri::command]
pub async fn agent_engines_set(
    state: State<'_, AppState>,
    engines: AgentEngines,
) -> AppResult<AgentEngines> {
    Ok(state.settings.update(|s| s.agent_engines = engines)?.agent_engines)
}

/// Statut des quatre agents : rail latéral et console des agents.
#[tauri::command]
pub async fn agents_overview(state: State<'_, AppState>) -> AppResult<Vec<AgentOverview>> {
    let config = match state.require_project().await {
        Ok(_) => crate::commands::settings::load(&state).await.ok().map(|c| c.config),
        Err(_) => None,
    };
    let mut out = Vec::new();
    for kind in [AgentKind::Cto, AgentKind::Ceo, AgentKind::Dev, AgentKind::Deploy] {
        let autonomy = config
            .as_ref()
            .map(|c| match kind {
                AgentKind::Cto => c.agents.cto.autonomy,
                AgentKind::Ceo => c.agents.ceo.autonomy,
                AgentKind::Dev => c.agents.dev.autonomy,
                AgentKind::Deploy => c.agents.deploy.autonomy,
            })
            .unwrap_or_else(|| kind.default_autonomy());
        let run = match state.require_project().await {
            Ok(_) => engine::latest(&state, kind).await.ok().flatten(),
            Err(_) => None,
        };
        let (activity, pending, current, last) = match run {
            Some(handle) => {
                let d = handle.detail.read().await;
                let pending = d.items.iter().filter(|i| i.state.awaits_review()).count() as u32;
                let running = matches!(d.state, RunState::Prepare | RunState::Running)
                    || d.items.iter().any(|i| i.state == CtoItemState::Running);
                let activity = if running {
                    AgentActivity::Running
                } else if pending > 0 {
                    AgentActivity::Waiting
                } else {
                    AgentActivity::Idle
                };
                let done = d.items.iter().filter(|i| !matches!(i.state, CtoItemState::Queued | CtoItemState::Running)).count();
                let current = (!d.state.is_terminal()).then(|| RunSummary {
                    run: d.run.clone(),
                    state: d.state,
                    issue: d.items.iter().find(|i| i.state == CtoItemState::Running).map(|i| i.number),
                    step: Some(done.min(255) as u8),
                    steps: Some(d.items.len().min(255) as u8),
                    started_at: d.started_at.clone(),
                });
                (activity, pending, current, Some(d.ended_at.clone().unwrap_or_else(|| d.started_at.clone())))
            }
            None => (AgentActivity::Idle, 0, None, None),
        };
        out.push(AgentOverview { kind, autonomy, activity, pending_actions: pending, current_run: current, last_run_at: last });
    }
    Ok(out)
}

/// Lance une exécution ; le détail arrive par `agent://event` et `agent://state`.
/// Seul l'agent CTO existe pour l'instant (0.1.3).
#[tauri::command]
pub async fn agent_start(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: AgentKind,
    input: AgentInput,
) -> AppResult<String> {
    match kind {
        AgentKind::Cto => match input.issues.clone().filter(|i| !i.is_empty()) {
            Some(numbers) => engine::send_issues(&app, &state, numbers).await,
            None => engine::start_cto(&app, &state, input).await,
        },
        AgentKind::Ceo | AgentKind::Dev => Err(AppError::NotImplemented("cet agent (version 0.2.0)")),
        AgentKind::Deploy => Err(AppError::NotImplemented("l'agent déploiement (version 0.2.1)")),
    }
}

/// Dernière exécution d'un agent sur le dépôt ouvert, avec sa file et son journal.
#[tauri::command]
pub async fn agent_latest_run(state: State<'_, AppState>, kind: AgentKind) -> AppResult<Option<AgentRunDetail>> {
    match engine::latest(&state, kind).await? {
        Some(handle) => Ok(Some(handle.detail.read().await.clone())),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn agent_run_get(state: State<'_, AppState>, run: String) -> AppResult<AgentRunDetail> {
    Ok(engine::handle(&state, &run).await?.detail.read().await.clone())
}

/// Arrêt d'une exécution. Pause, reprise et intervention arrivent avec l'agent développeur.
#[tauri::command]
pub async fn agent_control(
    app: AppHandle,
    state: State<'_, AppState>,
    run: String,
    cmd: AgentControl,
) -> AppResult<()> {
    match cmd {
        AgentControl::Stop => engine::stop(&app, &state, &run).await,
        _ => Err(AppError::NotImplemented("pause et intervention (version 0.2.0)")),
    }
}

/// Actions en attente de validation.
#[tauri::command]
pub async fn agent_actions(state: State<'_, AppState>, run: String) -> AppResult<Vec<ProposedAction>> {
    let handle = engine::handle(&state, &run).await?;
    let d = handle.detail.read().await;
    Ok(engine::proposed_actions(&d))
}

/// Seul chemin d'écriture d'un agent vers GitHub.
#[tauri::command]
pub async fn agent_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    run: String,
    ids: Vec<String>,
) -> AppResult<ApplyReport> {
    engine::apply(&app, &state, &run, ids).await
}

/// Passe des issues de la file sans rien écrire.
#[tauri::command]
pub async fn agent_reject(app: AppHandle, state: State<'_, AppState>, run: String, numbers: Vec<u32>) -> AppResult<()> {
    engine::reject(&app, &state, &run, numbers).await
}

/// Remplace la description proposée avant de l'appliquer.
#[tauri::command]
pub async fn agent_edit_body(
    app: AppHandle,
    state: State<'_, AppState>,
    run: String,
    number: u32,
    body: String,
) -> AppResult<()> {
    engine::edit(&app, &state, &run, number, body).await
}
