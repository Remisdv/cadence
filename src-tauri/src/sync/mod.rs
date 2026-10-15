//! Moteur de synchronisation : GitHub → cache SQLite.
//!
//! Déclenché à la demande par les commandes de lecture (le front relit toutes les 60 s),
//! jamais plus d'une passe par ressource et par minute :
//! - issues : incrémental (`since` = dernier `updated_at` vu), requête conditionnelle
//!   (ETag) ; un 304 ne consomme pas de quota ;
//! - milestones : relus d'un bloc, conditionnellement.
//!
//! Prévu : tâche `tokio` par projet, intervalle donné par `RateLimitMode`, et
//! `sync://updated` émis avec les numéros modifiés.

use reqwest::Client;
use secrecy::SecretString;

use crate::{
    api::github::{
        issues::{self, IssuesFetch},
        repos::{self, MilestonesFetch},
    },
    domain::sync::SyncResource,
    error::{AppError, AppResult},
    state::AppState,
    store::{RepoCache, SyncCursor},
};

/// Passes de 10 pages enchaînées au plus par synchronisation (premier import d'un gros dépôt).
const MAX_ROUNDS: usize = 5;

/// Met `resource` à jour si la dernière passe date de plus de 60 s. Hors ligne, un cache
/// déjà rempli est servi tel quel plutôt que d'échouer.
pub async fn refresh(state: &AppState, cache: &RepoCache, resource: SyncResource) -> AppResult<()> {
    let _guard = cache.sync_lock.lock().await;
    let age = cache.since_last_sync(resource).await?;
    if age.is_some_and(|a| a < issues::REFRESH_EVERY) {
        return Ok(());
    }
    let token = state.github_token()?;
    let result = match resource {
        SyncResource::Issues => sync_issues(&state.http, &token, cache).await.map(drop),
        SyncResource::Milestones => sync_milestones(&state.http, &token, cache).await.map(drop),
        _ => Err(AppError::NotImplemented("synchronisation de cette ressource")),
    };
    match result {
        Err(AppError::Network(err)) if age.is_some() => {
            tracing::warn!(%err, ?resource, "GitHub injoignable, cache servi");
            Ok(())
        }
        other => other,
    }
}

/// Lit les issues modifiées depuis la dernière passe et les range dans le cache.
/// Retourne les numéros réellement modifiés.
pub async fn sync_issues(http: &Client, token: &SecretString, cache: &RepoCache) -> AppResult<Vec<u32>> {
    let mut cursor = cache.cursor(SyncResource::Issues).await?;
    let mut changed = Vec::new();
    for _ in 0..MAX_ROUNDS {
        let since = cursor.last_seen.clone();
        let fetch = issues::fetch_issues(http, token, &cache.repo, since.as_deref(), cursor.etag.as_deref()).await?;
        let IssuesFetch::Fresh { etag, issues, truncated } = fetch else {
            cache.save_cursor(SyncResource::Issues, &cursor).await?;
            break;
        };
        cache.upsert_issues(&issues).await?;
        changed.extend(
            issues.iter().filter(|(i, _)| Some(&i.updated_at) != since.as_ref()).map(|(i, _)| i.number),
        );
        cursor = advance(&cursor, issues.iter().map(|(i, _)| i.updated_at.as_str()), etag, truncated);
        cache.save_cursor(SyncResource::Issues, &cursor).await?;
        if !truncated {
            break;
        }
    }
    Ok(changed)
}

/// Curseur après une réponse. `since` est inclusif : la dernière issue vue revient à chaque
/// passe. L'ETag n'est gardé que si le curseur n'a pas bougé, car il vaut pour cette URL
/// précise : c'est ainsi qu'un dépôt calme répond 304 dès la troisième passe.
pub fn advance<'a>(
    cursor: &SyncCursor,
    updated: impl Iterator<Item = &'a str>,
    etag: Option<String>,
    truncated: bool,
) -> SyncCursor {
    let newest = updated.max().map(str::to_string);
    let last_seen = match (&cursor.last_seen, newest) {
        (Some(prev), Some(new)) if new.as_str() > prev.as_str() => Some(new),
        (None, new) => new,
        (prev, _) => prev.clone(),
    };
    let unchanged = last_seen == cursor.last_seen && !truncated;
    SyncCursor { etag: if unchanged { etag } else { None }, last_seen }
}

/// Relit les milestones (un appel, conditionnel). Retourne vrai s'ils ont changé.
pub async fn sync_milestones(http: &Client, token: &SecretString, cache: &RepoCache) -> AppResult<bool> {
    let cursor = cache.cursor(SyncResource::Milestones).await?;
    match repos::fetch_milestones(http, token, &cache.repo, cursor.etag.as_deref()).await? {
        MilestonesFetch::NotModified => {
            cache.save_cursor(SyncResource::Milestones, &cursor).await?;
            Ok(false)
        }
        MilestonesFetch::Fresh { etag, milestones } => {
            cache.replace_milestones(&milestones).await?;
            cache.save_cursor(SyncResource::Milestones, &SyncCursor { last_seen: None, etag }).await?;
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor(last_seen: Option<&str>, etag: Option<&str>) -> SyncCursor {
        SyncCursor { last_seen: last_seen.map(str::to_string), etag: etag.map(str::to_string) }
    }

    #[test]
    fn first_import_moves_the_cursor_and_drops_the_etag() {
        let next = advance(
            &SyncCursor::default(),
            ["2026-10-01T10:00:00Z", "2026-10-03T08:00:00Z"].into_iter(),
            Some("e1".into()),
            false,
        );
        assert_eq!(next, cursor(Some("2026-10-03T08:00:00Z"), None));
    }

    #[test]
    fn a_quiet_repository_keeps_the_etag_for_the_next_304() {
        let prev = cursor(Some("2026-10-03T08:00:00Z"), None);
        let next = advance(&prev, ["2026-10-03T08:00:00Z"].into_iter(), Some("e2".into()), false);
        assert_eq!(next, cursor(Some("2026-10-03T08:00:00Z"), Some("e2")));
    }

    #[test]
    fn a_truncated_read_never_keeps_its_etag() {
        let prev = cursor(Some("2026-10-03T08:00:00Z"), Some("e2"));
        let next = advance(&prev, std::iter::empty(), Some("e3".into()), true);
        assert_eq!(next, cursor(Some("2026-10-03T08:00:00Z"), None));
    }

    #[test]
    fn the_cursor_never_goes_back() {
        let prev = cursor(Some("2026-10-03T08:00:00Z"), None);
        let next = advance(&prev, ["2026-09-01T00:00:00Z"].into_iter(), None, false);
        assert_eq!(next.last_seen.as_deref(), Some("2026-10-03T08:00:00Z"));
    }
}
