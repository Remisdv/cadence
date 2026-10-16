//! IA de code (Claude Code, Codex CLI, OpenCode), plan Cadence 2 § 4.
//!
//! - `events` : format d'événements normalisé (`AiEvent`) ;
//! - `claude_stream` : lecture du flux stream-json de Claude Code ;
//! - `claude` : reprise de l'ancien moteur d'agents (session headless d'un seul tour).
//!
//! Codex et OpenCode viendront avec l'étape « adaptateurs ».

pub mod claude;
pub mod claude_stream;
pub mod events;
