//! Plugins de Cadence 2 (plan technique § 3, révision 1.1) : un plugin installé est activé
//! globalement, se range dans une catégorie du rail et garde ses réglages et secrets globaux.
//!
//! - `manifest` : lecture et validation de `plugin.toml` ;
//! - `registry` : plugins installés, activation, ordre du rail (`installed.json`) ;
//! - `settings` : réglages globaux par plugin, validés par son schéma.
//!
//! Catalogue, installation, hôte de processus MCP et passerelle viennent aux étapes suivantes.

pub mod manifest;
pub mod registry;
pub mod settings;
