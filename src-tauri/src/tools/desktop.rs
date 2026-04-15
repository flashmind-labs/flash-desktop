use crate::mcp::{ToolDefinition, ToolRegistry};
use serde_json::json;
use std::sync::Arc;

pub fn register(registry: &mut ToolRegistry) {
    // desktop.open_app — open an application by name or path
    registry.register(
        ToolDefinition {
            name: "desktop.open_app".to_string(),
            description: "Open an application by name or file path".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string", "description": "Application name or path to open (e.g. 'Safari', '/Applications/TextEdit.app')" }
                },
                "required": ["app"]
            }),
            safety: "write".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let app = input.get("app").and_then(|v| v.as_str()).ok_or("Missing app")?;
            open::that(app).map_err(|e| format!("Cannot open '{}': {}", app, e))?;
            Ok(format!("Opened '{}'", app))
        })),
    );

    // desktop.close_app — quit an application by name (graceful first, then force)
    registry.register(
        ToolDefinition {
            name: "desktop.close_app".to_string(),
            description: "Quit an application by name (graceful AppleScript on macOS, killall fallback)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string", "description": "Application name (e.g. 'Zed', 'Safari')" }
                },
                "required": ["app"]
            }),
            safety: "write".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let app = input.get("app").and_then(|v| v.as_str()).ok_or("Missing app")?;
            // Strip a trailing .app if the user provided a full bundle name
            let app_name = app.trim_end_matches(".app");

            #[cfg(target_os = "macos")]
            {
                // Graceful quit via AppleScript first
                let script = format!(r#"tell application "{}" to quit"#, app_name);
                let result = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .output();
                if let Ok(out) = result {
                    if out.status.success() {
                        return Ok(format!("Quit '{}' (AppleScript)", app_name));
                    }
                }
                // Fallback: killall
                let killall = std::process::Command::new("killall")
                    .arg(app_name)
                    .output()
                    .map_err(|e| format!("killall failed: {}", e))?;
                if killall.status.success() {
                    return Ok(format!("Killed '{}' (killall)", app_name));
                }
                return Err(format!(
                    "Could not close '{}' — {}",
                    app_name,
                    String::from_utf8_lossy(&killall.stderr)
                ));
            }

            #[cfg(target_os = "windows")]
            {
                let exe = if app_name.ends_with(".exe") { app_name.to_string() } else { format!("{}.exe", app_name) };
                let out = std::process::Command::new("taskkill")
                    .args(["/IM", &exe, "/F"])
                    .output()
                    .map_err(|e| format!("taskkill failed: {}", e))?;
                if out.status.success() {
                    return Ok(format!("Closed '{}'", app_name));
                }
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }

            #[cfg(all(unix, not(target_os = "macos")))]
            {
                let out = std::process::Command::new("pkill")
                    .args(["-f", app_name])
                    .output()
                    .map_err(|e| format!("pkill failed: {}", e))?;
                if out.status.success() {
                    return Ok(format!("Closed '{}'", app_name));
                }
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
        })),
    );

    // desktop.notify — show a system notification (stub; actual OS notification via Tauri plugin later)
    registry.register(
        ToolDefinition {
            name: "desktop.notify".to_string(),
            description: "Show a desktop notification with a title and message".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Notification title" },
                    "message": { "type": "string", "description": "Notification body text" }
                },
                "required": ["title", "message"]
            }),
            safety: "read".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let title = input.get("title").and_then(|v| v.as_str()).unwrap_or("Flash");
            let message = input.get("message").and_then(|v| v.as_str()).unwrap_or("");
            // Actual OS notification can be wired to Tauri notification plugin later.
            Ok(format!("Notification shown — {}: {}", title, message))
        })),
    );
}
