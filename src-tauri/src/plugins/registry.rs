//! Registre des plugins installés : `<données de l'app>/plugins/installed.json`.
//!
//! Révision 1.1 du plan : plus d'espace de travail, un plugin installé est activé globalement
//! (désactivable), et l'utilisateur ordonne le rail. Le fichier garde, par plugin, la version
//! installée, sa provenance et son activation, plus l'ordre du rail :
//!
//! ```json
//! { "plugins": { "gestion-projet": { "version": "0.1.0", "source": "catalog", "enabled": true } },
//!   "order": ["gestion-projet"] }
//! ```
//!
//! Les fichiers d'un plugin sont sous `<données>/plugins/<id>/<version>/` (ou `devPath` pour
//! un plugin de développement). L'installation elle-même arrive avec l'étape « Plugins ».

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::manifest::{self, Manifest, PluginCategory, PluginColor, PluginSecretInfo, PluginService, PluginViewInfo};
use crate::error::{AppError, AppResult};

/// Provenance d'un plugin installé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum PluginSource {
    Catalog,
    /// Dossier local non zippé (mode développement).
    Dev,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledEntry {
    pub version: String,
    /// Version précédente, gardée pour un retour arrière.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
    pub source: PluginSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev_path: Option<String>,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstalledFile {
    pub plugins: BTreeMap<String, InstalledEntry>,
    /// Ordre du rail (dans chaque catégorie) ; un plugin absent se range après, par nom.
    pub order: Vec<String>,
}

/// État d'un plugin installé vu par l'hôte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum PluginStatus {
    Ok,
    /// Un secret requis manque : la barre du plugin affiche « à configurer ».
    NeedsSetup,
    /// Manifeste absent ou invalide : jamais démarré.
    Invalid,
}

/// Plugin installé, tel que le rail et la barre du plugin l'affichent.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstalledPluginView {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub category: PluginCategory,
    pub color: PluginColor,
    /// Icône du kit pour la tuile du rail.
    pub rail_icon: String,
    pub views: Vec<PluginViewInfo>,
    pub secrets: Vec<PluginSecretInfo>,
    pub services: Vec<PluginService>,
    pub has_settings: bool,
    pub enabled: bool,
    pub source: PluginSource,
    pub status: PluginStatus,
    /// Erreur du manifeste (statut `invalid`), ou avertissements en mode dev.
    pub problem: Option<String>,
}

pub struct PluginRegistry {
    /// `<données de l'app>/plugins`.
    root: PathBuf,
    app_version: String,
    file: Mutex<InstalledFile>,
}

impl PluginRegistry {
    pub fn load(root: PathBuf, app_version: &str) -> Self {
        let path = root.join("installed.json");
        let file = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|err| {
                tracing::warn!(%err, path = %path.display(), "registre des plugins illisible, ignoré");
                InstalledFile::default()
            }),
            Err(_) => InstalledFile::default(),
        };
        Self { root, app_version: app_version.to_string(), file: Mutex::new(file) }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn app_version(&self) -> &str {
        &self.app_version
    }

    pub fn snapshot(&self) -> InstalledFile {
        self.file.lock().unwrap().clone()
    }

    /// Dossier des fichiers d'un plugin installé.
    pub fn plugin_dir(&self, id: &str, entry: &InstalledEntry) -> PathBuf {
        match (&entry.source, &entry.dev_path) {
            (PluginSource::Dev, Some(path)) => PathBuf::from(path),
            _ => self.root.join(id).join(&entry.version),
        }
    }

    /// Manifeste d'un plugin installé (erreur si absent ou invalide).
    pub fn manifest(&self, id: &str) -> AppResult<Manifest> {
        let entry = self.entry(id)?;
        manifest::load(&self.plugin_dir(id, &entry), &self.app_version)
    }

    fn entry(&self, id: &str) -> AppResult<InstalledEntry> {
        self.file
            .lock()
            .unwrap()
            .plugins
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::Validation(format!("Le plugin « {id} » n'est pas installé.")))
    }

    /// Ids dans l'ordre du rail : d'abord l'ordre choisi, puis les autres par id.
    pub fn ordered_ids(&self) -> Vec<String> {
        let file = self.file.lock().unwrap();
        let mut ids: Vec<String> = file.order.iter().filter(|id| file.plugins.contains_key(*id)).cloned().collect();
        let rest: Vec<String> = file.plugins.keys().filter(|id| !ids.contains(id)).cloned().collect();
        ids.extend(rest);
        ids
    }

    /// Ajoute ou remplace l'entrée d'un plugin (utilisé par l'installation et les tests).
    pub fn put(&self, id: &str, entry: InstalledEntry) -> AppResult<()> {
        self.change(|f| {
            f.plugins.insert(id.to_string(), entry);
            if !f.order.iter().any(|o| o == id) {
                f.order.push(id.to_string());
            }
            Ok(())
        })
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> AppResult<()> {
        self.change(|f| {
            let entry = f
                .plugins
                .get_mut(id)
                .ok_or_else(|| AppError::Validation(format!("Le plugin « {id} » n'est pas installé.")))?;
            entry.enabled = enabled;
            Ok(())
        })
    }

    /// Nouvel ordre du rail. Doit citer chaque plugin installé une seule fois ; l'ordre ne
    /// compte qu'à l'intérieur d'une catégorie (les catégories ont un ordre fixe).
    pub fn reorder(&self, order: Vec<String>) -> AppResult<()> {
        self.change(|f| {
            let mut sorted = order.clone();
            sorted.sort();
            sorted.dedup();
            let installed: Vec<&String> = f.plugins.keys().collect();
            if sorted.len() != order.len() || sorted.iter().collect::<Vec<_>>() != installed {
                return Err(AppError::Validation(
                    "L'ordre du rail doit citer chaque plugin installé exactement une fois.".into(),
                ));
            }
            f.order = order;
            Ok(())
        })
    }

    fn change(&self, edit: impl FnOnce(&mut InstalledFile) -> AppResult<()>) -> AppResult<()> {
        let mut guard = self.file.lock().unwrap();
        let mut next = guard.clone();
        edit(&mut next)?;
        crate::app_settings::write_atomic(&self.root.join("installed.json"), &next)?;
        *guard = next;
        Ok(())
    }

    /// Vue d'un plugin pour le front ; `has_secret` répond pour les secrets requis.
    pub fn view(&self, id: &str, has_secret: impl Fn(&str) -> bool) -> AppResult<InstalledPluginView> {
        let entry = self.entry(id)?;
        let dir = self.plugin_dir(id, &entry);
        Ok(match manifest::load(&dir, &self.app_version) {
            Ok(m) => {
                let missing = m.secrets.iter().any(|s| s.required && !has_secret(&s.id));
                let problem = (!m.warnings.is_empty() && entry.source == PluginSource::Dev).then(|| m.warnings.join(" "));
                InstalledPluginView {
                    rail_icon: m.rail_icon(),
                    has_settings: m.settings_schema.is_some(),
                    id: m.id,
                    name: m.name,
                    version: m.version,
                    description: m.description,
                    category: m.category,
                    color: m.color,
                    views: m.views,
                    secrets: m.secrets,
                    services: m.services,
                    enabled: entry.enabled,
                    source: entry.source,
                    status: if missing { PluginStatus::NeedsSetup } else { PluginStatus::Ok },
                    problem,
                }
            }
            Err(err) => InstalledPluginView {
                id: id.to_string(),
                name: id.to_string(),
                version: entry.version.clone(),
                description: String::new(),
                category: PluginCategory::Outils,
                color: PluginColor::derived(id),
                rail_icon: "warning".into(),
                views: vec![],
                secrets: vec![],
                services: vec![],
                has_settings: false,
                enabled: entry.enabled,
                source: entry.source,
                status: PluginStatus::Invalid,
                problem: Some(err.to_string()),
            },
        })
    }

    /// Tous les plugins installés, dans l'ordre du rail : catégories dans l'ordre fixe, puis
    /// ordre choisi par l'utilisateur.
    pub fn views(&self, has_secret: impl Fn(&str, &str) -> bool) -> Vec<InstalledPluginView> {
        let ids = self.ordered_ids();
        let mut views: Vec<(usize, InstalledPluginView)> = ids
            .iter()
            .enumerate()
            .filter_map(|(rank, id)| self.view(id, |secret| has_secret(id, secret)).ok().map(|v| (rank, v)))
            .collect();
        views.sort_by_key(|(rank, v)| (v.category, *rank));
        views.into_iter().map(|(_, v)| v).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_plugin(root: &Path, id: &str, category: &str, required_secret: bool) {
        let dir = root.join(id).join("0.1.0");
        fs::create_dir_all(&dir).unwrap();
        let secret = if required_secret { "[[secrets]]\nid = \"token\"\nlabel = \"Jeton\"\nrequired = true\n" } else { "" };
        fs::write(
            dir.join("plugin.toml"),
            format!(
                "api = 1\nid = \"{id}\"\nname = \"Plugin {id}\"\nversion = \"0.1.0\"\nauthors = [\"a\"]\nmin_cadence = \"0.1.0\"\ncategory = \"{category}\"\n\n[ui]\nentry = \"index.html\"\nviews =[{{ id = \"home\", title = \"Accueil\", icon = \"book\", home = true }}]\n{secret}"
            ),
        )
        .unwrap();
    }

    fn catalog() -> InstalledEntry {
        InstalledEntry { version: "0.1.0".into(), previous: None, source: PluginSource::Catalog, dev_path: None, enabled: true }
    }

    #[test]
    fn lists_by_category_then_chosen_order_and_persists() {
        let root = crate::app_settings::temp_dir("plugins-registry");
        write_plugin(&root, "documents", "etudes", false);
        write_plugin(&root, "biblio", "etudes", true);
        write_plugin(&root, "gestion-projet", "projet", false);
        let registry = PluginRegistry::load(root.clone(), "0.3.0");
        for id in ["documents", "biblio", "gestion-projet"] {
            registry.put(id, catalog()).unwrap();
        }
        // Le registre n'existe pas encore sur disque pour le dossier cassé.
        registry.put("casse", catalog()).unwrap();

        let ids = |r: &PluginRegistry| r.views(|_, _| false).into_iter().map(|v| v.id).collect::<Vec<_>>();
        assert_eq!(ids(&registry), ["gestion-projet", "documents", "biblio", "casse"]);

        registry.reorder(vec!["biblio".into(), "casse".into(), "documents".into(), "gestion-projet".into()]).unwrap();
        registry.set_enabled("documents", false).unwrap();

        let reloaded = PluginRegistry::load(root.clone(), "0.3.0");
        assert_eq!(ids(&reloaded), ["gestion-projet", "biblio", "documents", "casse"]);
        let views = reloaded.views(|_, _| false);
        assert!(!views.iter().find(|v| v.id == "documents").unwrap().enabled);
        let biblio = views.iter().find(|v| v.id == "biblio").unwrap();
        assert_eq!((biblio.status, biblio.rail_icon.as_str()), (PluginStatus::NeedsSetup, "book"));
        assert_eq!(reloaded.views(|_, _| true).iter().find(|v| v.id == "biblio").unwrap().status, PluginStatus::Ok);
        let casse = views.iter().find(|v| v.id == "casse").unwrap();
        assert_eq!(casse.status, PluginStatus::Invalid);
        assert!(casse.problem.is_some());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_unknown_plugins_and_incomplete_orders() {
        let root = crate::app_settings::temp_dir("plugins-registry-errors");
        let registry = PluginRegistry::load(root.clone(), "0.3.0");
        registry.put("documents", catalog()).unwrap();
        registry.put("biblio", catalog()).unwrap();
        assert!(registry.set_enabled("absent", false).is_err());
        assert!(registry.reorder(vec!["documents".into()]).is_err());
        assert!(registry.reorder(vec!["documents".into(), "documents".into()]).is_err());
        assert!(registry.reorder(vec!["documents".into(), "autre".into()]).is_err());
        assert_eq!(registry.ordered_ids(), ["documents", "biblio"]);
        let _ = fs::remove_dir_all(&root);
    }
}
