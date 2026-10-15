use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Ressource GitHub mise en cache (une table SQLite chacune).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum SyncResource {
    Milestones,
    Issues,
    PullRequests,
    Reviews,
    Labels,
    Releases,
}

/// Régime de synchronisation selon le quota restant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum RateLimitMode {
    /// > 1 000 requêtes restantes : sync toutes les 60 s.
    Normal,
    /// 200 – 1 000 : sync toutes les 5 min, préchargement coupé.
    Econome,
    /// < 200 : cache seul, agents suspendus.
    Gel,
}
