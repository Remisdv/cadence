use serde::{Deserialize, Serialize};
use ts_rs::TS;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum OwnerKind {
    User,
    Organization,
}

/// Compte sous lequel un dépôt peut être créé : l'utilisateur ou l'une de ses organisations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepoOwner {
    pub login: String,
    pub kind: OwnerKind,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, optional_fields)]
pub struct RepoCreateInput {
    /// Login de l'utilisateur ou de l'organisation propriétaire.
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub private: bool,
    /// Crée un premier commit (README) : la branche par défaut existe tout de suite.
    pub auto_init: bool,
}

impl RepoCreateInput {
    /// Mêmes règles que GitHub : lettres, chiffres, `-`, `_`, `.` ; 100 caractères au plus.
    pub fn validate(&self) -> crate::error::AppResult<()> {
        let name = self.name.trim();
        let valid = !name.is_empty()
            && name.len() <= 100
            && name != "."
            && name != ".."
            && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if !valid {
            return Err(crate::error::AppError::Validation(
                "Nom de dépôt invalide : lettres, chiffres, « - », « _ » et « . » uniquement.".into(),
            ));
        }
        if self.owner.trim().is_empty() {
            return Err(crate::error::AppError::Validation("Choisissez un propriétaire.".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_repository_names_like_github() {
        let input = |name: &str| RepoCreateInput {
            owner: "me".into(),
            name: name.into(),
            ..Default::default()
        };
        assert!(input("cadence-site").validate().is_ok());
        assert!(input("my_repo.v2").validate().is_ok());
        assert!(input("").validate().is_err());
        assert!(input("avec espace").validate().is_err());
        assert!(input("..").validate().is_err());
        let no_owner = RepoCreateInput { owner: " ".into(), name: "ok".into(), ..Default::default() };
        assert!(no_owner.validate().is_err());
    }
}
