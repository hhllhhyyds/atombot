//! Read file tool — reads file contents with pagination and image detection.

use async_trait::async_trait;

use crate::agent::tools::{Tool, ToolError};
use crate::security::allowed_dir::AllowedDirectoriesConfig;

/// Tool for reading file contents with line numbering and pagination.
pub struct ReadFileTool {
    /// Configuration for allowed directory access
    allowed_dirs_config: AllowedDirectoriesConfig,
}

impl ReadFileTool {
    /// Maximum characters to return in a single read
    pub const fn max_chars() -> usize {
        128_000
    }

    /// Default number of lines to return per page
    pub fn default_limit() -> usize {
        2000
    }

    pub fn new(allowed_dirs_config: AllowedDirectoriesConfig) -> Self {
        Self {
            allowed_dirs_config,
        }
    }
}

/// Returns true if the MIME type is an image.
fn is_image_mime(mime: &str) -> bool {
    mime.starts_with("image/")
}

/// Detect image MIME type from raw bytes using magic number signatures.
/// Supports PNG, JPEG, GIF, WebP, and BMP.
fn detect_image_mime(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, 0x50, 0x4E, 0x47, ..] => Some("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [0x47, 0x49, 0x46, ..] => Some("image/gif"),
        [0x52, 0x49, 0x46, 0x46, ..] => Some("image/webp"),
        [0x42, 0x4D, ..] => Some("image/bmp"),
        _ => None,
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
        // offset is 1-indexed (user-facing), convert to 0-indexed internally
        let offset = args
            .get("offset")
            .and_then(|v| v.as_i64())
            .unwrap_or(1)
            .max(1) as usize;
        let limit = args
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(Self::default_limit() as i64) as usize;

        // Security: canonicalize and verify path is within allowed directories
        let path = self.allowed_dirs_config.canonicalize_under_allowed(path)?;

        if !path.exists() {
            return Ok(format!("Error: File not found: {}", path.display()));
        }

        if !path.is_file() {
            return Ok(format!("Error: Not a file: {}", path.display()));
        }

        let raw = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => return Ok(format!("Error reading file: {}", e)),
        };

        if raw.is_empty() {
            return Ok(format!("(Empty file: {})", path.display()));
        }

        // Detect and handle image files
        if let Some(mime) = detect_image_mime(&raw) {
            if is_image_mime(mime) {
                let size = raw.len();
                return Ok(format!(
                    "(Image file: {}, {} bytes, MIME: {})\n\n\
                     Image content cannot be displayed as text. \
                     The file is available at: {}",
                    path.display(),
                    size,
                    mime,
                    path.display()
                ));
            }
        }

        // Try to read as UTF-8 text
        let content = match String::from_utf8(raw) {
            Ok(c) => c,
            Err(_) => {
                let mime = mime_guess::from_path(&path)
                    .first()
                    .map(|m| m.as_ref().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                return Ok(format!(
                    "Error: Cannot read binary file {} (MIME: {}). Only UTF-8 text and images are supported.",
                    path.display(),
                    mime
                ));
            }
        };

        let all_lines: Vec<&str> = content.split('\n').collect();
        let total = all_lines.len();

        if offset > total {
            return Ok(format!(
                "Error: offset {} is beyond end of file ({} lines)",
                offset, total
            ));
        }

        let start = offset - 1;
        let end = std::cmp::min(start + limit, total);

        // Build output with line numbers (e.g., `1| let x = 1;`)
        let mut numbered = Vec::with_capacity(end - start);
        for (i, line) in all_lines[start..end].iter().enumerate() {
            numbered.push(format!("{}| {}", start + i + 1, line));
        }

        let mut result = numbered.join("\n");

        // Truncate by character count if still too large
        if result.len() > Self::max_chars() {
            let mut trimmed: Vec<String> = Vec::new();
            let mut chars = 0;
            for line in &numbered {
                chars += line.len() + 1;
                if chars > Self::max_chars() {
                    break;
                }
                trimmed.push(line.clone());
            }
            let trimmed_end = start + trimmed.len();
            result = trimmed.join("\n");
            return Ok(format!(
                "{}...\n\n(truncated, showing first {} chars)\n\
                 (Showing lines {}-{} of {} total. Use offset={} to continue.)",
                result,
                Self::max_chars(),
                offset,
                trimmed_end,
                total,
                trimmed_end + 1
            ));
        }

        // Add pagination hints
        if end < total {
            result.push_str(&format!(
                "\n\n(Showing lines {}-{} of {}. Use offset={} to continue.)",
                offset,
                end,
                total,
                end + 1
            ));
        } else {
            result.push_str(&format!("\n\n(End of file — {} lines total)", total));
        }

        Ok(result)
    }
}
