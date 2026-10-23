//! Miroir de `.cadence/config.toml`, versionné avec le dépôt. Aucun secret ici.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectConfig {
    pub repo: String,
    pub tag_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VersioningConfig {
    pub scheme: String,
    pub confirm_major_bump: bool,
    pub sprint_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IssuesConfig {
    pub require_milestone: bool,
    pub template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Config {
    pub project: ProjectConfig,
    pub versioning: VersioningConfig,
    pub issues: IssuesConfig,
}
