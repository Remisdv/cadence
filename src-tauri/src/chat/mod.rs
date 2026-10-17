//! Chat du cœur (plan § 5, révision 1.1 : un chat global, plus de sessions par espace).
//!
//! Une conversation = une session d'IA (Claude Code, Codex CLI ou OpenCode), fixée à la création. Un
//! tour : `send` enregistre le message, démarre au besoin le processus de la CLI (préparation,
//! jeton de passerelle, reprise par `native_id`), envoie le texte ; les événements normalisés
//! sont enregistrés (deltas exclus) et poussés au front (`chat://event`, `chat://state`).
//!
//! Un processus inactif depuis 15 min est fermé (la conversation reste reprenable) ; 4
//! processus vivants au plus (le plus ancien inactif est fermé). Une interruption qui n'a pas
//! abouti en 5 s tue le processus.
//!
//! Worktrees ouverts par `cadence__repo_checkout` : la CLI les reçoit au lancement (`--add-dir`,
//! racine inscriptible, permission de chemin). Un worktree ouvert en cours de route relance la
//! session (reprise par `native_id`) au tour suivant ; pendant le tour, l'adaptateur accorde
//! d'office les accès qui le visent. Ils disparaissent avec la conversation.

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
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore, mpsc};
use ts_rs::TS;

use self::store::{ChatEventRow, ChatMode, ChatSessionDetail, ChatSessionRow, ChatState, ChatStore};
use crate::{
    ai::{
        claude_code::{Permissions, SessionMode},
        events::{AiErrorKind, AiEvent, ApprovalDecision},
        grants::DirGrants,
        session::{AiLaunch, AiSession},
        staging::{self, StagedSkill},
    },
    app_settings::AiId,
    domain::version::now_iso,
    error::{AppError, AppResult},
    gateway::{core_tools::RunReport, scope::SessionScope, server::GatewayContext},
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

/// Lancement d'une action dans le chat : nouvelle conversation (mode `chat`) ou exécution de
/// fond (mode `background`), dont le premier message est le prompt de l'action.
#[derive(Debug, Clone)]
pub struct ActionStart {
    pub ai: AiId,
    pub mode: ChatMode,
    pub title: String,
    /// `action:<plugin>/<action>`.
    pub origin: String,
    pub prompt: String,
}

/// Ce que le chat pousse au front.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatNotice {
    Event(ChatEventPayload),
    State(ChatStatePayload),
}

pub type ChatSink = Arc<dyn Fn(ChatNotice) + Send + Sync>;
/// Chemin de la CLI d'une IA, lu à chaque démarrage de processus (réglages à jour).
pub type AiLocator = Arc<dyn Fn(AiId) -> AppResult<PathBuf> + Send + Sync>;
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
    /// Exécutions de fond simultanées (les suivantes attendent leur tour).
    pub max_background: usize,
    /// Modèle imposé à Claude Code (alias `haiku`…), pour les tests ; les autres IA gardent le leur.
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
            max_background: 2,
            model: None,
        }
    }
}

struct Live {
    session: Arc<AiSession>,
    turn: Arc<AtomicU32>,
    last_used: Instant,
    mode: SessionMode,
    /// Worktrees ouverts à la CLI à son lancement.
    dirs: Vec<PathBuf>,
    /// Place dans la file des exécutions de fond, rendue à la fermeture du processus.
    _permit: Option<OwnedSemaphorePermit>,
}

pub struct ChatService {
    store: Arc<ChatStore>,
    config: ChatConfig,
    locate: AiLocator,
    skills: SkillSource,
    gateway: Option<GatewayAccess>,
    sink: ChatSink,
    live: Mutex<HashMap<String, Live>>,
    background: Arc<Semaphore>,
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
        locate: AiLocator,
        skills: SkillSource,
        gateway: Option<GatewayAccess>,
        sink: ChatSink,
    ) -> Arc<Self> {
        let background = Arc::new(Semaphore::new(config.max_background.max(1)));
        Arc::new(Self { store: Arc::new(store), config, locate, skills, gateway, sink, live: Mutex::default(), background })
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
                    model: (ai == AiId::ClaudeCode).then(|| self.config.model.clone()).flatten(),
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
        if matches!(row.state, ChatState::Running | ChatState::WaitingApproval) {
            return Err(AppError::Validation("Un tour est déjà en cours dans cette conversation.".into()));
        }
        let turn = self.store.next_turn(&id).await?;
        self.record(&id, turn, AiEvent::UserMessage { text: text.clone() }).await?;
        self.turn(&id, turn, &text, SessionMode::Chat, None).await?;
        Ok(ChatSendResult { session: id })
    }

    /// Tour déjà enregistré (`UserMessage`) : processus lancé au besoin, texte envoyé.
    async fn turn(
        self: &Arc<Self>,
        id: &str,
        turn: u32,
        text: &str,
        mode: SessionMode,
        permit: Option<OwnedSemaphorePermit>,
    ) -> AppResult<()> {
        let (session, turn_slot) = match self.ensure_live(id, mode, permit).await {
            Ok(live) => live,
            Err(err) => {
                self.record(id, turn, AiEvent::Error { kind: AiErrorKind::Other, message: err.to_string() }).await?;
                self.set_state(id, ChatState::Failed).await?;
                return Err(err);
            }
        };
        turn_slot.store(turn, Ordering::SeqCst);
        self.set_state(id, ChatState::Running).await?;
        if let Err(err) = session.send(text).await {
            self.record(id, turn, AiEvent::Error { kind: AiErrorKind::Crash, message: err.to_string() }).await?;
            self.set_state(id, ChatState::Failed).await?;
            return Err(err);
        }
        Ok(())
    }

    /// Lance une action : conversation ouverte tout de suite (mode chat), ou exécution de fond
    /// qui attend sa place dans la file. Renvoie l'id de la conversation.
    pub async fn start_action(self: &Arc<Self>, start: ActionStart) -> AppResult<String> {
        let now = now_iso();
        let row = ChatSessionRow {
            id: new_id(),
            title: start.title,
            ai: start.ai,
            model: (start.ai == AiId::ClaudeCode).then(|| self.config.model.clone()).flatten(),
            origin: start.origin,
            mode: start.mode,
            state: ChatState::Idle,
            created_at: now.clone(),
            updated_at: now,
        };
        self.store.create(&row).await?;
        let id = row.id;
        let turn = self.store.next_turn(&id).await?;
        self.record(&id, turn, AiEvent::UserMessage { text: start.prompt.clone() }).await?;
        match start.mode {
            ChatMode::Chat => self.turn(&id, turn, &start.prompt, SessionMode::Chat, None).await?,
            ChatMode::Background => {
                self.set_state(&id, ChatState::Running).await?;
                let (service, run) = (self.clone(), id.clone());
                tokio::spawn(async move {
                    let Ok(permit) = service.background.clone().acquire_owned().await else { return };
                    if let Err(err) = service.turn(&run, turn, &start.prompt, SessionMode::Background, Some(permit)).await {
                        tracing::warn!(run, %err, "exécution de fond non lancée");
                    }
                });
            }
        }
        Ok(id)
    }

    /// Rapport `cadence__run_report` d'une exécution de fond, ajouté à son historique.
    pub async fn record_report(&self, report: RunReport) -> AppResult<()> {
        let id = report.run.clone();
        if self.store.get(&id).await.is_err() {
            return Ok(());
        }
        let turn = self.store.next_turn(&id).await?.saturating_sub(1);
        let event = AiEvent::Report { text: report.text, progress: report.progress, is_final: report.is_final, data: report.data };
        self.record(&id, turn, event).await.map(|_| ())
    }

    /// Répond à une question posée par l'IA (Codex, OpenCode).
    pub async fn answer(&self, id: &str, request: &str, answer: &str) -> AppResult<()> {
        let answer = answer.trim();
        if answer.is_empty() {
            return Err(AppError::Validation("La réponse est vide.".into()));
        }
        let Some(session) = self.live_session(id).await else {
            return Err(AppError::Validation("Cette question n'attend plus de réponse.".into()));
        };
        session.answer_question(request, answer).await?;
        let turn = self.store.next_turn(id).await?.saturating_sub(1);
        self.record(id, turn, AiEvent::QuestionAnswered { request: request.into(), answer: answer.into() }).await?;
        self.set_state(id, ChatState::Running).await
    }

    /// Processus vivant de la conversation, lancé au besoin.
    async fn ensure_live(
        self: &Arc<Self>,
        id: &str,
        mode: SessionMode,
        permit: Option<OwnedSemaphorePermit>,
    ) -> AppResult<(Arc<AiSession>, Arc<AtomicU32>)> {
        let dirs = self.granted(id);
        let mut live = self.live.lock().await;
        if let Some(entry) = live.get_mut(id) {
            // Un tour en cours garde son processus ; sinon, un worktree ouvert depuis le
            // lancement relance la CLI (reprise de la session native) pour qu'elle le reçoive.
            if entry.session.is_alive() && (entry.dirs == dirs || entry.session.is_running()) {
                entry.last_used = Instant::now();
                return Ok((entry.session.clone(), entry.turn.clone()));
            }
            if let Some(entry) = live.remove(id)
                && entry.session.is_alive()
            {
                self.close_now(id, entry.session).await;
            }
        }
        // Plafond de processus vivants : le plus ancien inactif part.
        while live.len() >= self.config.max_live {
            let oldest = live.iter().filter(|(_, l)| !l.session.is_running()).min_by_key(|(_, l)| l.last_used).map(|(k, _)| k.clone());
            let Some(oldest) = oldest else { break };
            if let Some(entry) = live.remove(&oldest) {
                self.spawn_close(&oldest, entry.session);
            }
        }

        let row = self.store.get(id).await?;
        let cli = (self.locate)(row.ai)?;
        let resume = self.store.native_id(id).await?;
        let run = (mode == SessionMode::Background).then(|| id.to_string());
        let token = self.gateway.as_ref().map(|g| g.context.tokens.issue(SessionScope { session: id.into(), run }));
        let files = staging::stage(&self.config.staging_root, id, self.gateway.as_ref().map(|g| g.url.as_str()), &(self.skills)())?;
        let (tx, rx) = mpsc::unbounded_channel();
        let turn = Arc::new(AtomicU32::new(0));
        // Le consommateur part avant le démarrage : Codex et OpenCode annoncent leur session
        // pendant la poignée de main.
        tokio::spawn(consume(Arc::downgrade(self), id.to_string(), turn.clone(), rx));
        let session = Arc::new(
            AiSession::start(
                AiLaunch {
                    ai: row.ai,
                    cli,
                    cwd: self.config.sessions_root.join(id),
                    files,
                    gateway_url: self.gateway.as_ref().map(|g| g.url.clone()),
                    token,
                    resume,
                    model: row.model.clone().filter(|_| row.ai == AiId::ClaudeCode),
                    mode,
                    permissions: if mode == SessionMode::Background { Permissions::AutoEdit } else { Permissions::Ask },
                    title: row.title.clone(),
                    add_dirs: dirs.clone(),
                    granted: self.gateway.as_ref().map(|g| {
                        let (context, id) = (g.context.clone(), id.to_string());
                        let granted: DirGrants = Arc::new(move || context.granted(&id));
                        granted
                    }),
                },
                tx,
            )
            .await?,
        );
        live.insert(
            id.to_string(),
            Live { session: session.clone(), turn: turn.clone(), last_used: Instant::now(), mode, dirs, _permit: permit },
        );
        Ok((session, turn))
    }

    /// Worktrees ouverts à la conversation (`cadence__repo_checkout`).
    fn granted(&self, id: &str) -> Vec<PathBuf> {
        self.gateway.as_ref().map(|g| g.context.granted(id)).unwrap_or_default()
    }

    /// Ferme le processus et attend sa fin : la session native est libre pour une reprise.
    /// Le jeton de passerelle reste valide (la conversation continue).
    async fn close_now(&self, id: &str, session: Arc<AiSession>) {
        tracing::info!(session = id, "worktree ouvert : la CLI est relancée avec ce dossier");
        match Arc::try_unwrap(session) {
            Ok(session) => session.close(self.config.close_grace).await,
            Err(shared) => shared.kill().await,
        }
    }

    fn spawn_close(&self, id: &str, session: Arc<AiSession>) {
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

    async fn live_session(&self, id: &str) -> Option<Arc<AiSession>> {
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
        if let Some(gateway) = &self.gateway {
            gateway.context.forget_grants(id);
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

    /// Fin du tour d'une exécution de fond : son processus est fermé, sa place libérée.
    async fn end_background(&self, id: &str) {
        let mut live = self.live.lock().await;
        if live.get(id).is_some_and(|l| l.mode == SessionMode::Background)
            && let Some(entry) = live.remove(id)
        {
            self.spawn_close(id, entry.session);
        }
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
            AiEvent::ApprovalRequested { .. } | AiEvent::Question { .. } => Some(ChatState::WaitingApproval),
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
        if matches!(state, Some(ChatState::Idle | ChatState::Failed | ChatState::Interrupted)) {
            service.end_background(&id).await;
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
