//! Types du contrat front / back.
//!
//! Chaque type marqué `#[ts(export)]` est généré en TypeScript dans `src/lib/bindings`
//! (`pnpm bindings`). Règles :
//! - champs en camelCase côté TS (`#[serde(rename_all = "camelCase")]`) ;
//! - pas d'entier 64 bits dans le contrat (numéros d'issue/PR en `u32`) ;
//! - dates en chaînes ISO 8601 ;
//! - les structures d'entrée acceptent des champs absents (`#[serde(default)]` + `optional_fields`).

pub mod agent;
pub mod auth;
pub mod issue;
pub mod pr;
pub mod project;
pub mod settings;
pub mod sync;
pub mod version;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Personne GitHub telle qu'affichée (avatar + initiales en repli).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserRef {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    /// Vrai si c'est l'utilisateur connecté (affiché « vous »).
    pub is_self: bool,
}
