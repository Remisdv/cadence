use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::version::Version;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Repo {
    /// `owner/name`
    pub full_name: String,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub private: bool,
    pub default_branch: String,
    pub open_issues: u32,
    /// Nombre de milestones (ouverts et fermés).
    pub milestones: u32,
    pub pushed_at: Option<String>,
}

/// État d'un projet ouvert : le dépôt et la version (sprint) en cours.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectState {
    pub repo: Repo,
    /// Titre semver du milestone en cours, ex. `0.1.2`.
    pub current_version: Option<String>,
    /// Vrai si `.cadence/config.toml` existe déjà dans le dépôt.
    pub has_config: bool,
}

/// Ce que Cadence trouve dans un dépôt avant de l'ouvrir (écran « Projet et mapping »).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectPreview {
    pub repo: Repo,
    /// Milestones semver importés comme versions, triés.
    pub versions: Vec<Version>,
    /// Version qui deviendra le sprint en cours.
    pub current_version: Option<String>,
    /// Titres de milestones ignorés (pas un numéro semver).
    pub ignored_milestones: Vec<String>,
    pub has_config: bool,
}
