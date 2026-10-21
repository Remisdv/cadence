//! « Autoriser pour la session » (recette du 21/10, P12) : une approbation accordée pour la
//! session vaut pour la famille de la demande, pas pour la seule commande exacte.
//!
//! - commande shell : les programmes qu'elle lance (`git`, `python`…) ; une commande suivante est
//!   couverte quand tous ses programmes ont déjà été accordés (`cd`, `echo` et les affectations
//!   de variables ne comptent pas) ;
//! - modification de fichier : toutes les modifications de fichier de la conversation ;
//! - autre outil : cet outil (`WebFetch`, un outil MCP…).
//!
//! Les règles sont tenues par Cadence pour les trois IA (le chat répond d'office aux demandes
//! couvertes) ; Claude Code reçoit en plus les règles équivalentes (`Bash(git:*)`) avec ses
//! propres `permission_suggestions`, Codex `acceptForSession` et OpenCode `always`.

use std::collections::BTreeSet;

use serde_json::Value;

use super::events::ApprovalKind;

/// Programmes sans effet à eux seuls : ils ne demandent pas d'accord propre.
const NEUTRAL: &[&str] = &["cd", "set-location", "sl", "pushd", "popd", "echo", "write-output", "write-host", "true", ":"];

/// Ce qu'une demande d'approbation laisse savoir de sa famille.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalInfo {
    pub kind: ApprovalKind,
    /// Outil de la CLI (`Bash`, `WebFetch`, `mcp__…`), quand elle le donne.
    pub tool: Option<String>,
    /// Ligne de commande d'une demande de commande shell.
    pub command: Option<String>,
    /// Outil d'un plugin qui écrit (N12) : demandé en « Demander », accordé d'office en
    /// « Édition auto » et en « Auto ». Rempli par le chat, qui connaît la passerelle.
    pub plugin_write: bool,
}

impl ApprovalInfo {
    /// Lit la demande : `detail` est l'entrée de l'outil (Claude Code), les paramètres de la
    /// requête (Codex) ou les propriétés de `permission.asked` (OpenCode).
    pub fn of(kind: ApprovalKind, tool: Option<&str>, title: &str, detail: &Value) -> Self {
        let command = (kind == ApprovalKind::Command).then(|| command_of(detail).unwrap_or_else(|| title.to_string()));
        Self { kind, tool: tool.filter(|t| !t.is_empty()).map(str::to_string), command, plugin_write: false }
    }

    /// Accord de session que cette demande donnerait.
    pub fn grant(&self) -> Option<Grant> {
        match self.kind {
            ApprovalKind::Command => {
                let programs = command_programs(self.command.as_deref()?);
                (!programs.is_empty()).then_some(Grant::Programs(programs))
            }
            ApprovalKind::FileChange => Some(Grant::Files),
            ApprovalKind::Tool => self.tool.clone().map(Grant::Tool),
        }
    }
}

/// Ligne de commande d'une demande : `command` (texte ou liste d'arguments), sinon
/// `metadata.command` ou les `patterns` d'OpenCode.
fn command_of(detail: &Value) -> Option<String> {
    let text = |value: &Value| match value {
        Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
        Value::Array(parts) => {
            let parts: Vec<&str> = parts.iter().filter_map(Value::as_str).collect();
            (!parts.is_empty()).then(|| parts.join(" "))
        }
        _ => None,
    };
    detail
        .get("command")
        .and_then(text)
        .or_else(|| detail.get("metadata").and_then(|m| m.get("command")).and_then(text))
        .or_else(|| {
            let patterns: Vec<&str> = detail.get("patterns")?.as_array()?.iter().filter_map(Value::as_str).collect();
            (!patterns.is_empty()).then(|| patterns.join(" && "))
        })
}

/// Famille accordée pour la session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grant {
    Programs(BTreeSet<String>),
    Files,
    Tool(String),
}

impl Grant {
    /// Libellé montré sous la demande (« commandes git, python »).
    pub fn label(&self) -> String {
        match self {
            Grant::Programs(programs) => {
                let list = programs.iter().cloned().collect::<Vec<_>>().join(", ");
                if programs.len() > 1 { format!("commandes {list}") } else { format!("commande {list}") }
            }
            Grant::Files => "modifications de fichiers".into(),
            Grant::Tool(tool) => format!("outil {}", short_tool(tool)),
        }
    }
}

/// `mcp__cadence__gestion-projet__list_issues` → `gestion-projet__list_issues`.
fn short_tool(tool: &str) -> &str {
    tool.strip_prefix("mcp__cadence__").unwrap_or(tool)
}

/// Accords de session d'une conversation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionRules {
    programs: BTreeSet<String>,
    files: bool,
    tools: BTreeSet<String>,
}

impl SessionRules {
    pub fn add(&mut self, grant: Grant) {
        match grant {
            Grant::Programs(programs) => self.programs.extend(programs),
            Grant::Files => self.files = true,
            Grant::Tool(tool) => {
                self.tools.insert(tool);
            }
        }
    }

    /// La demande est couverte par un accord déjà donné.
    pub fn covers(&self, info: &ApprovalInfo) -> bool {
        match info.kind {
            ApprovalKind::Command => {
                let Some(command) = info.command.as_deref() else { return false };
                if self.programs.is_empty() {
                    return false;
                }
                command_programs(command).is_subset(&self.programs)
            }
            ApprovalKind::FileChange => self.files,
            ApprovalKind::Tool => info.tool.as_ref().is_some_and(|t| self.tools.contains(t)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.programs.is_empty() && !self.files && self.tools.is_empty()
    }
}

/// Programmes lancés par une ligne de commande (bash ou PowerShell) : premier mot de chaque
/// segment (`;`, `&&`, `||`, `|`, `&`, retour à la ligne), sans chemin ni `.exe`, en
/// minuscules. Les affectations (`A=1 git…`, `$r = "…"`), les chaînes seules, l'opérateur
/// d'appel `&` et les programmes neutres sont ignorés.
pub fn command_programs(command: &str) -> BTreeSet<String> {
    split_segments(command)
        .iter()
        .filter_map(|segment| segment_program(segment))
        .filter(|p| !NEUTRAL.contains(&p.as_str()))
        .collect()
}

/// Découpe aux séparateurs de commandes, hors guillemets.
fn split_segments(command: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                current.push(c);
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '\'' | '"' => {
                    quote = Some(c);
                    current.push(c);
                }
                ';' | '\n' | '|' => {
                    if c == '|' && chars.peek() == Some(&'|') {
                        chars.next();
                    }
                    segments.push(std::mem::take(&mut current));
                }
                '&' => {
                    // `&&` sépare ; `&` seul en tête est l'opérateur d'appel de PowerShell, en
                    // fin de commande la mise en arrière-plan de bash.
                    if chars.peek() == Some(&'&') {
                        chars.next();
                        segments.push(std::mem::take(&mut current));
                    } else if current.trim().is_empty() {
                        current.push_str("& ");
                    } else {
                        segments.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(c),
            },
        }
    }
    segments.push(current);
    segments.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// Programme d'un segment, s'il en lance un.
fn segment_program(segment: &str) -> Option<String> {
    let segment = segment.trim_start_matches(['(', '{', ' ', '\t']);
    let mut words = words(segment).into_iter();
    let mut called = false;
    loop {
        let word = words.next()?;
        // Opérateur d'appel de PowerShell : le mot suivant (même entre guillemets) est lancé.
        if word == "&" {
            called = true;
            continue;
        }
        let quoted = word.starts_with('"') || word.starts_with('\'');
        // Variable PowerShell (affectation ou simple lecture) ou chaîne seule : rien n'est lancé.
        if word.starts_with('$') || (quoted && !called) {
            return None;
        }
        // Affectation de variable d'environnement devant la commande (bash).
        if !quoted
            && let Some((name, _)) = word.split_once('=')
            && !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        let word = word.trim_matches(['"', '\'', ')', '}']);
        let name = word.rsplit(['/', '\\']).next().unwrap_or(word).to_ascii_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name).to_string();
        return (!name.is_empty()).then_some(name);
    }
}

/// Mots d'un segment, les guillemets gardés (un mot entre guillemets peut contenir des espaces).
fn words(segment: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in segment.chars() {
        match quote {
            Some(q) => {
                current.push(c);
                if c == q {
                    quote = None;
                }
            }
            None if c.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                }
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn finds_the_programs_of_bash_and_powershell_command_lines() {
        assert_eq!(command_programs("git status"), set(&["git"]));
        assert_eq!(command_programs("cd C:\\repo; git fetch origin --prune; git branch -r"), set(&["git"]));
        assert_eq!(
            command_programs(r#"$r="C:\a b\repo"; git -C $r ls-tree -r origin/x | Select-Object -First 50; "---ARCHI---""#),
            set(&["git", "select-object"])
        );
        assert_eq!(command_programs("cd /tmp && FOO=1 python3 a.py || echo raté"), set(&["python3"]));
        assert_eq!(command_programs("& 'C:\\Program Files\\Git\\bin\\git.exe' log"), set(&["git"]));
        assert_eq!(command_programs("/usr/bin/grep 'a;b|c' f.txt"), set(&["grep"]));
        assert_eq!(command_programs("sudo rm -rf /"), set(&["sudo"]), "sudo n'est jamais transparent");
        assert!(command_programs("cd x; echo ok").is_empty());
    }

    #[test]
    fn a_session_grant_covers_the_family_not_only_the_exact_command() {
        let command = |c: &str| ApprovalInfo::of(ApprovalKind::Command, Some("Bash"), c, &json!({ "command": c }));
        let mut rules = SessionRules::default();
        assert!(!rules.covers(&command("git status")));
        rules.add(command("cd /r; git fetch; git branch -r").grant().unwrap());
        assert!(rules.covers(&command("git -C /r log --oneline -5")));
        assert!(rules.covers(&command("cd /r && git add -A && git commit -m 'x'")));
        assert!(!rules.covers(&command("git push && python a.py")), "python pas encore accordé");
        assert!(rules.covers(&command("cd /r")), "commande sans programme, une fois des commandes accordées");
        rules.add(command("python a.py").grant().unwrap());
        assert!(rules.covers(&command("git push && python a.py")));
        assert!(!rules.covers(&command("rm -rf build")));

        let file = ApprovalInfo::of(ApprovalKind::FileChange, Some("Write"), "Write a.txt", &json!({ "file_path": "a.txt" }));
        assert!(!rules.covers(&file));
        rules.add(file.grant().unwrap());
        assert!(rules.covers(&ApprovalInfo::of(ApprovalKind::FileChange, Some("Edit"), "Edit b.txt", &json!({}))));

        let tool = |t: &str| ApprovalInfo::of(ApprovalKind::Tool, Some(t), t, &json!({}));
        rules.add(tool("WebFetch").grant().unwrap());
        assert!(rules.covers(&tool("WebFetch")));
        assert!(!rules.covers(&tool("WebSearch")));
        assert!(ApprovalInfo::of(ApprovalKind::Tool, None, "outil", &json!({})).grant().is_none());
    }

    #[test]
    fn reads_the_command_of_each_cli_and_labels_the_grant() {
        // Codex : `command` texte ; OpenCode : `patterns` (ou `metadata.command`).
        let codex = ApprovalInfo::of(ApprovalKind::Command, None, "git status", &json!({ "command": "git status", "cwd": "/r" }));
        assert_eq!(codex.command.as_deref(), Some("git status"));
        let opencode = ApprovalInfo::of(ApprovalKind::Command, Some("bash"), "bash : npm test", &json!({ "patterns": ["npm test"] }));
        assert_eq!(opencode.grant().unwrap().label(), "commande npm");
        let argv = ApprovalInfo::of(ApprovalKind::Command, None, "", &json!({ "command": ["git", "log"] }));
        assert_eq!(argv.command.as_deref(), Some("git log"));
        assert_eq!(Grant::Programs(set(&["git", "python"])).label(), "commandes git, python");
        assert_eq!(Grant::Files.label(), "modifications de fichiers");
        assert_eq!(Grant::Tool("mcp__cadence__gestion-projet__list_issues".into()).label(), "outil gestion-projet__list_issues");
    }
}
