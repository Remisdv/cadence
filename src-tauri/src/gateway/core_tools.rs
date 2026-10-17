//! Outils du cœur (préfixe réservé `cadence__`), servis par la passerelle elle-même (plan
//! § 3.4, révision 1.1 : plus d'espace, donc `cadence__plugins_info` remplace
//! `cadence__workspace_info`).
//!
//! - `cadence__plugins_info` : plugins activés et leurs vues (pour les liens `cadence:`) ;
//! - `cadence__open_view` : ouvre une vue de plugin dans l'app (événement `ui://open-view`) ;
//! - `cadence__run_report` : progression et résultat d'une exécution de fond (`run://report`) ;
//! - `cadence__repo_checkout` : clone d'un dépôt suivi et worktree d'une tâche (service Dépôts),
//!   ouvert à la session qui l'a demandé, et à elle seule.

use std::{collections::BTreeMap, sync::Arc};

use rmcp::model::{CallToolResult, ContentBlock, JsonObject, Tool, ToolAnnotations};
use serde::Serialize;
use serde_json::{Value, json};
use ts_rs::TS;

use super::{
    scope::{CORE, SessionScope, exposed},
    server::GatewayContext,
};

/// Demande d'ouverture d'une vue, poussée au front par `ui://open-view`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpenViewRequest {
    /// Session d'IA à l'origine de la demande.
    pub session: String,
    pub plugin: String,
    pub view: String,
    pub params: BTreeMap<String, String>,
}

/// Rapport d'une exécution de fond, poussé au front par `run://report`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunReport {
    pub run: String,
    pub session: String,
    pub text: String,
    /// Avancement en pourcents.
    pub progress: Option<u32>,
    /// Résultat structuré, libre.
    #[ts(type = "Record<string, unknown> | null")]
    pub data: Option<Value>,
    /// Dernier rapport : le résultat de l'exécution.
    #[serde(rename = "final")]
    #[ts(rename = "final")]
    pub is_final: bool,
}

/// Ce que les outils du cœur demandent à l'app.
#[derive(Debug, Clone, PartialEq)]
pub enum GatewayEvent {
    OpenView(OpenViewRequest),
    RunReport(RunReport),
}

fn schema(value: Value) -> Arc<JsonObject> {
    Arc::new(value.as_object().cloned().unwrap_or_default())
}

fn core_tool(name: &str, description: &str, input: Value, read_only: bool) -> Tool {
    let mut tool = Tool::new(exposed(CORE, name), description.to_string(), schema(input));
    tool.annotations = Some(ToolAnnotations::new().read_only(read_only).destructive(false));
    tool
}

/// Définitions des outils du cœur.
pub fn tools() -> Vec<Tool> {
    vec![
        core_tool(
            "plugins_info",
            "Liste les plugins activés dans Cadence et leurs vues. Utile pour proposer à \
             l'utilisateur un lien `cadence:<plugin>/<vue>?<paramètres>` qui ouvre une vue.",
            json!({ "type": "object", "properties": {} }),
            true,
        ),
        core_tool(
            "open_view",
            "Ouvre une vue d'un plugin dans Cadence (par exemple le détail d'une issue). \
             `params` : paramètres texte de la vue.",
            json!({
                "type": "object",
                "required": ["plugin", "view"],
                "properties": {
                    "plugin": { "type": "string", "description": "Identifiant du plugin." },
                    "view": { "type": "string", "description": "Identifiant de la vue." },
                    "params": { "type": "object", "additionalProperties": { "type": "string" } }
                }
            }),
            false,
        ),
        core_tool(
            REPO_CHECKOUT,
            "Donne accès au code d'un dépôt GitHub suivi dans Cadence : clone au besoin, puis              prépare un worktree pour la tâche (`task`, ex. `issue-42`) et renvoie son chemin              (`path`). Un appel répété pour la même tâche renvoie le même worktree, avec le              travail en cours. Le worktree est détaché : crée ta branche dedans. Tu tournes dans              un dossier neutre ; ce dossier est ouvert à ta session, et à elle seule.",
            json!({
                "type": "object",
                "required": ["repo"],
                "properties": {
                    "repo": { "type": "string", "description": "Dépôt suivi, `owner/name`." },
                    "task": { "type": "string", "description": "Tâche (un worktree par tâche) ; sans tâche, le clone lui-même." }
                }
            }),
            false,
        ),
        core_tool(
            "run_report",
            "Tâche de fond : signale la progression (`progress` en %, `text`) et, en dernier, le \
             résultat (`final: true`, `data` structuré facultatif). Personne ne peut répondre \
             pendant une tâche de fond : c'est le seul moyen de rendre compte.",
            json!({
                "type": "object",
                "required": ["text"],
                "properties": {
                    "text": { "type": "string", "description": "Où en est la tâche, ou son résultat." },
                    "progress": { "type": "integer", "minimum": 0, "maximum": 100 },
                    "final": { "type": "boolean", "description": "Dernier rapport : résultat de la tâche." },
                    "data": { "type": "object", "description": "Résultat structuré." },
                    "run": { "type": "string", "description": "Exécution visée (par défaut celle de la session)." }
                }
            }),
            false,
        ),
    ]
}

/// Nom (sans préfixe) de l'outil du service Dépôts, asynchrone (clone, fetch).
pub const REPO_CHECKOUT: &str = "repo_checkout";

/// `cadence__repo_checkout { repo, task? }` → `{ path }` : le dossier est ouvert à la session.
pub async fn repo_checkout(ctx: &GatewayContext, scope: &SessionScope, args: &JsonObject) -> CallToolResult {
    let text = |key: &str| args.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let Some(repos) = ctx.repos() else {
        return refused("Le service Dépôts de Cadence n'est pas disponible.");
    };
    let Some(repo) = text("repo") else { return refused("`repo` (owner/name) est obligatoire.") };
    if !repos.is_tracked(repo, None) {
        return refused(format!(
            "Le dépôt {repo} n'est suivi par aucun plugin : l'utilisateur peut l'ajouter dans Réglages › Dépôts GitHub."
        ));
    }
    let task = text("task");
    match repos.checkout(repo, task).await {
        Ok(path) => {
            ctx.grant(&scope.session, path.clone());
            ok(json!({
                "path": path.display().to_string(),
                "repo": repo,
                "task": task,
                "note": "Dossier ouvert à cette session. S'il n'est pas encore accessible pendant ce tour                          (refus ou demande d'approbation), il le sera dès le prochain message."
            }))
        }
        Err(err) => refused(err.to_string()),
    }
}

fn ok(value: Value) -> CallToolResult {
    let mut result = CallToolResult::success(vec![ContentBlock::text(value.to_string())]);
    result.structured_content = Some(value);
    result
}

fn refused(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

/// Exécute un outil du cœur (`name` sans le préfixe `cadence__`).
pub fn call(ctx: &GatewayContext, scope: &SessionScope, name: &str, args: &JsonObject) -> Option<CallToolResult> {
    let text = |key: &str| args.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    Some(match name {
        "plugins_info" => {
            let plugins: Vec<Value> = (ctx.plugins)()
                .into_iter()
                .map(|p| {
                    json!({
                        "id": p.id,
                        "name": p.name,
                        "description": p.description,
                        "views": p.views.iter().map(|v| json!({ "id": v.id, "title": v.title, "hidden": v.hidden })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            ok(json!({ "plugins": plugins }))
        }
        "open_view" => {
            let (Some(plugin), Some(view)) = (text("plugin"), text("view")) else {
                return Some(refused("`plugin` et `view` sont obligatoires."));
            };
            let plugins = (ctx.plugins)();
            let Some(found) = plugins.iter().find(|p| p.id == plugin) else {
                return Some(refused(format!("Le plugin « {plugin} » n'est pas activé.")));
            };
            if !found.views.iter().any(|v| v.id == view) {
                return Some(refused(format!("Le plugin « {plugin} » n'a pas de vue « {view} ».")));
            }
            let params = args
                .get("params")
                .and_then(Value::as_object)
                .map(|map| {
                    map.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            (ctx.events)(GatewayEvent::OpenView(OpenViewRequest {
                session: scope.session.clone(),
                plugin: plugin.to_string(),
                view: view.to_string(),
                params,
            }));
            ok(json!({ "opened": true }))
        }
        "run_report" => {
            let Some(run) = text("run").map(str::to_string).or_else(|| scope.run.clone()) else {
                return Some(refused("Aucune exécution de fond : `cadence__run_report` sert aux tâches de fond."));
            };
            let Some(report_text) = text("text") else { return Some(refused("`text` est obligatoire.")) };
            let progress = args.get("progress").and_then(Value::as_u64).map(|p| p.min(100) as u32);
            let report = RunReport {
                run,
                session: scope.session.clone(),
                text: report_text.to_string(),
                progress,
                data: args.get("data").filter(|d| !d.is_null()).cloned(),
                is_final: args.get("final").and_then(Value::as_bool).unwrap_or(false),
            };
            ctx.record_report(report.clone());
            (ctx.events)(GatewayEvent::RunReport(report));
            ok(json!({ "recorded": true }))
        }
        _ => return None,
    })
}
