#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod config;
mod mcp;
mod tools;
mod ws;

use config::Config;
use mcp::ToolRegistry;
use std::sync::Arc;
use tauri::{Emitter, Manager, WindowEvent};
#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;
use tauri_plugin_autostart::{ManagerExt, MacosLauncher};

/// Hide the app icon from the macOS Dock / Windows taskbar so when the
/// window is closed only the menu-bar / system-tray icon remains.
/// On macOS this is done via Activation Policy (LSUIElement equivalent).
/// On Windows we set `skip_taskbar` per-window. Linux has no analogue —
/// the WM handles the taskbar entry from the window's own visibility.
#[allow(unused_variables)]
fn set_chrome_visible(app: &tauri::AppHandle, visible: bool) {
    #[cfg(target_os = "macos")]
    {
        // Regular = shows in Dock + Cmd-Tab. Accessory = tray-only (no Dock).
        let policy = if visible { ActivationPolicy::Regular } else { ActivationPolicy::Accessory };
        let _ = app.set_activation_policy(policy);
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(window) = app.get_webview_window("main") {
            // skip_taskbar(true) hides from the Windows taskbar.
            let _ = window.set_skip_taskbar(!visible);
        }
    }
}

#[tauri::command]
fn get_config() -> Config {
    let mut cfg = Config::load();
    // Don't expose the access token to the webview
    cfg.access_token = None;
    cfg
}

#[tauri::command]
fn save_config(config: Config) -> Result<(), String> {
    // Preserve the existing access token — never let the webview overwrite it
    let existing = Config::load();
    let merged = Config {
        access_token: existing.access_token,
        ..config
    };
    merged.save()
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

#[tauri::command]
fn get_server_url() -> String {
    Config::load().server_url
}

/// Register this machine as a Flash Desktop device.
/// Called from the web UI (useflash.com) once the user confirms.
#[tauri::command]
fn register_device(
    access_token: String,
    server_id: String,
    device_name: String,
) -> Result<(), String> {
    eprintln!(
        "[flash-desktop] register_device called: server_id={}, device_name={}, token_len={}",
        server_id,
        device_name,
        access_token.len()
    );
    auth::store_token(&access_token).map_err(|e| {
        eprintln!("[flash-desktop] store_token FAILED: {}", e);
        e
    })?;
    eprintln!("[flash-desktop] token stored in keychain ✓");
    let mut cfg = Config::load();
    cfg.server_id = Some(server_id);
    cfg.device_name = device_name;
    cfg.save().map_err(|e| {
        eprintln!("[flash-desktop] config save FAILED: {}", e);
        e
    })?;
    eprintln!("[flash-desktop] config saved ✓");
    Ok(())
}

#[tauri::command]
fn show_main_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        set_chrome_visible(&app, true);
        window.show().ok();
        window.unminimize().ok();
        window.set_focus().ok();
    }
}

#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}


fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]), // no extra args
        ))
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            is_authenticated,
            store_token,
            logout,
            get_hostname,
            get_server_url,
            register_device,
            show_main_window,
            get_autostart,
            set_autostart
        ])
        .on_window_event(|window, event| {
            // Close button hides the window instead of quitting the app
            // (tray icon keeps the daemon running). Cmd-Q still quits.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    window.hide().ok();
                    set_chrome_visible(&window.app_handle(), false);
                }
            }
        })
        .setup(|app| {
            // Build the main window programmatically so we can inject the
            // drag region script on every navigation (tauri.conf.json windows
            // don't support initialization_script).
            // Build the main window. macOS-only options (title bar style,
            // background color, hidden title) are gated below.
            let mut builder = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Flash")
            .inner_size(1200.0, 800.0)
            .min_inner_size(720.0, 480.0)
            .center();

            #[cfg(target_os = "macos")]
            {
                // Transparent macOS titlebar — native drag, traffic lights visible,
                // but the titlebar blends into the window background (which we set
                // to Flash's dark sidebar chrome color so the top strip is themed).
                builder = builder
                    .title_bar_style(tauri::TitleBarStyle::Transparent)
                    .hidden_title(true)
                    .background_color(tauri::webview::Color(0x2e, 0x2c, 0x29, 0xff));
            }

            let _main_window = builder.build()?;

            // Build tray menu — autostart toggle lives in the web Settings →
            // Devices page now (uses get_autostart / set_autostart IPC).
            let open_i = tauri::menu::MenuItem::with_id(app, "open", "Open Flash", true, None::<&str>)?;
            let sep_i = tauri::menu::PredefinedMenuItem::separator(app)?;
            let quit_i = tauri::menu::MenuItem::with_id(app, "quit", "Quit Flash Desktop", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&open_i, &sep_i, &quit_i])?;

            let _tray = tauri::tray::TrayIconBuilder::with_id("flash-tray")
                .tooltip("Flash Desktop — Starting...")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            set_chrome_visible(app, true);
                            window.show().ok();
                            window.unminimize().ok();
                            window.set_focus().ok();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                // Left-click on the tray icon also opens the main window
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            match window.is_visible() {
                                Ok(true) => {
                                    window.hide().ok();
                                    set_chrome_visible(app, false);
                                }
                                _ => {
                                    set_chrome_visible(app, true);
                                    window.show().ok();
                                    window.unminimize().ok();
                                    window.set_focus().ok();
                                }
                            }
                        }
                    }
                })
                .build(app)?;

            // Start WebSocket connection in background
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
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
                            ws::ConnectionStatus::AuthError => "Flash Desktop — Not signed in",
                        };
                        if let Some(tray) = handle.tray_by_id("flash-tray") {
                            tray.set_tooltip(Some(tooltip)).ok();
                        }
                        let _ = handle.emit("flash:status", tooltip);
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
