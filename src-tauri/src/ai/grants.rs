//! Worktrees ouverts à une session en cours de route (`cadence__repo_checkout`).
//!
//! La CLI ne reçoit ses dossiers (`--add-dir`…) qu'à son lancement ; pendant le tour où le
//! worktree est ouvert, les demandes d'accès qui le visent sont accordées d'office par
//! l'adaptateur, sans carte d'approbation. Rien d'autre n'est accordé ainsi : une commande
//! shell reste soumise aux règles de la session.

use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use serde_json::Value;

/// Worktrees ouverts à la session, relus à chaque demande.
pub type DirGrants = Arc<dyn Fn() -> Vec<PathBuf> + Send + Sync>;

/// Chemin sans `.` ni `..` (lexical : le fichier peut ne pas exister encore). `None` si le
/// chemin est relatif ou remonte au-dessus de sa racine.
fn normalize(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() || out.parent().is_none() {
                    return None;
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

/// Comparaison des composants, sans casse sous Windows.
fn starts_with(path: &Path, dir: &Path) -> bool {
    let (mut path, mut dir) = (path.components(), dir.components());
    loop {
        match (dir.next(), path.next()) {
            (None, _) => return true,
            (Some(_), None) => return false,
            (Some(a), Some(b)) => {
                let (a, b) = (a.as_os_str().to_string_lossy(), b.as_os_str().to_string_lossy());
                let same = if cfg!(windows) { a.eq_ignore_ascii_case(&b) } else { a == b };
                if !same {
                    return false;
                }
            }
        }
    }
}

/// Worktree accordé qui contient `path`, s'il y en a un.
pub fn covering(path: &Path, dirs: &[PathBuf]) -> Option<PathBuf> {
    let path = normalize(path)?;
    dirs.iter().find(|dir| normalize(dir).is_some_and(|dir| starts_with(&path, &dir))).cloned()
}

/// Chemin visé par un outil de fichiers de Claude Code ; `None` pour les autres outils (shell
/// compris) et pour un outil sans chemin explicite.
pub fn claude_tool_path(tool: &str, input: &Value) -> Option<PathBuf> {
    let key = match tool {
        "Read" | "Write" | "Edit" | "MultiEdit" => "file_path",
        "NotebookEdit" => "notebook_path",
        "Glob" | "Grep" | "LS" => "path",
        _ => return None,
    };
    input.get(key).and_then(Value::as_str).filter(|p| !p.is_empty()).map(PathBuf::from)
}

/// Demande `external_directory` d'OpenCode dont tous les motifs tombent dans un worktree
/// accordé (`<dossier>/*`, `<dossier>/**` ou un chemin).
pub fn opencode_covered(props: &Value, dirs: &[PathBuf]) -> bool {
    if props.get("permission").and_then(Value::as_str) != Some("external_directory") {
        return false;
    }
    let patterns: Vec<&str> =
        props.get("patterns").and_then(Value::as_array).map(|p| p.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    !patterns.is_empty()
        && patterns.iter().all(|pattern| {
            let base = pattern.trim_end_matches("**").trim_end_matches('*').trim_end_matches(['/', '\\']);
            !base.contains(['*', '?', '[']) && covering(Path::new(base), dirs).is_some()
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn root() -> PathBuf {
        std::env::temp_dir().join("repos").join(".worktrees").join("me").join("app").join("issue-42")
    }

    #[test]
    fn grants_paths_inside_a_worktree_only() {
        let dirs = vec![root()];
        assert_eq!(covering(&root().join("src").join("main.rs"), &dirs), Some(root()));
        assert_eq!(covering(&root(), &dirs), Some(root()));
        assert_eq!(covering(&root().join("..").join("issue-43").join("x"), &dirs), None, "pas de sortie par `..`");
        assert_eq!(covering(&root().with_file_name("issue-421"), &dirs), None, "préfixe de nom ≠ dossier");
        assert_eq!(covering(Path::new("src/main.rs"), &dirs), None, "chemin relatif refusé");
        if cfg!(windows) {
            let upper = PathBuf::from(root().to_string_lossy().to_uppercase().replace('\\', "/")).join("a.txt");
            assert_eq!(covering(&upper, &dirs), Some(root()), "casse et séparateurs de Windows");
        }
    }

    #[test]
    fn reads_the_path_of_claude_file_tools() {
        assert_eq!(claude_tool_path("Edit", &json!({ "file_path": "/a/b.rs" })), Some(PathBuf::from("/a/b.rs")));
        assert_eq!(claude_tool_path("Grep", &json!({ "pattern": "x", "path": "/a" })), Some(PathBuf::from("/a")));
        assert_eq!(claude_tool_path("Grep", &json!({ "pattern": "x" })), None);
        assert_eq!(claude_tool_path("Bash", &json!({ "command": "cat /a/b.rs" })), None, "jamais le shell");
    }

    #[test]
    fn accepts_opencode_external_directories_inside_worktrees() {
        let dirs = vec![root()];
        let ask = |patterns: Value| json!({ "permission": "external_directory", "patterns": patterns });
        let inside = format!("{}/*", root().display());
        assert!(opencode_covered(&ask(json!([inside])), &dirs));
        let outside = format!("{}/*", root().with_file_name("issue-43").display());
        assert!(!opencode_covered(&ask(json!([inside, outside])), &dirs));
        assert!(!opencode_covered(&ask(json!([])), &dirs));
        assert!(!opencode_covered(&json!({ "permission": "bash", "patterns": [inside] }), &dirs));
    }
}
