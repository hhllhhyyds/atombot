//! Shell execution tool — runs arbitrary commands with safety guards.

use std::process::Stdio;

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::agent::tools::{Tool, ToolError};

/// Tool for executing shell commands with timeout and safety guards.
pub struct ExecTool {
    /// Default timeout in seconds
    timeout_secs: u64,
    /// Max characters to include in output (truncates large outputs)
    max_output_chars: usize,
    /// Regex patterns for dangerous commands that are blocked
    deny_patterns: Vec<String>,
    /// Whether to block path traversal patterns (`..`)
    restrict_to_workspace: bool,
    #[allow(dead_code)]
    workspace: Option<String>,
}

impl ExecTool {
    /// Create a new ExecTool.
    ///
    /// - `timeout_secs` — default command timeout
    /// - `workspace` — workspace path for path traversal detection
    /// - `restrict_to_workspace` — if true, blocks `..` in commands
    pub fn new(timeout_secs: u64, workspace: Option<String>, restrict_to_workspace: bool) -> Self {
        Self {
            timeout_secs,
            max_output_chars: 10_000,
            // Block patterns for destructive/harmful commands
            deny_patterns: vec![
                r"rm\s+-[rf]{1,2}\b".to_string(),        // rm -rf
                r"del\s+/[fq]\b".to_string(),            // Windows del /f/q
                r"rmdir\s+/s\b".to_string(),             // Windows rmdir /s
                r"\bformat\b".to_string(),              // format
                r"\b(mkfs|diskpart)\b".to_string(),      // mkfs, diskpart
                r"\bdd\s+if=".to_string(),               // dd if= (disk writing)
                r">\s*/dev/sd".to_string(),              // writing to disk devices
                r"\b(shutdown|reboot|poweroff)\b".to_string(), // system control
                r":\(\)\s*\{.*\};\s*:".to_string(),     // fork bomb
                r"\bsudo\s+su\b".to_string(),            // sudo su
                r"\bchmod\s+777\b".to_string(),          // chmod 777
                r"\bcurl\s+.*\|\s*sh\b".to_string(),     // curl | sh
                r"\bwget\s+.*\|\s*sh\b".to_string(),     // wget | sh
            ],
            restrict_to_workspace,
            workspace,
        }
    }

    /// Check if a command should be blocked by safety guards.
    /// Returns `Some(error_message)` if blocked, `None` if allowed.
    fn guard_command(&self, command: &str) -> Option<String> {
        let lower = command.to_lowercase();

        for pattern in &self.deny_patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(&lower) {
                    return Some(format!(
                        "Error: Command blocked by safety guard (dangerous pattern: {})",
                        pattern
                    ));
                }
            }
        }

        // Block path traversal when workspace restriction is enabled
        if self.restrict_to_workspace {
            if command.contains("..") {
                return Some("Error: Command blocked (path traversal detected)".to_string());
            }
        }

        None
    }
}

#[async_trait]
impl Tool for ExecTool {
    fn name(&self) -> &'static str {
        "exec"
    }

    fn description(&self) -> &'static str {
        "Execute a shell command and return its output. Use with caution."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default 60, max 600)",
                    "minimum": 1,
                    "maximum": 600
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("command is required".to_string()))?;

        // Check safety guards before execution
        if let Some(err) = self.guard_command(command) {
            return Err(ToolError::Execution(err));
        }

        let timeout_secs = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.timeout_secs)
            .min(600);

        let result = timeout(
            Duration::from_secs(timeout_secs),
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let mut output_parts = Vec::new();

                // Collect stdout
                if !output.stdout.is_empty() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    output_parts.push(stdout.to_string());
                }

                // Collect stderr (only if non-empty)
                if !output.stderr.is_empty() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if !stderr.trim().is_empty() {
                        output_parts.push(format!("STDERR:\n{}", stderr));
                    }
                }

                // Include exit code
                output_parts.push(format!("Exit code: {}", output.status.code().unwrap_or(-1)));

                let result = output_parts.join("\n");

                // Truncate very large outputs to avoid flooding the context window
                if result.chars().count() > self.max_output_chars {
                    let half = self.max_output_chars / 2;
                    let first_half: String = result.chars().take(half).collect();
                    let second_half: String = result.chars().rev().take(half).collect::<String>().chars().rev().collect();
                    let truncated_chars = result.chars().count() - self.max_output_chars;
                    return Ok(format!(
                        "{}\n\n... ({} chars truncated) ...\n\n{}",
                        first_half,
                        truncated_chars,
                        second_half
                    ));
                }

                Ok(result)
            }
            Ok(Err(e)) => Err(ToolError::Execution(format!(
                "Failed to execute command: {}",
                e
            ))),
            // Timeout
            Err(_) => Err(ToolError::Execution(format!(
                "Command timed out after {} seconds",
                timeout_secs
            ))),
        }
    }
}
