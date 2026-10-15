//! Cache local SQLite (prévu : `sqlx`).
//!
//! Le cache n'est jamais la vérité : une écriture part d'abord sur GitHub, puis met le
//! cache à jour. Une table par type d'objet GitHub (etag, updated_at, JSON brut), plus
//! `sync_state` (curseurs), `agent_runs` (journal) et `pending_actions` (file hors-ligne).

/// Migrations embarquées, appliquées dans l'ordre à l'ouverture du cache.
pub const MIGRATIONS: &[(&str, &str)] =
    &[("0001_init", include_str!("../../migrations/0001_init.sql"))];
