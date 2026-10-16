//! IA de code (Claude Code, Codex CLI, OpenCode), plan Cadence 2 § 4.
//!
//! - `events` : format d'événements normalisé (`AiEvent`) ;
//! - `staging` : dossier de travail et fichiers de préparation d'une session ;
//! - `claude_stream` : lecture du flux stream-json de Claude Code ;
//! - `claude_code` : processus `claude` d'une session (tours, interruption, approbations) ;
//! - `claude` : reprise de l'ancien moteur d'agents (session headless d'un seul tour).
//!
//! Codex et OpenCode viendront avec l'étape « adaptateurs ».

pub mod claude;
pub mod claude_code;
pub mod claude_stream;
pub mod events;
pub mod staging;
