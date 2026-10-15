//! Cadence — cœur Rust.
//!
//! Tout ce qui touche au réseau, au disque, aux secrets et aux processus vit ici.
//! Le webview ne fait que rendre et appeler des commandes : il n'a ni jeton,
//! ni accès direct à l'API GitHub.

pub mod agents;
pub mod api;
pub mod auth;
pub mod commands;
pub mod domain;
pub mod error;
pub mod events;
pub mod local_settings;
pub mod project_config;
pub mod state;
pub mod store;
pub mod sync;
pub mod vcs;

use std::sync::Arc;

use tauri::Manager;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("cadence_lib=info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let cache_dir = app.path().app_local_data_dir()?.join("cache");
            let tokens = Arc::new(auth::token_store::KeyringStore::github());
            app.manage(state::AppState::new(&config_dir, cache_dir, tokens));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::auth_start_device_flow,
            commands::auth::auth_start_web_flow,
            commands::auth::auth_use_gh_cli,
            commands::auth::auth_status,
            commands::auth::auth_set_token,
            commands::auth::auth_sign_out,
            commands::auth::cli_agent_test,
            commands::auth::cli_agent_set_path,
            commands::auth::cli_agent_set_enabled,
            commands::project::project_current,
            commands::project::project_list_repos,
            commands::project::project_open,
            commands::project::project_owners,
            commands::project::project_create,
            commands::versions::versions_list,
            commands::versions::version_current,
            commands::versions::version_close,
            commands::issues::issues_list,
            commands::issues::issue_get,
            commands::issues::issue_create,
            commands::issues::issue_answer_cto,
            commands::prs::prs_list,
            commands::prs::reviews_pending,
            commands::prs::pr_detail,
            commands::agents::agent_engines_get,
            commands::agents::agent_engines_set,
            commands::agents::agents_overview,
            commands::agents::agent_start,
            commands::agents::agent_control,
            commands::agents::agent_actions,
            commands::agents::agent_apply,
            commands::agents::agent_latest_run,
            commands::agents::agent_run_get,
            commands::agents::agent_reject,
            commands::agents::agent_edit_body,
            commands::agents::agent_edit_proposal,
            commands::settings::settings_get,
            commands::settings::settings_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
