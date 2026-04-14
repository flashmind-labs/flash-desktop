use serde::Serialize;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub safety: String, // "read", "write", "destructive"
}

pub type ToolFn = Arc<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
        + Send
        + Sync,
>;

pub struct ToolRegistry {
    tools: HashMap<String, (ToolDefinition, ToolFn)>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, def: ToolDefinition, handler: ToolFn) {
        self.tools.insert(def.name.clone(), (def, handler));
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|(def, _)| def.clone()).collect()
    }

    pub async fn call(&self, name: &str, input: serde_json::Value) -> Result<String, String> {
        let (_, handler) = self
            .tools
            .get(name)
            .ok_or_else(|| format!("Unknown tool: {}", name))?;
        handler(input).await
    }
}
