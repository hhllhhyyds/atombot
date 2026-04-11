use async_trait::async_trait;
use serde_json::Value;

use crate::agent::tools::{allowed_dir::AllowedDirectoriesConfig, Tool, ToolError};

pub struct EditFileTool {
    allowed_dirs_config: AllowedDirectoriesConfig,
}

impl EditFileTool {
    pub fn new(allowed_dirs_config: AllowedDirectoriesConfig) -> Self {
        Self { allowed_dirs_config }
    }
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn description(&self) -> &'static str {
        "Edit a file by replacing old_text with new_text. Supports minor whitespace/line-ending differences. Set replace_all=true to replace every occurrence."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file path to edit"
                },
                "old_text": {
                    "type": "string",
                    "description": "The text to find and replace"
                },
                "new_text": {
                    "type": "string",
                    "description": "The text to replace with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default false)"
                },
            },
            "required": ["path", "old_text", "new_text"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let old_text = args.get("old_text").and_then(|v| v.as_str()).unwrap_or("");
        let new_text = args.get("new_text").and_then(|v| v.as_str()).unwrap_or("");
        let replace_all = args.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);

        if path_str.is_empty() {
            return Err(ToolError::InvalidArgs("path is required".to_string()));
        }
        if old_text.is_empty() {
            return Err(ToolError::InvalidArgs("old_text is required".to_string()));
        }

        let path = self.allowed_dirs_config.canonicalize_under_allowed(path_str)?;

        let raw = std::fs::read(&path)?;
        let uses_crlf = raw.windows(2).any(|w| w == b"\r\n");

        let content = String::from_utf8_lossy(&raw).replace("\r\n", "\n");
        let old_normalized = old_text.replace("\r\n", "\n");

        // Try exact match first
        let count = content.matches(&old_normalized).count();
        if count == 0 {
            return Err(ToolError::Execution(format!(
                "old_text not found in {}. Provide more context to make it unique.",
                path_str
            )));
        }

        if count > 1 && !replace_all {
            return Err(ToolError::Execution(format!(
                "Warning: old_text appears {} times. Provide more context to make it unique, or set replace_all=true.",
                count
            )));
        }

        let new_normalized = new_text.replace("\r\n", "\n");
        let new_content = if replace_all {
            content.replace(&old_normalized, &new_normalized)
        } else {
            content.replacen(&old_normalized, &new_normalized, 1)
        };

        let final_content = if uses_crlf {
            new_content.replace("\n", "\r\n")
        } else {
            new_content
        };

        std::fs::write(&path, final_content.as_bytes())?;

        Ok(format!("Successfully edited {}", path.display()))
    }
}
