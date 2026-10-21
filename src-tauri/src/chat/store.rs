//! Historique du chat : `<données>/chat.sqlite3` (révision 1.1 : une seule base, plus de base
//! par espace). On enregistre nos événements normalisés (deltas exclus), pas les transcripts
//! des CLI, dont le format change.

use std::{path::Path, str::FromStr, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{
    AssertSqlSafe, Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use ts_rs::TS;

use crate::{
    ai::{claude_code::Permissions, events::AiEvent},
    app_settings::AiId,
    domain::version::now_iso,
    error::{AppError, AppResult},
};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    ai TEXT NOT NULL,
    model TEXT,
    native_id TEXT,
    origin TEXT NOT NULL,
    mode TEXT NOT NULL,
    permissions TEXT NOT NULL DEFAULT 'ask',
    state TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    group_id TEXT,
    group_label TEXT,
    hidden INTEGER NOT NULL DEFAULT 0,
    context TEXT
);
CREATE TABLE IF NOT EXISTS events (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    turn INTEGER NOT NULL,
    at TEXT NOT NULL,
    event TEXT NOT NULL,
    PRIMARY KEY (session_id, seq)
);
";

/// État d'une conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ChatState {
    Idle,
    Running,
    /// Une demande d'accord ou une question à choix attend l'utilisateur, en plein tour.
    WaitingApproval,
    /// Tour terminé sur une question posée en texte (« Réponds oui pour fusionner ») : la
    /// conversation attend la réponse de l'utilisateur (recette du 21/10, N10). Comme `Idle`,
    /// elle accepte un nouveau message ; elle survit au redémarrage de l'app. Le SDK des
    /// plugins la voit en `waiting-approval` avec `waiting: "reply"` (compatibilité).
    WaitingReply,
    /// Dernier tour en échec (connexion, quota, plantage).
    Failed,
    /// Dernier tour interrompu ; la conversation reste reprenable.
    Interrupted,
}

/// Mode d'une session : conversation, ou exécution de fond d'une action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ChatMode {
    Chat,
    Background,
}

/// Groupe d'une conversation lancée par une action (un sprint…). L'`id` est préfixé par l'id du
/// plugin (`<plugin>:<id du plugin>`) : un plugin ne voit que ses groupes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatGroup {
    pub id: String,
    pub label: String,
}

impl ChatGroup {
    /// Id d'un groupe tel que l'hôte l'enregistre : préfixé par l'id du plugin.
    pub fn scoped_id(plugin: &str, local: &str) -> String {
        format!("{plugin}:{local}")
    }

    /// Le même groupe vu par son plugin (id sans préfixe) ; `None` s'il ne lui appartient pas.
    pub fn local_to(&self, plugin: &str) -> Option<ChatGroup> {
        let local = self.id.strip_prefix(plugin)?.strip_prefix(':')?;
        Some(ChatGroup { id: local.to_string(), label: self.label.clone() })
    }
}

/// Plugin et action d'une origine `action:<plugin>/<action>`.
pub fn origin_action(origin: &str) -> Option<(&str, &str)> {
    origin.strip_prefix("action:")?.split_once('/')
}

/// Filtre de la liste des conversations. Sans `group` ni `hidden`, les conversations cachées
/// sont exclues.
#[derive(Debug, Clone, Copy, Default)]
pub struct SessionFilter<'a> {
    /// Conversations ouvertes par une action de ce plugin.
    pub plugin: Option<&'a str>,
    /// Groupe (id préfixé par le plugin) : ses conversations, cachées comprises.
    pub group: Option<&'a str>,
    /// Sans `group` : les conversations cachées aussi (recherche du chat, demandes en attente).
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatSessionRow {
    pub id: String,
    pub title: String,
    pub ai: AiId,
    pub model: Option<String>,
    /// `user`, ou `action:<plugin>/<action>`.
    pub origin: String,
    pub mode: ChatMode,
    /// Mode de la conversation (approbations de la CLI), modifiable entre deux tours.
    pub permissions: Permissions,
    pub state: ChatState,
    pub created_at: String,
    pub updated_at: String,
    /// Groupe de la conversation (sprint…), fixé par l'action qui l'a ouverte.
    pub group: Option<ChatGroup>,
    /// Absente du chat global (conversation de groupe) ; elle s'ouvre quand même dans le chat.
    pub hidden: bool,
    /// Contexte de l'action qui l'a ouverte (lu par son plugin, pas envoyé au chat).
    #[serde(skip)]
    #[ts(skip)]
    pub context: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatEventRow {
    pub seq: u32,
    pub turn: u32,
    pub at: String,
    pub event: AiEvent,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatSessionDetail {
    pub session: ChatSessionRow,
    pub events: Vec<ChatEventRow>,
    /// Worktrees ouverts à la session par `cadence__repo_checkout` (tant que l'app tourne).
    pub worktrees: Vec<String>,
}

fn kebab<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn parse<T: for<'de> Deserialize<'de>>(text: &str) -> AppResult<T> {
    serde_json::from_value(serde_json::Value::String(text.to_string()))
        .map_err(|_| AppError::Internal(format!("valeur inattendue dans l'historique du chat : {text}")))
}

pub struct ChatStore {
    pool: SqlitePool,
}

impl ChatStore {
    pub async fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        Self::with_options(options).await
    }

    pub async fn in_memory() -> AppResult<Self> {
        Self::with_options(SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true)).await
    }

    async fn with_options(options: SqliteConnectOptions) -> AppResult<Self> {
        // Une seule connexion : la base en mémoire est propre à sa connexion, et l'ordre des
        // écritures d'une session suit l'ordre des événements.
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await?;
        sqlx::raw_sql(SCHEMA).execute(&pool).await?;
        // Historique plus ancien : colonnes ajoutées au fil des versions. Le mode (0.3.0) arrive
        // avec « Demander » ; les groupes (0.3.2) avec des conversations visibles, sans groupe.
        for (column, definition) in [
            ("permissions", "TEXT NOT NULL DEFAULT 'ask'"),
            ("group_id", "TEXT"),
            ("group_label", "TEXT"),
            ("hidden", "INTEGER NOT NULL DEFAULT 0"),
            ("context", "TEXT"),
        ] {
            let present: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name = ?")
                .bind(column)
                .fetch_one(&pool)
                .await?;
            if present == 0 {
                // Noms et définitions constants, ci-dessus.
                sqlx::query(AssertSqlSafe(format!("ALTER TABLE sessions ADD COLUMN {column} {definition}"))).execute(&pool).await?;
            }
        }
        sqlx::query("CREATE INDEX IF NOT EXISTS sessions_group ON sessions(group_id)").execute(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn create(&self, session: &ChatSessionRow) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO sessions (id, title, ai, model, native_id, origin, mode, permissions, state, created_at, updated_at,
                                   group_id, group_label, hidden, context)
             VALUES (?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&session.id)
        .bind(&session.title)
        .bind(kebab(&session.ai))
        .bind(&session.model)
        .bind(&session.origin)
        .bind(kebab(&session.mode))
        .bind(kebab(&session.permissions))
        .bind(kebab(&session.state))
        .bind(&session.created_at)
        .bind(&session.updated_at)
        .bind(session.group.as_ref().map(|g| g.id.as_str()))
        .bind(session.group.as_ref().map(|g| g.label.as_str()))
        .bind(session.hidden)
        .bind(session.context.as_ref().map(Value::to_string))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    fn row(row: &sqlx::sqlite::SqliteRow) -> AppResult<ChatSessionRow> {
        Ok(ChatSessionRow {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            ai: parse(&row.try_get::<String, _>("ai")?)?,
            model: row.try_get("model")?,
            origin: row.try_get("origin")?,
            mode: parse(&row.try_get::<String, _>("mode")?)?,
            permissions: parse(&row.try_get::<String, _>("permissions")?).unwrap_or_default(),
            state: parse(&row.try_get::<String, _>("state")?)?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            group: match row.try_get::<Option<String>, _>("group_id")? {
                Some(id) => {
                    let label = row.try_get::<Option<String>, _>("group_label")?.unwrap_or_else(|| id.clone());
                    Some(ChatGroup { id, label })
                }
                None => None,
            },
            hidden: row.try_get("hidden")?,
            // Un contexte abîmé est ignoré, pas bloquant.
            context: row.try_get::<Option<String>, _>("context")?.and_then(|text| serde_json::from_str(&text).ok()),
        })
    }

    /// Conversations, la plus récemment active d'abord ; les cachées seulement avec un filtre
    /// `group` explicite.
    pub async fn list(&self, filter: SessionFilter<'_>) -> AppResult<Vec<ChatSessionRow>> {
        Ok(self.list_with_turns(filter).await?.into_iter().map(|(row, _)| row).collect())
    }

    /// Comme `list`, avec le nombre de tours de chaque conversation.
    pub async fn list_with_turns(&self, filter: SessionFilter<'_>) -> AppResult<Vec<(ChatSessionRow, u32)>> {
        let prefix = filter.plugin.map(|plugin| format!("action:{plugin}/"));
        let rows = sqlx::query(
            "SELECT s.*, (SELECT COALESCE(MAX(e.turn), 0) FROM events e WHERE e.session_id = s.id) AS turns
             FROM sessions s
             WHERE (CASE WHEN ?1 IS NULL THEN (?3 = 1 OR s.hidden = 0) ELSE s.group_id = ?1 END)
               AND (?2 IS NULL OR substr(s.origin, 1, length(?2)) = ?2)
             ORDER BY s.updated_at DESC, s.created_at DESC",
        )
        .bind(filter.group)
        .bind(prefix)
        .bind(filter.hidden)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| Ok((Self::row(row)?, u32::try_from(row.try_get::<i64, _>("turns")?).unwrap_or(u32::MAX))))
            .collect()
    }

    /// Nombre de tours d'une conversation.
    pub async fn turns(&self, id: &str) -> AppResult<u32> {
        Ok(self.next_turn(id).await?.saturating_sub(1))
    }

    pub async fn get(&self, id: &str) -> AppResult<ChatSessionRow> {
        let row = sqlx::query("SELECT * FROM sessions WHERE id = ?").bind(id).fetch_optional(&self.pool).await?;
        row.as_ref()
            .map(Self::row)
            .transpose()?
            .ok_or_else(|| AppError::Validation(format!("Conversation « {id} » introuvable.")))
    }

    pub async fn native_id(&self, id: &str) -> AppResult<Option<String>> {
        Ok(sqlx::query_scalar("SELECT native_id FROM sessions WHERE id = ?").bind(id).fetch_optional(&self.pool).await?.flatten())
    }

    pub async fn set_native(&self, id: &str, native_id: &str, model: Option<&str>) -> AppResult<()> {
        sqlx::query("UPDATE sessions SET native_id = ?, model = COALESCE(?, model) WHERE id = ?")
            .bind(native_id)
            .bind(model)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_state(&self, id: &str, state: ChatState) -> AppResult<()> {
        sqlx::query("UPDATE sessions SET state = ?, updated_at = ? WHERE id = ?")
            .bind(kebab(&state))
            .bind(now_iso())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn rename(&self, id: &str, title: &str) -> AppResult<ChatSessionRow> {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::Validation("Le titre est vide.".into()));
        }
        sqlx::query("UPDATE sessions SET title = ? WHERE id = ?").bind(title).bind(id).execute(&self.pool).await?;
        self.get(id).await
    }

    /// Change le mode ; le processus vivant est relancé au tour suivant (voir `ChatService`).
    pub async fn set_permissions(&self, id: &str, permissions: Permissions) -> AppResult<ChatSessionRow> {
        self.get(id).await?;
        sqlx::query("UPDATE sessions SET permissions = ? WHERE id = ?").bind(kebab(&permissions)).bind(id).execute(&self.pool).await?;
        self.get(id).await
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM events WHERE session_id = ?").bind(id).execute(&self.pool).await?;
        sqlx::query("DELETE FROM sessions WHERE id = ?").bind(id).execute(&self.pool).await?;
        Ok(())
    }

    /// Prochain numéro de tour d'une session.
    pub async fn next_turn(&self, id: &str) -> AppResult<u32> {
        let last: Option<i64> =
            sqlx::query_scalar("SELECT MAX(turn) FROM events WHERE session_id = ?").bind(id).fetch_one(&self.pool).await?;
        Ok(last.map(|t| t as u32 + 1).unwrap_or(1))
    }

    /// Ajoute un événement ; renvoie son numéro d'ordre.
    pub async fn append(&self, id: &str, turn: u32, event: &AiEvent) -> AppResult<ChatEventRow> {
        let at = now_iso();
        let text = serde_json::to_string(event).map_err(|e| AppError::Internal(e.to_string()))?;
        let seq: i64 = sqlx::query_scalar(
            "INSERT INTO events (session_id, seq, turn, at, event)
             VALUES (?, (SELECT COALESCE(MAX(seq), 0) + 1 FROM events WHERE session_id = ?), ?, ?, ?)
             RETURNING seq",
        )
        .bind(id)
        .bind(id)
        .bind(i64::from(turn))
        .bind(&at)
        .bind(text)
        .fetch_one(&self.pool)
        .await?;
        sqlx::query("UPDATE sessions SET updated_at = ? WHERE id = ?").bind(&at).bind(id).execute(&self.pool).await?;
        Ok(ChatEventRow { seq: seq as u32, turn, at, event: event.clone() })
    }

    pub async fn events(&self, id: &str) -> AppResult<Vec<ChatEventRow>> {
        let rows = sqlx::query("SELECT seq, turn, at, event FROM events WHERE session_id = ? ORDER BY seq")
            .bind(id)
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .filter_map(|row| {
                let text: String = row.try_get("event").ok()?;
                // Un événement d'un format plus récent ou abîmé est sauté, pas bloquant.
                let event = serde_json::from_str(&text).ok()?;
                Some(Ok(ChatEventRow {
                    seq: row.try_get::<i64, _>("seq").ok()? as u32,
                    turn: row.try_get::<i64, _>("turn").ok()? as u32,
                    at: row.try_get("at").ok()?,
                    event,
                }))
            })
            .collect()
    }

    /// Au lancement : une session restée « en cours » a perdu son processus.
    pub async fn reset_running(&self) -> AppResult<()> {
        sqlx::query("UPDATE sessions SET state = 'interrupted' WHERE state IN ('running', 'waiting-approval')")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str) -> ChatSessionRow {
        let now = now_iso();
        ChatSessionRow {
            id: id.into(),
            title: "Bonjour".into(),
            ai: AiId::ClaudeCode,
            model: None,
            origin: "user".into(),
            mode: ChatMode::Chat,
            permissions: Permissions::Ask,
            state: ChatState::Idle,
            created_at: now.clone(),
            updated_at: now,
            group: None,
            hidden: false,
            context: None,
        }
    }

    fn grouped(id: &str, origin: &str, group: Option<(&str, &str)>, hidden: bool) -> ChatSessionRow {
        ChatSessionRow {
            origin: origin.into(),
            group: group.map(|(id, label)| ChatGroup { id: id.into(), label: label.into() }),
            hidden,
            context: Some(serde_json::json!({ "type": "issue", "issue": 42 })),
            ..session(id)
        }
    }

    #[tokio::test]
    async fn hides_group_conversations_unless_the_group_is_asked() {
        let store = ChatStore::in_memory().await.unwrap();
        store.create(&session("libre")).await.unwrap();
        let sprint = Some(("gestion-projet:sprint:o/n:3", "Sprint 3 · o/n"));
        store.create(&grouped("cachee", "action:gestion-projet/developper-issue", sprint, true)).await.unwrap();
        store.create(&grouped("visible", "action:gestion-projet/developper-issue", sprint, false)).await.unwrap();
        store.create(&grouped("autre", "action:documents/fiche", Some(("documents:sprint:o/n:3", "Autre")), true)).await.unwrap();
        store.create(&grouped("sans-groupe", "action:gestion-projet/estimer", None, false)).await.unwrap();
        store.append("cachee", 1, &AiEvent::UserMessage { text: "go".into() }).await.unwrap();
        store.append("cachee", 2, &AiEvent::UserMessage { text: "encore".into() }).await.unwrap();

        let ids = |rows: Vec<ChatSessionRow>| {
            let mut ids: Vec<String> = rows.into_iter().map(|r| r.id).collect();
            ids.sort();
            ids
        };
        assert_eq!(ids(store.list(SessionFilter::default()).await.unwrap()), ["libre", "sans-groupe", "visible"], "chat global");
        let all = SessionFilter { hidden: true, ..SessionFilter::default() };
        assert_eq!(ids(store.list(all).await.unwrap()), ["autre", "cachee", "libre", "sans-groupe", "visible"], "cachées comprises sur demande");
        let group = SessionFilter { group: Some("gestion-projet:sprint:o/n:3"), ..SessionFilter::default() };
        assert_eq!(ids(store.list(group).await.unwrap()), ["cachee", "visible"], "groupe explicite : cachées comprises");
        let plugin = SessionFilter { plugin: Some("gestion-projet"), ..SessionFilter::default() };
        assert_eq!(ids(store.list(plugin).await.unwrap()), ["sans-groupe", "visible"]);
        // Isolation : le groupe d'un autre plugin, même demandé, reste vide pour ce plugin.
        let foreign = SessionFilter { plugin: Some("gestion-projet"), group: Some("documents:sprint:o/n:3"), hidden: false };
        assert!(store.list(foreign).await.unwrap().is_empty());
        let documents = SessionFilter { plugin: Some("documents"), group: Some("gestion-projet:sprint:o/n:3"), hidden: false };
        assert!(store.list(documents).await.unwrap().is_empty());

        let with_turns = store.list_with_turns(group).await.unwrap();
        let cachee = with_turns.iter().find(|(r, _)| r.id == "cachee").unwrap();
        assert_eq!(cachee.1, 2);
        assert_eq!(cachee.0.group.as_ref().unwrap().label, "Sprint 3 · o/n");
        assert_eq!(cachee.0.context, Some(serde_json::json!({ "type": "issue", "issue": 42 })));
        assert!(store.get("cachee").await.unwrap().hidden);
        assert_eq!(store.turns("visible").await.unwrap(), 0);
    }

    #[test]
    fn scopes_group_ids_by_plugin() {
        let id = ChatGroup::scoped_id("gestion-projet", "sprint:o/n:3");
        assert_eq!(id, "gestion-projet:sprint:o/n:3");
        let group = ChatGroup { id, label: "Sprint 3".into() };
        assert_eq!(group.local_to("gestion-projet").unwrap().id, "sprint:o/n:3");
        assert!(group.local_to("gestion").is_none(), "préfixe exact, pas un début d'id");
        assert!(group.local_to("documents").is_none());
        assert_eq!(origin_action("action:gestion-projet/developper-issue"), Some(("gestion-projet", "developper-issue")));
        assert_eq!(origin_action("user"), None);
    }

    #[tokio::test]
    async fn stores_sessions_and_events() {
        let store = ChatStore::in_memory().await.unwrap();
        store.create(&session("a")).await.unwrap();
        store.create(&session("b")).await.unwrap();
        assert_eq!(store.next_turn("a").await.unwrap(), 1);
        let first = store.append("a", 1, &AiEvent::UserMessage { text: "salut".into() }).await.unwrap();
        let second = store.append("a", 1, &AiEvent::MessageCompleted { message: "m:0".into(), text: "bonjour".into() }).await.unwrap();
        assert_eq!((first.seq, second.seq), (1, 2));
        assert_eq!(store.next_turn("a").await.unwrap(), 2);
        assert_eq!(store.events("a").await.unwrap().len(), 2);
        assert_eq!(store.list(SessionFilter::default()).await.unwrap()[0].id, "a", "la plus récemment active d'abord");

        store.set_native("a", "native-1", Some("haiku")).await.unwrap();
        store.set_state("a", ChatState::Running).await.unwrap();
        assert_eq!(store.native_id("a").await.unwrap().as_deref(), Some("native-1"));
        store.reset_running().await.unwrap();
        let a = store.get("a").await.unwrap();
        assert_eq!((a.state, a.model.as_deref()), (ChatState::Interrupted, Some("haiku")));
        assert_eq!(store.rename("a", "  Nouveau titre ").await.unwrap().title, "Nouveau titre");
        assert!(store.rename("a", " ").await.is_err());
        assert_eq!(store.get("a").await.unwrap().permissions, Permissions::Ask);
        assert_eq!(store.set_permissions("a", Permissions::AutoEdit).await.unwrap().permissions, Permissions::AutoEdit);
        assert!(store.set_permissions("absente", Permissions::Full).await.is_err());

        store.delete("a").await.unwrap();
        assert!(store.get("a").await.is_err());
        assert!(store.events("a").await.unwrap().is_empty());
        assert_eq!(store.list(SessionFilter::default()).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn adds_the_mode_and_group_columns_to_an_older_history() {
        let dir = crate::app_settings::temp_dir("chat-store-migration");
        let path = dir.join("chat.sqlite3");
        std::fs::create_dir_all(&dir).unwrap();
        {
            let options = SqliteConnectOptions::new().filename(&path).create_if_missing(true);
            let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap();
            // Table `sessions` de la 0.3.0, sans la colonne du mode.
            sqlx::raw_sql(
                "CREATE TABLE sessions (
                    id TEXT PRIMARY KEY, title TEXT NOT NULL, ai TEXT NOT NULL, model TEXT,
                    native_id TEXT, origin TEXT NOT NULL, mode TEXT NOT NULL, state TEXT NOT NULL,
                    created_at TEXT NOT NULL, updated_at TEXT NOT NULL
                )",
            )
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO sessions (id, title, ai, origin, mode, state, created_at, updated_at)
                 VALUES ('a', 'Ancienne', 'claude-code', 'user', 'chat', 'idle', '2026-10-01', '2026-10-01')",
            )
            .execute(&pool)
            .await
            .unwrap();
            pool.close().await;
        }
        let store = ChatStore::open(&path).await.unwrap();
        let old = store.get("a").await.unwrap();
        assert_eq!(old.permissions, Permissions::Ask, "« Demander » par défaut");
        assert_eq!((old.group, old.hidden, old.context), (None, false, None), "colonnes des groupes ajoutées, visible");
        assert_eq!(store.list(SessionFilter::default()).await.unwrap().len(), 1, "reste dans le chat global");
        store.set_permissions("a", Permissions::Full).await.unwrap();
        let mut grouped = session("b");
        grouped.group = Some(ChatGroup { id: "gp:sprint:1".into(), label: "Sprint 1".into() });
        grouped.hidden = true;
        store.create(&grouped).await.unwrap();
        drop(store);
        let store = ChatStore::open(&path).await.unwrap();
        assert_eq!(store.get("a").await.unwrap().permissions, Permissions::Full, "mémorisé");
        assert_eq!(store.get("b").await.unwrap().group.unwrap().label, "Sprint 1", "migration rejouée sans perte");
        drop(store);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
