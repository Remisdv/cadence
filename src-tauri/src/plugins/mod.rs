//! Plugins de Cadence 2 (plan technique § 3, révision 1.1) : un plugin installé est activé
//! globalement, se range dans une catégorie du rail et garde ses réglages et secrets globaux.
//!
//! - `manifest`, `skills` : lecture et validation de `plugin.toml` et des `SKILL.md` ;
//! - `registry` : plugins installés, activation, ordre du rail (`installed.json`) ;
//! - `install` : installation d'une archive locale ou d'un dossier de dev, désinstallation ;
//! - `host`, `process`, `env` : un processus serveur MCP (stdio) par plugin ;
//! - `settings` : réglages globaux par plugin, validés par son schéma.
//!
//! Catalogue en ligne et passerelle MCP viennent aux étapes suivantes.

pub mod install;
pub mod manifest;
pub mod registry;
pub mod settings;
pub mod skills;
