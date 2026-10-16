//! Historique du chat : `<données>/chat.sqlite3` (révision 1.1 : une seule base, plus de base
//! par espace). On enregistre nos événements normalisés (deltas exclus), pas les transcripts
//! des CLI, dont le format change.

use std::{path::Path, str::FromStr, time::Duration};

use serde::{Deserialize, Serialize};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use ts_rs::TS;

use crate::{
    ai::events::AiEvent,
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
    state TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
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
    WaitingApproval,
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
    pub state: ChatState,
    pub created_at: String,
    pub updated_at: String,
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
        Ok(Self { pool })
    }

    pub async fn create(&self, session: &ChatSessionRow) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO sessions (id, title, ai, model, native_id, origin, mode, state, created_at, updated_at)
             VALUES (?, ?, ?, ?, NULL, ?, ?, ?, ?, ?)",
        )
        .bind(&session.id)
        .bind(&session.title)
        .bind(kebab(&session.ai))
        .bind(&session.model)
        .bind(&session.origin)
        .bind(kebab(&session.mode))
        .bind(kebab(&session.state))
        .bind(&session.created_at)
        .bind(&session.updated_at)
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
            state: parse(&row.try_get::<String, _>("state")?)?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    /// Conversations, la plus récemment active d'abord.
    pub async fn list(&self) -> AppResult<Vec<ChatSessionRow>> {
        let rows = sqlx::query("SELECT * FROM sessions ORDER BY updated_at DESC, created_at DESC").fetch_all(&self.pool).await?;
        rows.iter().map(Self::row).collect()
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
            state: ChatState::Idle,
            created_at: now.clone(),
            updated_at: now,
        }
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
        assert_eq!(store.list().await.unwrap()[0].id, "a", "la plus récemment active d'abord");

        store.set_native("a", "native-1", Some("haiku")).await.unwrap();
        store.set_state("a", ChatState::Running).await.unwrap();
        assert_eq!(store.native_id("a").await.unwrap().as_deref(), Some("native-1"));
        store.reset_running().await.unwrap();
        let a = store.get("a").await.unwrap();
        assert_eq!((a.state, a.model.as_deref()), (ChatState::Interrupted, Some("haiku")));
        assert_eq!(store.rename("a", "  Nouveau titre ").await.unwrap().title, "Nouveau titre");
        assert!(store.rename("a", " ").await.is_err());

        store.delete("a").await.unwrap();
        assert!(store.get("a").await.is_err());
        assert!(store.events("a").await.unwrap().is_empty());
        assert_eq!(store.list().await.unwrap().len(), 1);
    }
}
