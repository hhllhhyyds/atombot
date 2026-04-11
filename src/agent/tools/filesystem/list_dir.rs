//! List directory tool — explores directory structure with filtering.

use std::collections::HashSet;
use std::io;

use async_trait::async_trait;
use serde_json::Value;

use crate::agent::tools::{allowed_dir::AllowedDirectoriesConfig, Tool, ToolError};

/// Tool for listing directory contents.
pub struct ListDirTool {
    /// Configuration for allowed directory access
    allowed_dirs_config: AllowedDirectoriesConfig,
}

impl ListDirTool {
    pub fn new(allowed_dirs_config: AllowedDirectoriesConfig) -> Self {
        Self { allowed_dirs_config }
    }
}

/// Directories that are auto-ignored when listing (common build/cache/artifacts).
const IGNORE_DIRS: [&str; 12] = [
    ".git",
    "node_modules",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".coverage",
];

#[async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &'static str {
        "list_dir"
    }

    fn description(&self) -> &'static str {
        "List the contents of a directory. Set recursive=true to explore nested structure. Common noise directories (.git, node_modules, __pycache__, etc.) are auto-ignored."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to list"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Recursively list all files (default false)"
                },
                "max_entries": {
                    "type": "integer",
                    "description": "Maximum entries to return (default 200)",
                    "minimum": 1
                },
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let recursive = args.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);
        let max_entries = args
            .get("max_entries")
            .and_then(|v| v.as_u64())
            .unwrap_or(200) as usize;

        if path_str.is_empty() {
            return Err(ToolError::InvalidArgs("path is required".to_string()));
        }

        // Security: validate path is within allowed directories
        let path = self.allowed_dirs_config.canonicalize_under_allowed(path_str)?;

        if !path.is_dir() {
            return Err(ToolError::Execution(format!("Not a directory: {}", path_str)).into());
        }

        let ignore_set: HashSet<&str> = IGNORE_DIRS.iter().cloned().collect();

        let mut items: Vec<String> = Vec::new();
        let mut total = 0;

        if recursive {
            // Flatten directory tree into a sorted list
            for entry in walkdir(&path, &ignore_set)? {
                total += 1;
                if items.len() < max_entries {
                    let rel_path = entry.strip_prefix(&path).unwrap_or(&entry);
                    if entry.is_dir() {
                        items.push(format!("{}/", rel_path.display()));
                    } else {
                        items.push(rel_path.display().to_string());
                    }
                }
            }
        } else {
            // Single-level listing with emoji prefixes
            let mut entries: Vec<_> = std::fs::read_dir(&path)?
                .filter_map(|e| e.ok())
                .collect();
            entries.sort_by_key(|e| e.file_name());

            for entry in entries {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if ignore_set.contains(name_str.as_ref()) {
                    continue;
                }
                total += 1;
                if items.len() < max_entries {
                    if entry.path().is_dir() {
                        items.push(format!("📁 {}", name_str));
                    } else {
                        items.push(format!("📄 {}", name_str));
                    }
                }
            }
        }

        if items.is_empty() && total == 0 {
            return Ok(format!("Directory {} is empty", path_str));
        }

        let mut result = items.join("\n");
        if total > max_entries {
            result.push_str(&format!("\n\n(truncated, showing first {} of {} entries)", max_entries, total));
        }

        Ok(result)
    }
}

/// Walk a directory tree (non-recursive) collecting paths.
/// Uses an explicit stack instead of recursion for performance and safety.
fn walkdir(path: &std::path::Path, ignore_set: &HashSet<&str>) -> io::Result<Vec<std::path::PathBuf>> {
    let mut results = Vec::new();
    let mut stack = vec![path.to_path_buf()];

    while let Some(current) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&current) {
            let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            entries.sort_by_key(|e| e.file_name());

            for entry in entries {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if ignore_set.contains(name_str.as_ref()) {
                    continue;
                }
                if entry.path().is_dir() {
                    stack.push(entry.path());
                    results.push(entry.path());
                } else {
                    results.push(entry.path());
                }
            }
        }
    }

    results.sort();
    Ok(results)
}
