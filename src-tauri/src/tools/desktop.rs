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
