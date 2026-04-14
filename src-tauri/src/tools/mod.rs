pub mod filesystem;
pub mod shell;
pub mod system;
pub mod desktop;

use crate::mcp::ToolRegistry;

pub fn register_all(registry: &mut ToolRegistry) {
    filesystem::register(registry);
    shell::register(registry);
    system::register(registry);
    desktop::register(registry);
}
