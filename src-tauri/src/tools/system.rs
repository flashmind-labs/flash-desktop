use crate::mcp::{ToolDefinition, ToolRegistry};
use serde_json::json;
use std::sync::Arc;

pub fn register(registry: &mut ToolRegistry) {
    // system.info — OS name, hostname, home directory
    registry.register(
        ToolDefinition {
            name: "system.info".to_string(),
            description: "Return OS name, hostname, and home directory".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            safety: "read".to_string(),
        },
        Arc::new(|_input| Box::pin(async move {
            let os = std::env::consts::OS;
            let hostname = hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            let home = dirs::home_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            Ok(format!("OS: {}\nHostname: {}\nHome: {}", os, hostname, home))
        })),
    );

    // system.clipboard — read clipboard text
    registry.register(
        ToolDefinition {
            name: "system.clipboard".to_string(),
            description: "Read the current clipboard text".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            safety: "read".to_string(),
        },
        Arc::new(|_input| Box::pin(async move {
            let mut clipboard = arboard::Clipboard::new()
                .map_err(|e| format!("Cannot access clipboard: {}", e))?;
            let text = clipboard.get_text()
                .map_err(|e| format!("Cannot read clipboard: {}", e))?;
            Ok(text)
        })),
    );

    // system.set_clipboard — write clipboard text
    registry.register(
        ToolDefinition {
            name: "system.set_clipboard".to_string(),
            description: "Set the clipboard text".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Text to copy to clipboard" }
                },
                "required": ["text"]
            }),
            safety: "write".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let text = input.get("text").and_then(|v| v.as_str()).ok_or("Missing text")?;
            let mut clipboard = arboard::Clipboard::new()
                .map_err(|e| format!("Cannot access clipboard: {}", e))?;
            clipboard.set_text(text)
                .map_err(|e| format!("Cannot set clipboard: {}", e))?;
            Ok(format!("Copied {} chars to clipboard", text.len()))
        })),
    );
}
