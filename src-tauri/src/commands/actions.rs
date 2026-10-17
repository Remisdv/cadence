//! Actions des plugins (plan § 3.5) : un bouton = un skill + un contexte JSON + un mode + une
//! IA. Mode `chat` : nouvelle conversation ouverte dans le shell ; mode `background` :
//! exécution de fond, dans la file (2 à la fois), visible dans la colonne des conversations.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;
use ts_rs::TS;

use crate::{
    ai::prompt::{action_prompt, action_title},
    app_settings::{AiId, AppSettings},
    chat::{ActionStart, store::ChatMode},
    error::{AppError, AppResult},
    plugins::manifest::{ActionMode, PluginActionInfo},
    state::AppState,
};

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct ActionRunInput {
    pub plugin: String,
    pub action: String,
    /// Objet libre `{ type, … }` ; `type` doit correspondre au contexte déclaré par l'action.
    #[ts(type = "Record<string, unknown> | null")]
    pub context: Option<Value>,
    /// IA imposée par le clic (« Lancer avec… »).
    pub ai: Option<AiId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActionRunResult {
    /// Conversation de l'action (exécution de fond comprise).
    pub session: String,
    pub mode: ActionMode,
    pub ai: AiId,
}

/// IA d'une action : choix du clic > IA réglée pour l'action > IA par défaut.
pub fn choose_ai(settings: &AppSettings, plugin: &str, action: &str, explicit: Option<AiId>) -> AppResult<AiId> {
    explicit
        .or_else(|| settings.action_ai.get(&format!("{plugin}/{action}")).copied())
        .or(settings.default_ai)
        .ok_or_else(|| AppError::Validation("Aucune IA par défaut : choisissez-en une dans Réglages › IA de code.".into()))
}

/// Le contexte fourni correspond-il à celui que l'action attend ?
pub fn check_context(action: &PluginActionInfo, context: Option<&Value>) -> AppResult<()> {
    let Some(expected) = action.context.as_deref() else { return Ok(()) };
    let given = context.and_then(|c| c.get("type")).and_then(Value::as_str);
    if given == Some(expected) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "L'action « {} » attend un contexte de type « {expected} ».",
            action.label
        )))
    }
}

#[tauri::command]
pub async fn action_run(state: State<'_, AppState>, input: ActionRunInput) -> AppResult<ActionRunResult> {
    let entry = state.plugins.entry(&input.plugin)?;
    if !entry.enabled {
        return Err(AppError::Validation(format!("Le plugin « {} » est désactivé.", input.plugin)));
    }
    let manifest = state.plugins.manifest(&input.plugin)?;
    let action = manifest
        .actions
        .iter()
        .find(|a| a.id == input.action)
        .ok_or_else(|| AppError::Validation(format!("« {} » ne déclare pas l'action « {} ».", manifest.name, input.action)))?;
    check_context(action, input.context.as_ref())?;
    let settings = state.settings.get();
    let ai = choose_ai(&settings, &input.plugin, &input.action, input.ai)?;
    if !settings.ai.get(ai).enabled {
        return Err(AppError::Validation("Cette IA est désactivée dans les réglages.".into()));
    }
    let background = action.mode == ActionMode::Background;
    let prompt = action_prompt(ai, &input.plugin, &action.skill, &action.label, input.context.as_ref(), background);
    let chat = state.chat.clone().ok_or_else(|| AppError::Internal("Le chat n'a pas pu démarrer.".into()))?;
    let session = chat
        .start_action(ActionStart {
            ai,
            mode: if background { ChatMode::Background } else { ChatMode::Chat },
            title: action_title(&action.label, input.context.as_ref()),
            origin: format!("action:{}/{}", input.plugin, input.action),
            prompt,
        })
        .await?;
    Ok(ActionRunResult { session, mode: action.mode, ai })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    fn action(context: Option<&str>) -> PluginActionInfo {
        PluginActionInfo {
            id: "preparer".into(),
            label: "Préparer l'issue".into(),
            skill: "preparer-issue".into(),
            mode: ActionMode::Chat,
            context: context.map(str::to_string),
            description: None,
            icon: None,
            confirm: None,
        }
    }

    #[test]
    fn chooses_the_ai_by_priority() {
        let mut settings = AppSettings { default_ai: Some(AiId::ClaudeCode), ..AppSettings::default() };
        assert_eq!(choose_ai(&settings, "gp", "preparer", None).unwrap(), AiId::ClaudeCode);
        settings.action_ai = BTreeMap::from([("gp/preparer".to_string(), AiId::Codex)]);
        assert_eq!(choose_ai(&settings, "gp", "preparer", None).unwrap(), AiId::Codex);
        assert_eq!(choose_ai(&settings, "gp", "preparer", Some(AiId::Opencode)).unwrap(), AiId::Opencode);
        assert!(choose_ai(&AppSettings::default(), "gp", "preparer", None).is_err());
    }

    #[test]
    fn checks_the_declared_context() {
        assert!(check_context(&action(None), None).is_ok());
        assert!(check_context(&action(Some("issue")), Some(&json!({ "type": "issue", "number": 1 }))).is_ok());
        assert!(check_context(&action(Some("issue")), Some(&json!({ "type": "version" }))).is_err());
        assert!(check_context(&action(Some("issue")), None).is_err());
    }
}
