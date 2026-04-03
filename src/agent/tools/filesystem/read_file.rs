use std::io;

use async_trait::async_trait;

use crate::agent::tools::{allowed_dir::AllowedDirectoriesConfig, Tool, ToolError};

pub struct ReadFileTool {
    allowed_dirs_config: AllowedDirectoriesConfig,
}

impl ReadFileTool {
    pub const fn max_chars() -> usize {
        128_000
    }

    pub fn default_limit() -> usize {
        2000
    }

    pub fn new(allowed_dirs_config: AllowedDirectoriesConfig) -> Self {
        Self {
            allowed_dirs_config,
        }
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a file. Returns numbered lines. \
        Use offset and limit to paginate through large files."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file path to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-indexed, default 1)",
                    "minimum": 1,
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (default 2000)",
                    "minimum": 1,
                },
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");

        let path = self.allowed_dirs_config.canonicalize_under_allowed(path)?;

        if !path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Path {} not found", path.display()),
            )
            .into());
        }
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Path {} is not a file", path.display()),
            )
            .into());
        }

        let content = std::fs::read_to_string(path)?;

        Ok(if content.len() > ReadFileTool::max_chars() {
            format!(
                "{}...\n\n(truncated, showing first 5000 chars)",
                &content[..5000]
            )
        } else {
            content
        })
    }
}
