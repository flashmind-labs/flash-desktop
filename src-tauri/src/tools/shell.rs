use crate::mcp::{ToolDefinition, ToolRegistry};
use serde_json::json;
use std::sync::Arc;

pub fn register(registry: &mut ToolRegistry) {
    // shell.run — execute arbitrary shell command
    registry.register(
        ToolDefinition {
            name: "shell.run".to_string(),
            description: "Execute a shell command and return its output (stdout, stderr, exit code)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to execute" },
                    "cwd": { "type": "string", "description": "Working directory (optional)" }
                },
                "required": ["command"]
            }),
            safety: "destructive".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let command = input.get("command").and_then(|v| v.as_str()).ok_or("Missing command")?;
            let cwd = input.get("cwd").and_then(|v| v.as_str());

            let shell = if cfg!(target_os = "windows") { "cmd" } else { "sh" };
            let flag = if cfg!(target_os = "windows") { "/C" } else { "-c" };

            let mut cmd = std::process::Command::new(shell);
            cmd.arg(flag).arg(command);
            if let Some(dir) = cwd {
                cmd.current_dir(shellexpand::tilde(dir).to_string());
            }

            let output = cmd.output().map_err(|e| format!("Failed to run: {}", e))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            let mut result = format!("Exit code: {}\n", output.status.code().unwrap_or(-1));
            if !stdout.is_empty() {
                result.push_str(&format!("STDOUT:\n{}\n", stdout));
            }
            if !stderr.is_empty() {
                result.push_str(&format!("STDERR:\n{}\n", stderr));
            }

            // Truncate if too long
            if result.len() > 50_000 {
                result.truncate(50_000);
                result.push_str("\n...[truncated]");
            }

            Ok(result)
        })),
    );

    // shell.run_safe — pre-approved commands only
    registry.register(
        ToolDefinition {
            name: "shell.run_safe".to_string(),
            description: "Run a safe, read-only shell command (ls, cat, head, tail, git status, git log, git diff, pwd, whoami, df, du, uname, date, which, wc, file, echo, env, printenv)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Safe shell command to execute" },
                    "cwd": { "type": "string", "description": "Working directory (optional)" }
                },
                "required": ["command"]
            }),
            safety: "write".to_string(),
        },
        Arc::new(|input| Box::pin(async move {
            let command = input.get("command").and_then(|v| v.as_str()).ok_or("Missing command")?;

            let safe_prefixes = [
                "ls", "cat", "head", "tail", "git status", "git log", "git diff",
                "pwd", "whoami", "df", "du", "uname", "date", "which", "wc", "file",
                "echo", "env", "printenv",
            ];
            let is_safe = safe_prefixes.iter().any(|p| command.trim_start().starts_with(p));
            if !is_safe {
                return Err(format!(
                    "Command '{}' is not in the safe list. Use shell.run for arbitrary commands.",
                    command
                ));
            }

            let shell = if cfg!(target_os = "windows") { "cmd" } else { "sh" };
            let flag = if cfg!(target_os = "windows") { "/C" } else { "-c" };

            let mut cmd = std::process::Command::new(shell);
            cmd.arg(flag).arg(command);
            if let Some(cwd) = input.get("cwd").and_then(|v| v.as_str()) {
                cmd.current_dir(shellexpand::tilde(cwd).to_string());
            }

            let output = cmd.output().map_err(|e| format!("Failed to run: {}", e))?;
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        })),
    );
}
