//! Qui appelle la passerelle, et sous quels noms elle expose les outils.
//!
//! Chaque session d'IA (chat ou exécution de fond) reçoit son propre jeton porteur, passé à la
//! CLI par variable d'environnement (`CADENCE_MCP_TOKEN`) : la passerelle refuse toute requête
//! sans jeton connu (401) et rattache chaque appel à sa session. Seule l'empreinte sha256 d'un
//! jeton est gardée en mémoire.
//!
//! Noms (plan § 3.4) : un outil de plugin `[a-z0-9_]{1,40}` sans `__` est exposé sous
//! `<plugin>__<outil>` ; un outil marqué `_meta["cadence/visibility"] = "ui"` n'est jamais
//! exposé ; le préfixe `cadence__` est réservé aux outils du cœur.

use std::{collections::HashMap, sync::Mutex};

use base64::Engine;
use rmcp::model::Tool;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

/// Préfixe des outils servis par la passerelle elle-même.
pub const CORE: &str = "cadence";
/// Séparateur entre le plugin et l'outil.
pub const SEPARATOR: &str = "__";

/// Session d'IA autorisée à appeler la passerelle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionScope {
    /// Session de chat ou exécution de fond.
    pub session: String,
    /// Exécution de fond (`cadence__run_report` s'y rattache par défaut).
    pub run: Option<String>,
}

/// Jetons en cours de validité.
#[derive(Default)]
pub struct Tokens(Mutex<HashMap<String, SessionScope>>);

fn digest(token: &str) -> String {
    Sha256::digest(token.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

impl Tokens {
    /// Nouveau jeton pour une session ; les jetons précédents de la session restent valides
    /// jusqu'à `revoke` (une CLI relancée peut garder l'ancien le temps de s'arrêter).
    pub fn issue(&self, scope: SessionScope) -> SecretString {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).expect("générateur aléatoire du système indisponible");
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        self.0.lock().unwrap().insert(digest(&token), scope);
        SecretString::from(token)
    }

    /// Invalide tous les jetons d'une session (fin du chat ou de l'exécution).
    pub fn revoke(&self, session: &str) {
        self.0.lock().unwrap().retain(|_, scope| scope.session != session);
    }

    /// Session d'un en-tête `Authorization: Bearer <jeton>`.
    pub fn resolve(&self, authorization: &str) -> Option<SessionScope> {
        let token = authorization.strip_prefix("Bearer ").or_else(|| authorization.strip_prefix("bearer "))?.trim();
        if token.is_empty() {
            return None;
        }
        self.0.lock().unwrap().get(&digest(token)).cloned()
    }

    /// Pour les tests et les adaptateurs : valeur à placer dans `CADENCE_MCP_TOKEN`.
    pub fn header(token: &SecretString) -> String {
        format!("Bearer {}", token.expose_secret())
    }
}

/// `[a-z0-9_]{1,40}`, sans `__`.
pub fn is_tool_name(name: &str) -> bool {
    (1..=40).contains(&name.len())
        && !name.contains(SEPARATOR)
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Nom exposé d'un outil de plugin.
pub fn exposed(plugin: &str, tool: &str) -> String {
    format!("{plugin}{SEPARATOR}{tool}")
}

/// `(plugin, outil)` d'un nom exposé. Un id de plugin ne contient jamais `_` : le premier
/// `__` sépare sans ambiguïté.
pub fn split(name: &str) -> Option<(&str, &str)> {
    name.split_once(SEPARATOR).filter(|(plugin, tool)| !plugin.is_empty() && !tool.is_empty())
}

/// Outil réservé à l'interface du plugin.
pub fn is_ui_only(tool: &Tool) -> bool {
    tool.meta
        .as_ref()
        .and_then(|meta| meta.0.get("cadence/visibility"))
        .and_then(|v| v.as_str())
        .is_some_and(|v| v == "ui")
}

/// Outils d'un plugin tels que la passerelle les expose : renommés, outils `ui` et noms
/// invalides écartés (avec un avertissement pour ces derniers).
pub fn expose_plugin_tools(plugin: &str, tools: Vec<Tool>) -> Vec<Tool> {
    tools
        .into_iter()
        .filter(|tool| !is_ui_only(tool))
        .filter(|tool| {
            let ok = is_tool_name(&tool.name);
            if !ok {
                tracing::warn!(plugin, tool = %tool.name, "nom d'outil invalide, non exposé");
            }
            ok
        })
        .map(|mut tool| {
            tool.name = exposed(plugin, &tool.name).into();
            tool
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::model::MetaObject;
    use serde_json::json;

    use super::*;

    fn tool(name: &str, ui: bool) -> Tool {
        let mut tool = Tool::new(name.to_string(), "outil", Arc::new(serde_json::Map::new()));
        if ui {
            tool.meta = Some(MetaObject(json!({ "cadence/visibility": "ui" }).as_object().unwrap().clone()));
        }
        tool
    }

    #[test]
    fn issues_resolves_and_revokes_tokens() {
        let tokens = Tokens::default();
        let scope = SessionScope { session: "chat-1".into(), run: None };
        let token = tokens.issue(scope.clone());
        let other = tokens.issue(SessionScope { session: "run-2".into(), run: Some("run-2".into()) });
        assert_eq!(token.expose_secret().len(), 43, "32 octets en base64url");
        assert_eq!(tokens.resolve(&Tokens::header(&token)), Some(scope));
        assert_eq!(tokens.resolve(token.expose_secret()), None, "sans « Bearer »");
        assert_eq!(tokens.resolve("Bearer inconnu"), None);
        assert_eq!(tokens.resolve("Bearer "), None);
        tokens.revoke("chat-1");
        assert_eq!(tokens.resolve(&Tokens::header(&token)), None);
        assert_eq!(tokens.resolve(&Tokens::header(&other)).unwrap().run.as_deref(), Some("run-2"));
    }

    #[test]
    fn prefixes_and_filters_plugin_tools() {
        let tools = vec![tool("issue_get", false), tool("ui_sprint", true), tool("Mauvais", false), tool("a__b", false)];
        let names: Vec<String> = expose_plugin_tools("gestion-projet", tools).into_iter().map(|t| t.name.into_owned()).collect();
        assert_eq!(names, ["gestion-projet__issue_get"]);
        assert_eq!(split("gestion-projet__issue_get"), Some(("gestion-projet", "issue_get")));
        assert_eq!(split("cadence__run_report"), Some((CORE, "run_report")));
        assert_eq!(split("sans_separateur"), None);
        assert_eq!(split("__x"), None);
        assert!(is_tool_name(&"a".repeat(40)) && !is_tool_name(&"a".repeat(41)));
    }
}
