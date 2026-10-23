//! Passerelle MCP (plan § 2.2, § 3.4, révision 1.1) : un seul serveur MCP HTTP local qui
//! présente aux IA de code les outils de tous les plugins activés (`<plugin>__<outil>`) et les
//! outils du cœur (`cadence__*`). Chaque session d'IA s'y connecte avec son propre jeton.
//!
//! - `scope` : jetons de session, noms exposés, visibilité `ui` ;
//! - `server` : serveur HTTP (axum + streamable HTTP de rmcp), agrégation et routage ;
//! - `core_tools` : outils servis par la passerelle elle-même.

pub mod core_tools;
pub mod scope;
pub mod server;

use std::sync::Arc;

use crate::plugins::{manifest::PluginViewInfo, registry::PluginRegistry};

/// Plugin activé tel que la passerelle le voit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub views: Vec<PluginViewInfo>,
    /// Déclare un `[server]` : ses outils sont exposés.
    pub has_server: bool,
}

/// Plugins activés du registre dont le manifeste est valide, dans l'ordre du rail.
pub fn enabled_plugins(registry: &PluginRegistry) -> Vec<PluginSummary> {
    let installed = registry.snapshot();
    registry
        .ordered_ids()
        .into_iter()
        .filter(|id| installed.plugins.get(id).is_some_and(|e| e.enabled))
        .filter_map(|id| registry.manifest(&id).ok())
        .map(|m| PluginSummary { has_server: m.server.is_some(), id: m.id, name: m.name, description: m.description, views: m.views })
        .collect()
}

/// `PluginLister` adossé au registre.
pub fn registry_lister(registry: Arc<PluginRegistry>) -> server::PluginLister {
    Arc::new(move || enabled_plugins(&registry))
}
