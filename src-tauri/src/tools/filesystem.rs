use crate::mcp::{ToolDefinition, ToolRegistry};
use serde_json::json;
use std::sync::Arc;

pub fn register(registry: &mut ToolRegistry) {
    // fs.list
    registry.register(
        ToolDefinition {
            name: "fs.list".to_string(),
            description: "List files and directories at a path with metadata (size, modified date)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path to list" },
                    "recursive": { "type": "boolean", "description": "List recursively", "default": false }
                },
                "required": ["path"]
            }),
            safety: "read".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let path = input.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?;
            let _recursive = input.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);

            let path = shellexpand::tilde(path).to_string();
            let entries = std::fs::read_dir(&path).map_err(|e| format!("Cannot read {}: {}", path, e))?;

            let mut results = Vec::new();
            for entry in entries.flatten() {
                let meta = entry.metadata().ok();
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                results.push(format!(
                    "{}{} ({})",
                    name,
                    if is_dir { "/" } else { "" },
                    if is_dir { "dir".to_string() } else { format!("{} bytes", size) }
                ));
            }

            Ok(results.join("\n"))
        })),
    );

    // fs.read
    registry.register(
        ToolDefinition {
            name: "fs.read".to_string(),
            description: "Read the contents of a file".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read" },
                    "max_bytes": { "type": "integer", "description": "Max bytes to read (default 100000)", "default": 100000 }
                },
                "required": ["path"]
            }),
            safety: "read".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let path = input.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?;
            let max = input.get("max_bytes").and_then(|v| v.as_u64()).unwrap_or(100_000) as usize;

            let path = shellexpand::tilde(path).to_string();
            let content = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read {}: {}", path, e))?;

            if content.len() > max {
                Ok(format!("{}...[truncated at {} bytes]", &content[..max], max))
            } else {
                Ok(content)
            }
        })),
    );

    // fs.write
    registry.register(
        ToolDefinition {
            name: "fs.write".to_string(),
            description: "Write content to a file (creates or overwrites)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to write" },
                    "content": { "type": "string", "description": "Content to write" }
                },
                "required": ["path", "content"]
            }),
            safety: "write".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let path = input.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?;
            let content = input.get("content").and_then(|v| v.as_str()).ok_or("Missing content")?;

            let path = shellexpand::tilde(path).to_string();
            if let Some(parent) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(&path, content).map_err(|e| format!("Cannot write {}: {}", path, e))?;
            Ok(format!("Written {} bytes to {}", content.len(), path))
        })),
    );

    // fs.delete
    registry.register(
        ToolDefinition {
            name: "fs.delete".to_string(),
            description: "Delete a file or directory".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to delete" }
                },
                "required": ["path"]
            }),
            safety: "destructive".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let path = input.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?;
            let path = shellexpand::tilde(path).to_string();
            let meta = std::fs::metadata(&path).map_err(|e| format!("Cannot access {}: {}", path, e))?;
            if meta.is_dir() {
                std::fs::remove_dir_all(&path).map_err(|e| format!("Cannot delete {}: {}", path, e))?;
            } else {
                std::fs::remove_file(&path).map_err(|e| format!("Cannot delete {}: {}", path, e))?;
            }
            Ok(format!("Deleted {}", path))
        })),
    );

    // fs.move
    registry.register(
        ToolDefinition {
            name: "fs.move".to_string(),
            description: "Move or rename a file or directory".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "from": { "type": "string", "description": "Source path" },
                    "to": { "type": "string", "description": "Destination path" }
                },
                "required": ["from", "to"]
            }),
            safety: "write".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let from = input.get("from").and_then(|v| v.as_str()).ok_or("Missing from")?;
            let to = input.get("to").and_then(|v| v.as_str()).ok_or("Missing to")?;
            let from = shellexpand::tilde(from).to_string();
            let to = shellexpand::tilde(to).to_string();
            std::fs::rename(&from, &to).map_err(|e| format!("Cannot move {} to {}: {}", from, to, e))?;
            Ok(format!("Moved {} to {}", from, to))
        })),
    );

    // fs.search
    registry.register(
        ToolDefinition {
            name: "fs.search".to_string(),
            description: "Search for files by glob pattern in a directory".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory to search in" },
                    "pattern": { "type": "string", "description": "Glob pattern (e.g., *.txt)" }
                },
                "required": ["path", "pattern"]
            }),
            safety: "read".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let path = input.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?;
            let pattern = input.get("pattern").and_then(|v| v.as_str()).ok_or("Missing pattern")?;
            let path = shellexpand::tilde(path).to_string();

            let full_pattern = format!("{}/{}", path, pattern);
            let matches: Vec<String> = glob::glob(&full_pattern)
                .map_err(|e| format!("Invalid pattern: {}", e))?
                .filter_map(|r| r.ok())
                .take(100)
                .map(|p| p.to_string_lossy().to_string())
                .collect();

            if matches.is_empty() {
                Ok("No files found".to_string())
            } else {
                Ok(matches.join("\n"))
            }
        })),
    );
}
