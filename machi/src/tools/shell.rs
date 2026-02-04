//! Shell execution tool for agents.
//!
//! Provides the ability to execute shell commands with configurable timeout.

use crate::tool::{Tool, ToolError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

/// Tool for executing shell commands.
#[derive(Debug, Clone)]
pub struct ExecTool {
    /// Default working directory for commands.
    pub working_dir: Option<String>,
    /// Command timeout in seconds. Default: 60.
    pub timeout_secs: u64,
    /// Maximum output size in bytes. Default: 100KB.
    pub max_output_size: usize,
}

impl Default for ExecTool {
    fn default() -> Self {
        Self {
            working_dir: None,
            timeout_secs: 60,
            max_output_size: 100 * 1024, // 100KB
        }
    }
}

impl ExecTool {
    /// Create a new exec tool with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default working directory.
    #[must_use]
    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set the command timeout in seconds.
    #[must_use]
    pub const fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set the maximum output size in bytes.
    #[must_use]
    pub const fn with_max_output(mut self, size: usize) -> Self {
        self.max_output_size = size;
        self
    }
}

/// Arguments for executing a shell command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExecArgs {
    /// The command to execute.
    pub command: String,
    /// Working directory for the command. Optional.
    pub cwd: Option<String>,
    /// Timeout in seconds. Optional (uses tool default).
    pub timeout: Option<u64>,
}

/// Result of command execution.
#[derive(Debug, Clone, Serialize)]
pub struct ExecResult {
    /// Exit code of the command.
    pub exit_code: Option<i32>,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// Whether the command timed out.
    pub timed_out: bool,
}

impl std::fmt::Display for ExecResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.timed_out {
            writeln!(f, "[TIMEOUT]")?;
        }

        if let Some(code) = self.exit_code {
            writeln!(f, "[Exit code: {code}]")?;
        }

        if !self.stdout.is_empty() {
            writeln!(f, "[stdout]\n{}", self.stdout)?;
        }

        if !self.stderr.is_empty() {
            writeln!(f, "[stderr]\n{}", self.stderr)?;
        }

        if self.stdout.is_empty() && self.stderr.is_empty() {
            write!(f, "(no output)")?;
        }

        Ok(())
    }
}

#[async_trait]
impl Tool for ExecTool {
    const NAME: &'static str = "exec";
    type Args = ExecArgs;
    type Output = String;
    type Error = ToolError;

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn description(&self) -> String {
        "Execute a shell command and return its output. Supports timeout and working directory."
            .to_string()
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory for the command. Optional."
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds. Optional (default: 60)."
                }
            },
            "required": ["command"]
        })
    }

    fn output_type(&self) -> &'static str {
        "string"
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let timeout_secs = args.timeout.unwrap_or(self.timeout_secs);
        let working_dir = args.cwd.as_ref().or(self.working_dir.as_ref());

        // Build command based on platform
        #[cfg(target_os = "windows")]
        let mut cmd = {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", &args.command]);
            cmd
        };

        #[cfg(not(target_os = "windows"))]
        let mut cmd = {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", &args.command]);
            cmd
        };

        // Set working directory
        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        // Configure stdio
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Spawn process
        let mut child = cmd
            .spawn()
            .map_err(|e| ToolError::execution(format!("Failed to spawn command: {e}")))?;

        // Execute with timeout
        let result = timeout(Duration::from_secs(timeout_secs), async {
            let status = child.wait().await;

            let mut stdout = String::new();
            let mut stderr = String::new();

            if let Some(mut out) = child.stdout.take() {
                let _ = out.read_to_string(&mut stdout).await;
            }
            if let Some(mut err) = child.stderr.take() {
                let _ = err.read_to_string(&mut stderr).await;
            }

            (status, stdout, stderr)
        })
        .await;

        if let Ok((status, stdout, stderr)) = result {
            let exit_code = status.ok().and_then(|s| s.code());

            // Truncate output if too large
            let stdout = truncate_output(&stdout, self.max_output_size);
            let stderr = truncate_output(&stderr, self.max_output_size);

            let exec_result = ExecResult {
                exit_code,
                stdout,
                stderr,
                timed_out: false,
            };

            Ok(exec_result.to_string())
        } else {
            // Timeout - try to kill the process
            let _ = child.kill().await;

            let exec_result = ExecResult {
                exit_code: None,
                stdout: String::new(),
                stderr: format!("Command timed out after {timeout_secs} seconds"),
                timed_out: true,
            };

            Ok(exec_result.to_string())
        }
    }
}

/// Truncate output to maximum size, adding indicator if truncated.
fn truncate_output(output: &str, max_size: usize) -> String {
    if output.len() <= max_size {
        output.to_string()
    } else {
        let truncated = &output[..max_size];
        format!(
            "{}\n... [truncated, {} bytes total]",
            truncated,
            output.len()
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exec_echo() {
        let tool = ExecTool::new();
        let result = tool
            .call(ExecArgs {
                command: "echo hello".to_string(),
                cwd: None,
                timeout: None,
            })
            .await
            .unwrap();

        assert!(result.contains("hello"));
        assert!(result.contains("Exit code: 0"));
    }

    #[tokio::test]
    async fn test_exec_with_cwd() {
        let tool = ExecTool::new();

        #[cfg(target_os = "windows")]
        let command = "cd".to_string();
        #[cfg(not(target_os = "windows"))]
        let command = "pwd".to_string();

        let temp = std::env::temp_dir();
        let result = tool
            .call(ExecArgs {
                command,
                cwd: Some(temp.to_string_lossy().to_string()),
                timeout: None,
            })
            .await
            .unwrap();

        // Should contain temp directory path in output
        assert!(result.contains("Exit code: 0"));
    }

    #[tokio::test]
    async fn test_exec_timeout() {
        let tool = ExecTool::new().with_timeout(1);

        #[cfg(target_os = "windows")]
        let command = "ping -n 10 127.0.0.1".to_string();
        #[cfg(not(target_os = "windows"))]
        let command = "sleep 10".to_string();

        let result = tool
            .call(ExecArgs {
                command,
                cwd: None,
                timeout: Some(1),
            })
            .await
            .unwrap();

        assert!(result.contains("TIMEOUT") || result.contains("timed out"));
    }

    #[tokio::test]
    async fn test_exec_stderr() {
        let tool = ExecTool::new();

        #[cfg(target_os = "windows")]
        let command = "cmd /c \"echo error 1>&2\"".to_string();
        #[cfg(not(target_os = "windows"))]
        let command = "echo error >&2".to_string();

        let result = tool
            .call(ExecArgs {
                command,
                cwd: None,
                timeout: None,
            })
            .await
            .unwrap();

        assert!(result.contains("stderr") || result.contains("error"));
    }
}
