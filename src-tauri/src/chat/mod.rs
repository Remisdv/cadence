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
//!
//! Mode d'une conversation (`Permissions` : Demander, Édition auto, Auto), choisi dans
//! le composeur et mémorisé avec elle. Les trois CLI le reçoivent au lancement : un changement
//! en cours de route relance le processus (reprise par `native_id`) au tour suivant, comme un
//! worktree ouvert ; le tour en cours garde son mode. Une exécution de fond reste en édition
//! auto (personne pour répondre aux demandes). Exception : les demandes d'accord encore en
//! attente, et celles du tour en cours, suivent tout de suite le nouveau mode (« Auto » les
//! accepte, « Édition auto » accepte les modifications de fichiers).
//!
//! « Autoriser pour la session » vaut pour la famille de la demande (`ai::session_rules` :
//! programmes d'une commande, modifications de fichiers, outil) jusqu'à la fermeture de Cadence :
//! les demandes couvertes reçoivent un accord d'office, sans carte.

pub mod conversations;
pub mod store;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Weak,
        Mutex as StdMutex,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore, mpsc};
use ts_rs::TS;

use self::{
    conversations::{ConversationDetail, ConversationInfo},
    store::{ChatEventRow, ChatGroup, ChatMode, ChatSessionDetail, ChatSessionRow, ChatState, ChatStore, SessionFilter, origin_action},
};
use crate::{
    ai::{
        claude_code::{Permissions, SessionMode},
        events::{
            AiErrorKind, AiEvent, AiQuestion, ApprovalDecision, ApprovalKind, QuestionAnswer, QuestionOption, answers_summary, check_answers,
        },
        grants::DirGrants,
        session::{AiLaunch, AiSession},
        session_rules::{ApprovalInfo, SessionRules},
        staging::{self, StagedSkill},
    },
    app_settings::AiId,
    domain::version::now_iso,
    error::{AppError, AppResult},
    gateway::{
        core_tools::RunReport,
        scope::{self, SessionScope},
        server::{GatewayContext, ToolAccess},
    },
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
    /// Plugin dont une action a ouvert la conversation (relais de l'événement `conversations`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub plugin: Option<String>,
    /// Groupe de la conversation, vu par ce plugin (id sans le préfixe de l'hôte).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub group: Option<ChatGroup>,
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
    /// Fichiers joints (chemins absolus), copiés dans le dossier de travail de la conversation.
    #[serde(default)]
    #[ts(optional)]
    pub attachments: Option<Vec<String>>,
    /// Mode d'une nouvelle conversation (défaut : Demander) ; ignoré pour une conversation
    /// existante (`chat_set_permissions`).
    #[serde(default)]
    #[ts(optional)]
    pub permissions: Option<Permissions>,
    /// Nouvelle conversation ouverte depuis la barre d'un plugin (« + » de ses conversations) :
    /// origine `plugin:<id>`, listée dans cette barre avec la puce du plugin.
    #[serde(default)]
    #[ts(optional)]
    pub plugin: Option<String>,
}

/// Origine d'une nouvelle conversation : `plugin:<id>` depuis la barre d'un plugin, sinon `user`.
pub fn send_origin(plugin: Option<&str>) -> AppResult<String> {
    match plugin.map(str::trim).filter(|p| !p.is_empty()) {
        None => Ok("user".into()),
        Some(id) if id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') => {
            Ok(format!("plugin:{id}"))
        }
        Some(id) => Err(AppError::Validation(format!("« {id} » n'est pas un id de plugin."))),
    }
}

/// Dossier des pièces jointes, dans le dossier de travail de la conversation.
pub const ATTACHMENTS_DIR: &str = "pieces-jointes";
/// Taille maximale d'une pièce jointe.
const ATTACHMENT_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// Copie les pièces jointes dans `<cwd>/pieces-jointes/` (noms dédoublonnés) ; renvoie leurs
/// chemins relatifs au dossier de travail, à donner à l'IA.
pub fn copy_attachments(cwd: &std::path::Path, files: &[String]) -> AppResult<Vec<String>> {
    let dir = cwd.join(ATTACHMENTS_DIR);
    let mut copied = Vec::new();
    for file in files {
        let source = std::path::Path::new(file.trim());
        let meta = std::fs::metadata(source)
            .map_err(|_| AppError::Validation(format!("Pièce jointe introuvable : {}", source.display())))?;
        if !meta.is_file() {
            return Err(AppError::Validation(format!("{} n'est pas un fichier.", source.display())));
        }
        if meta.len() > ATTACHMENT_MAX_BYTES {
            return Err(AppError::Validation(format!("{} dépasse 50 Mo.", source.display())));
        }
        std::fs::create_dir_all(&dir)?;
        let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "fichier".into());
        let (stem, ext) = match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
            _ => (name.clone(), String::new()),
        };
        let mut target = name.clone();
        let mut n = 2;
        while dir.join(&target).exists() {
            target = format!("{stem}-{n}{ext}");
            n += 1;
        }
        std::fs::copy(source, dir.join(&target))?;
        copied.push(format!("{ATTACHMENTS_DIR}/{target}"));
    }
    Ok(copied)
}

/// Message envoyé à l'IA : le texte, puis la liste des pièces jointes copiées.
pub fn with_attachments(text: &str, copied: &[String]) -> String {
    if copied.is_empty() {
        return text.to_string();
    }
    let list: Vec<String> = copied.iter().map(|p| format!("- {p}")).collect();
    format!("{text}\n\nPièces jointes (copiées dans ton dossier de travail) :\n{}", list.join("\n"))
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
    /// Groupe (id déjà préfixé par le plugin) et visibilité dans le chat global.
    pub group: Option<ChatGroup>,
    pub hidden: bool,
    /// Contexte de l'action, rendu au plugin par `conversations.list/get`.
    pub context: Option<serde_json::Value>,
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
    /// Mode de la conversation au lancement de la CLI.
    permissions: Permissions,
    /// Worktrees ouverts à la CLI à son lancement.
    dirs: Vec<PathBuf>,
    /// Place dans la file des exécutions de fond, rendue à la fermeture du processus.
    _permit: Option<OwnedSemaphorePermit>,
}

/// Demande de la CLI qui attend l'utilisateur.
#[derive(Debug, Clone)]
enum Waiting {
    Approval(ApprovalInfo),
    Question,
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
    /// Accords « pour la session » de chaque conversation (jusqu'à la fermeture de Cadence).
    rules: StdMutex<HashMap<String, SessionRules>>,
    /// Demandes en attente de chaque conversation (tour en cours), dans l'ordre.
    waiting: StdMutex<HashMap<String, Vec<(String, Waiting)>>>,
    /// Conversations dont l'utilisateur a arrêté le tour : sa fin, même en échec côté CLI
    /// (`error_during_execution` quand un accord attendait), est un arrêt (recette du 21/10, R05).
    stopping: StdMutex<HashSet<String>>,
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
        Arc::new(Self {
            store: Arc::new(store),
            config,
            locate,
            skills,
            gateway,
            sink,
            live: Mutex::default(),
            background,
            rules: StdMutex::default(),
            waiting: StdMutex::default(),
            stopping: StdMutex::default(),
        })
    }

    /// Conversations du chat global ; avec `group` (id préfixé), celles du groupe, cachées
    /// comprises ; avec `hidden`, toutes (recherche du chat, demandes en attente).
    pub async fn sessions(&self, group: Option<&str>, hidden: bool) -> AppResult<Vec<ChatSessionRow>> {
        self.store.list(SessionFilter { plugin: None, group, hidden, involved: false }).await
    }

    /// Conversations ouvertes par les actions d'un plugin (`conversations.list` du SDK) ; `group`
    /// est l'id vu par le plugin. `involved` : aussi celles, du chat global ou d'un groupe caché,
    /// qui ont appelé un outil du plugin (export d'un compte rendu complet).
    pub async fn conversations(&self, plugin: &str, group: Option<&str>, involved: bool) -> AppResult<Vec<ConversationInfo>> {
        let scoped = group.map(|g| ChatGroup::scoped_id(plugin, g));
        let filter = SessionFilter { plugin: Some(plugin), group: scoped.as_deref(), hidden: involved, involved };
        let rows = self.store.list_with_turns(filter).await?;
        Ok(rows.into_iter().map(|(row, turns)| ConversationInfo::of(plugin, &row, turns)).collect())
    }

    /// Une conversation d'un plugin (`conversations.get`) : messages complets, outils sans
    /// leurs sorties. Celle d'un autre plugin est introuvable.
    pub async fn conversation(&self, plugin: &str, id: &str) -> AppResult<ConversationDetail> {
        let row = self.plugin_session(plugin, id).await?;
        let turns = self.store.turns(id).await?;
        Ok(ConversationDetail::of(plugin, ConversationInfo::of(plugin, &row, turns), &self.store.events(id).await?))
    }

    /// Conversation ouverte par une action de ce plugin, ou qui a appelé un de ses outils ;
    /// sinon « introuvable ».
    pub async fn plugin_session(&self, plugin: &str, id: &str) -> AppResult<ChatSessionRow> {
        let missing = || AppError::Validation(format!("Conversation « {id} » introuvable."));
        let row = self.store.get(id).await.map_err(|_| missing())?;
        if origin_action(&row.origin).is_some_and(|(p, _)| p == plugin) || self.store.uses_plugin(id, plugin).await? {
            Ok(row)
        } else {
            Err(missing())
        }
    }

    pub async fn detail(&self, id: &str) -> AppResult<ChatSessionDetail> {
        Ok(ChatSessionDetail {
            session: self.store.get(id).await?,
            events: self.store.events(id).await?,
            worktrees: self.granted(id).into_iter().map(|dir| dir.display().to_string()).collect(),
        })
    }

    pub async fn rename(&self, id: &str, title: &str) -> AppResult<ChatSessionRow> {
        self.store.rename(id, title).await
    }

    /// Change le mode de la conversation. La CLI le reçoit au tour suivant : le processus
    /// vivant, lancé avec l'ancien mode, est relancé (reprise de la session native) à ce
    /// moment-là. Les demandes d'accord en attente, et celles qui arrivent d'ici la fin du tour,
    /// suivent le nouveau mode tout de suite.
    pub async fn set_permissions(&self, id: &str, permissions: Permissions) -> AppResult<ChatSessionRow> {
        let row = self.store.set_permissions(id, permissions).await?;
        self.answer_covered(id).await?;
        Ok(self.store.get(id).await.unwrap_or(row))
    }

    /// Accord d'office pour une demande : mode Auto, Édition auto pour une modification de
    /// fichier, ou famille déjà accordée pour la session.
    async fn allowed_now(&self, id: &str, info: &ApprovalInfo) -> bool {
        let permissions = self.store.get(id).await.map(|row| row.permissions).unwrap_or_default();
        let by_mode = match permissions {
            Permissions::Full => true,
            // « Édition auto » laisse passer les modifications de fichiers et les outils des
            // plugins qui écrivent (N12) ; les commandes restent demandées.
            Permissions::AutoEdit => info.kind == ApprovalKind::FileChange || info.plugin_write,
            Permissions::Ask => false,
        };
        by_mode || self.rules.lock().unwrap().get(id).is_some_and(|rules| rules.covers(info))
    }

    /// Accès d'un outil de la passerelle nommé par la CLI (`mcp__cadence__…`, `cadence_…`) ;
    /// `None` pour un outil qui n'est pas servi par la passerelle.
    async fn gateway_access(&self, tool: Option<&str>) -> Option<ToolAccess> {
        let exposed = scope::exposed_of_cli(tool?)?;
        Some(self.gateway.as_ref()?.context.tool_access(exposed).await)
    }

    fn wait_for(&self, id: &str, request: &str, waiting: Waiting) {
        self.waiting.lock().unwrap().entry(id.to_string()).or_default().push((request.to_string(), waiting));
    }

    fn take_waiting(&self, id: &str, request: &str) -> Option<Waiting> {
        let mut all = self.waiting.lock().unwrap();
        let list = all.get_mut(id)?;
        let at = list.iter().position(|(r, _)| r == request)?;
        Some(list.remove(at).1)
    }

    fn still_waiting(&self, id: &str) -> bool {
        self.waiting.lock().unwrap().get(id).is_some_and(|list| !list.is_empty())
    }

    /// Répond d'office aux demandes en attente que le mode ou les accords de session couvrent
    /// désormais ; la conversation repart si plus rien n'attend l'utilisateur.
    async fn answer_covered(&self, id: &str) -> AppResult<()> {
        let pending: Vec<(String, ApprovalInfo)> = self
            .waiting
            .lock()
            .unwrap()
            .get(id)
            .into_iter()
            .flatten()
            .filter_map(|(request, waiting)| match waiting {
                Waiting::Approval(info) => Some((request.clone(), info.clone())),
                Waiting::Question => None,
            })
            .collect();
        let Some(session) = self.live_session(id).await else { return Ok(()) };
        let mut covered = Vec::new();
        for (request, info) in pending {
            if self.allowed_now(id, &info).await {
                self.take_waiting(id, &request);
                covered.push(request);
            }
        }
        // L'état change avant les réponses : la CLI repart aussitôt et peut redemander.
        if !covered.is_empty() && !self.still_waiting(id) {
            self.set_state(id, ChatState::Running).await?;
        }
        for request in covered {
            if let Err(err) = session.answer(&request, ApprovalDecision::Allow).await {
                tracing::warn!(session = id, %err, "accord d'office non transmis");
                continue;
            }
            let turn = self.store.next_turn(id).await?.saturating_sub(1);
            self.record(id, turn, AiEvent::ApprovalAnswered { request, decision: ApprovalDecision::Allow }).await?;
        }
        Ok(())
    }

    async fn notify_state(&self, session: &str, state: ChatState) {
        // Plugin et groupe de la conversation : le front relaie l'état à l'iframe du plugin.
        let row = self.store.get(session).await.ok();
        let plugin = row.as_ref().and_then(|r| origin_action(&r.origin)).map(|(p, _)| p.to_string());
        let group = match (&plugin, row.as_ref().and_then(|r| r.group.as_ref())) {
            (Some(plugin), Some(group)) => group.local_to(plugin),
            _ => None,
        };
        (self.sink)(ChatNotice::State(ChatStatePayload { session: session.into(), state, plugin, group }));
    }

    async fn record(&self, session: &str, turn: u32, event: AiEvent) -> AppResult<ChatEventRow> {
        let row = self.store.append(session, turn, &event).await?;
        (self.sink)(ChatNotice::Event(ChatEventPayload { session: session.into(), seq: Some(row.seq), turn, event }));
        Ok(row)
    }

    async fn set_state(&self, session: &str, state: ChatState) -> AppResult<()> {
        self.store.set_state(session, state).await?;
        self.notify_state(session, state).await;
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
                    origin: send_origin(input.plugin.as_deref())?,
                    mode: ChatMode::Chat,
                    permissions: input.permissions.unwrap_or_default(),
                    state: ChatState::Idle,
                    created_at: now.clone(),
                    updated_at: now,
                    group: None,
                    hidden: false,
                    context: None,
                };
                self.store.create(&row).await?;
                row.id
            }
        };
        let row = self.store.get(&id).await?;
        if matches!(row.state, ChatState::Running | ChatState::WaitingApproval) {
            return Err(AppError::Validation("Un tour est déjà en cours dans cette conversation.".into()));
        }
        let files = input.attachments.unwrap_or_default();
        let copied = copy_attachments(&self.config.sessions_root.join(&id), &files)?;
        let text = with_attachments(&text, &copied);
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
        self.stopping.lock().unwrap().remove(id);
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
            permissions: match start.mode {
                ChatMode::Chat => Permissions::Ask,
                ChatMode::Background => Permissions::AutoEdit,
            },
            state: ChatState::Idle,
            created_at: now.clone(),
            updated_at: now,
            group: start.group,
            hidden: start.hidden,
            context: start.context,
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

    /// Répond aux questions à choix de l'IA (Claude Code, Codex, OpenCode) : une entrée par
    /// question posée (libellés choisis ou texte libre), remise dans l'ordre des questions.
    pub async fn answer(&self, id: &str, request: &str, answers: Vec<QuestionAnswer>) -> AppResult<()> {
        let questions = self.asked(id, request).await?;
        let answers: Vec<QuestionAnswer> = questions
            .iter()
            .filter_map(|q| answers.iter().find(|a| a.id == q.id))
            .map(|a| QuestionAnswer {
                id: a.id.clone(),
                answers: a.answers.iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            })
            .collect();
        check_answers(&questions, &answers).map_err(AppError::Validation)?;
        let Some(session) = self.live_session(id).await else {
            return Err(AppError::Validation("Cette question n'attend plus de réponse.".into()));
        };
        let waiting = self.take_waiting(id, request);
        if waiting.is_some() && !self.still_waiting(id) {
            self.set_state(id, ChatState::Running).await?;
        }
        if let Err(err) = session.answer_question(request, &answers).await {
            if let Some(waiting) = waiting {
                self.wait_for(id, request, waiting);
                self.set_state(id, ChatState::WaitingApproval).await?;
            }
            return Err(err);
        }
        let turn = self.store.next_turn(id).await?.saturating_sub(1);
        let answer = answers_summary(&questions, &answers);
        self.record(id, turn, AiEvent::QuestionAnswered { request: request.into(), answer, answers }).await.map(|_| ())
    }

    /// Questions d'une demande `request`, lues dans l'historique (une seule pour une question
    /// d'avant les questions multiples).
    async fn asked(&self, id: &str, request: &str) -> AppResult<Vec<AiQuestion>> {
        let events = self.store.events(id).await?;
        let asked = events.into_iter().rev().find_map(|row| match row.event {
            AiEvent::Question { request: r, text, options, questions } if r == request => Some(if questions.is_empty() {
                let options = options.into_iter().map(|label| QuestionOption { label, description: None }).collect();
                vec![AiQuestion { id: text.clone(), header: None, question: text, options, multi_select: false, other: true }]
            } else {
                questions
            }),
            _ => None,
        });
        asked.ok_or_else(|| AppError::Validation("Cette question n'attend plus de réponse.".into()))
    }

    /// Processus vivant de la conversation, lancé au besoin.
    async fn ensure_live(
        self: &Arc<Self>,
        id: &str,
        mode: SessionMode,
        permit: Option<OwnedSemaphorePermit>,
    ) -> AppResult<(Arc<AiSession>, Arc<AtomicU32>)> {
        let dirs = self.granted(id);
        let row = self.store.get(id).await?;
        let permissions = launch_permissions(row.permissions, mode);
        let mut live = self.live.lock().await;
        if let Some(entry) = live.get_mut(id) {
            // Un tour en cours garde son processus ; sinon, un worktree ouvert ou un mode changé
            // depuis le lancement relance la CLI (reprise de la session native) pour qu'elle les
            // reçoive.
            let same = entry.dirs == dirs && entry.permissions == permissions;
            if entry.session.is_alive() && (same || entry.session.is_running()) {
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
                    permissions,
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
            Live { session: session.clone(), turn: turn.clone(), last_used: Instant::now(), mode, permissions, dirs, _permit: permit },
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
        tracing::info!(session = id, "worktree ouvert ou mode changé : la CLI est relancée (reprise)");
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
        self.stopping.lock().unwrap().insert(id.to_string());
        if let Err(err) = session.interrupt().await {
            self.stopping.lock().unwrap().remove(id);
            return Err(err);
        }
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
        // Règle et état d'abord : la CLI repart dès la réponse et peut redemander aussitôt (la
        // demande suivante doit déjà trouver l'accord de session).
        let waiting = self.take_waiting(id, request);
        let grant = match &waiting {
            Some(Waiting::Approval(info)) if decision == ApprovalDecision::AllowSession => info.grant(),
            _ => None,
        };
        if let Some(grant) = &grant {
            self.rules.lock().unwrap().entry(id.to_string()).or_default().add(grant.clone());
        }
        if waiting.is_some() && !self.still_waiting(id) {
            self.set_state(id, ChatState::Running).await?;
        }
        if let Err(err) = session.answer(request, decision).await {
            if let Some(waiting) = waiting {
                self.wait_for(id, request, waiting);
                self.set_state(id, ChatState::WaitingApproval).await?;
            }
            return Err(err);
        }
        let turn = self.store.next_turn(id).await?.saturating_sub(1);
        self.record(id, turn, AiEvent::ApprovalAnswered { request: request.into(), decision }).await?;
        if grant.is_some() {
            // Les autres demandes en attente de la même famille sont acceptées avec elle.
            self.answer_covered(id).await?;
        }
        Ok(())
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
        self.rules.lock().unwrap().remove(id);
        self.waiting.lock().unwrap().remove(id);
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

/// Mode passé à la CLI : celui de la conversation ; une exécution de fond reste en édition auto.
pub fn launch_permissions(chosen: Permissions, mode: SessionMode) -> Permissions {
    match mode {
        SessionMode::Chat => chosen,
        SessionMode::Background => Permissions::AutoEdit,
    }
}

/// Arguments d'un appel d'outil dans le détail d'une demande d'accord : l'entrée de l'outil
/// (Claude Code), ou ce que Codex et OpenCode en donnent.
fn tool_arguments(detail: &serde_json::Value) -> Option<&serde_json::Map<String, serde_json::Value>> {
    const NESTED: &[&str] = &["/arguments", "/input", "/metadata/input", "/metadata/args", "/request/_meta/tool_params", "/_meta/tool_params"];
    if let Some(found) = NESTED.iter().find_map(|path| detail.pointer(path).and_then(|v| v.as_object())) {
        return Some(found);
    }
    let object = detail.as_object()?;
    let envelope = ["permission", "serverName", "threadId", "sessionID", "request", "message"];
    (!envelope.iter().any(|key| object.contains_key(*key))).then_some(object)
}

/// Titre de la carte d'accord d'un outil de plugin (N12) : plugin, outil et arguments
/// résumés (« Gestion de projet › merge_pull_request · pull_number: 22, delete_branch: true »).
pub fn plugin_tool_title(plugin: &str, tool: &str, detail: &serde_json::Value) -> String {
    use serde_json::Value;
    let short = |text: &str| {
        let line = text.lines().next().unwrap_or_default();
        if line.chars().count() > 40 || text.contains('\n') { format!("« {}… »", line.chars().take(40).collect::<String>().trim_end()) } else { format!("« {line} »") }
    };
    let args: Vec<String> = tool_arguments(detail)
        .into_iter()
        .flatten()
        .filter(|(_, v)| !v.is_null())
        .take(5)
        .map(|(key, value)| {
            let shown = match value {
                Value::String(text) => short(text),
                Value::Array(items) => format!("[{}]", items.iter().take(4).map(|i| match i {
                    Value::String(t) => t.chars().take(20).collect(),
                    other => other.to_string(),
                }).collect::<Vec<String>>().join(", ")),
                Value::Object(_) => "{…}".into(),
                other => other.to_string(),
            };
            format!("{key}: {shown}")
        })
        .collect();
    if args.is_empty() { format!("{plugin} › {tool}") } else { format!("{plugin} › {tool} · {}", args.join(", ")) }
}

/// Débuts de phrase qui invitent l'utilisateur à répondre (impératifs).
const REPLY_OPENERS: &[&str] = &[
    "réponds",
    "répondez",
    "confirme",
    "confirmez",
    "dis-moi",
    "dites-moi",
    "indique-moi",
    "indiquez-moi",
    "précise-moi",
    "précisez-moi",
    "choisis",
    "choisissez",
    "reply",
    "confirm",
    "tell me",
    "let me know",
];

/// Formules qui invitent l'utilisateur à répondre, où qu'elles soient dans la phrase.
const REPLY_CUES: &[&str] = &[
    "dis-moi",
    "dites-moi",
    "confirme-moi",
    "confirmez-moi",
    "réponds-moi",
    "répondez-moi",
    "j'attends ta",
    "j'attends votre",
    "veux-tu",
    "voulez-vous",
    "souhaites-tu",
    "souhaitez-vous",
    "let me know",
    "please confirm",
    "please reply",
];

/// Phrases d'attente qui suivent souvent la question (« Je reste en attente. ») : elles ne
/// comptent pas comme la fin du message.
const WAITING_FILLERS: &[&str] = &[
    "en attente",
    "j'attends",
    "je t'attends",
    "je vous attends",
    "à toi de voir",
    "à toi de jouer",
    "à vous de voir",
    "je m'arrête là",
    "je n'avance pas",
    "je ne touche à rien",
    "waiting for",
    "i'll wait",
];

/// Débuts d'une phrase qui répond à la question précédente : celle-ci était rhétorique
/// (« Pourquoi squash ? Parce que c'est le réglage du plugin. »).
const ANSWER_OPENERS: &[&str] =
    &["parce que", "parce qu'", "car ", "c'est", "oui", "non", "simplement", "réponse", "because", "it's", "yes", "no "];

/// Ponctuation et mise en forme qui peuvent suivre la fin d'une phrase.
fn is_closer(c: char) -> bool {
    matches!(c, '*' | '_' | '`' | ')' | ']' | '»' | '"' | '\'' | '”' | '’')
}

/// Phrases d'un paragraphe, coupées après `.`, `!`, `?` ou `…` suivis d'un blanc (ou de la
/// fin), ponctuation fermante comprise ; les retours à la ligne comptent comme des blancs.
fn sentences(paragraph: &str) -> Vec<String> {
    let chars: Vec<char> = paragraph.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        current.push(if c == '\n' { ' ' } else { c });
        i += 1;
        if matches!(c, '.' | '!' | '?' | '…') {
            while i < chars.len() && is_closer(chars[i]) {
                current.push(chars[i]);
                i += 1;
            }
            if i >= chars.len() || chars[i].is_whitespace() {
                out.push(std::mem::take(&mut current));
            }
        }
    }
    out.push(current);
    out.into_iter().map(|s| s.trim().to_string()).filter(|s| s.chars().any(char::is_alphanumeric)).collect()
}

/// Phrase en minuscules, sans mise en forme ni ponctuation de tête (puces, émojis).
fn plain(sentence: &str) -> String {
    let lower = sentence.to_lowercase().replace(['*', '`'], "").replace('’', "'");
    lower.trim_start_matches(|c: char| !c.is_alphanumeric()).to_string()
}

fn is_question(sentence: &str) -> bool {
    sentence.trim_end_matches(|c: char| c.is_whitespace() || is_closer(c) || c == '…').ends_with('?')
}

fn invites_reply(sentence: &str) -> bool {
    let s = plain(sentence);
    REPLY_OPENERS.iter().any(|o| s.starts_with(o)) || REPLY_CUES.iter().any(|cue| s.contains(cue))
}

fn is_filler(sentence: &str) -> bool {
    let s = plain(sentence);
    !is_question(sentence) && WAITING_FILLERS.iter().any(|f| s.contains(f))
}

/// Phrases de fin d'un paragraphe, phrases d'attente finales mises à part.
fn without_fillers(paragraph: &[String]) -> Vec<&String> {
    let mut tail: Vec<&String> = paragraph.iter().collect();
    while tail.last().is_some_and(|s| is_filler(s)) {
        tail.pop();
    }
    tail
}

/// Le dernier message de l'IA attend une réponse (recette du 21/10, N10). On lit les
/// dernières phrases du dernier paragraphe :
/// - l'une des trois dernières invite à répondre (« Réponds **oui** pour que je fusionne. »,
///   « Dis-moi si… », « Confirme… ») ;
/// - ou, phrases d'attente finales mises à part (« Je reste en attente. »), la dernière est
///   une question, ou l'avant-dernière si la dernière n'y répond pas (« … comme décrit ?
///   Aucune des deux n'est refusée. »). Un paragraphe fait seulement de phrases d'attente
///   renvoie au précédent.
///
/// Une question plus haut dans un long compte rendu final ne compte pas.
pub fn awaits_reply(text: &str) -> bool {
    let paragraphs: Vec<Vec<String>> = text.split("\n\n").map(sentences).filter(|p| !p.is_empty()).collect();
    let Some(last) = paragraphs.last() else { return false };
    if last.iter().rev().take(3).any(|s| invites_reply(s)) {
        return true;
    }
    let mut tail = without_fillers(last);
    if tail.is_empty() && paragraphs.len() > 1 {
        tail = without_fillers(&paragraphs[paragraphs.len() - 2]);
    }
    match tail.as_slice() {
        [.., last] if is_question(last) => true,
        [.., question, after] => {
            let after = plain(after);
            is_question(question) && !ANSWER_OPENERS.iter().any(|o| after.starts_with(o))
        }
        _ => false,
    }
}

/// État d'une conversation à la fin d'un tour : `WaitingReply` quand le tour réussi finit
/// par une question.
pub fn state_after_turn(ok: bool, interrupted: bool, last_message: Option<&str>) -> ChatState {
    match (interrupted, ok) {
        (true, _) => ChatState::Interrupted,
        (false, false) => ChatState::Failed,
        (false, true) if last_message.is_some_and(awaits_reply) => ChatState::WaitingReply,
        (false, true) => ChatState::Idle,
    }
}

/// Fin d'un tour que l'utilisateur a arrêté : un échec de la CLI (refus des accords en attente,
/// `error_during_execution`) devient un arrêt, sans message d'erreur. Un tour qui a réussi
/// malgré tout reste réussi.
fn stopped_turn(stopped: bool, ok: &mut bool, interrupted: &mut bool, error: &mut Option<String>) {
    if stopped && !*ok {
        *interrupted = true;
        *error = None;
    }
}

/// Enregistre et diffuse les événements d'un processus, dans l'ordre.
async fn consume(service: Weak<ChatService>, id: String, turn: Arc<AtomicU32>, mut events: mpsc::UnboundedReceiver<AiEvent>) {
    // Dernier message de l'IA du tour en cours : décide de « En attente de votre réponse ».
    let mut last_message: Option<String> = None;
    while let Some(mut event) = events.recv().await {
        let Some(service) = service.upgrade() else { return };
        let turn = turn.load(Ordering::SeqCst);
        match &mut event {
            AiEvent::ApprovalRequested { request, kind, title, detail, tool, scope } => {
                let mut info = ApprovalInfo::of(*kind, tool.as_deref(), title, detail);
                // Outil de la passerelle (N12) : ceux du cœur et des plugins en lecture seule
                // passent d'office ; un outil de plugin qui écrit devient une carte lisible.
                if *kind == ApprovalKind::Tool {
                    match service.gateway_access(tool.as_deref()).await {
                        Some(ToolAccess::Core | ToolAccess::ReadOnly) => {
                            if let Some(session) = service.live_session(&id).await
                                && session.answer(request, ApprovalDecision::Allow).await.is_ok()
                            {
                                continue;
                            }
                        }
                        Some(ToolAccess::Writes { plugin, tool: name }) => {
                            info.plugin_write = true;
                            *title = plugin_tool_title(&plugin, &name, detail);
                        }
                        Some(ToolAccess::Unknown) | None => {}
                    }
                }
                // Couverte par le mode ou un accord de session : accordée d'office, sans carte.
                if service.allowed_now(&id, &info).await
                    && let Some(session) = service.live_session(&id).await
                    && session.answer(request, ApprovalDecision::Allow).await.is_ok()
                {
                    continue;
                }
                *scope = info.grant().map(|grant| grant.label());
                service.wait_for(&id, request, Waiting::Approval(info));
            }
            AiEvent::Question { request, .. } => service.wait_for(&id, request, Waiting::Question),
            AiEvent::TurnCompleted { ok, interrupted, error, .. } => {
                service.waiting.lock().unwrap().remove(&id);
                let stopped = service.stopping.lock().unwrap().remove(&id);
                stopped_turn(stopped, ok, interrupted, error);
            }
            _ => {}
        }
        if let AiEvent::MessageCompleted { text, .. } = &event
            && !text.trim().is_empty()
        {
            last_message = Some(text.clone());
        }
        let state = match &event {
            AiEvent::ApprovalRequested { .. } | AiEvent::Question { .. } => Some(ChatState::WaitingApproval),
            AiEvent::TurnCompleted { ok, interrupted, .. } => Some(state_after_turn(*ok, *interrupted, last_message.take().as_deref())),
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
        if matches!(state, Some(ChatState::Idle | ChatState::WaitingReply | ChatState::Failed | ChatState::Interrupted)) {
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
    fn copies_attachments_into_the_session_folder() {
        let root = crate::app_settings::temp_dir("chat-attachments");
        let source = root.join("source");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("cours.pdf"), "pdf").unwrap();
        std::fs::write(source.join("notes"), "sans extension").unwrap();
        let cwd = root.join("sessions").join("c1");
        let files = |names: &[&str]| names.iter().map(|n| source.join(n).display().to_string()).collect::<Vec<_>>();
        let copied = copy_attachments(&cwd, &files(&["cours.pdf", "notes"])).unwrap();
        assert_eq!(copied, ["pieces-jointes/cours.pdf", "pieces-jointes/notes"]);
        assert_eq!(copy_attachments(&cwd, &files(&["cours.pdf"])).unwrap(), ["pieces-jointes/cours-2.pdf"], "pas d'écrasement");
        assert!(cwd.join("pieces-jointes").join("cours-2.pdf").is_file());
        assert!(copy_attachments(&cwd, &files(&["absent.txt"])).is_err());
        assert!(copy_attachments(&cwd, &[source.display().to_string()]).is_err(), "pas un dossier");
        let text = with_attachments("Résume", &copied);
        assert!(text.starts_with("Résume\n\nPièces jointes") && text.ends_with("- pieces-jointes/notes"));
        assert_eq!(with_attachments("Bonjour", &[]), "Bonjour");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_conversation_opened_from_a_plugin_bar_keeps_its_plugin() {
        assert_eq!(send_origin(None).unwrap(), "user");
        assert_eq!(send_origin(Some(" ")).unwrap(), "user");
        assert_eq!(send_origin(Some("gestion-projet")).unwrap(), "plugin:gestion-projet");
        assert!(send_origin(Some("../x")).is_err());
        assert!(send_origin(Some("Gestion")).is_err());
    }

    #[test]
    fn background_runs_keep_auto_edit() {
        assert_eq!(launch_permissions(Permissions::Full, SessionMode::Chat), Permissions::Full);
        assert_eq!(launch_permissions(Permissions::Ask, SessionMode::Chat), Permissions::Ask);
        assert_eq!(launch_permissions(Permissions::Full, SessionMode::Background), Permissions::AutoEdit);
        assert_eq!(launch_permissions(Permissions::Ask, SessionMode::Background), Permissions::AutoEdit);
    }

    #[test]
    fn plugin_tool_cards_summarize_their_arguments() {
        let claude = serde_json::json!({ "repo": "o/n", "pull_number": 22, "delete_branch": true, "close_issues": [21], "body": "Revue\nlongue", "x": null });
        let title = plugin_tool_title("Gestion de projet", "merge_pull_request", &claude);
        assert!(title.starts_with("Gestion de projet › merge_pull_request · "), "{title}");
        for part in ["repo: « o/n »", "pull_number: 22", "delete_branch: true", "close_issues: [21]", "body: « Revue… »"] {
            assert!(title.contains(part), "{part} dans {title}");
        }
        assert!(!title.contains("x:"), "valeurs nulles omises : {title}");
        let opencode = serde_json::json!({ "permission": "cadence_gestion-projet__create_issue", "metadata": { "input": { "title": "QA" } } });
        assert_eq!(plugin_tool_title("Gestion de projet", "create_issue", &opencode), "Gestion de projet › create_issue · title: « QA »");
        let codex = serde_json::json!({ "serverName": "cadence", "message": "Allow?" });
        assert_eq!(plugin_tool_title("Gestion de projet", "create_issue", &codex), "Gestion de projet › create_issue");
    }

    #[test]
    fn a_turn_ending_with_a_question_waits_for_a_reply() {
        for text in [
            "Les deux PR sont prêtes.\n\nJe fusionne #22 et #24 ?",
            "Revue terminée.\n\nRéponds **oui** pour que je fusionne.",
            "Tout est vert. Veux-tu que je supprime la branche ?**",
            "Done.\n\nLet me know if I should merge.",
            "On part sur quelle option (A ou B ?)",
        ] {
            assert!(awaits_reply(text), "{text}");
        }
        for text in ["Fusion faite : #22 et #24 sont dans release/0.2.0.", "Question réglée ?\n\nJ'ai tout fusionné.", "", "   "] {
            assert!(!awaits_reply(text), "{text}");
        }
        assert_eq!(state_after_turn(true, false, Some("On fusionne ?")), ChatState::WaitingReply);
        assert_eq!(state_after_turn(true, false, Some("C'est fusionné.")), ChatState::Idle);
        assert_eq!(state_after_turn(true, false, None), ChatState::Idle);
        assert_eq!(state_after_turn(false, false, Some("On fusionne ?")), ChatState::Failed);
        assert_eq!(state_after_turn(false, true, Some("On fusionne ?")), ChatState::Interrupted);
    }

    #[test]
    fn a_question_followed_by_a_sentence_still_waits_for_a_reply() {
        // Recette du 21/10 (N10, partiel) : la question n'est pas la dernière phrase.
        for text in [
            "Les deux PR sont prêtes, veux-tu que je fusionne ? Je reste en attente.",
            "Revue faite.\n\nJe valide et fusionne les deux tâches ✅ (squash — réglage par défaut du plugin —, branche supprimée, issue fermée), en commençant par #28, puis en résolvant le conflit de #29 comme décrit ? Aucune des deux n'est refusée, donc pas de demande de changements à envoyer.",
            "Tout est prêt. On fusionne #28 maintenant ?\n\nJ'attends ton feu vert.",
            "Les tests passent. Dis-moi si je peux fusionner, ou s'il faut reprendre #29.",
            "PR ouvertes.\n\nConfirme le périmètre et je lance le développement.",
            "Je m'arrête ici. Quelle option préfères-tu (A ou B) ? Je n'avance pas sans ta réponse.",
            "Deux pistes possibles. Laquelle ?\n\n*Je reste en attente.*",
            "Done. Should I merge both? I'll wait for your answer.",
        ] {
            assert!(awaits_reply(text), "{text}");
        }
        for text in [
            // Question rhétorique au milieu d'un long compte rendu final.
            "Sprint livré.\n\nJ'ai fusionné #28 puis #29. Pourquoi en squash ? Le plugin le règle ainsi. Le conflit sur saluer() est réglé. Les branches sont supprimées et les issues fermées. Tout est dans release/0.3.0.",
            "Pourquoi squash ? Parce que c'est le réglage du plugin.",
            "Fusion faite. Qui l'eût cru ? C'est passé du premier coup.",
            "Je confirme que les 14 tests passent. Les deux PR sont fusionnées.",
            "Le serveur répond bien. Rien d'autre à signaler.",
            "Les deux PR sont fusionnées.\n\nJe reste disponible.",
        ] {
            assert!(!awaits_reply(text), "{text}");
        }
        assert_eq!(sentences("A ou B ?) Fin. release/0.3.0 ok"), ["A ou B ?)", "Fin.", "release/0.3.0 ok"]);
    }

    #[test]
    fn a_turn_stopped_by_the_user_ends_as_stopped_not_failed() {
        // Recette du 21/10 (R05) : arrêt pendant un accord → `error_during_execution`.
        let (mut ok, mut interrupted, mut error) = (false, false, Some("Tour en échec (error_during_execution).".to_string()));
        stopped_turn(true, &mut ok, &mut interrupted, &mut error);
        assert_eq!((ok, interrupted, error.as_deref()), (false, true, None));
        assert_eq!(state_after_turn(ok, interrupted, None), ChatState::Interrupted);
        // Sans arrêt demandé, l'échec reste un échec ; un tour réussi reste réussi.
        let (mut ok, mut interrupted, mut error) = (false, false, Some("panne".to_string()));
        stopped_turn(false, &mut ok, &mut interrupted, &mut error);
        assert_eq!((ok, interrupted, error.as_deref()), (false, false, Some("panne")));
        let (mut ok, mut interrupted, mut error) = (true, false, None);
        stopped_turn(true, &mut ok, &mut interrupted, &mut error);
        assert_eq!((ok, interrupted, error), (true, false, None));
    }

    #[test]
    fn titles_come_from_the_first_line() {
        assert_eq!(title_of("\n  Prépare la v0.4  \nDétails"), "Prépare la v0.4");
        assert_eq!(title_of(&"a".repeat(80)).chars().count(), 60);
        assert_eq!(title_of("   "), "Conversation");
        assert!(new_id().starts_with('c'));
    }
}
