//! Chat du cœur : conversations, envoi, interruption, approbations. Les réponses arrivent par
//! `chat://event` et `chat://state`.

use std::sync::Arc;

use tauri::State;

use crate::{
    ai::{
        claude_code::Permissions,
        events::{ApprovalDecision, QuestionAnswer},
    },
    chat::{
        ChatSendInput, ChatSendResult, ChatService,
        conversations::{ConversationDetail, ConversationInfo},
        store::{ChatSessionDetail, ChatSessionRow},
    },
    error::{AppError, AppResult},
    state::AppState,
};

fn chat(state: &AppState) -> AppResult<Arc<ChatService>> {
    state.chat.clone().ok_or_else(|| AppError::Internal("Le chat n'a pas pu démarrer (historique local illisible).".into()))
}

/// Conversations, la plus récemment active d'abord. Les conversations cachées (de groupe) n'y
/// sont qu'avec un filtre `group` explicite (id préfixé par le plugin).
#[tauri::command]
pub async fn chat_sessions(state: State<'_, AppState>, group: Option<String>) -> AppResult<Vec<ChatSessionRow>> {
    chat(&state)?.sessions(group.as_deref()).await
}

/// Pont de l'iframe (`conversations.list`) : conversations ouvertes par les actions du plugin,
/// celles d'un de ses groupes avec `group` (id vu par le plugin).
#[tauri::command]
pub async fn plugin_conversations(
    state: State<'_, AppState>,
    plugin: String,
    group: Option<String>,
) -> AppResult<Vec<ConversationInfo>> {
    chat(&state)?.conversations(&plugin, group.as_deref()).await
}

/// Pont de l'iframe (`conversations.get`, `conversations.open`) : une conversation du plugin,
/// messages complets, sans sorties d'outils. Celle d'un autre plugin est introuvable.
#[tauri::command]
pub async fn plugin_conversation_get(state: State<'_, AppState>, plugin: String, id: String) -> AppResult<ConversationDetail> {
    chat(&state)?.conversation(&plugin, &id).await
}

#[tauri::command]
pub async fn chat_session_get(state: State<'_, AppState>, id: String) -> AppResult<ChatSessionDetail> {
    chat(&state)?.detail(&id).await
}

/// Envoie un message ; sans `session`, ouvre une conversation avec l'IA demandée (ou celle
/// des réglages).
#[tauri::command]
pub async fn chat_send(state: State<'_, AppState>, mut input: ChatSendInput) -> AppResult<ChatSendResult> {
    if input.session.is_none() && input.ai.is_none() {
        input.ai = state.settings.get().default_ai;
    }
    chat(&state)?.send(input).await
}

/// Répond aux questions à choix de l'IA (Claude Code, Codex, OpenCode) : une entrée par
/// question posée, libellés choisis ou texte libre.
#[tauri::command]
pub async fn chat_answer(
    state: State<'_, AppState>,
    session: String,
    request: String,
    answers: Vec<QuestionAnswer>,
) -> AppResult<()> {
    chat(&state)?.answer(&session, &request, answers).await
}

#[tauri::command]
pub async fn chat_interrupt(state: State<'_, AppState>, session: String) -> AppResult<()> {
    chat(&state)?.interrupt(&session).await
}

#[tauri::command]
pub async fn chat_approve(
    state: State<'_, AppState>,
    session: String,
    request: String,
    decision: ApprovalDecision,
) -> AppResult<()> {
    chat(&state)?.approve(&session, &request, decision).await
}

#[tauri::command]
pub async fn chat_session_rename(state: State<'_, AppState>, id: String, title: String) -> AppResult<ChatSessionRow> {
    chat(&state)?.rename(&id, &title).await
}

/// Mode de la conversation (Demander, Édition auto, Auto) ; s'applique au tour suivant.
#[tauri::command]
pub async fn chat_set_permissions(state: State<'_, AppState>, session: String, permissions: Permissions) -> AppResult<ChatSessionRow> {
    chat(&state)?.set_permissions(&session, permissions).await
}

/// Supprime la conversation, son historique et son dossier de travail.
#[tauri::command]
pub async fn chat_session_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    chat(&state)?.delete(&id).await
}
