//! Préparation d'une session d'IA (plan § 4.1, révision 1.1) : un dossier de travail neutre
//! et persistant par session (`<données>/sessions/<id>/`, le `cwd` de la CLI, nécessaire à la
//! reprise), et un dossier de préparation éphémère (`<cache>/sessions/<id>-<n>/`) qui ne vit
//! que le temps du processus :
//!
//! - `mcp.json` : la passerelle, jeton lu dans `CADENCE_MCP_TOKEN` (jamais écrit sur disque) ;
//! - `plugin/` : plugin Claude Code éphémère `cadence` qui porte les skills des plugins activés,
//!   renommés `<plugin>-<skill>` (invoqués par `/cadence:<plugin>-<skill>`) ;
//! - `CADENCE.md` : consignes du cœur, ajoutées au prompt système ;
//! - `settings.json` : `disableAllHooks` (les hooks du dossier ne s'exécutent pas).
//!
//! Rien n'est écrit dans les réglages de la CLI ni dans un dépôt de l'utilisateur.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering},
};

use serde_json::json;

use crate::{
    error::AppResult,
    plugins::{registry::PluginRegistry, skills},
};

/// Consignes du cœur, communes à toutes les sessions.
pub const INSTRUCTIONS: &str = "\
# Cadence

Tu travailles dans Cadence, une application de bureau qui relie l'utilisateur à des plugins
(gestion de projet, documents de cours…). Réponds en français.

- Les outils des plugins sont servis par le serveur MCP `cadence` et se nomment
  `<plugin>__<outil>` ; les outils `cadence__*` appartiennent à Cadence. Appelle-les
  directement : ils sont déjà autorisés.
- Ton dossier de travail est un dossier neutre de Cadence, pas un dépôt de l'utilisateur :
  passe par les outils des plugins pour lire ou modifier ses données.
- Pour montrer une vue d'un plugin, écris un lien `cadence:<plugin>/<vue>?<paramètres>`
  (`cadence__plugins_info` liste les vues) ou appelle `cadence__open_view`.
";

/// Skill d'un plugin activé, à copier dans la session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSkill {
    pub plugin: String,
    pub skill: String,
    /// `skills/<skill>/` dans le dossier du plugin.
    pub source: PathBuf,
}

impl StagedSkill {
    /// Nom dans le plugin éphémère : `<plugin>-<skill>`.
    pub fn staged_name(&self) -> String {
        format!("{}-{}", self.plugin, self.skill)
    }
}

/// Skills des plugins activés dont le manifeste est valide.
pub fn plugin_skills(registry: &PluginRegistry) -> Vec<StagedSkill> {
    let installed = registry.snapshot();
    let mut out = Vec::new();
    for id in registry.ordered_ids() {
        let Some(entry) = installed.plugins.get(&id).filter(|e| e.enabled) else { continue };
        let Ok(manifest) = registry.manifest(&id) else { continue };
        let dir = registry.plugin_dir(&id, entry).join("skills");
        for skill in manifest.skills {
            out.push(StagedSkill { plugin: id.clone(), source: dir.join(&skill.name), skill: skill.name });
        }
    }
    out
}

/// Fichiers d'une session, supprimés avec elle.
#[derive(Debug)]
pub struct SessionFiles {
    pub dir: PathBuf,
    pub mcp_config: PathBuf,
    pub plugin_dir: PathBuf,
    pub instructions: PathBuf,
    pub settings: PathBuf,
}

impl Drop for SessionFiles {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Supprime les restes d'un lancement précédent (`<cache>/sessions/`).
pub fn sweep(staging_root: &Path) {
    if staging_root.exists()
        && let Err(err) = fs::remove_dir_all(staging_root)
    {
        tracing::warn!(%err, "restes des sessions d'IA non supprimés");
    }
}

static NEXT: AtomicU32 = AtomicU32::new(0);

/// Prépare les fichiers d'un processus de session.
pub fn stage(staging_root: &Path, session: &str, gateway_url: Option<&str>, skills: &[StagedSkill]) -> AppResult<SessionFiles> {
    let dir = staging_root.join(format!("{session}-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = fs::remove_dir_all(&dir);
    let plugin_dir = dir.join("plugin");
    fs::create_dir_all(plugin_dir.join(".claude-plugin"))?;
    let files = SessionFiles {
        mcp_config: dir.join("mcp.json"),
        instructions: dir.join("CADENCE.md"),
        settings: dir.join("settings.json"),
        plugin_dir,
        dir,
    };

    let servers = match gateway_url {
        Some(url) => json!({ "cadence": {
            "type": "http",
            "url": url,
            "headers": { "Authorization": "Bearer ${CADENCE_MCP_TOKEN}" },
            "timeout": 600000
        } }),
        None => json!({}),
    };
    fs::write(&files.mcp_config, json!({ "mcpServers": servers }).to_string())?;
    fs::write(&files.instructions, INSTRUCTIONS)?;
    fs::write(&files.settings, json!({ "disableAllHooks": true }).to_string())?;
    fs::write(
        files.plugin_dir.join(".claude-plugin").join("plugin.json"),
        json!({ "name": "cadence", "description": "Skills des plugins de Cadence" }).to_string(),
    )?;

    for skill in skills {
        let name = skill.staged_name();
        if !skills::is_skill_name(&name) {
            tracing::warn!(skill = name, "nom de skill trop long une fois préfixé, ignoré");
            continue;
        }
        let target = files.plugin_dir.join("skills").join(&name);
        if let Err(err) = copy_dir(&skill.source, &target).and_then(|()| rename_skill(&target.join(skills::FILE_NAME), &name)) {
            tracing::warn!(skill = name, %err, "skill non copié dans la session");
            let _ = fs::remove_dir_all(&target);
        }
    }
    Ok(files)
}

impl SessionFiles {
    /// Racine des skills de la session (`<plugin>-<skill>/SKILL.md`), commune aux trois CLI :
    /// `--plugin-dir` (Claude Code), `skills/extraRoots/set` (Codex), `skills.paths` (OpenCode).
    pub fn skills_root(&self) -> PathBuf {
        self.plugin_dir.join("skills")
    }
}

/// Surcharges de configuration d'un fil Codex (`thread/start.config`) : la passerelle, jeton lu
/// dans `CADENCE_MCP_TOKEN`, outils pré-approuvés (sinon Codex demande pour chaque outil
/// sans `readOnlyHint`, et refuse en fond).
pub fn codex_config(gateway_url: Option<&str>) -> serde_json::Value {
    match gateway_url {
        Some(url) => json!({ "mcp_servers": { "cadence": {
            "url": url,
            "bearer_token_env_var": "CADENCE_MCP_TOKEN",
            "default_tools_approval_mode": "approve",
            "tool_timeout_sec": 600
        } } }),
        None => json!({}),
    }
}

/// Règle de permission OpenCode d'un profil (`ask`, `allow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpencodePermissions {
    pub edit: &'static str,
    pub bash: &'static str,
}

/// Configuration d'un serveur OpenCode (`OPENCODE_CONFIG_CONTENT`, chargée après celle du
/// projet) : passerelle sans OAuth, outils `cadence_*` autorisés, consignes et skills de la
/// session, pas de partage ni de mise à jour automatique.
pub fn opencode_config(gateway_url: Option<&str>, files: &SessionFiles, permissions: OpencodePermissions) -> serde_json::Value {
    let mut config = json!({
        "$schema": "https://opencode.ai/config.json",
        "autoupdate": false,
        "share": "disabled",
        "instructions": [files.instructions.display().to_string()],
        "skills": { "paths": [files.skills_root().display().to_string()] },
        "permission": { "edit": permissions.edit, "bash": permissions.bash, "cadence_*": "allow" },
    });
    if let Some(url) = gateway_url {
        config["mcp"] = json!({ "cadence": {
            "type": "remote",
            "url": url,
            "oauth": false,
            "headers": { "Authorization": "Bearer {env:CADENCE_MCP_TOKEN}" },
            "timeout": 600000
        } });
    }
    config
}

/// Copie récursive ; les liens symboliques sont ignorés.
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Réécrit `name:` dans le frontmatter du `SKILL.md` copié.
fn rename_skill(path: &Path, name: &str) -> std::io::Result<()> {
    let text = fs::read_to_string(path)?;
    let mut in_front = false;
    let mut done = false;
    let lines: Vec<String> = text
        .lines()
        .enumerate()
        .map(|(i, line)| {
            if line.trim_end() == "---" {
                in_front = i == 0;
            } else if in_front && !done && line.starts_with("name:") {
                done = true;
                return format!("name: {name}");
            }
            line.to_string()
        })
        .collect();
    fs::write(path, lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_the_gateway_skills_and_instructions() {
        let root = crate::app_settings::temp_dir("ai-staging");
        let source = root.join("plugin-src").join("skills").join("resumer");
        fs::create_dir_all(source.join("exemples")).unwrap();
        fs::write(source.join("SKILL.md"), "---\nname: resumer\ndescription: Résume.\n---\nCorps avec name: inchangé\n").unwrap();
        fs::write(source.join("exemples").join("a.md"), "annexe").unwrap();
        let skills = vec![
            StagedSkill { plugin: "documents".into(), skill: "resumer".into(), source: source.clone() },
            StagedSkill { plugin: "documents".into(), skill: "absent".into(), source: root.join("absent") },
        ];

        let files = stage(&root.join("staging"), "s1", Some("http://127.0.0.1:5000/mcp"), &skills).unwrap();
        let config: serde_json::Value = serde_json::from_str(&fs::read_to_string(&files.mcp_config).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["cadence"]["url"], "http://127.0.0.1:5000/mcp");
        assert_eq!(config["mcpServers"]["cadence"]["headers"]["Authorization"], "Bearer ${CADENCE_MCP_TOKEN}");
        let staged = files.plugin_dir.join("skills").join("documents-resumer");
        let skill = fs::read_to_string(staged.join("SKILL.md")).unwrap();
        assert!(skill.starts_with("---\nname: documents-resumer\n"), "{skill}");
        assert!(skill.contains("Corps avec name: inchangé"));
        assert!(staged.join("exemples").join("a.md").is_file());
        assert!(!files.plugin_dir.join("skills").join("documents-absent").exists(), "skill illisible écarté");
        assert!(crate::plugins::skills::load_all(&files.plugin_dir.join("skills")).is_ok(), "skills valides pour Claude Code");
        assert!(fs::read_to_string(&files.settings).unwrap().contains("disableAllHooks"));

        let codex = codex_config(Some("http://127.0.0.1:5000/mcp"));
        assert_eq!(codex["mcp_servers"]["cadence"]["bearer_token_env_var"], "CADENCE_MCP_TOKEN");
        assert_eq!(codex["mcp_servers"]["cadence"]["default_tools_approval_mode"], "approve");
        let opencode =
            opencode_config(Some("http://127.0.0.1:5000/mcp"), &files, OpencodePermissions { edit: "ask", bash: "ask" });
        assert_eq!(opencode["mcp"]["cadence"]["headers"]["Authorization"], "Bearer {env:CADENCE_MCP_TOKEN}");
        assert_eq!(opencode["mcp"]["cadence"]["oauth"], false);
        assert_eq!(opencode["permission"]["cadence_*"], "allow");
        assert!(opencode["skills"]["paths"][0].as_str().unwrap().ends_with("skills"));
        assert!(!opencode.to_string().contains("jeton"), "jeton jamais dans la configuration");

        let dir = files.dir.clone();
        drop(files);
        assert!(!dir.exists(), "dossier de préparation supprimé avec la session");
        let without = stage(&root.join("staging"), "s2", None, &[]).unwrap();
        assert!(fs::read_to_string(&without.mcp_config).unwrap().contains("\"mcpServers\":{}"));
        drop(without);
        sweep(&root.join("staging"));
        assert!(!root.join("staging").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
