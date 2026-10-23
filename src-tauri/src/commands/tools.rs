//! Registre d'outils : détection, installation et connexion guidées, session de test des IA.
//! Rien ne s'installe sans un clic ; la commande exacte est affichée avant d'être lancée.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::{
    app_settings::AiId,
    domain::auth::CliTest,
    error::{AppError, AppResult},
    events,
    plugins::manifest::Manifest,
    state::AppState,
    tools::{RunSink, ToolId, ToolRunResult, ToolsReport},
};

/// Manifestes des plugins activés : leurs services et `tools` désignent les outils demandés.
fn enabled_manifests(state: &AppState) -> Vec<Manifest> {
    let installed = state.plugins.snapshot();
    state
        .plugins
        .ordered_ids()
        .into_iter()
        .filter(|id| installed.plugins.get(id).is_some_and(|e| e.enabled))
        .filter_map(|id| state.plugins.manifest(&id).ok())
        .collect()
}

async fn report(state: &AppState, refresh: bool) -> ToolsReport {
    state
        .tools
        .report(&state.settings.get(), &enabled_manifests(state), refresh)
        .await
}

fn sink(app: &AppHandle) -> RunSink {
    let app = app.clone();
    Arc::new(move |event| events::emit(&app, events::TOOL_RUN, event))
}

/// Détection au premier appel puis cache ; `refresh` relance tout (« Revérifier »).
#[tauri::command]
pub async fn tools_list(
    state: State<'_, AppState>,
    refresh: Option<bool>,
) -> AppResult<ToolsReport> {
    Ok(report(&state, refresh.unwrap_or(false)).await)
}

/// Lance une recette (`recipe` : `winget`, `npm`… ; absente = la première utilisable). Le
/// journal arrive par `tool://install` ; la réponse vient à la fin, outil redétecté.
#[tauri::command]
pub async fn tool_install(
    app: AppHandle,
    state: State<'_, AppState>,
    tool: ToolId,
    recipe: Option<String>,
) -> AppResult<ToolRunResult> {
    let settings = state.settings.get();
    let result = state
        .tools
        .install(&settings, tool, recipe.as_deref(), sink(&app))
        .await?;
    if tool.ai().is_some() {
        // Les CLI d'IA de Cadence 1 (`auth_status`) relisent la détection.
        *state.cli.write().await = Default::default();
    }
    Ok(result)
}

/// « Se connecter » : commande de connexion de l'outil (navigateur ou code d'appairage).
#[tauri::command]
pub async fn tool_login(
    app: AppHandle,
    state: State<'_, AppState>,
    tool: ToolId,
) -> AppResult<ToolRunResult> {
    let settings = state.settings.get();
    state.tools.login(&settings, tool, sink(&app)).await
}

/// Répond à une invite de la commande en cours (texte puis Entrée) ; `false` s'il n'y a pas de
/// commande ou pas de pseudo-terminal.
#[tauri::command]
pub fn tool_input(state: State<'_, AppState>, tool: ToolId, text: String) -> bool {
    state.tools.send_input(tool, &text)
}

/// Arrête l'installation ou la connexion en cours ; `false` s'il n'y en avait pas.
#[tauri::command]
pub fn tool_cancel(state: State<'_, AppState>, tool: ToolId) -> bool {
    state.tools.cancel(tool)
}

/// « Localiser le binaire… » : vérifie le fichier, l'enregistre ; `None` = détection automatique.
#[tauri::command]
pub async fn tool_set_path(
    state: State<'_, AppState>,
    tool: ToolId,
    path: Option<String>,
) -> AppResult<ToolsReport> {
    let path = path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
    if let Some(path) = &path {
        let candidate = std::path::PathBuf::from(path);
        if !candidate.is_file() {
            return Err(AppError::Validation(format!(
                "{path} n'est pas un fichier."
            )));
        }
        crate::tools::detect::version(&candidate)
            .await
            .map_err(AppError::Validation)?;
    }
    state.settings.update(|s| match tool.ai() {
        Some(ai) => {
            let slot = s.ai.get_mut(ai);
            slot.path = path.clone();
            if path.is_some() {
                slot.enabled = true;
            }
        }
        None => {
            match &path {
                Some(p) => s.tool_paths.insert(tool, p.clone()),
                None => s.tool_paths.remove(&tool),
            };
        }
    })?;
    *state.cli.write().await = Default::default();
    state.tools.refresh_one(&state.settings.get(), tool).await;
    Ok(report(&state, false).await)
}

/// Session de test de quelques secondes : l'IA répond-elle avec le compte connecté ?
#[tauri::command]
pub async fn ai_test(state: State<'_, AppState>, ai: AiId) -> AppResult<CliTest> {
    let settings = state.settings.get();
    state.tools.test(&settings, ToolId::from_ai(ai)).await
}
