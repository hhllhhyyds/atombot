//! Tool system — extensible set of tools the agent can call.
//!
//! Each tool implements the [`Tool`] trait, providing:
//!//! - Metadata (name, description, JSON schema for parameters)
//! - Execution logic that returns a string result
//!
//! The [`ToolRegistry`] collects tools and converts them to OpenAI tool format.

use std::collections::HashMap;

use async_openai::types::chat::{ChatCompletionTool, ChatCompletionTools, FunctionObject};
use async_trait::async_trait;

use crate::security::allowed_dir::AllowedDirectoriesConfig;

/// Errors that can occur during tool execution.
#[derive(thiserror::Error, Debug)]
pub enum ToolError {
    /// Tool was called with invalid arguments
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),
    /// Tool execution itself failed
    #[error("Execution failed: {0}")]
    Execution(String),
    /// I/O error (file system, network, etc.)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Requested tool does not exist in the registry
    #[error("Tool not found: {0}")]
    NotFound(String),
}

/// Core trait for all agent tools.
///
/// Implement this trait to add new capabilities to the agent.
/// The trait is `async_trait` for ergonomic async execution.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique name identifying this tool (e.g., `"read_file"`)
    fn name(&self) -> &'static str;
    /// Human-readable description shown to the LLM
    fn description(&self) -> &'static str;
    /// JSON Schema describing the tool's parameters
    fn parameters_schema(&self) -> serde_json::Value;

    /// Build the OpenAI tool definition for this tool.
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

    /// Execute the tool with the given JSON arguments.
    /// Returns a string result to send back to the LLM.
    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError>;
}

/// Registry that holds and manages all available tools.
#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool. Panics if a tool with the same name already exists.
    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        self.tools.insert(name, Box::new(tool));
    }

    /// Create a registry pre-populated with all built-in tools.
    ///
    /// Includes: ReadFile, WriteFile, EditFile, ListDir, Exec, WebSearch, WebFetch.
    /// The workspace path is used for path sandboxing.
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

    /// Execute a tool by name with the given arguments.
    ///
    /// # Errors
    /// Returns [`ToolError::NotFound`] if the tool doesn't exist.
    pub async fn execute(&self, name: &str, args: serde_json::Value) -> Result<String, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        tool.execute(args).await
    }

    /// Build all tools as OpenAI chat completion tools.
    pub fn build_chat_completion_tools(&self) -> Vec<ChatCompletionTools> {
        self.tools
            .values()
            .map(|t| t.build_chat_completion_tools())
            .collect()
    }
}

mod exec;
mod filesystem;
mod web_fetch;
mod web_search;

pub use exec::ExecTool;
pub use filesystem::edit_file::EditFileTool;
pub use filesystem::list_dir::ListDirTool;
pub use filesystem::read_file::ReadFileTool;
pub use filesystem::write_file::WriteFileTool;
pub use web_fetch::WebFetchTool;
pub use web_search::{WebSearchConfig, WebSearchTool};
