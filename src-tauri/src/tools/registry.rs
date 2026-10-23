//! Ce que Cadence sait de chaque outil : binaire, version minimale, recettes d'installation par
//! système et gestionnaire, commande de connexion. Les commandes des installeurs officiels sont à
//! revérifier à chaque version de l'app (plan, révision 1.2).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app_settings::AiId;

/// Programme de la machine dont dépendent les IA, les services et les plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ToolId {
    Git,
    Gh,
    Claude,
    Codex,
    Opencode,
    Node,
}

impl ToolId {
    pub const ALL: [ToolId; 6] = [
        ToolId::Git,
        ToolId::Gh,
        ToolId::Claude,
        ToolId::Codex,
        ToolId::Opencode,
        ToolId::Node,
    ];

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == id)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ToolId::Git => "git",
            ToolId::Gh => "gh",
            ToolId::Claude => "claude",
            ToolId::Codex => "codex",
            ToolId::Opencode => "opencode",
            ToolId::Node => "node",
        }
    }

    /// Nom du binaire cherché dans le `PATH`.
    pub fn binary(self) -> &'static str {
        self.as_str()
    }

    pub fn name(self) -> &'static str {
        match self {
            ToolId::Git => "Git",
            ToolId::Gh => "GitHub CLI",
            ToolId::Claude => "Claude Code",
            ToolId::Codex => "Codex CLI",
            ToolId::Opencode => "OpenCode",
            ToolId::Node => "Node.js",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ToolId::Git => "Cloner, préparer les worktrees",
            ToolId::Gh => "Réutiliser votre connexion GitHub",
            ToolId::Claude => "IA de code d'Anthropic",
            ToolId::Codex => "IA de code d'OpenAI",
            ToolId::Opencode => "IA de code ouverte, multi-fournisseurs",
            ToolId::Node => "Prérequis des installations npm",
        }
    }

    pub fn min_version(self) -> &'static str {
        match self {
            ToolId::Git => "2.30.0",
            ToolId::Gh => "2.40.0",
            ToolId::Claude => "2.0.0",
            ToolId::Codex => "0.40.0",
            ToolId::Opencode => "0.15.0",
            ToolId::Node => "18.0.0",
        }
    }

    pub fn homepage(self) -> &'static str {
        match self {
            ToolId::Git => "https://git-scm.com",
            ToolId::Gh => "https://cli.github.com",
            ToolId::Claude => "https://docs.claude.com/claude-code",
            ToolId::Codex => "https://github.com/openai/codex",
            ToolId::Opencode => "https://opencode.ai",
            ToolId::Node => "https://nodejs.org",
        }
    }

    pub fn ai(self) -> Option<AiId> {
        match self {
            ToolId::Claude => Some(AiId::ClaudeCode),
            ToolId::Codex => Some(AiId::Codex),
            ToolId::Opencode => Some(AiId::Opencode),
            _ => None,
        }
    }

    pub fn from_ai(ai: AiId) -> Self {
        match ai {
            AiId::ClaudeCode => ToolId::Claude,
            AiId::Codex => ToolId::Codex,
            AiId::Opencode => ToolId::Opencode,
        }
    }

    /// Commande de connexion ; `None` : rien à connecter.
    pub fn login(self) -> Option<LoginSpec> {
        let spec =
            |args: &'static [&'static str], interactive| Some(LoginSpec { args, interactive });
        match self {
            ToolId::Gh => spec(
                &["auth", "login", "--web", "--git-protocol", "https"],
                false,
            ),
            ToolId::Claude => spec(&["auth", "login"], false),
            ToolId::Codex => spec(&["login"], false),
            // Choix du fournisseur au clavier : il faut un vrai terminal.
            ToolId::Opencode => spec(&["auth", "login"], true),
            ToolId::Git | ToolId::Node => None,
        }
    }
}

pub struct LoginSpec {
    pub args: &'static [&'static str],
    /// Demande une saisie au clavier : lancée dans un terminal du système, pas dans le journal.
    pub interactive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Macos,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Macos
        } else {
            Os::Linux
        }
    }
}

/// Gestionnaire qui exécute une recette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Manager {
    Winget,
    Brew,
    Apt,
    Dnf,
    Pacman,
    Npm,
    /// Installeur officiel (script PowerShell ou shell).
    Script,
    XcodeSelect,
}

impl Manager {
    /// Binaire dont la présence rend la recette utilisable.
    pub fn probe(self) -> &'static str {
        match self {
            Manager::Winget => "winget",
            Manager::Brew => "brew",
            Manager::Apt => "apt-get",
            Manager::Dnf => "dnf",
            Manager::Pacman => "pacman",
            Manager::Npm => "npm",
            Manager::Script => {
                if cfg!(windows) {
                    "powershell"
                } else {
                    "curl"
                }
            }
            Manager::XcodeSelect => "xcode-select",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Manager::Winget => "winget",
            Manager::Brew => "brew",
            Manager::Apt => "apt",
            Manager::Dnf => "dnf",
            Manager::Pacman => "pacman",
            Manager::Npm => "npm",
            Manager::Script => "script",
            Manager::XcodeSelect => "xcode",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Manager::Winget => "recette winget",
            Manager::Brew => "recette Homebrew",
            Manager::Apt => "paquet apt",
            Manager::Dnf => "paquet dnf",
            Manager::Pacman => "paquet pacman",
            Manager::Npm => "paquet npm",
            Manager::Script => "installeur officiel",
            Manager::XcodeSelect => "outils de développement Apple",
        }
    }
}

/// Recette d'installation : programme et arguments exacts, affichés avant d'être lancés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeSpec {
    pub manager: Manager,
    pub program: &'static str,
    pub args: Vec<&'static str>,
    /// Outil à installer d'abord (`npm` ⇒ Node.js).
    pub needs: Option<ToolId>,
    /// Droits d'administrateur demandés par le système (UAC, pkexec).
    pub elevated: bool,
}

impl RecipeSpec {
    /// Ligne de commande telle que l'utilisateur la taperait.
    pub fn display(&self) -> String {
        let mut parts = vec![self.program.to_string()];
        parts.extend(self.args.iter().map(|a| {
            if a.contains(' ') {
                format!("\"{a}\"")
            } else {
                a.to_string()
            }
        }));
        parts.join(" ")
    }
}

fn winget(id: &'static str) -> RecipeSpec {
    RecipeSpec {
        manager: Manager::Winget,
        program: "winget",
        args: vec![
            "install",
            "--id",
            id,
            "-e",
            "--source",
            "winget",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ],
        needs: None,
        elevated: true,
    }
}

fn brew(formula: &'static str) -> RecipeSpec {
    RecipeSpec {
        manager: Manager::Brew,
        program: "brew",
        args: vec!["install", formula],
        needs: None,
        elevated: false,
    }
}

fn npm(package: &'static str) -> RecipeSpec {
    RecipeSpec {
        manager: Manager::Npm,
        program: "npm",
        args: vec!["install", "-g", package],
        needs: Some(ToolId::Node),
        elevated: false,
    }
}

/// Paquet de la distribution, avec la fenêtre de mot de passe du système (`pkexec`).
fn distro(manager: Manager, packages: &[&'static str]) -> RecipeSpec {
    let mut args = match manager {
        Manager::Apt => vec!["apt-get", "install", "-y"],
        Manager::Dnf => vec!["dnf", "install", "-y"],
        _ => vec!["pacman", "-S", "--noconfirm"],
    };
    args.extend_from_slice(packages);
    RecipeSpec {
        manager,
        program: "pkexec",
        args,
        needs: None,
        elevated: true,
    }
}

fn script_unix(command: &'static str) -> RecipeSpec {
    RecipeSpec {
        manager: Manager::Script,
        program: "sh",
        args: vec!["-c", command],
        needs: None,
        elevated: false,
    }
}

fn script_windows(command: &'static str) -> RecipeSpec {
    RecipeSpec {
        manager: Manager::Script,
        program: "powershell",
        args: vec![
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            command,
        ],
        needs: None,
        elevated: false,
    }
}

/// Recettes de l'outil pour ce système, dans l'ordre de préférence.
pub fn recipes(tool: ToolId, os: Os) -> Vec<RecipeSpec> {
    use Manager::{Apt, Dnf, Pacman};
    match (tool, os) {
        (ToolId::Git, Os::Windows) => vec![winget("Git.Git")],
        (ToolId::Git, Os::Macos) => vec![
            RecipeSpec {
                manager: Manager::XcodeSelect,
                program: "xcode-select",
                args: vec!["--install"],
                needs: None,
                elevated: false,
            },
            brew("git"),
        ],
        (ToolId::Git, Os::Linux) => vec![
            distro(Apt, &["git"]),
            distro(Dnf, &["git"]),
            distro(Pacman, &["git"]),
        ],

        (ToolId::Gh, Os::Windows) => vec![winget("GitHub.cli")],
        (ToolId::Gh, Os::Macos) => vec![brew("gh")],
        (ToolId::Gh, Os::Linux) => vec![
            distro(Apt, &["gh"]),
            distro(Dnf, &["gh"]),
            distro(Pacman, &["github-cli"]),
        ],

        (ToolId::Claude, Os::Windows) => {
            vec![
                script_windows("irm https://claude.ai/install.ps1 | iex"),
                npm("@anthropic-ai/claude-code"),
            ]
        }
        (ToolId::Claude, _) => {
            vec![
                script_unix("curl -fsSL https://claude.ai/install.sh | bash"),
                npm("@anthropic-ai/claude-code"),
            ]
        }

        (ToolId::Codex, Os::Macos) => vec![npm("@openai/codex"), brew("codex")],
        (ToolId::Codex, _) => vec![npm("@openai/codex")],

        (ToolId::Opencode, Os::Windows) => vec![npm("opencode-ai")],
        (ToolId::Opencode, Os::Macos) => {
            vec![
                script_unix("curl -fsSL https://opencode.ai/install | bash"),
                npm("opencode-ai"),
                brew("opencode"),
            ]
        }
        (ToolId::Opencode, Os::Linux) => {
            vec![
                script_unix("curl -fsSL https://opencode.ai/install | bash"),
                npm("opencode-ai"),
            ]
        }

        (ToolId::Node, Os::Windows) => vec![winget("OpenJS.NodeJS.LTS")],
        (ToolId::Node, Os::Macos) => vec![brew("node")],
        (ToolId::Node, Os::Linux) => {
            vec![
                distro(Apt, &["nodejs", "npm"]),
                distro(Dnf, &["nodejs"]),
                distro(Pacman, &["nodejs", "npm"]),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_a_recipe_on_every_os() {
        for tool in ToolId::ALL {
            for os in [Os::Windows, Os::Macos, Os::Linux] {
                assert!(!recipes(tool, os).is_empty(), "{tool:?} sur {os:?}");
            }
        }
    }

    #[test]
    fn displays_the_exact_command() {
        let gh = &recipes(ToolId::Gh, Os::Windows)[0];
        assert!(
            gh.display()
                .starts_with("winget install --id GitHub.cli -e")
        );
        let claude = &recipes(ToolId::Claude, Os::Windows)[0];
        assert_eq!(
            claude.display(),
            "powershell -NoProfile -ExecutionPolicy Bypass -Command \"irm https://claude.ai/install.ps1 | iex\""
        );
        assert_eq!(
            recipes(ToolId::Codex, Os::Linux)[0].needs,
            Some(ToolId::Node)
        );
    }

    #[test]
    fn parses_ids() {
        assert_eq!(ToolId::parse("gh"), Some(ToolId::Gh));
        assert_eq!(ToolId::parse("ffmpeg"), None);
        assert_eq!(ToolId::from_ai(AiId::Opencode).ai(), Some(AiId::Opencode));
    }
}
