//! Surface IPC : une commande par intention métier, pas un miroir de l'API GitHub.
//! Le front ne compose jamais plusieurs appels pour obtenir une vue.
//!
//! Pour ajouter une commande :
//! 1. l'écrire ici dans le module du domaine (`AppResult<T>`, types de `crate::domain`) ;
//! 2. l'enregistrer dans `lib.rs` (`generate_handler!`) ;
//! 3. la déclarer dans `src/lib/ipc/commands.ts` et lui donner un mock dans `src/lib/ipc/mock`.

pub mod actions;
pub mod app;
pub mod auth;
pub mod chat;
pub mod issues;
pub mod plugins;
pub mod project;
pub mod prs;
pub mod repos;
pub mod tools;
pub mod versions;
