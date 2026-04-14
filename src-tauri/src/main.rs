#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod config;
mod mcp;
mod tools;
mod ws;

use config::Config;
use mcp::ToolRegistry;
use std::sync::Arc;
use tauri::Manager;

#[tauri::command]
fn get_config() -> Config {
    Config::load()
}

#[tauri::command]
fn save_config(config: Config) -> Result<(), String> {
    config.save()
}

#[tauri::command]
fn is_authenticated() -> bool {
    auth::is_authenticated()
}

#[tauri::command]
fn store_token(token: String) -> Result<(), String> {
    auth::store_token(&token)
}

#[tauri::command]
fn logout() -> Result<(), String> {
    auth::delete_token()
}

#[tauri::command]
fn get_hostname() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown".to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_config, save_config, is_authenticated, store_token, logout, get_hostname
        ])
        .setup(|app| {
            // Build tray menu
            let settings_i = tauri::menu::MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
            let quit_i = tauri::menu::MenuItem::with_id(app, "quit", "Quit Flash Desktop", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&settings_i, &quit_i])?;

            let _tray = tauri::tray::TrayIconBuilder::with_id("flash-tray")
                .tooltip("Flash Desktop — Starting...")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "settings" => {
                        if let Some(window) = app.get_webview_window("settings") {
                            window.show().ok();
                            window.set_focus().ok();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // Hide settings window on startup (tray-only)
            if let Some(window) = app.get_webview_window("settings") {
                window.hide().ok();
            }

            // Start WebSocket connection in background
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Wait a moment for the app to fully initialize
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                let mut registry = ToolRegistry::new();
                tools::register_all(&mut registry);
                let registry = Arc::new(registry);

                let tool_handler: ws::ToolHandler = {
                    let reg = registry.clone();
                    Arc::new(move |name, input| {
                        let reg = reg.clone();
                        Box::pin(async move { reg.call(&name, input).await })
                    })
                };

                let on_status: ws::StatusCallback = {
                    let handle = app_handle.clone();
                    Arc::new(move |status| {
                        let tooltip = match status {
                            ws::ConnectionStatus::Connected => "Flash Desktop — Connected",
                            ws::ConnectionStatus::Reconnecting => "Flash Desktop — Reconnecting...",
                            ws::ConnectionStatus::Disconnected => "Flash Desktop — Disconnected",
                            ws::ConnectionStatus::AuthError => "Flash Desktop — Auth Error",
                        };
                        if let Some(tray) = handle.tray_by_id("flash-tray") {
                            tray.set_tooltip(Some(tooltip)).ok();
                        }
                    })
                };

                let client = ws::WsClient::new();
                client.connect(on_status, tool_handler).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Flash Desktop");
}
