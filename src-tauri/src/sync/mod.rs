//! Moteur de synchronisation (prévu : tâche `tokio` par projet).
//!
//! Trois déclencheurs :
//! - lancement / changement de projet : sync complète du milestone courant ;
//! - minuterie : sync incrémentale (`since` = dernier `updated_at` vu), intervalle
//!   donné par `RateLimitMode::sync_interval` ;
//! - après chaque écriture : invalidation ciblée de la ressource touchée.
//!
//! Chaque passe émet `sync://updated` avec les numéros modifiés, et
//! `sync://rate-limit` quand le régime change.
