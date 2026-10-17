//! IA de code (Claude Code, Codex CLI, OpenCode), plan Cadence 2 § 4.
//!
//! - `events` : format d'événements normalisé (`AiEvent`) ;
//! - `staging` : dossier de travail et fichiers de préparation d'une session ;
//! - `claude_stream` : lecture du flux stream-json de Claude Code ;
//! - `claude_code` : processus `claude` d'une session (tours, interruption, approbations) ;
//! - `codex_stream`, `codex` : protocole et processus `codex app-server` ;
//! - `opencode_stream`, `opencode` : flux et serveur `opencode serve` ;
//! - `session` : contrat commun (`AiSession`), seul connu du chat ;
//! - `claude` : reprise de l'ancien moteur d'agents (session headless d'un seul tour).

pub mod claude;
pub mod claude_code;
pub mod claude_stream;
pub mod codex;
pub mod codex_stream;
pub mod events;
pub mod opencode;
pub mod opencode_stream;
pub mod process;
pub mod prompt;
pub mod session;
pub mod staging;
