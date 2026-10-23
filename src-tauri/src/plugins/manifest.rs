//! Lecture de `plugin.toml` (API 1, plan Cadence 2 § 3.1, révision 1.1).
//!
//! `parse` valide le texte seul : identité, catégorie du rail, vues, secrets, `[server]`,
//! `[[actions]]`, `[[setup]]`. `load` lit en plus le dossier du plugin et applique les règles
//! « (R) » qui portent sur ses fichiers : skills (`skills/<s>/SKILL.md`), skill de chaque
//! action, binaire du serveur. Une clé inconnue est ignorée (compatibilité ascendante dans
//! l'API 1).

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use semver::Version;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::skills::{self, SkillInfo};
use crate::{
    error::{AppError, AppResult},
    tools::ToolId,
};

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

/// Programme de la machine demandé par un plugin (`[[requires]]`, registre d'outils).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRequirement {
    pub id: ToolId,
    /// Optionnel : son absence ne bloque pas le plugin.
    pub optional: bool,
    /// Le plugin a besoin d'un compte connecté (`gh auth status`…).
    pub auth: bool,
    /// Pourquoi, affiché à côté de l'outil.
    pub reason: Option<String>,
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

/// Les quatre cibles de compilation des binaires de plugin (`bin/<cible>/`).
pub const TARGETS: [&str; 4] =
    ["x86_64-pc-windows-msvc", "aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu"];

/// Nom court de chaque cible, utilisé par le dépôt de plugins (`bin/windows-x86_64/`).
pub const SHORT_TARGETS: [&str; 4] = ["windows-x86_64", "macos-aarch64", "macos-x86_64", "linux-x86_64"];

/// Cible sous sa forme longue, qu'elle soit donnée courte ou longue.
pub fn canonical_target(target: &str) -> Option<&'static str> {
    TARGETS.iter().zip(SHORT_TARGETS).find(|(long, short)| **long == target || *short == target).map(|(long, _)| *long)
}

/// Système et variante lisibles d'une cible (« macOS », « Apple Silicon »).
fn target_parts(target: &str) -> (&'static str, &'static str) {
    match canonical_target(target) {
        Some("x86_64-pc-windows-msvc") => ("Windows", "x64"),
        Some("aarch64-apple-darwin") => ("macOS", "Apple Silicon"),
        Some("x86_64-apple-darwin") => ("macOS", "Intel"),
        Some("x86_64-unknown-linux-gnu") => ("Linux", "x64"),
        _ => ("", ""),
    }
}

/// Nom lisible d'une cible (recette du 21/10, R08) : « Windows x64 », « macOS (Apple
/// Silicon) », « Linux x64 » ; une cible inconnue garde son nom.
pub fn target_label(target: &str) -> String {
    match target_parts(target) {
        ("", _) => target.to_string(),
        ("macOS", variant) => format!("macOS ({variant})"),
        (os, variant) => format!("{os} {variant}"),
    }
}

/// Liste lisible de cibles, les variantes d'un même système réunies : « macOS (Apple
/// Silicon, Intel) et Linux x64 ».
pub fn targets_label(targets: &[&str]) -> String {
    let mut groups: Vec<(&str, Vec<String>)> = Vec::new();
    for target in targets {
        let (os, variant) = target_parts(target);
        match groups.iter_mut().find(|(o, _)| !os.is_empty() && *o == os) {
            Some((_, variants)) => variants.push(variant.to_string()),
            None if os.is_empty() => groups.push(("", vec![target.to_string()])),
            None => groups.push((os, vec![variant.to_string()])),
        }
    }
    let names: Vec<String> = groups
        .into_iter()
        .map(|(os, variants)| match os {
            "" => variants.join(", "),
            "macOS" => format!("macOS ({})", variants.join(", ")),
            _ => format!("{os} {}", variants.join(", ")),
        })
        .collect();
    match names.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} et {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// Cible de la machine courante, `None` hors des quatre cibles prises en charge.
pub fn current_target() -> Option<&'static str> {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some(TARGETS[0])
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some(TARGETS[1])
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some(TARGETS[2])
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some(TARGETS[3])
    } else {
        None
    }
}

/// Arrêt sur inactivité par défaut (secondes).
pub const DEFAULT_IDLE_SHUTDOWN_S: u32 = 600;

/// Serveur MCP déclaré par `[server]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSpec {
    /// Nom du binaire, sans extension.
    pub bin: String,
    /// Cibles annoncées (défaut : les quatre).
    pub targets: Vec<String>,
    pub args: Vec<String>,
    /// `0` = jamais.
    pub idle_shutdown_s: u32,
}

impl ServerSpec {
    /// `bin/<cible>/<bin>[.exe]` dans le dossier du plugin.
    /// Dossier long (`x86_64-pc-windows-msvc`) ou court (`windows-x86_64`, celui des archives
    /// du dépôt de plugins) : le premier qui contient le binaire, le long à défaut.
    pub fn binary(&self, dir: &Path, target: &str) -> PathBuf {
        let name = if target.contains("windows") { format!("{}.exe", self.bin) } else { self.bin.clone() };
        let long = canonical_target(target).unwrap_or(target);
        let short = TARGETS.iter().position(|t| *t == long).map(|i| SHORT_TARGETS[i]);
        let long_path = dir.join("bin").join(long).join(&name);
        match short.map(|s| dir.join("bin").join(s).join(&name)) {
            Some(short_path) if !long_path.is_file() && short_path.is_file() => short_path,
            _ => long_path,
        }
    }
}

/// Mode de lancement d'une action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ActionMode {
    /// Nouvelle session de chat ouverte dans le shell.
    Chat,
    /// Exécution de fond, visible dans le panneau Activité.
    Background,
}

/// Bouton déclaré par `[[actions]]` : un skill lancé avec un contexte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginActionInfo {
    pub id: String,
    pub label: String,
    /// Dossier sous `skills/`.
    pub skill: String,
    pub mode: ActionMode,
    /// Type de contexte attendu (`issue`, `version`…) ; `None` = action globale.
    pub context: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    /// Texte d'une confirmation avant lancement.
    pub confirm: Option<String>,
}

/// Étape de configuration déclarée par `[[setup]]`, montrée par l'onboarding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginSetupStep {
    pub id: String,
    pub title: String,
    /// Vue du plugin (`ui.views[].id`) qui mène l'étape.
    pub view: String,
    /// Étape bloquante de l'onboarding.
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
    /// Lien « Documentation » du catalogue (https).
    pub homepage: Option<String>,
    pub category: PluginCategory,
    pub color: PluginColor,
    pub services: Vec<PluginService>,
    /// Programmes de la machine dont le plugin a besoin (`[[requires]]`).
    pub tools: Vec<ToolRequirement>,
    pub server: Option<ServerSpec>,
    /// Fichier HTML de l'UI, sous `ui/` (obligatoire dès qu'il y a une section `[ui]`).
    pub ui_entry: Option<String>,
    /// `ui.dev_url` : serveur de dev local de l'UI (`http://localhost:<port>`), utilisé
    /// seulement quand le plugin est chargé en mode développement.
    pub ui_dev_url: Option<String>,
    pub views: Vec<PluginViewInfo>,
    pub secrets: Vec<PluginSecretInfo>,
    pub setup: Vec<PluginSetupStep>,
    pub actions: Vec<PluginActionInfo>,
    /// Skills du dossier `skills/`, lus par `load` (vide après `parse` seul).
    pub skills: Vec<SkillInfo>,
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
    homepage: Option<String>,
    category: Option<String>,
    color: Option<String>,
    #[serde(default)]
    services: Vec<String>,
    #[serde(default)]
    requires: Vec<RawRequire>,
    server: Option<RawServer>,
    ui: Option<RawUi>,
    #[serde(default)]
    secrets: Vec<RawSecret>,
    #[serde(default)]
    setup: Vec<RawSetup>,
    #[serde(default)]
    actions: Vec<RawAction>,
    settings: Option<RawSettings>,
}

#[derive(Deserialize)]
struct RawRequire {
    tool: String,
    #[serde(default)]
    optional: bool,
    #[serde(default)]
    auth: bool,
    reason: Option<String>,
}

#[derive(Deserialize)]
struct RawServer {
    bin: String,
    targets: Option<Vec<String>>,
    #[serde(default)]
    args: Vec<String>,
    idle_shutdown_s: Option<u32>,
}

#[derive(Deserialize)]
struct RawUi {
    entry: Option<String>,
    /// Serveur de dev de l'UI (Vite…), servi à la place de `ui/` pour un plugin de dev.
    dev_url: Option<String>,
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
struct RawSetup {
    id: String,
    title: String,
    view: String,
    #[serde(default)]
    required: bool,
}

#[derive(Deserialize)]
struct RawAction {
    id: String,
    label: String,
    skill: String,
    mode: String,
    context: Option<String>,
    description: Option<String>,
    icon: Option<String>,
    confirm: Option<String>,
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

/// Identifiant d'action ou d'étape : minuscules, chiffres, `-` et `_`.
fn is_item_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Nom de binaire : lettres, chiffres, `-`, `_`, `.` ; ni chemin ni extension `.exe`.
fn is_bin_name(bin: &str) -> bool {
    !bin.is_empty()
        && bin.len() <= 64
        && !bin.starts_with('.')
        && !bin.to_ascii_lowercase().ends_with(".exe")
        && bin.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Chemin relatif sous le dossier du plugin, sans `..` ni racine.
/// `http://localhost:<port>` ou `http://127.0.0.1:<port>`, chemin facultatif.
fn is_local_dev_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://localhost:").or_else(|| url.strip_prefix("http://127.0.0.1:")) else {
        return false;
    };
    let port = rest.split('/').next().unwrap_or_default();
    !port.is_empty() && port.parse::<u16>().is_ok()
}

fn is_relative_path(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty() && !p.is_absolute() && p.components().all(|c| matches!(c, std::path::Component::Normal(_)))
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::Validation(message.into())
}

/// Lit `plugin.toml` dans `dir`, puis ses skills, et vérifie les fichiers qu'il désigne.
///
/// Le binaire du serveur n'est exigé que pour la cible de la machine (un plugin de dev ou une
/// archive locale n'est souvent compilé que pour elle) ; `check_all_targets` signale les autres
/// à l'installation d'une archive.
pub fn load(dir: &Path, app_version: &str) -> AppResult<Manifest> {
    let text = std::fs::read_to_string(dir.join(FILE_NAME))
        .map_err(|e| invalid(format!("{FILE_NAME} illisible ({e}).")))?;
    let mut manifest = parse(&text, app_version)?;
    check_dir(dir, &mut manifest)?;
    Ok(manifest)
}

/// Règles « (R) » sur les fichiers du plugin.
fn check_dir(dir: &Path, m: &mut Manifest) -> AppResult<()> {
    m.skills = skills::load_all(&dir.join("skills"))?;
    for action in &m.actions {
        if !m.skills.iter().any(|s| s.name == action.skill) {
            return Err(invalid(format!(
                "Action « {} » : le skill « {} » n'existe pas sous `skills/`.",
                action.id, action.skill
            )));
        }
    }
    if let Some(server) = &m.server {
        match current_target() {
            Some(target) if server.targets.iter().any(|t| t == target) => {
                let bin = server.binary(dir, target);
                if !bin.is_file() {
                    return Err(invalid(format!("Binaire du serveur absent : `bin/{target}/{}`.", file_name(&bin))));
                }
            }
            Some(target) => m.warnings.push(format!(
                "Aucun binaire du serveur pour cette machine ({}) : le plugin n'y fonctionnera pas.",
                target_label(target)
            )),
            None => m.warnings.push("Aucun binaire du serveur pour cette machine : le plugin n'y fonctionnera pas.".into()),
        }
    }
    if let Some(entry) = &m.ui_entry
        && !dir.join("ui").join(entry).is_file()
    {
        m.warnings.push(format!("`ui/{entry}` absent."));
    }
    if let Some(icon) = m.icon.as_deref().filter(|i| i.ends_with(".svg")) {
        match std::fs::metadata(dir.join(icon)) {
            Ok(meta) if meta.len() > 64 * 1024 => m.warnings.push(format!("`{icon}` dépasse 64 Kio.")),
            Ok(_) => {}
            Err(_) => m.warnings.push(format!("Icône `{icon}` absente.")),
        }
    }
    Ok(())
}

/// Binaires des autres cibles annoncées (recette du 21/10, N02). Seul celui de la machine est
/// exigé, par `load` : une archive locale ou du catalogue construite pour cette machine
/// s'installe. Les cibles annoncées sans binaire reviennent en un avertissement, à montrer
/// après l'installation, avec des noms lisibles et la machine nommée (R08) : « Installé pour
/// Windows x64 (cette machine). Binaire du serveur absent pour macOS (Apple Silicon, Intel) et
/// Linux x64 : le plugin ne fonctionnera pas sur ces systèmes. »
pub fn check_all_targets(dir: &Path, m: &Manifest) -> Vec<String> {
    let Some(server) = &m.server else { return Vec::new() };
    let current = current_target();
    let missing: Vec<&str> = server
        .targets
        .iter()
        .map(String::as_str)
        .filter(|target| canonical_target(target) != current && !server.binary(dir, target).is_file())
        .collect();
    if missing.is_empty() {
        return Vec::new();
    }
    let here = current
        .filter(|c| server.targets.iter().any(|t| canonical_target(t) == Some(*c)))
        .map(|c| format!("Installé pour {} (cette machine). ", target_label(c)))
        .unwrap_or_default();
    let systems = if missing.len() > 1 { "ces systèmes" } else { "ce système" };
    vec![format!(
        "{here}Binaire du serveur absent pour {} : le plugin ne fonctionnera pas sur {systems}.",
        targets_label(&missing)
    )]
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
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

    let homepage = match raw.homepage.filter(|h| !h.trim().is_empty()) {
        Some(h) if h.starts_with("https://") => Some(h),
        Some(_) => {
            warnings.push("`homepage` doit être une URL https : ignorée.".into());
            None
        }
        None => None,
    };

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

    let mut tools: Vec<ToolRequirement> = Vec::new();
    for r in raw.requires {
        match ToolId::parse(&r.tool) {
            Some(id) if !tools.iter().any(|t| t.id == id) => tools.push(ToolRequirement {
                id,
                optional: r.optional,
                auth: r.auth,
                reason: r.reason.filter(|s| !s.trim().is_empty()),
            }),
            Some(_) => warnings.push(format!("Outil « {} » déclaré deux fois.", r.tool)),
            None => warnings.push(format!("Outil « {} » inconnu, ignoré.", r.tool)),
        }
    }

    let server = match raw.server {
        None => None,
        Some(srv) => Some(parse_server(srv)?),
    };

    let (ui_entry, ui_dev_url, raw_views) = match raw.ui {
        None => (None, None, Vec::new()),
        Some(ui) => {
            let entry = ui.entry.filter(|e| !e.trim().is_empty()).ok_or_else(|| invalid("`ui.entry` absent."))?;
            // Accepté depuis la racine du plugin (`ui/index.html`, forme du dépôt de plugins) ou
            // depuis `ui/` (`index.html`) ; gardé relatif à `ui/`.
            let entry = entry.strip_prefix("ui/").map(str::to_string).unwrap_or(entry);
            if !is_relative_path(&entry) {
                return Err(invalid(format!("`ui.entry` « {entry} » doit être un chemin relatif sous `ui/`.")));
            }
            let dev_url = ui.dev_url.map(|u| u.trim().trim_end_matches('/').to_string()).filter(|u| !u.is_empty());
            let dev_url = match dev_url {
                Some(url) if is_local_dev_url(&url) => Some(url),
                Some(url) => {
                    warnings.push(format!("`ui.dev_url` « {url} » ignoré : seul http://localhost:<port> ou http://127.0.0.1:<port> est admis."));
                    None
                }
                None => None,
            };
            (Some(entry), dev_url, ui.views)
        }
    };

    let mut seen = HashSet::new();
    let mut views = Vec::new();
    for v in raw_views {
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
        if secrets.iter().any(|known: &PluginSecretInfo| known.id == s.id) {
            return Err(invalid(format!("Secret « {} » déclaré deux fois.", s.id)));
        }
        secrets.push(PluginSecretInfo { id: s.id, label: s.label, description: s.description, required: s.required });
    }

    let mut setup = Vec::new();
    for step in raw.setup {
        if !is_item_id(&step.id) || setup.iter().any(|known: &PluginSetupStep| known.id == step.id) {
            return Err(invalid(format!("Étape de configuration « {} » : identifiant invalide ou en double.", step.id)));
        }
        // La vue d'une étape est une route de l'interface du plugin, pas forcément une vue du
        // rail (contrat du dépôt de plugins) : il suffit que le plugin ait une interface.
        if ui_entry.is_none() || !is_item_id(&step.view) {
            return Err(invalid(format!(
                "Étape de configuration « {} » : la route « {} » demande une section [ui].",
                step.id, step.view
            )));
        }
        setup.push(PluginSetupStep { id: step.id, title: step.title, view: step.view, required: step.required });
    }

    let mut actions = Vec::new();
    for a in raw.actions {
        if !is_item_id(&a.id) || actions.iter().any(|known: &PluginActionInfo| known.id == a.id) {
            return Err(invalid(format!("Action « {} » : identifiant invalide ou en double.", a.id)));
        }
        let mode = match a.mode.as_str() {
            "chat" => ActionMode::Chat,
            "background" => ActionMode::Background,
            other => return Err(invalid(format!("Action « {} » : mode « {other} » inconnu (chat, background).", a.id))),
        };
        if !skills::is_skill_name(&a.skill) {
            return Err(invalid(format!("Action « {} » : « {} » n'est pas un nom de skill.", a.id, a.skill)));
        }
        let some = |v: Option<String>| v.filter(|s| !s.trim().is_empty());
        actions.push(PluginActionInfo {
            id: a.id,
            label: a.label,
            skill: a.skill,
            mode,
            context: some(a.context),
            description: some(a.description),
            icon: some(a.icon),
            confirm: some(a.confirm),
        });
    }

    Ok(Manifest {
        id,
        name,
        version,
        description,
        authors: raw.authors,
        icon: raw.icon.filter(|i| !i.trim().is_empty()),
        min_cadence,
        homepage,
        category,
        color,
        services,
        tools,
        server,
        ui_entry,
        ui_dev_url,
        views,
        secrets,
        setup,
        actions,
        skills: Vec::new(),
        settings_schema: raw.settings.and_then(|s| s.schema),
        warnings,
    })
}

fn parse_server(srv: RawServer) -> AppResult<ServerSpec> {
    if !is_bin_name(&srv.bin) {
        return Err(invalid(format!("`server.bin` « {} » : nom de binaire sans chemin ni extension attendu.", srv.bin)));
    }
    let targets = match srv.targets {
        None => TARGETS.iter().map(|t| t.to_string()).collect(),
        Some(list) => {
            if list.is_empty() {
                return Err(invalid("`server.targets` est vide."));
            }
            let mut seen = HashSet::new();
            let mut targets = Vec::new();
            for t in &list {
                let Some(long) = canonical_target(t) else {
                    return Err(invalid(format!("`server.targets` : cible « {t} » inconnue ({}).", TARGETS.join(", "))));
                };
                if !seen.insert(long) {
                    return Err(invalid(format!("`server.targets` : cible « {t} » en double.")));
                }
                targets.push(long.to_string());
            }
            targets
        }
    };
    Ok(ServerSpec {
        bin: srv.bin,
        targets,
        args: srv.args,
        idle_shutdown_s: srv.idle_shutdown_s.unwrap_or(DEFAULT_IDLE_SHUTDOWN_S),
    })
}

#[cfg(test)]
mod tests {

    /// Manifestes du dépôt officiel : `CADENCE_PLUGINS_REPO=<chemin de cadence-plugins>
    /// cargo test --lib -- --ignored parses_the_official_manifests`.
    #[test]
    #[ignore]
    fn parses_the_official_manifests() {
        let repo = std::path::PathBuf::from(std::env::var("CADENCE_PLUGINS_REPO").expect("CADENCE_PLUGINS_REPO"));
        for entry in std::fs::read_dir(repo.join("plugins")).unwrap() {
            let path = entry.unwrap().path().join(FILE_NAME);
            let text = std::fs::read_to_string(&path).unwrap();
            let m = parse(&text, "0.3.0").unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            println!("{} {} · tools {:?} · services {:?} · avertissements {:?}", m.id, m.version, m.tools, m.services, m.warnings);
        }
    }
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

[[requires]]
tool = "node"

[[requires]]
tool = "gh"
optional = true
auth = true
reason = "Réutiliser votre connexion GitHub existante"
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
        assert_eq!(m.tools.len(), 2);
        assert_eq!((m.tools[0].id, m.tools[0].optional), (ToolId::Node, false));
        assert_eq!((m.tools[1].id, m.tools[1].optional, m.tools[1].auth), (ToolId::Gh, true, true));
        assert_eq!(m.tools[1].reason.as_deref(), Some("Réutiliser votre connexion GitHub existante"));
        let unknown =
            parse(&GESTION_PROJET.replacen("tool = \"node\"", "tool = \"ffmpeg\"", 1), "0.3.0").unwrap();
        assert_eq!(unknown.tools.len(), 1);
        assert!(unknown.warnings.iter().any(|w| w.contains("ffmpeg")));
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

    const ACTIONS: &str = r#"
[[setup]]
id = "github"
title = "Connexion GitHub"
view = "issue"
required = true

[[actions]]
id = "preparer-issue"
label = "Préparer l'issue"
skill = "preparer-issue"
mode = "chat"
context = "issue"
confirm = ""

[[actions]]
id = "rapport_version"
label = "Rapport de version"
skill = "rapport-version"
mode = "background"
"#;

    #[test]
    fn reads_server_actions_and_setup() {
        let m = parse(&format!("{GESTION_PROJET}{ACTIONS}"), "0.3.0").unwrap();
        let server = m.server.as_ref().unwrap();
        assert_eq!(server.bin, "cadence-gestion-projet");
        assert_eq!(server.targets.len(), 4, "défaut : les quatre cibles");
        assert_eq!(server.idle_shutdown_s, DEFAULT_IDLE_SHUTDOWN_S);
        assert!(server.binary(Path::new("p"), TARGETS[0]).ends_with("bin/x86_64-pc-windows-msvc/cadence-gestion-projet.exe"));
        assert!(server.binary(Path::new("p"), TARGETS[3]).ends_with("bin/x86_64-unknown-linux-gnu/cadence-gestion-projet"));
        // Archive du dépôt de plugins : dossiers aux noms courts.
        let root = crate::app_settings::temp_dir("manifest-short-target");
        std::fs::create_dir_all(root.join("bin").join("linux-x86_64")).unwrap();
        std::fs::write(root.join("bin").join("linux-x86_64").join("cadence-gestion-projet"), "").unwrap();
        assert!(server.binary(&root, TARGETS[3]).ends_with("bin/linux-x86_64/cadence-gestion-projet"));
        assert_eq!(canonical_target("macos-aarch64"), Some("aarch64-apple-darwin"));
        assert_eq!(canonical_target("riscv64"), None);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(m.ui_entry.as_deref(), Some("index.html"));
        let rooted = parse(&GESTION_PROJET.replacen("entry = \"index.html\"", "entry = \"ui/index.html\"", 1), "0.3.0").unwrap();
        assert_eq!(rooted.ui_entry.as_deref(), Some("index.html"), "forme du dépôt de plugins");
        assert_eq!(m.ui_dev_url, None);
        let dev = |url: &str| {
            let toml = GESTION_PROJET.replacen("entry = \"index.html\"", &format!("entry = \"index.html\"\ndev_url = \"{url}\""), 1);
            parse(&toml, "0.3.0").unwrap()
        };
        assert_eq!(dev("http://localhost:5174/").ui_dev_url.as_deref(), Some("http://localhost:5174"));
        assert_eq!(dev("http://127.0.0.1:5174/ui").ui_dev_url.as_deref(), Some("http://127.0.0.1:5174/ui"));
        let distant = dev("https://exemple.org");
        assert_eq!(distant.ui_dev_url, None, "jamais une page distante");
        assert!(distant.warnings.iter().any(|w| w.contains("ui.dev_url")));
        assert_eq!(m.setup[0].view, "issue");
        assert!(m.setup[0].required);
        assert_eq!(m.actions.len(), 2);
        assert_eq!((m.actions[0].mode, m.actions[0].context.as_deref()), (ActionMode::Chat, Some("issue")));
        assert_eq!(m.actions[0].confirm, None, "texte vide = pas de confirmation");
        assert_eq!(m.actions[1].mode, ActionMode::Background);
        assert!(m.skills.is_empty(), "`parse` ne lit pas le dossier");

        let custom = GESTION_PROJET.replacen(
            "bin = \"cadence-gestion-projet\"",
            "bin = \"srv\"\ntargets = [\"x86_64-unknown-linux-gnu\"]\nargs = [\"--stdio\"]\nidle_shutdown_s = 0",
            1,
        );
        let server = parse(&custom, "0.3.0").unwrap().server.unwrap();
        assert_eq!((server.targets.len(), server.args.as_slice(), server.idle_shutdown_s), (1, ["--stdio".to_string()].as_slice(), 0));
    }

    #[test]
    fn rejects_invalid_server_actions_and_setup() {
        let full = format!("{GESTION_PROJET}{ACTIONS}");
        let with = |from: &str, to: &str| full.replacen(from, to, 1);
        for (text, why) in [
            (with("bin = \"cadence-gestion-projet\"", "bin = \"bin/srv\""), "chemin dans bin"),
            (with("bin = \"cadence-gestion-projet\"", "bin = \"srv.exe\""), "extension"),
            (with("bin = \"cadence-gestion-projet\"", "bin = \"srv\"\ntargets = [\"riscv64\"]"), "cible inconnue"),
            (with("bin = \"cadence-gestion-projet\"", "bin = \"srv\"\ntargets = []"), "aucune cible"),
            (with("entry = \"index.html\"", "entry = \"../index.html\""), "entrée hors de ui/"),
            (with("entry = \"index.html\"", ""), "entrée absente"),
            (with("view = \"issue\"", "view = \"Pas une route\""), "route de setup invalide"),
            (with("mode = \"chat\"", "mode = \"auto\""), "mode inconnu"),
            (with("skill = \"preparer-issue\"", "skill = \"Préparer\""), "nom de skill"),
            (with("id = \"rapport_version\"", "id = \"preparer-issue\""), "action en double"),
            (with("[[actions]]\nid = \"preparer-issue\"\nlabel", "[[actions]]\nid = \"preparer-issue\"\nlabelle"), "label absent"),
            (format!("{full}\n[[secrets]]\nid = \"github_token\"\nlabel = \"Bis\"\n"), "secret en double"),
        ] {
            assert!(parse(&text, "0.3.0").is_err(), "{why} devrait être refusé");
        }
        let http = parse(&GESTION_PROJET.replacen("min_cadence", "homepage = \"http://x.fr\"\nmin_cadence", 1), "0.3.0").unwrap();
        assert_eq!((http.homepage, http.warnings.len()), (None, 1));
    }

    #[test]
    fn checks_the_plugin_files() {
        let dir = crate::app_settings::temp_dir("plugin-manifest-files");
        let target = current_target().expect("tests lancés sur une des quatre cibles");
        let write = |path: &str, text: &str| {
            let path = dir.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(FILE_NAME, &format!("{GESTION_PROJET}{ACTIONS}"));
        write("ui/index.html", "<html></html>");
        write("icon.svg", "<svg/>");
        write("skills/preparer-issue/SKILL.md", "---\nname: preparer-issue\ndescription: Prépare une issue.\n---\n");

        let err = load(&dir, "0.3.0").unwrap_err().to_string();
        assert!(err.contains("rapport-version"), "skill d'action absent : {err}");
        write("skills/rapport-version/SKILL.md", "---\nname: rapport-version\ndescription: Rapport.\n---\n");

        let err = load(&dir, "0.3.0").unwrap_err().to_string();
        assert!(err.contains("Binaire du serveur absent"), "{err}");
        let bin = if target.contains("windows") { "cadence-gestion-projet.exe" } else { "cadence-gestion-projet" };
        write(&format!("bin/{target}/{bin}"), "");

        let m = load(&dir, "0.3.0").unwrap();
        assert_eq!(m.skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["preparer-issue", "rapport-version"]);
        assert!(m.warnings.is_empty(), "{:?}", m.warnings);
        // Les autres cibles manquent : un avertissement lisible, la machine courante nommée à
        // part (R08), sans nom technique de cible.
        let missing = check_all_targets(&dir, &m);
        assert_eq!(missing.len(), 1, "{missing:?}");
        let others: Vec<&str> = TARGETS.into_iter().filter(|t| *t != target).collect();
        assert_eq!(
            missing[0],
            format!(
                "Installé pour {} (cette machine). Binaire du serveur absent pour {} : le plugin ne fonctionnera pas sur ces systèmes.",
                target_label(target),
                targets_label(&others)
            )
        );
        assert!(!missing[0].contains(target) && !missing[0].contains('`'), "{missing:?}");
        for t in TARGETS {
            let bin = if t.contains("windows") { "cadence-gestion-projet.exe" } else { "cadence-gestion-projet" };
            write(&format!("bin/{t}/{bin}"), "");
        }
        assert!(check_all_targets(&dir, &m).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_targets_for_people() {
        assert_eq!(target_label("x86_64-pc-windows-msvc"), "Windows x64");
        assert_eq!(target_label("macos-aarch64"), "macOS (Apple Silicon)");
        assert_eq!(target_label("x86_64-apple-darwin"), "macOS (Intel)");
        assert_eq!(target_label("x86_64-unknown-linux-gnu"), "Linux x64");
        assert_eq!(target_label("riscv64"), "riscv64");
        assert_eq!(
            targets_label(&["aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu"]),
            "macOS (Apple Silicon, Intel) et Linux x64"
        );
        assert_eq!(
            targets_label(&["x86_64-pc-windows-msvc", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu"]),
            "Windows x64, macOS (Intel) et Linux x64"
        );
        assert_eq!(targets_label(&["x86_64-unknown-linux-gnu"]), "Linux x64");
        assert_eq!(targets_label(&[]), "");
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
