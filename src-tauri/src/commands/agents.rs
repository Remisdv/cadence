use tauri::State;

use crate::{
    domain::{
        agent::{
            AgentActivity, AgentControl, AgentInput, AgentKind, AgentOverview, ApplyReport,
            ProposedAction,
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
pub async fn agents_overview() -> AppResult<Vec<AgentOverview>> {
    // TODO(0.1.3) : lire `agent_runs` et la file d'actions en attente.
    Ok([AgentKind::Cto, AgentKind::Ceo, AgentKind::Dev, AgentKind::Deploy]
        .into_iter()
        .map(|kind| AgentOverview {
            kind,
            autonomy: kind.default_autonomy(),
            activity: AgentActivity::Idle,
            pending_actions: 0,
            current_run: None,
            last_run_at: None,
        })
        .collect())
}

/// Lance une exécution ; le détail arrive par `agent://event` et `agent://state`.
#[tauri::command]
pub async fn agent_start(
    state: State<'_, AppState>,
    kind: AgentKind,
    input: AgentInput,
) -> AppResult<String> {
    let _ = (kind, input);
    state.require_project().await?;
    Err(AppError::NotImplemented("agent_start"))
}

#[tauri::command]
pub async fn agent_control(run: String, cmd: AgentControl) -> AppResult<()> {
    let _ = (run, cmd);
    Err(AppError::NotImplemented("agent_control"))
}

/// Actions en attente de validation.
#[tauri::command]
pub async fn agent_actions(run: String) -> AppResult<Vec<ProposedAction>> {
    let _ = run;
    Err(AppError::NotImplemented("agent_actions"))
}

/// Seul chemin d'écriture d'un agent vers GitHub.
#[tauri::command]
pub async fn agent_apply(run: String, ids: Vec<String>) -> AppResult<ApplyReport> {
    let _ = (run, ids);
    Err(AppError::NotImplemented("agent_apply"))
}
