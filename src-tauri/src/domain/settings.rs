//! Miroir de `.cadence/config.toml`, versionné avec le dépôt. Aucun secret ici.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::agent::Autonomy;

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
    pub send_to_cto_on_create: bool,
    pub template: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AgentEngine {
    ClaudeCode,
    Codex,
}

/// Moteur et modèle affectés à un agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EngineChoice {
    pub engine: AgentEngine,
    /// `opus`, `sonnet`… ; `None` = modèle par défaut du moteur.
    pub model: Option<String>,
}

impl EngineChoice {
    pub fn claude(model: &str) -> Self {
        Self { engine: AgentEngine::ClaudeCode, model: Some(model.to_string()) }
    }
}

/// Affectation des rôles faite à l'onboarding (préférence de la machine).
/// Sert de valeur par défaut au `config.toml` d'un nouveau projet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AgentEngines {
    pub cto: EngineChoice,
    pub ceo: EngineChoice,
    pub dev: EngineChoice,
    pub deploy: EngineChoice,
}

impl Default for AgentEngines {
    /// Le CEO tourne bien sur un modèle rapide, le développeur mérite le plus capable.
    fn default() -> Self {
        Self {
            cto: EngineChoice::claude("opus"),
            ceo: EngineChoice::claude("sonnet"),
            dev: EngineChoice::claude("opus"),
            deploy: EngineChoice::claude("sonnet"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentConfig {
    pub engine: AgentEngine,
    /// Modèle demandé au moteur (`opus`, `sonnet`…).
    pub model: Option<String>,
    pub autonomy: Autonomy,
    /// Plafond de durée d'une session, en minutes.
    pub max_session_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentsConfig {
    pub cto: AgentConfig,
    pub ceo: AgentConfig,
    pub dev: AgentConfig,
    pub deploy: AgentConfig,
    pub worktree_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Config {
    pub project: ProjectConfig,
    pub versioning: VersioningConfig,
    pub issues: IssuesConfig,
    pub agents: AgentsConfig,
}
