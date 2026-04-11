//! Write file tool — creates or overwrites files with path validation.

use async_trait::async_trait;
use serde_json::Value;

use crate::agent::tools::{Tool, ToolError};
use crate::security::allowed_dir::AllowedDirectoriesConfig;

/// Tool for writing content to files.
pub struct WriteFileTool {
    /// Configuration for allowed directory access
    allowed_dirs_config: AllowedDirectoriesConfig,
}

impl WriteFileTool {
    pub fn new(allowed_dirs_config: AllowedDirectoriesConfig) -> Self {
        Self { allowed_dirs_config }
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Write content to a file at the given path. Creates parent directories if needed."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file path to write to"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write"
                },
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");

        if path_str.is_empty() {
            return Err(ToolError::InvalidArgs("path is required".to_string()));
        }

        if content.is_empty() {
            return Err(ToolError::InvalidArgs("content is required".to_string()));
        }

        // Security: validate path is within allowed directories
        // Also handles non-existent files by checking parent directory
        let path = self.allowed_dirs_config.resolve_for_write(path_str)?;

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&path, content)?;

        Ok(format!("Successfully wrote {} bytes to {}", content.len(), path.display()))
    }
}
