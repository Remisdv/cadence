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
pub mod chat;
pub mod commands;
pub mod domain;
pub mod error;
pub mod events;
pub mod gateway;
pub mod plugins;
pub mod project_config;
pub mod protocol;
pub mod repos;
pub mod secrets;
pub mod state;
pub mod store;
pub mod sync;
pub mod tools;
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
        // Fichiers `ui/` des plugins, servis à leur iframe (plan § 2.7).
        .register_asynchronous_uri_scheme_protocol(protocol::SCHEME, |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                let state = app.state::<state::AppState>();
                responder.respond(protocol::respond(&state.plugins, &request));
            });
        })
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
            let handle = app.handle().clone();
            let plugin_notifications: plugins::host::PluginEventSink =
                Arc::new(move |event| events::emit(&handle, events::PLUGIN_EVENT, event));
            let mut state = state::AppState::new(paths, &version, secrets, tokens, plugin_events, plugin_notifications);
            tauri::async_runtime::spawn(plugins::host::run_idle_reaper(Arc::downgrade(&state.plugin_host)));

            // Passerelle MCP des IA, sur un port libre de 127.0.0.1.
            let handle = app.handle().clone();
            let gateway_events: gateway::server::GatewayEventSink = Arc::new(move |event| match event {
                gateway::core_tools::GatewayEvent::OpenView(request) => events::emit(&handle, events::UI_OPEN_VIEW, request),
                gateway::core_tools::GatewayEvent::RunReport(report) => {
                    events::emit(&handle, events::RUN_REPORT, report.clone());
                    // Le rapport rejoint l'historique de l'exécution (sa conversation).
                    if let Some(chat) = handle.try_state::<state::AppState>().and_then(|s| s.chat.clone()) {
                        tauri::async_runtime::spawn(async move {
                            if let Err(err) = chat.record_report(report).await {
                                tracing::warn!(%err, "rapport d'exécution non enregistré");
                            }
                        });
                    }
                }
            });
            let context = gateway::server::GatewayContext::new(
                &version,
                state.plugin_host.clone(),
                gateway::registry_lister(state.plugins.clone()),
                gateway_events,
            );
            match tauri::async_runtime::block_on(gateway::server::Gateway::start(context, 0)) {
                Ok(gateway) => state.gateway = Some(gateway),
                Err(err) => tracing::error!(%err, "passerelle MCP impossible à démarrer"),
            }

            // Chat du cœur : historique local, une IA dans un dossier neutre par conversation.
            let config = chat::ChatConfig::new(
                app.path().app_data_dir()?.join("sessions"),
                app.path().app_local_data_dir()?.join("ai-sessions"),
            );
            ai::staging::sweep(&config.staging_root);
            let settings = state.settings.clone();
            let locate: chat::AiLocator = Arc::new(move |ai| {
                let tool = tools::ToolId::from_ai(ai);
                let current = settings.get();
                if !current.ai.get(ai).enabled {
                    return Err(error::AppError::Validation(format!("{} est désactivé dans les réglages.", tool.name())));
                }
                tools::detect::locate(tool, tools::configured_path(&current, tool).as_deref()).ok_or_else(|| {
                    error::AppError::Validation(format!(
                        "{} est introuvable : installez-le ou indiquez son chemin dans Réglages › Outils.",
                        tool.name()
                    ))
                })
            });
            let registry = state.plugins.clone();
            let skills: chat::SkillSource = Arc::new(move || ai::staging::plugin_skills(&registry));
            let gateway_access =
                state.gateway.as_ref().map(|g| chat::GatewayAccess { url: g.url(), context: g.context().clone() });
            let handle = app.handle().clone();
            let chat_events: chat::ChatSink = Arc::new(move |notice| match notice {
                chat::ChatNotice::Event(payload) => events::emit(&handle, events::CHAT_EVENT, payload),
                chat::ChatNotice::State(payload) => events::emit(&handle, events::CHAT_STATE, payload),
            });
            let history = app.path().app_data_dir()?.join("chat.sqlite3");
            let opened = tauri::async_runtime::block_on(async move {
                let store = chat::store::ChatStore::open(&history).await?;
                store.reset_running().await?;
                Ok::<_, error::AppError>(store)
            });
            match opened {
                Ok(store) => {
                    let service = chat::ChatService::new(store, config, locate, skills, gateway_access, chat_events);
                    tauri::async_runtime::spawn(chat::run_idle_closer(
                        Arc::downgrade(&service),
                        std::time::Duration::from_secs(60),
                    ));
                    state.chat = Some(service);
                }
                Err(err) => tracing::error!(%err, "historique du chat illisible"),
            }
            app.manage(state);

            // Catalogue : relu au lancement puis toutes les 6 h ; badge « mises à jour ».
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let state = handle.state::<state::AppState>();
                    let url = state.settings.get().catalog_url.unwrap_or_else(|| plugins::catalog::DEFAULT_URL.to_string());
                    match state.catalog.get(&url, false).await {
                        Ok(fetched) => {
                            let updates = plugins::catalog::view(&url, &fetched, &state.plugins).updates;
                            events::emit(&handle, events::CATALOG_UPDATED, events::CatalogUpdated { updates });
                        }
                        Err(err) => tracing::info!(%err, "catalogue des plugins injoignable"),
                    }
                    tokio::time::sleep(plugins::catalog::MAX_AGE).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_status,
            commands::app::app_settings_get,
            commands::app::app_settings_set,
            commands::tools::tools_list,
            commands::tools::tool_install,
            commands::tools::tool_login,
            commands::tools::tool_cancel,
            commands::tools::tool_set_path,
            commands::tools::ai_test,
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
            commands::plugins::plugin_call,
            commands::plugins::catalog_get,
            commands::plugins::plugin_install,
            commands::plugins::plugin_update,
            commands::chat::chat_sessions,
            commands::chat::chat_session_get,
            commands::chat::chat_send,
            commands::chat::chat_interrupt,
            commands::chat::chat_answer,
            commands::actions::action_run,
            commands::chat::chat_approve,
            commands::chat::chat_session_rename,
            commands::chat::chat_session_delete,
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
                let state = app.state::<state::AppState>();
                if let Some(chat) = &state.chat {
                    tauri::async_runtime::block_on(chat.shutdown());
                }
                if let Some(gateway) = &state.gateway {
                    gateway.shutdown();
                }
                tauri::async_runtime::block_on(state.plugin_host.stop_all());
            }
        });
}
