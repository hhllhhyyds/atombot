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
}

#[async_trait]
pub trait Tool {
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

mod allowed_dir;
mod filesystem;

pub use allowed_dir::AllowedDirectoriesConfig;
pub use filesystem::read_file::ReadFileTool;
