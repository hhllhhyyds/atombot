use std::collections::HashMap;

use async_openai::types::chat::{ChatCompletionTool, ChatCompletionTools, FunctionObject};
use async_trait::async_trait;

#[derive(thiserror::Error, Debug)]
pub enum ToolError {
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),
    #[error("Execution failed: {0}")]
    Execution(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Tool not found: {0}")]
    NotFound(String),
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters_schema(&self) -> serde_json::Value;

    fn build_chat_completion_tools(&self) -> ChatCompletionTools {
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: self.name().to_string(),
                description: Some(self.description().to_string()),
                parameters: Some(self.parameters_schema()),
                strict: None,
            },
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError>;
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        self.tools.insert(name, Box::new(tool));
    }

    /// Create a registry with all default tools registered.
    pub fn with_defaults(workspace: &str) -> Self {
        let mut registry = Self::new();
        let config = AllowedDirectoriesConfig::default().with_workspace(workspace);
        registry.register(ReadFileTool::new(config.clone()));
        registry.register(WriteFileTool::new(config.clone()));
        registry.register(EditFileTool::new(config.clone()));
        registry.register(ListDirTool::new(config));
        registry.register(ExecTool::new(60, Some(workspace.to_string()), true));
        registry.register(WebSearchTool::new(WebSearchConfig::from_env(), std::env::var("WEB_SEARCH_PROXY").ok()));
        registry.register(WebFetchTool::new(50_000, std::env::var("WEB_SEARCH_PROXY").ok()));
        registry
    }

    pub async fn execute(&self, name: &str, args: serde_json::Value) -> Result<String, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        tool.execute(args).await
    }

    pub fn build_chat_completion_tools(&self) -> Vec<ChatCompletionTools> {
        self.tools
            .values()
            .map(|t| t.build_chat_completion_tools())
            .collect()
    }
}

mod allowed_dir;
mod exec;
mod filesystem;
mod web_fetch;
mod web_search;

pub use allowed_dir::AllowedDirectoriesConfig;

pub use exec::ExecTool;
pub use filesystem::edit_file::EditFileTool;
pub use filesystem::list_dir::ListDirTool;
pub use filesystem::read_file::ReadFileTool;
pub use filesystem::write_file::WriteFileTool;
pub use web_fetch::WebFetchTool;
pub use web_search::{WebSearchConfig, WebSearchTool};
