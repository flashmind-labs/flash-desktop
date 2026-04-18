# Flash Desktop

**Type**: Tauri 2 Desktop Application (Rust backend + web frontend)
**Bundle ID**: com.flashmindlabs.desktop
**Version**: 0.1.1
**Package Manager**: npm (root) + Cargo (src-tauri/)

**CRITICAL**: This codebase uses `rustfmt` with specific style rules. Run `cargo fmt` before committing. The formatter enforces:
- Imports grouped and sorted (std, external, local)
- Match arms and closures on separate lines with block format
- Function arguments wrapped to multiple lines with proper indentation

Flash Desktop is a menu-bar/system-tray application that connects to the Flash daemon service (useflash.com) via WebSocket. It exposes local tools (filesystem, shell, desktop automation) through the MCP (Model Context Protocol) tool registry to the remote agent.

## Project Structure

```
flash-desktop/
├── src/                     # Web frontend (served by Tauri)
├── src-tauri/
│   ├── src/
│   │   ├── main.rs          # Entry point, Tauri setup, tray, window
│   │   ├── auth.rs          # Token storage and authentication
│   │   ├── config.rs        # Configuration load/save
│   │   ├── mcp.rs           # Tool registry and MCP definitions
│   │   ├── ws.rs            # WebSocket client (daemon communication)
│   │   └── tools/           # MCP tool implementations
│   │       ├── mod.rs       # Tool registration dispatcher
│   │       ├── desktop.rs   # desktop.open_app, desktop.close_app, desktop.notify
│   │       ├── filesystem.rs # File read/write/list operations
│   │       ├── shell.rs     # Shell command execution
│   │       └── system.rs    # System info tools
│   ├── icons/               # App icons (32x32, 128x128, 128x128@2x, icns, ico)
│   ├── capabilities/        # Tauri capability files
│   ├── tauri.conf.json      # Tauri configuration
│   ├── Cargo.toml           # Rust dependencies
│   └── Cargo.lock
├── package.json             # npm scripts and devDependencies
└── package-lock.json
```

## Build & Run

```bash
# Development (with hot reload)
npm run dev

# Production build
npm run build

# Rust-only build (inside src-tauri)
cd src-tauri && cargo build --release

# MANDATORY: Format check before commit
cargo fmt

# Lint
cargo clippy --all-targets

# Run tests
cargo test --all
```

## Architecture

### Core Components

1. **main.rs** - Application entry point
   - Sets up tray icon (left-click toggles window visibility)
   - Creates main WebView window programmatically
   - Initializes WebSocket connection to daemon
   - Handles close button (hides window instead of quitting)
   - Platform-specific chrome control (macOS activation policy, Windows skip_taskbar)

2. **ws.rs** - WebSocket Client
   - Connects to `{server_url}/ws/daemon/{server_id}?token={token}&serverId={server_id}`
   - Handles tool_call messages (delegates to ToolRegistry)
   - Responds with tool_result or error
   - Ping/pong heartbeat
   - Exponential backoff reconnection (1s, 2s, 4s, ... max 60s)
   - Updates tray tooltip based on ConnectionStatus

3. **mcp.rs** - MCP Tool Registry
   - ToolRegistry holds named tools with input schemas
   - Each tool has a name, description, JSON schema, safety tag, and handler function
   - Tools are called via `registry.call(name, input)` returning Result<String, String>

4. **auth.rs** - Authentication
   - Stores/retrieves access token
   - Token stored in config file (mode 0600) — NOT keychain anymore

5. **config.rs** - Configuration
   - Loads from platform config directory (AppData/AppConfig on Windows/macOS)
   - Fields: server_url, server_id, device_name, access_token (optional)
   - save() merges with existing config to preserve access_token

### Tool Registry Pattern

Tools follow the MCP convention:

```rust
registry.register(
    ToolDefinition {
        name: "module.action".to_string(),
        description: "What the tool does".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "param": { "type": "string", "description": "Description" }
            },
            "required": ["param"]
        }),
        safety: "read".to_string(),  // "read" | "write" | "dangerous"
    },
    Arc::new(|input| Box::pin(async move {
        // Handler logic
        Ok("success".to_string())
    })),
);
```

### Platform-Specific Behaviors

**macOS:**
- Tray icon uses template image (monochrome, system-tinted)
- `title_bar_style(Transparent)` for unified toolbar
- `ActivationPolicy::Accessory` hides dock icon when window hidden
- AppleScript for graceful app closing (with killall fallback)

**Windows:**
- `skip_taskbar(true)` when window hidden
- taskkill for app closing

**Unix (non-macOS):**
- pkill for app closing

## Verification Commands

```bash
# MANDATORY: Format code (must run before commit)
cargo fmt

# Lint with clippy
cargo clippy --all-targets 2>&1 | head -50

# Run all tests
cargo test --all

# Build release binary
cargo build --release
ls -la target/release/flash-desktop  # or flash-desktop.exe on Windows

# Check for security issues in dependencies
cargo audit 2>/dev/null || echo "Install cargo-audit for security checks"
```

## Code Conventions

1. **Formatting**: MANDATORY — run `cargo fmt` before every commit. The project uses rustfmt with specific rules:
   - Imports sorted by precedence: std → external crates → local modules
   - Closures use block format: `Arc::new(|input| { Box::pin(async move { ... }) })`
   - Match arms and function arguments wrap to multiple lines
   - Empty lines inside closures are significant

2. **Error Handling**: Return `Result<T, String>` with descriptive error messages
3. **Async**: Use `tokio::spawn` for concurrent tool execution
4. **Safety Tags**: Mark tools with "read", "write", or "dangerous" per MCP spec
5. **Logging**: Use `eprintln!` for diagnostic output (stderr, not stdout)
6. **Config**: Never expose access_token to webview; always merge on save
7. **Tray**: Update tooltip on connection status change
8. **Platform Gates**: Use `#[cfg(target_os = "...")]` for OS-specific code

## Tool Safety Levels

| Level | Description |
|-------|-------------|
| read | Read-only operations (no side effects) |
| write | Operations that modify local state |
| dangerous | Potentially destructive operations |

## Connection Lifecycle

```
App Start
    └── Check for existing token + server_id
        ├── Missing credentials → AuthError status, retry every 2s
        └── Credentials found → Connect to WebSocket
            ├── Connection success → Connected status, handle messages
            ├── Connection failure → Reconnecting status, exponential backoff
            └── Remote close → Reconnecting status, reconnect
```

## Configuration File Location

| OS | Path |
|----|------|
| macOS | `~/Library/Application Support/com.flashmindlabs.desktop/config.json` |
| Linux | `~/.config/com.flashmindlabs.desktop/config.json` |
| Windows | `%APPDATA%\com.flashmindlabs.desktop\config.json` |

## Important Notes

- The app runs as a menu-bar/status-bar only daemon by default
- Window shows on tray icon click or menu "Open Flash"
- Close button hides window (app stays running in tray)
- Cmd-Q (macOS) or "Quit" menu item fully exits
- Tray tooltip reflects connection status: "Connected", "Reconnecting...", "Disconnected", "Not signed in"
- Token is stored in config file (mode 0600) — was previously keychain

## Environment Variables

No special environment variables required. Configuration is loaded from platform-specific config directories.

## Troubleshooting

```bash
# Check if app is running
ps aux | grep flash-desktop

# View recent stderr output (macOS)
log show --predicate 'process == "Flash Desktop"' --last 1h 2>/dev/null || echo "Use Console.app"

# Reset configuration (quit app first)
rm -rf ~/Library/Application\ Support/com.flashmindlabs.desktop

# Rebuild from scratch
cd src-tauri && cargo clean && cargo build --release

# Fix formatting issues (run before committing)
cargo fmt

# Verify formatting
cargo fmt --check
```