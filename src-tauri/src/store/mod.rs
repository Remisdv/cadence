//! Cache local SQLite (`sqlx`), une base par dépôt.
//!
//! Le cache n'est jamais la vérité : une écriture part d'abord sur GitHub, puis met le
//! cache à jour. Une table par type d'objet GitHub (etag, updated_at, JSON brut), plus
//! `sync_state` (curseurs), `agent_runs` (journal) et `pending_actions` (file hors-ligne).
//! Il sert l'app hors ligne et au lancement, avant la première réponse de GitHub.

use std::{
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use serde::Deserialize;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    AssertSqlSafe, Row, SqlitePool,
};
use tokio::sync::Mutex;

use crate::{
    api::github::{issues::RawIssue, repos::Milestone},
    domain::sync::SyncResource,
    error::{AppError, AppResult},
};

/// Migrations embarquées, appliquées dans l'ordre à l'ouverture du cache.
/// La position dans la liste est le numéro de schéma (`PRAGMA user_version`).
pub const MIGRATIONS: &[(&str, &str)] =
    &[("0001_init", include_str!("../../migrations/0001_init.sql"))];

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Internal(format!("cache local : {err}"))
    }
}

/// Curseur de synchronisation d'une ressource (table `sync_state`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncCursor {
    /// Plus grand `updated_at` reçu : la prochaine passe demande `since=last_seen`.
    pub last_seen: Option<String>,
    /// ETag de la dernière réponse, valable pour la requête `since=last_seen`.
    pub etag: Option<String>,
}

/// Base du dépôt ouvert.
pub struct RepoCache {
    /// `owner/name`
    pub repo: String,
    pool: SqlitePool,
    /// Une seule synchronisation à la fois : la seconde attend puis trouve le cache frais.
    pub sync_lock: Mutex<()>,
}

impl RepoCache {
    /// Ouvre (ou crée) `<dir>/<owner>/<name>.sqlite3` et applique les migrations.
    pub async fn open(dir: &Path, repo: &str) -> AppResult<Self> {
        let path = db_path(dir, repo)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new().max_connections(4).connect_with(options).await?;
        Self::with_pool(repo, pool).await
    }

    /// Base en mémoire, pour les tests.
    pub async fn in_memory(repo: &str) -> AppResult<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        // Une seule connexion : chaque connexion `:memory:` aurait sa propre base.
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await?;
        Self::with_pool(repo, pool).await
    }

    async fn with_pool(repo: &str, pool: SqlitePool) -> AppResult<Self> {
        migrate(&pool).await?;
        Ok(Self { repo: repo.to_string(), pool, sync_lock: Mutex::new(()) })
    }

    /// Ferme les connexions (nécessaire avant de supprimer le fichier sous Windows).
    pub async fn close(&self) {
        self.pool.close().await;
    }

    /* ---------- Curseurs ---------- */

    pub async fn cursor(&self, resource: SyncResource) -> AppResult<SyncCursor> {
        let row = sqlx::query("SELECT last_seen, etag FROM sync_state WHERE resource = ?")
            .bind(resource_key(resource))
            .fetch_optional(&self.pool)
            .await?;
        Ok(row
            .map(|r| SyncCursor { last_seen: r.get("last_seen"), etag: r.get("etag") })
            .unwrap_or_default())
    }

    /// Enregistre le curseur et l'heure de la passe (réponse 304 comprise).
    pub async fn save_cursor(&self, resource: SyncResource, cursor: &SyncCursor) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO sync_state (resource, last_synced_at, last_seen, etag)
             VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), ?2, ?3)
             ON CONFLICT (resource) DO UPDATE SET
               last_synced_at = excluded.last_synced_at,
               last_seen = excluded.last_seen,
               etag = excluded.etag",
        )
        .bind(resource_key(resource))
        .bind(&cursor.last_seen)
        .bind(&cursor.etag)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Âge de la dernière passe sur `resource` ; `None` si elle n'a jamais été synchronisée.
    pub async fn since_last_sync(&self, resource: SyncResource) -> AppResult<Option<Duration>> {
        let seconds: Option<f64> = sqlx::query_scalar(
            "SELECT (julianday('now') - julianday(last_synced_at)) * 86400.0
             FROM sync_state WHERE resource = ? AND last_synced_at IS NOT NULL",
        )
        .bind(resource_key(resource))
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        Ok(seconds.map(|s| Duration::from_secs_f64(s.max(0.0))))
    }

    /* ---------- Issues ---------- */

    /// Issues en cache, la plus récemment modifiée d'abord.
    pub async fn issues(&self) -> AppResult<Vec<RawIssue>> {
        let rows = sqlx::query("SELECT raw FROM issues ORDER BY updated_at DESC, number DESC")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(|r| parse_raw(r.get("raw"))).collect()
    }

    /// Insère ou remplace des issues (charge GitHub brute + colonnes dénormalisées).
    pub async fn upsert_issues(&self, items: &[(RawIssue, serde_json::Value)]) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        for (issue, raw) in items {
            let labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
            sqlx::query(
                "INSERT OR REPLACE INTO issues
                   (number, title, state, milestone_number, labels, assignee, updated_at, raw)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(issue.number)
            .bind(&issue.title)
            .bind(&issue.state)
            .bind(issue.milestone.as_ref().map(|m| m.number))
            .bind(serde_json::to_string(&labels).unwrap_or_else(|_| "[]".into()))
            .bind(issue.assignees.first().map(|u| u.login.as_str()))
            .bind(&issue.updated_at)
            .bind(raw.to_string())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /* ---------- Milestones ---------- */

    pub async fn milestones(&self) -> AppResult<Vec<Milestone>> {
        let rows = sqlx::query("SELECT raw FROM milestones ORDER BY number")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(|r| parse_raw(r.get("raw"))).collect()
    }

    /// Remplace tous les milestones : la liste est petite et relue d'un bloc
    /// (un milestone supprimé sur GitHub disparaît du cache).
    pub async fn replace_milestones(&self, items: &[(Milestone, serde_json::Value)]) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM milestones").execute(&mut *tx).await?;
        for (milestone, raw) in items {
            sqlx::query(
                "INSERT INTO milestones (number, title, state, due_on, updated_at, raw)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(milestone.number)
            .bind(&milestone.title)
            .bind(&milestone.state)
            .bind(&milestone.due_on)
            .bind(milestone.updated_at.as_deref().unwrap_or(""))
            .bind(raw.to_string())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

/// Fichier de la base d'un dépôt. Refuse tout ce qui n'est pas `owner/name` GitHub
/// (le nom finit dans un chemin).
pub fn db_path(dir: &Path, repo: &str) -> AppResult<PathBuf> {
    let valid = |part: &str| {
        !part.is_empty()
            && !part.starts_with('.')
            && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    match repo.split_once('/') {
        Some((owner, name)) if valid(owner) && valid(name) => {
            Ok(dir.join(owner.to_lowercase()).join(format!("{}.sqlite3", name.to_lowercase())))
        }
        _ => Err(AppError::Validation(format!("« {repo} » n'est pas un dépôt owner/name."))),
    }
}

/// Applique les migrations pas encore passées, chacune dans sa transaction.
async fn migrate(pool: &SqlitePool) -> AppResult<()> {
    let current: i64 = sqlx::query_scalar("PRAGMA user_version").fetch_one(pool).await?;
    for (index, (name, sql)) in MIGRATIONS.iter().enumerate().skip(current.max(0) as usize) {
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(*sql).execute(&mut *tx).await?;
        sqlx::raw_sql(AssertSqlSafe(format!("PRAGMA user_version = {}", index + 1)))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        tracing::info!(migration = name, "cache local migré");
    }
    Ok(())
}

fn resource_key(resource: SyncResource) -> &'static str {
    match resource {
        SyncResource::Milestones => "milestones",
        SyncResource::Issues => "issues",
        SyncResource::PullRequests => "pull-requests",
        SyncResource::Reviews => "reviews",
        SyncResource::Labels => "labels",
        SyncResource::Releases => "releases",
    }
}

fn parse_raw<T: for<'de> Deserialize<'de>>(raw: &str) -> AppResult<T> {
    serde_json::from_str(raw).map_err(|err| AppError::Internal(format!("cache local illisible : {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn issue(number: u32, updated_at: &str, milestone: Option<u32>) -> (RawIssue, serde_json::Value) {
        let raw = json!({
            "number": number, "title": format!("Issue {number}"), "state": "open", "body": null,
            "labels": [{"name": "type/feat"}], "assignees": [{"login": "remisdv", "avatar_url": null}],
            "milestone": milestone.map(|n| json!({"number": n, "title": "0.1.2"})),
            "updated_at": updated_at, "html_url": "https://github.com/me/app/issues/1"
        });
        (serde_json::from_value(raw.clone()).unwrap(), raw)
    }

    #[tokio::test]
    async fn migrates_once_and_reopens() {
        let dir = std::env::temp_dir().join(format!("cadence-store-{}", std::process::id()));
        let cache = RepoCache::open(&dir, "Me/App").await.unwrap();
        cache.upsert_issues(&[issue(1, "2026-10-01T10:00:00Z", None)]).await.unwrap();
        cache.close().await;

        let reopened = RepoCache::open(&dir, "me/app").await.unwrap();
        assert_eq!(reopened.issues().await.unwrap().len(), 1);
        let version: i64 = sqlx::query_scalar("PRAGMA user_version").fetch_one(&reopened.pool).await.unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        reopened.close().await;
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn upserts_issues_and_keeps_the_raw_payload() {
        let cache = RepoCache::in_memory("me/app").await.unwrap();
        cache
            .upsert_issues(&[issue(1, "2026-10-01T10:00:00Z", Some(4)), issue(2, "2026-10-03T10:00:00Z", None)])
            .await
            .unwrap();
        let (mut edited, mut raw) = issue(1, "2026-10-05T10:00:00Z", Some(4));
        edited.title = "Renommée".into();
        raw["title"] = json!("Renommée");
        cache.upsert_issues(&[(edited, raw)]).await.unwrap();

        let issues = cache.issues().await.unwrap();
        assert_eq!(issues.iter().map(|i| i.number).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(issues[0].title, "Renommée");
        assert_eq!(issues[0].milestone.as_ref().map(|m| m.number), Some(4));
        let stored: String = sqlx::query_scalar("SELECT raw FROM issues WHERE number = 1")
            .fetch_one(&cache.pool)
            .await
            .unwrap();
        assert!(stored.contains("html_url"), "la charge brute est gardée entière");
    }

    #[tokio::test]
    async fn stores_cursors_and_their_age() {
        let cache = RepoCache::in_memory("me/app").await.unwrap();
        assert_eq!(cache.cursor(SyncResource::Issues).await.unwrap(), SyncCursor::default());
        assert_eq!(cache.since_last_sync(SyncResource::Issues).await.unwrap(), None);

        let cursor = SyncCursor { last_seen: Some("2026-10-05T10:00:00Z".into()), etag: Some("W/\"a\"".into()) };
        cache.save_cursor(SyncResource::Issues, &cursor).await.unwrap();
        assert_eq!(cache.cursor(SyncResource::Issues).await.unwrap(), cursor);
        let age = cache.since_last_sync(SyncResource::Issues).await.unwrap().unwrap();
        assert!(age < Duration::from_secs(5));
        assert_eq!(cache.since_last_sync(SyncResource::Milestones).await.unwrap(), None);
    }

    #[tokio::test]
    async fn replaces_milestones_as_a_whole() {
        let cache = RepoCache::in_memory("me/app").await.unwrap();
        let ms = |n: u32, title: &str| {
            let raw = json!({"number": n, "title": title, "state": "open", "description": null,
                "due_on": null, "open_issues": 1, "closed_issues": 0, "updated_at": "2026-10-01T10:00:00Z"});
            (serde_json::from_value::<Milestone>(raw.clone()).unwrap(), raw)
        };
        cache.replace_milestones(&[ms(1, "0.1.0"), ms(2, "0.1.1")]).await.unwrap();
        cache.replace_milestones(&[ms(2, "0.1.1")]).await.unwrap();
        let titles: Vec<_> = cache.milestones().await.unwrap().into_iter().map(|m| m.title).collect();
        assert_eq!(titles, ["0.1.1"]);
    }

    #[test]
    fn keeps_repository_names_inside_the_cache_directory() {
        let dir = Path::new("cache");
        assert_eq!(db_path(dir, "Cadence-App/desktop.rs").unwrap(), dir.join("cadence-app").join("desktop.rs.sqlite3"));
        for bad in ["../etc", "me/..", "me", "me/a/b", "me/a b", "/abs", "me/.hidden"] {
            assert!(db_path(dir, bad).is_err(), "{bad} devrait être refusé");
        }
    }
}
