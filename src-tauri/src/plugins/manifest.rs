//! Lecture de `plugin.toml` (API 1, plan Cadence 2 § 3.1, révision 1.1).
//!
//! Le socle ne lit que ce dont l'hôte a besoin pour afficher un plugin : identité, catégorie
//! du rail, vues, secrets déclarés, schéma de réglages. Le serveur MCP, les skills et les
//! actions sont lus tels quels et validés à l'étape « Plugins » (installation, hôte).
//! Une clé inconnue est ignorée (compatibilité ascendante dans l'API 1).

use std::{collections::HashSet, path::Path};

use semver::Version;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

/// Plus haute version de l'API plugins prise en charge.
pub const API_VERSION: u32 = 1;
pub const FILE_NAME: &str = "plugin.toml";

/// Catégorie du rail (liste fixe de la révision 1.1), dans l'ordre d'affichage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum PluginCategory {
    Projet,
    Etudes,
    Code,
    Donnees,
    Outils,
}

/// Couleur de la tuile du plugin dans le rail (couleurs sémantiques du kit).
///
/// Clé de manifeste `color` proposée en ajout au contrat ; absente, la couleur est tirée de
/// l'identifiant pour rester stable d'un lancement à l'autre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum PluginColor {
    Clay,
    Blue,
    Green,
    Purple,
    Amber,
    Red,
}

impl PluginColor {
    const ALL: [PluginColor; 6] =
        [PluginColor::Blue, PluginColor::Green, PluginColor::Purple, PluginColor::Amber, PluginColor::Red, PluginColor::Clay];

    pub fn derived(id: &str) -> Self {
        let hash = id.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
        Self::ALL[(hash % Self::ALL.len() as u32) as usize]
    }
}

/// Service de l'app activé par un plugin (`services = ["repos"]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum PluginService {
    /// Dépôts GitHub suivis, sélecteur en tête de la barre du plugin.
    Repos,
}

/// Vue déclarée par `[[ui.views]]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginViewInfo {
    pub id: String,
    pub title: String,
    /// Nom d'icône du kit (`Icon`).
    pub icon: String,
    /// Groupe dans la barre du plugin ; `None` = groupe sans titre.
    pub section: Option<String>,
    /// Routable mais absente de la barre (détail d'une issue, setup).
    pub hidden: bool,
    /// Vue d'accueil du plugin.
    pub home: bool,
}

/// Secret déclaré par `[[secrets]]` (valeur jamais lue par le front).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginSecretInfo {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub required: bool,
}

/// Manifeste validé.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    /// `icon` : chemin d'un SVG de l'archive (contrat), ou nom d'icône du kit.
    pub icon: Option<String>,
    pub min_cadence: String,
    pub category: PluginCategory,
    pub color: PluginColor,
    pub services: Vec<PluginService>,
    pub views: Vec<PluginViewInfo>,
    pub secrets: Vec<PluginSecretInfo>,
    /// Chemin relatif du JSON Schema des réglages.
    pub settings_schema: Option<String>,
    /// Clés ignorées ou valeurs ramenées à leur défaut, à montrer en mode dev.
    pub warnings: Vec<String>,
}

impl Manifest {
    /// Icône de la tuile du rail : `icon` quand c'est un nom d'icône du kit ; sinon (SVG, servi
    /// avec l'hôte d'iframe) celle de la vue d'accueil, à défaut de la première vue.
    pub fn rail_icon(&self) -> String {
        if let Some(icon) = self.icon.as_deref().filter(|i| !i.ends_with(".svg")) {
            return icon.to_string();
        }
        self.views
            .iter()
            .find(|v| v.home)
            .or_else(|| self.views.iter().find(|v| !v.hidden))
            .map(|v| v.icon.clone())
            .unwrap_or_else(|| "layers".into())
    }
}

#[derive(Deserialize)]
struct Raw {
    api: Option<u32>,
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    #[serde(default)]
    authors: Vec<String>,
    icon: Option<String>,
    min_cadence: Option<String>,
    category: Option<String>,
    color: Option<String>,
    #[serde(default)]
    services: Vec<String>,
    ui: Option<RawUi>,
    #[serde(default)]
    secrets: Vec<RawSecret>,
    settings: Option<RawSettings>,
}

#[derive(Deserialize)]
struct RawUi {
    #[serde(default)]
    views: Vec<RawView>,
}

#[derive(Deserialize)]
struct RawView {
    id: String,
    title: String,
    icon: String,
    section: Option<String>,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    home: bool,
}

#[derive(Deserialize)]
struct RawSecret {
    id: String,
    label: String,
    description: Option<String>,
    #[serde(default)]
    required: bool,
}

#[derive(Deserialize)]
struct RawSettings {
    schema: Option<String>,
}

/// `^[a-z0-9]+(-[a-z0-9]+)*$`, 2 à 24 caractères, `cadence` réservé au cœur.
pub fn is_plugin_id(id: &str) -> bool {
    (2..=24).contains(&id.len())
        && id != "cadence"
        && id.split('-').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
}

fn is_view_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::Validation(message.into())
}

/// Lit `plugin.toml` dans `dir`.
pub fn load(dir: &Path, app_version: &str) -> AppResult<Manifest> {
    let text = std::fs::read_to_string(dir.join(FILE_NAME))
        .map_err(|e| invalid(format!("{FILE_NAME} illisible ({e}).")))?;
    parse(&text, app_version)
}

/// Analyse et valide le texte d'un manifeste.
pub fn parse(text: &str, app_version: &str) -> AppResult<Manifest> {
    let raw: Raw = toml::from_str(text).map_err(|e| invalid(format!("{FILE_NAME} : {}", e.message())))?;
    let mut warnings = Vec::new();

    let api = raw.api.ok_or_else(|| invalid("Clé `api` absente."))?;
    if api > API_VERSION {
        return Err(invalid(format!("Plugin écrit pour l'API {api} : nécessite une version plus récente de Cadence.")));
    }
    if api == 0 {
        return Err(invalid("`api` doit valoir 1."));
    }

    let id = raw.id.ok_or_else(|| invalid("Clé `id` absente."))?;
    if !is_plugin_id(&id) {
        return Err(invalid(format!("« {id} » n'est pas un identifiant de plugin (minuscules, chiffres, tirets, 2 à 24 caractères).")));
    }
    let name = raw.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).ok_or_else(|| invalid("Clé `name` absente."))?;
    if name.chars().count() > 40 {
        return Err(invalid("`name` dépasse 40 caractères."));
    }
    let version = raw.version.ok_or_else(|| invalid("Clé `version` absente."))?;
    Version::parse(&version).map_err(|_| invalid(format!("`version` « {version} » n'est pas un semver.")))?;
    let description = raw.description.unwrap_or_default();
    if description.chars().count() > 200 {
        return Err(invalid("`description` dépasse 200 caractères."));
    }
    if raw.authors.is_empty() {
        warnings.push("`authors` vide.".into());
    }

    let min_cadence = raw.min_cadence.ok_or_else(|| invalid("Clé `min_cadence` absente."))?;
    let min = Version::parse(&min_cadence).map_err(|_| invalid(format!("`min_cadence` « {min_cadence} » n'est pas un semver.")))?;
    if let Ok(current) = Version::parse(app_version)
        && current < min
    {
        return Err(invalid(format!("Nécessite Cadence {min_cadence} ou plus récent (installé : {app_version}).")));
    }

    let category = match raw.category.as_deref() {
        Some(c) => serde_json::from_value(serde_json::Value::String(c.into()))
            .map_err(|_| invalid(format!("Catégorie « {c} » inconnue (projet, etudes, code, donnees, outils).")))?,
        None => {
            warnings.push("`category` absente : rangé dans « Outils ».".into());
            PluginCategory::Outils
        }
    };
    let color = match raw.color.as_deref() {
        Some(c) => serde_json::from_value(serde_json::Value::String(c.into())).unwrap_or_else(|_| {
            warnings.push(format!("Couleur « {c} » inconnue : couleur par défaut."));
            PluginColor::derived(&id)
        }),
        None => PluginColor::derived(&id),
    };

    let mut services = Vec::new();
    for s in raw.services {
        match s.as_str() {
            "repos" => services.push(PluginService::Repos),
            other => warnings.push(format!("Service « {other} » inconnu, ignoré.")),
        }
    }

    let mut seen = HashSet::new();
    let mut views = Vec::new();
    for v in raw.ui.map(|ui| ui.views).unwrap_or_default() {
        if !is_view_id(&v.id) {
            return Err(invalid(format!("Vue « {} » : identifiant invalide.", v.id)));
        }
        if !seen.insert(v.id.clone()) {
            return Err(invalid(format!("Vue « {} » déclarée deux fois.", v.id)));
        }
        views.push(PluginViewInfo {
            id: v.id,
            title: v.title,
            icon: v.icon,
            section: v.section.filter(|s| !s.trim().is_empty()),
            hidden: v.hidden,
            home: v.home,
        });
    }
    if views.iter().filter(|v| v.home).count() > 1 {
        return Err(invalid("Une seule vue peut porter `home = true`."));
    }

    let mut secrets = Vec::new();
    for s in raw.secrets {
        if !crate::secrets::is_secret_id(&s.id) {
            return Err(invalid(format!("Secret « {} » : identifiant invalide ([a-z0-9_]).", s.id)));
        }
        secrets.push(PluginSecretInfo { id: s.id, label: s.label, description: s.description, required: s.required });
    }

    Ok(Manifest {
        id,
        name,
        version,
        description,
        authors: raw.authors,
        icon: raw.icon.filter(|i| !i.trim().is_empty()),
        min_cadence,
        category,
        color,
        services,
        views,
        secrets,
        settings_schema: raw.settings.and_then(|s| s.schema),
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GESTION_PROJET: &str = r#"
api = 1
id = "gestion-projet"
name = "Gestion de projet"
version = "0.1.0"
description = "Sprints, issues, pull requests et versions d'un dépôt GitHub."
authors = ["Rémi Lhuillier"]
icon = "icon.svg"
min_cadence = "0.3.0"
category = "projet"
color = "blue"
services = ["repos"]
futur = "clé inconnue, ignorée"

[server]
bin = "cadence-gestion-projet"

[ui]
entry = "index.html"
views = [
  { id = "sprint", title = "Sprint en cours", icon = "sprint", section = "Pilotage", home = true },
  { id = "issues", title = "Issues", icon = "issue", section = "Travail" },
  { id = "issue", title = "Issue", icon = "issue", hidden = true },
]

[[secrets]]
id = "github_token"
label = "Jeton GitHub"
required = true

[settings]
schema = "settings.schema.json"
"#;

    #[test]
    fn reads_a_full_manifest() {
        let m = parse(GESTION_PROJET, "0.3.0").unwrap();
        assert_eq!(m.id, "gestion-projet");
        assert_eq!(m.category, PluginCategory::Projet);
        assert_eq!(m.color, PluginColor::Blue);
        assert_eq!(m.services, [PluginService::Repos]);
        assert_eq!(m.views.len(), 3);
        assert_eq!(m.views[0].section.as_deref(), Some("Pilotage"));
        assert!(m.views[2].hidden);
        assert_eq!(m.rail_icon(), "sprint", "icône SVG : celle de la vue d'accueil");
        let kit = parse(&GESTION_PROJET.replacen("icon = \"icon.svg\"", "icon = \"board\"", 1), "0.3.0").unwrap();
        assert_eq!(kit.rail_icon(), "board");
        assert_eq!(m.secrets[0].id, "github_token");
        assert!(m.secrets[0].required);
        assert_eq!(m.settings_schema.as_deref(), Some("settings.schema.json"));
        assert!(m.warnings.is_empty(), "{:?}", m.warnings);
    }

    #[test]
    fn defaults_category_and_color() {
        let text = "api = 1\nid = \"journal\"\nname = \"Journal\"\nversion = \"1.0.0\"\nauthors = [\"a\"]\nmin_cadence = \"0.3.0\"\n";
        let m = parse(text, "0.3.0").unwrap();
        assert_eq!(m.category, PluginCategory::Outils);
        assert_eq!(m.color, PluginColor::derived("journal"));
        assert_eq!(m.rail_icon(), "layers");
        assert_eq!(m.warnings.len(), 1);
    }

    #[test]
    fn rejects_invalid_manifests() {
        let with = |from: &str, to: &str| GESTION_PROJET.replacen(from, to, 1);
        for (text, why) in [
            (with("api = 1", "api = 2"), "API plus récente"),
            (with("id = \"gestion-projet\"", "id = \"Gestion\""), "identifiant"),
            (with("id = \"gestion-projet\"", "id = \"cadence\""), "réservé"),
            (with("version = \"0.1.0\"", "version = \"1.0\""), "semver"),
            (with("category = \"projet\"", "category = \"jeux\""), "catégorie"),
            (with("min_cadence = \"0.3.0\"", "min_cadence = \"9.0.0\""), "trop récent"),
            (with("id = \"issues\"", "id = \"sprint\""), "vue en double"),
            (with("hidden = true", "home = true"), "deux accueils"),
            (with("id = \"github_token\"", "id = \"GitHub\""), "secret"),
            ("api = 1\n[ui\n".to_string(), "toml cassé"),
        ] {
            assert!(parse(&text, "0.3.0").is_err(), "{why} devrait être refusé");
        }
    }

    #[test]
    fn checks_plugin_ids() {
        for ok in ["ab", "gestion-projet", "sql2", "a-b-c"] {
            assert!(is_plugin_id(ok), "{ok}");
        }
        for ko in ["a", "cadence", "-ab", "ab-", "a--b", "Ab", "a_b", "abcdefghijklmnopqrstuvwxy"] {
            assert!(!is_plugin_id(ko), "{ko}");
        }
    }
}
