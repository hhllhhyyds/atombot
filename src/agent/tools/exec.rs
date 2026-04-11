use std::process::Stdio;

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::agent::tools::{Tool, ToolError};

pub struct ExecTool {
    timeout_secs: u64,
    max_output_chars: usize,
    deny_patterns: Vec<String>,
    restrict_to_workspace: bool,
    #[allow(dead_code)]
    workspace: Option<String>,
}

impl ExecTool {
    pub fn new(timeout_secs: u64, workspace: Option<String>, restrict_to_workspace: bool) -> Self {
        Self {
            timeout_secs,
            max_output_chars: 10_000,
            deny_patterns: vec![
                r"rm\s+-[rf]{1,2}\b".to_string(),
                r"del\s+/[fq]\b".to_string(),
                r"rmdir\s+/s\b".to_string(),
                r"\bformat\b".to_string(),
                r"\b(mkfs|diskpart)\b".to_string(),
                r"\bdd\s+if=".to_string(),
                r">\s*/dev/sd".to_string(),
                r"\b(shutdown|reboot|poweroff)\b".to_string(),
                r":\(\)\s*\{.*\};\s*:".to_string(),
                r"\bsudo\s+su\b".to_string(),
                r"\bchmod\s+777\b".to_string(),
                r"\bcurl\s+.*\|\s*sh\b".to_string(),
                r"\bwget\s+.*\|\s*sh\b".to_string(),
            ],
            restrict_to_workspace,
            workspace,
        }
    }

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

                if !output.stdout.is_empty() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    output_parts.push(stdout.to_string());
                }

                if !output.stderr.is_empty() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if !stderr.trim().is_empty() {
                        output_parts.push(format!("STDERR:\n{}", stderr));
                    }
                }

                output_parts.push(format!("Exit code: {}", output.status.code().unwrap_or(-1)));

                let result = output_parts.join("\n");

                if result.len() > self.max_output_chars {
                    let half = self.max_output_chars / 2;
                    return Ok(format!(
                        "{}\n\n... ({} chars truncated) ...\n\n{}",
                        &result[..half],
                        result.len() - self.max_output_chars,
                        &result[result.len() - half..]
                    ));
                }

                Ok(result)
            }
            Ok(Err(e)) => Err(ToolError::Execution(format!(
                "Failed to execute command: {}",
                e
            ))),
            Err(_) => Err(ToolError::Execution(format!(
                "Command timed out after {} seconds",
                timeout_secs
            ))),
        }
    }
}
