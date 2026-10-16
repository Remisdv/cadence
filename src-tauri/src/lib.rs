//! Cadence — cœur Rust.
//!
//! Tout ce qui touche au réseau, au disque, aux secrets et aux processus vit ici.
//! Le webview ne fait que rendre et appeler des commandes : il n'a ni jeton,
//! ni accès direct à l'API GitHub.
//!
//! Cadence 2 (0.3.0) : hôte entre les IA de code et des plugins. Socle en place :
//! `app_settings` (préférences v2), `secrets` (trousseau générique), `plugins` (registre,
//! réglages globaux). Le code GitHub de Cadence 1 reste compilé jusqu'à son départ dans le
//! plugin `gestion-projet`.

pub mod ai;
pub mod api;
pub mod app_settings;
pub mod auth;
pub mod commands;
pub mod domain;
pub mod error;
pub mod events;
pub mod plugins;
pub mod project_config;
pub mod secrets;
pub mod state;
pub mod store;
pub mod sync;
pub mod vcs;

use std::sync::Arc;

use tauri::{Manager, RunEvent};
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
        // Mise à jour automatique : vérification, téléchargement et installation pilotés par
        // le front (Réglages › À propos) ; `process` sert au redémarrage qui suit.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let paths = state::AppPaths {
                config_dir: app.path().app_config_dir()?,
                data_dir: app.path().app_data_dir()?,
                cache_dir: app.path().app_local_data_dir()?.join("cache"),
                log_dir: app.path().app_log_dir()?,
            };
            let version = app.package_info().version.to_string();
            let secrets: Arc<dyn secrets::SecretStore> = Arc::new(secrets::KeyringSecrets);
            let tokens = Arc::new(auth::token_store::SecretEntry::github(secrets.clone()));
            let handle = app.handle().clone();
            let plugin_events: plugins::host::StateSink =
                Arc::new(move |event| events::emit(&handle, events::PLUGIN_STATE, event));
            let state = state::AppState::new(paths, &version, secrets, tokens, plugin_events);
            tauri::async_runtime::spawn(plugins::host::run_idle_reaper(Arc::downgrade(&state.plugin_host)));
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_status,
            commands::app::app_settings_get,
            commands::app::app_settings_set,
            commands::plugins::plugins_installed,
            commands::plugins::plugins_set_enabled,
            commands::plugins::plugins_reorder,
            commands::plugins::plugin_settings_get,
            commands::plugins::plugin_settings_set,
            commands::plugins::plugin_secret_set,
            commands::plugins::plugin_secret_has,
            commands::plugins::plugin_secret_delete,
            commands::plugins::plugin_install_zip,
            commands::plugins::plugin_install_dev,
            commands::plugins::plugin_uninstall,
            commands::plugins::plugin_status,
            commands::plugins::plugin_restart,
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
            commands::versions::version_detail,
            commands::versions::version_close,
            commands::issues::issues_list,
            commands::issues::issue_get,
            commands::issues::issue_create,
            commands::prs::prs_list,
            commands::prs::reviews_pending,
            commands::prs::pr_detail,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // Fermeture : les serveurs de plugin s'arrêtent proprement (stdin fermé, grâce, kill).
            if let RunEvent::Exit = event {
                let host = app.state::<state::AppState>().plugin_host.clone();
                tauri::async_runtime::block_on(host.stop_all());
            }
        });
}
