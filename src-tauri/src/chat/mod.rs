//! Chat du cœur (plan § 5, révision 1.1 : un chat global, plus de sessions par espace).
//!
//! Une conversation = une session d'IA (Claude Code pour l'instant), fixée à la création. Un
//! tour : `send` enregistre le message, démarre au besoin le processus de la CLI (préparation,
//! jeton de passerelle, reprise par `native_id`), envoie le texte ; les événements normalisés
//! sont enregistrés (deltas exclus) et poussés au front (`chat://event`, `chat://state`).
//!
//! Un processus inactif depuis 15 min est fermé (la conversation reste reprenable) ; 4
//! processus vivants au plus (le plus ancien inactif est fermé). Une interruption qui n'a pas
//! abouti en 5 s tue le processus.

pub mod store;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Weak,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, mpsc};
use ts_rs::TS;

use self::store::{ChatEventRow, ChatMode, ChatSessionDetail, ChatSessionRow, ChatState, ChatStore};
use crate::{
    ai::{
        claude_code::{ClaudeLaunch, ClaudeSession, Permissions, SessionMode},
        events::{AiErrorKind, AiEvent, ApprovalDecision},
        staging::{self, StagedSkill},
    },
    app_settings::AiId,
    domain::version::now_iso,
    error::{AppError, AppResult},
    gateway::{scope::SessionScope, server::GatewayContext},
};

/// Charge de `chat://event`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatEventPayload {
    pub session: String,
    /// Numéro d'ordre dans l'historique ; `None` pour un delta (jamais enregistré).
    pub seq: Option<u32>,
    pub turn: u32,
    pub event: AiEvent,
}

/// Charge de `chat://state`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatStatePayload {
    pub session: String,
    pub state: ChatState,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatSendInput {
    /// Conversation existante ; absente = nouvelle conversation.
    pub session: Option<String>,
    pub text: String,
    /// IA d'une nouvelle conversation (défaut : IA par défaut des réglages).
    pub ai: Option<AiId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatSendResult {
    pub session: String,
}

/// Ce que le chat pousse au front.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatNotice {
    Event(ChatEventPayload),
    State(ChatStatePayload),
}

pub type ChatSink = Arc<dyn Fn(ChatNotice) + Send + Sync>;
/// Chemin de `claude`, lu à chaque démarrage de processus (réglages à jour).
pub type ClaudeLocator = Arc<dyn Fn() -> AppResult<PathBuf> + Send + Sync>;
/// Skills des plugins activés, copiés dans chaque session.
pub type SkillSource = Arc<dyn Fn() -> Vec<StagedSkill> + Send + Sync>;

/// Accès à la passerelle MCP.
#[derive(Clone)]
pub struct GatewayAccess {
    pub url: String,
    pub context: Arc<GatewayContext>,
}

#[derive(Debug, Clone)]
pub struct ChatConfig {
    /// `<données>/sessions` : dossier de travail neutre de chaque conversation.
    pub sessions_root: PathBuf,
    /// Dossiers de préparation éphémères.
    pub staging_root: PathBuf,
    pub idle_close: Duration,
    pub max_live: usize,
    pub interrupt_grace: Duration,
    pub close_grace: Duration,
    pub model: Option<String>,
}

impl ChatConfig {
    pub fn new(sessions_root: PathBuf, staging_root: PathBuf) -> Self {
        Self {
            sessions_root,
            staging_root,
            idle_close: Duration::from_secs(15 * 60),
            max_live: 4,
            interrupt_grace: Duration::from_secs(5),
            close_grace: Duration::from_secs(3),
            model: None,
        }
    }
}

struct Live {
    session: Arc<ClaudeSession>,
    turn: Arc<AtomicU32>,
    last_used: Instant,
}

pub struct ChatService {
    store: Arc<ChatStore>,
    config: ChatConfig,
    claude: ClaudeLocator,
    skills: SkillSource,
    gateway: Option<GatewayAccess>,
    sink: ChatSink,
    live: Mutex<HashMap<String, Live>>,
}

/// Titre d'une conversation : la première ligne du premier message, 60 caractères au plus.
pub fn title_of(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("Conversation");
    if line.chars().count() <= 60 { line.to_string() } else { format!("{}…", line.chars().take(59).collect::<String>().trim_end()) }
}

fn new_id() -> String {
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or_default();
    let mut bytes = [0u8; 4];
    let _ = getrandom::fill(&mut bytes);
    format!("c{millis:x}{:08x}", u32::from_le_bytes(bytes))
}

impl ChatService {
    pub fn new(
        store: ChatStore,
        config: ChatConfig,
        claude: ClaudeLocator,
        skills: SkillSource,
        gateway: Option<GatewayAccess>,
        sink: ChatSink,
    ) -> Arc<Self> {
        Arc::new(Self { store: Arc::new(store), config, claude, skills, gateway, sink, live: Mutex::default() })
    }

    pub async fn sessions(&self) -> AppResult<Vec<ChatSessionRow>> {
        self.store.list().await
    }

    pub async fn detail(&self, id: &str) -> AppResult<ChatSessionDetail> {
        Ok(ChatSessionDetail { session: self.store.get(id).await?, events: self.store.events(id).await? })
    }

    pub async fn rename(&self, id: &str, title: &str) -> AppResult<ChatSessionRow> {
        self.store.rename(id, title).await
    }

    fn notify_state(&self, session: &str, state: ChatState) {
        (self.sink)(ChatNotice::State(ChatStatePayload { session: session.into(), state }));
    }

    async fn record(&self, session: &str, turn: u32, event: AiEvent) -> AppResult<ChatEventRow> {
        let row = self.store.append(session, turn, &event).await?;
        (self.sink)(ChatNotice::Event(ChatEventPayload { session: session.into(), seq: Some(row.seq), turn, event }));
        Ok(row)
    }

    async fn set_state(&self, session: &str, state: ChatState) -> AppResult<()> {
        self.store.set_state(session, state).await?;
        self.notify_state(session, state);
        Ok(())
    }

    /// Envoie un message (crée la conversation au besoin) ; la réponse arrive par événements.
    pub async fn send(self: &Arc<Self>, input: ChatSendInput) -> AppResult<ChatSendResult> {
        let text = input.text.trim().to_string();
        if text.is_empty() {
            return Err(AppError::Validation("Le message est vide.".into()));
        }
        let id = match input.session {
            Some(id) => self.store.get(&id).await?.id,
            None => {
                let ai = input.ai.unwrap_or(AiId::ClaudeCode);
                let now = now_iso();
                let row = ChatSessionRow {
                    id: new_id(),
                    title: title_of(&text),
                    ai,
                    model: self.config.model.clone(),
                    origin: "user".into(),
                    mode: ChatMode::Chat,
                    state: ChatState::Idle,
                    created_at: now.clone(),
                    updated_at: now,
                };
                self.store.create(&row).await?;
                row.id
            }
        };
        let row = self.store.get(&id).await?;
        if row.ai != AiId::ClaudeCode {
            return Err(AppError::Validation("Seul Claude Code est pris en charge pour l'instant dans le chat.".into()));
        }
        if matches!(row.state, ChatState::Running | ChatState::WaitingApproval) {
            return Err(AppError::Validation("Un tour est déjà en cours dans cette conversation.".into()));
        }

        let turn = self.store.next_turn(&id).await?;
        self.record(&id, turn, AiEvent::UserMessage { text: text.clone() }).await?;
        let (session, turn_slot) = match self.ensure_live(&id).await {
            Ok(live) => live,
            Err(err) => {
                self.record(&id, turn, AiEvent::Error { kind: AiErrorKind::Other, message: err.to_string() }).await?;
                self.set_state(&id, ChatState::Failed).await?;
                return Err(err);
            }
        };
        turn_slot.store(turn, Ordering::SeqCst);
        self.set_state(&id, ChatState::Running).await?;
        if let Err(err) = session.send(&text).await {
            self.record(&id, turn, AiEvent::Error { kind: AiErrorKind::Crash, message: err.to_string() }).await?;
            self.set_state(&id, ChatState::Failed).await?;
            return Err(err);
        }
        Ok(ChatSendResult { session: id })
    }

    /// Processus vivant de la conversation, lancé au besoin.
    async fn ensure_live(self: &Arc<Self>, id: &str) -> AppResult<(Arc<ClaudeSession>, Arc<AtomicU32>)> {
        let mut live = self.live.lock().await;
        if let Some(entry) = live.get_mut(id) {
            if entry.session.is_alive() {
                entry.last_used = Instant::now();
                return Ok((entry.session.clone(), entry.turn.clone()));
            }
            live.remove(id);
        }
        // Plafond de processus vivants : le plus ancien inactif part.
        while live.len() >= self.config.max_live {
            let oldest = live.iter().filter(|(_, l)| !l.session.is_running()).min_by_key(|(_, l)| l.last_used).map(|(k, _)| k.clone());
            let Some(oldest) = oldest else { break };
            if let Some(entry) = live.remove(&oldest) {
                self.spawn_close(&oldest, entry.session);
            }
        }

        let cli = (self.claude)()?;
        let resume = self.store.native_id(id).await?;
        let token = self.gateway.as_ref().map(|g| g.context.tokens.issue(SessionScope { session: id.into(), run: None }));
        let files = staging::stage(&self.config.staging_root, id, self.gateway.as_ref().map(|g| g.url.as_str()), &(self.skills)())?;
        let (tx, rx) = mpsc::unbounded_channel();
        let session = Arc::new(ClaudeSession::start(
            ClaudeLaunch {
                cli,
                cwd: self.config.sessions_root.join(id),
                files,
                token,
                resume,
                model: self.config.model.clone(),
                mode: SessionMode::Chat,
                permissions: Permissions::Ask,
            },
            tx,
        )?);
        let turn = Arc::new(AtomicU32::new(0));
        tokio::spawn(consume(Arc::downgrade(self), id.to_string(), turn.clone(), rx));
        live.insert(id.to_string(), Live { session: session.clone(), turn: turn.clone(), last_used: Instant::now() });
        Ok((session, turn))
    }

    fn spawn_close(&self, id: &str, session: Arc<ClaudeSession>) {
        if let Some(gateway) = &self.gateway {
            gateway.context.tokens.revoke(id);
        }
        let grace = self.config.close_grace;
        tokio::spawn(async move {
            match Arc::try_unwrap(session) {
                Ok(session) => session.close(grace).await,
                Err(shared) => shared.kill().await,
            }
        });
    }

    async fn live_session(&self, id: &str) -> Option<Arc<ClaudeSession>> {
        self.live.lock().await.get(id).map(|l| l.session.clone())
    }

    /// Arrête le tour en cours ; s'il ne s'arrête pas à temps, le processus est tué.
    pub async fn interrupt(&self, id: &str) -> AppResult<()> {
        let session = self.live_session(id).await.filter(|s| s.is_running());
        let Some(session) = session else { return Err(AppError::Validation("Aucun tour en cours dans cette conversation.".into())) };
        session.interrupt().await?;
        let grace = self.config.interrupt_grace;
        tokio::spawn(async move {
            tokio::time::sleep(grace).await;
            if session.is_running() {
                session.kill().await;
            }
        });
        Ok(())
    }

    /// Répond à une demande d'approbation de la CLI.
    pub async fn approve(&self, id: &str, request: &str, decision: ApprovalDecision) -> AppResult<()> {
        let Some(session) = self.live_session(id).await else {
            return Err(AppError::Validation("Cette demande d'approbation n'attend plus de réponse.".into()));
        };
        session.answer(request, decision).await?;
        let turn = self.store.next_turn(id).await?.saturating_sub(1);
        self.record(id, turn, AiEvent::ApprovalAnswered { request: request.into(), decision }).await?;
        self.set_state(id, ChatState::Running).await
    }

    /// Supprime une conversation, son processus et son dossier de travail.
    pub async fn delete(&self, id: &str) -> AppResult<()> {
        self.store.get(id).await?;
        if let Some(entry) = self.live.lock().await.remove(id) {
            self.spawn_close(id, entry.session);
        }
        self.store.delete(id).await?;
        let dir = self.config.sessions_root.join(id);
        if dir.exists() && std::fs::remove_dir_all(&dir).is_err() {
            tracing::warn!(session = id, "dossier de la conversation non supprimé");
        }
        Ok(())
    }

    /// Ferme les processus inactifs depuis plus de `idle_close` ; renvoie leurs ids.
    pub async fn close_idle(&self) -> Vec<String> {
        let mut live = self.live.lock().await;
        let idle: Vec<String> = live
            .iter()
            .filter(|(_, l)| !l.session.is_running() && l.last_used.elapsed() >= self.config.idle_close)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &idle {
            if let Some(entry) = live.remove(id) {
                self.spawn_close(id, entry.session);
            }
        }
        idle
    }

    /// Nombre de processus vivants (tests, réglages).
    pub async fn live_count(&self) -> usize {
        self.live.lock().await.values().filter(|l| l.session.is_alive()).count()
    }

    /// Fermeture de l'app : tous les processus s'arrêtent.
    pub async fn shutdown(&self) {
        let all: Vec<(String, Live)> = self.live.lock().await.drain().collect();
        for (id, entry) in all {
            if let Some(gateway) = &self.gateway {
                gateway.context.tokens.revoke(&id);
            }
            match Arc::try_unwrap(entry.session) {
                Ok(session) => session.close(self.config.close_grace).await,
                Err(shared) => shared.kill().await,
            }
        }
    }
}

/// Enregistre et diffuse les événements d'un processus, dans l'ordre.
async fn consume(service: Weak<ChatService>, id: String, turn: Arc<AtomicU32>, mut events: mpsc::UnboundedReceiver<AiEvent>) {
    while let Some(event) = events.recv().await {
        let Some(service) = service.upgrade() else { return };
        let turn = turn.load(Ordering::SeqCst);
        let state = match &event {
            AiEvent::ApprovalRequested { .. } => Some(ChatState::WaitingApproval),
            AiEvent::TurnCompleted { interrupted: true, .. } => Some(ChatState::Interrupted),
            AiEvent::TurnCompleted { ok, .. } => Some(if *ok { ChatState::Idle } else { ChatState::Failed }),
            _ => None,
        };
        let result = match &event {
            AiEvent::SessionStarted { native_id, model } => {
                let saved = service.store.set_native(&id, native_id, model.as_deref()).await;
                (service.sink)(ChatNotice::Event(ChatEventPayload { session: id.clone(), seq: None, turn, event }));
                saved
            }
            AiEvent::MessageDelta { .. } => {
                (service.sink)(ChatNotice::Event(ChatEventPayload { session: id.clone(), seq: None, turn, event }));
                Ok(())
            }
            _ => service.record(&id, turn, event).await.map(|_| ()),
        };
        if let Err(err) = result {
            tracing::warn!(session = id, %err, "événement du chat non enregistré");
        }
        if let Some(state) = state
            && let Err(err) = service.set_state(&id, state).await
        {
            tracing::warn!(session = id, %err, "état du chat non enregistré");
        }
    }
}

/// Boucle de fermeture des processus inactifs ; s'arrête quand le service disparaît.
pub async fn run_idle_closer(service: Weak<ChatService>, period: Duration) {
    loop {
        tokio::time::sleep(period).await;
        let Some(service) = service.upgrade() else { return };
        service.close_idle().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_come_from_the_first_line() {
        assert_eq!(title_of("\n  Prépare la v0.4  \nDétails"), "Prépare la v0.4");
        assert_eq!(title_of(&"a".repeat(80)).chars().count(), 60);
        assert_eq!(title_of("   "), "Conversation");
        assert!(new_id().starts_with('c'));
    }
}
