//! File system tools for agents.
//!
//! Provides tools for reading, writing, editing files and listing directories.

use crate::tool::{Tool, ToolError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Write as _;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use tokio::fs;

/// Tool for reading file contents.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReadFileTool {
    /// Maximum file size to read (in bytes). Default: 1MB.
    pub max_size: Option<usize>,
}

impl ReadFileTool {
    /// Create a new read file tool with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum file size to read.
    #[must_use]
    pub const fn with_max_size(mut self, max_size: usize) -> Self {
        self.max_size = Some(max_size);
        self
    }
}

/// Arguments for reading a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReadFileArgs {
    /// Path to the file to read.
    pub path: String,
    /// Optional line range start (1-indexed, inclusive).
    pub start_line: Option<usize>,
    /// Optional line range end (1-indexed, inclusive).
    pub end_line: Option<usize>,
}

#[async_trait]
impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";
    type Args = ReadFileArgs;
    type Output = String;
    type Error = ToolError;

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn description(&self) -> String {
        "Read the contents of a file. Supports optional line range selection.".to_string()
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Start line number (1-indexed, inclusive). Optional."
                },
                "end_line": {
                    "type": "integer",
                    "description": "End line number (1-indexed, inclusive). Optional."
                }
            },
            "required": ["path"]
        })
    }

    fn output_type(&self) -> &'static str {
        "string"
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = Path::new(&args.path);

        // Check if file exists
        if !path.exists() {
            return Err(ToolError::execution(format!(
                "File not found: {}",
                args.path
            )));
        }

        // Check file size
        let metadata = fs::metadata(path)
            .await
            .map_err(|e| ToolError::execution(format!("Failed to read file metadata: {e}")))?;

        let max_size = self.max_size.unwrap_or(1024 * 1024); // 1MB default
        if metadata.len() > max_size as u64 {
            return Err(ToolError::execution(format!(
                "File too large: {} bytes (max: {} bytes)",
                metadata.len(),
                max_size
            )));
        }

        // Read file content
        let content = fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::execution(format!("Failed to read file: {e}")))?;

        // Apply line range if specified
        match (args.start_line, args.end_line) {
            (Some(start), Some(end)) => {
                let lines: Vec<&str> = content.lines().collect();
                let start_idx = start.saturating_sub(1);
                let end_idx = end.min(lines.len());

                if start_idx >= lines.len() {
                    return Ok(String::new());
                }

                Ok(lines[start_idx..end_idx].join("\n"))
            }
            (Some(start), None) => {
                let lines: Vec<&str> = content.lines().collect();
                let start_idx = start.saturating_sub(1);

                if start_idx >= lines.len() {
                    return Ok(String::new());
                }

                Ok(lines[start_idx..].join("\n"))
            }
            (None, Some(end)) => {
                let lines: Vec<&str> = content.lines().collect();
                let end_idx = end.min(lines.len());
                Ok(lines[..end_idx].join("\n"))
            }
            (None, None) => Ok(content),
        }
    }
}

/// Tool for writing content to a file.
#[derive(Debug, Clone, Copy, Default)]
pub struct WriteFileTool;

/// Arguments for writing a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WriteFileArgs {
    /// Path to the file to write.
    pub path: String,
    /// Content to write to the file.
    pub content: String,
    /// Whether to append to existing content. Default: false (overwrite).
    #[serde(default)]
    pub append: bool,
    /// Create parent directories if they don't exist. Default: true.
    #[serde(default = "default_true")]
    pub create_dirs: bool,
}

const fn default_true() -> bool {
    true
}

#[async_trait]
impl Tool for WriteFileTool {
    const NAME: &'static str = "write_file";
    type Args = WriteFileArgs;
    type Output = String;
    type Error = ToolError;

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn description(&self) -> String {
        "Write content to a file. Can create new files or overwrite/append to existing ones."
            .to_string()
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                },
                "append": {
                    "type": "boolean",
                    "description": "Append to existing content instead of overwriting. Default: false"
                },
                "create_dirs": {
                    "type": "boolean",
                    "description": "Create parent directories if they don't exist. Default: true"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn output_type(&self) -> &'static str {
        "string"
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = Path::new(&args.path);

        // Create parent directories if needed
        if args.create_dirs
            && let Some(parent) = path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| ToolError::execution(format!("Failed to create directories: {e}")))?;
        }

        // Write or append content
        if args.append && path.exists() {
            let existing = fs::read_to_string(path)
                .await
                .map_err(|e| ToolError::execution(format!("Failed to read existing file: {e}")))?;
            let new_content = format!("{}{}", existing, args.content);
            fs::write(path, &new_content)
                .await
                .map_err(|e| ToolError::execution(format!("Failed to write file: {e}")))?;
            Ok(format!(
                "Appended {} bytes to '{}'",
                args.content.len(),
                args.path
            ))
        } else {
            fs::write(path, &args.content)
                .await
                .map_err(|e| ToolError::execution(format!("Failed to write file: {e}")))?;
            Ok(format!(
                "Wrote {} bytes to '{}'",
                args.content.len(),
                args.path
            ))
        }
    }
}

/// Tool for editing files with find-and-replace operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct EditFileTool;

/// Arguments for editing a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EditFileArgs {
    /// Path to the file to edit.
    pub path: String,
    /// Text to find and replace.
    pub old_text: String,
    /// New text to replace with.
    pub new_text: String,
    /// Replace all occurrences. Default: false (replace first only).
    #[serde(default)]
    pub replace_all: bool,
}

#[async_trait]
impl Tool for EditFileTool {
    const NAME: &'static str = "edit_file";
    type Args = EditFileArgs;
    type Output = String;
    type Error = ToolError;

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn description(&self) -> String {
        "Edit a file by replacing text. Finds old_text and replaces it with new_text.".to_string()
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "old_text": {
                    "type": "string",
                    "description": "Text to find and replace"
                },
                "new_text": {
                    "type": "string",
                    "description": "New text to replace with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences. Default: false (replace first only)"
                }
            },
            "required": ["path", "old_text", "new_text"]
        })
    }

    fn output_type(&self) -> &'static str {
        "string"
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = Path::new(&args.path);

        // Check if file exists
        if !path.exists() {
            return Err(ToolError::execution(format!(
                "File not found: {}",
                args.path
            )));
        }

        // Read current content
        let content = fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::execution(format!("Failed to read file: {e}")))?;

        // Check if old_text exists
        if !content.contains(&args.old_text) {
            return Err(ToolError::execution(format!(
                "Text not found in file: '{}'",
                args.old_text.chars().take(50).collect::<String>()
            )));
        }

        // Perform replacement
        let (new_content, count) = if args.replace_all {
            let count = content.matches(&args.old_text).count();
            (content.replace(&args.old_text, &args.new_text), count)
        } else {
            (content.replacen(&args.old_text, &args.new_text, 1), 1)
        };

        // Write back
        fs::write(path, &new_content)
            .await
            .map_err(|e| ToolError::execution(format!("Failed to write file: {e}")))?;

        Ok(format!(
            "Replaced {} occurrence(s) in '{}'",
            count, args.path
        ))
    }
}

/// Tool for listing directory contents.
#[derive(Debug, Clone, Copy, Default)]
pub struct ListDirTool {
    /// Maximum depth for recursive listing. None means no recursion.
    pub max_depth: Option<usize>,
}

impl ListDirTool {
    /// Create a new list directory tool.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum recursion depth.
    #[must_use]
    pub const fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }
}

/// Arguments for listing a directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ListDirArgs {
    /// Path to the directory to list.
    pub path: String,
    /// Include hidden files (starting with '.'). Default: false.
    #[serde(default)]
    pub show_hidden: bool,
    /// Recursion depth. 0 = current dir only, None = use tool default.
    pub depth: Option<usize>,
}

/// Entry information for directory listing.
#[derive(Debug, Clone, Serialize)]
struct DirEntry {
    name: String,
    is_dir: bool,
    size: Option<u64>,
}

#[async_trait]
impl Tool for ListDirTool {
    const NAME: &'static str = "list_dir";
    type Args = ListDirArgs;
    type Output = String;
    type Error = ToolError;

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn description(&self) -> String {
        "List contents of a directory. Shows files and subdirectories.".to_string()
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory to list"
                },
                "show_hidden": {
                    "type": "boolean",
                    "description": "Include hidden files (starting with '.'). Default: false"
                },
                "depth": {
                    "type": "integer",
                    "description": "Recursion depth. 0 = current dir only. Default: 0"
                }
            },
            "required": ["path"]
        })
    }

    fn output_type(&self) -> &'static str {
        "string"
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = Path::new(&args.path);

        // Check if directory exists
        if !path.exists() {
            return Err(ToolError::execution(format!(
                "Directory not found: {}",
                args.path
            )));
        }

        if !path.is_dir() {
            return Err(ToolError::execution(format!(
                "Not a directory: {}",
                args.path
            )));
        }

        let max_depth = args.depth.or(self.max_depth).unwrap_or(0);
        let entries = list_dir_recursive(path, args.show_hidden, 0, max_depth).await?;

        // Format output
        let mut output = String::new();
        for entry in entries {
            let type_indicator = if entry.is_dir { "📁" } else { "📄" };
            let size_str = entry
                .size
                .map(|s| format!(" ({s} bytes)"))
                .unwrap_or_default();
            let _ = writeln!(output, "{} {}{}", type_indicator, entry.name, size_str);
        }

        if output.is_empty() {
            output = "(empty directory)".to_string();
        }

        Ok(output.trim_end().to_string())
    }
}

/// Recursively list directory contents.
fn list_dir_recursive(
    path: &Path,
    show_hidden: bool,
    current_depth: usize,
    max_depth: usize,
) -> Pin<Box<dyn Future<Output = Result<Vec<DirEntry>, ToolError>> + Send + '_>> {
    Box::pin(async move {
        let mut entries = Vec::new();
        let mut read_dir = fs::read_dir(path)
            .await
            .map_err(|e| ToolError::execution(format!("Failed to read directory: {e}")))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| ToolError::execution(format!("Failed to read entry: {e}")))?
        {
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files if not requested
            if !show_hidden && name.starts_with('.') {
                continue;
            }

            let metadata = entry
                .metadata()
                .await
                .map_err(|e| ToolError::execution(format!("Failed to read metadata: {e}")))?;

            let is_dir = metadata.is_dir();
            let size = if is_dir { None } else { Some(metadata.len()) };

            let indent = "  ".repeat(current_depth);
            entries.push(DirEntry {
                name: format!("{indent}{name}"),
                is_dir,
                size,
            });

            // Recurse into subdirectories
            if is_dir && current_depth < max_depth {
                let sub_entries =
                    list_dir_recursive(&entry.path(), show_hidden, current_depth + 1, max_depth)
                        .await?;
                entries.extend(sub_entries);
            }
        }

        // Sort: directories first, then by name
        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        Ok(entries)
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3").unwrap();

        let tool = ReadFileTool::new();
        let result = tool
            .call(ReadFileArgs {
                path: file_path.to_string_lossy().to_string(),
                start_line: None,
                end_line: None,
            })
            .await
            .unwrap();

        assert_eq!(result, "line1\nline2\nline3");
    }

    #[tokio::test]
    async fn test_read_file_with_range() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3\nline4").unwrap();

        let tool = ReadFileTool::new();
        let result = tool
            .call(ReadFileArgs {
                path: file_path.to_string_lossy().to_string(),
                start_line: Some(2),
                end_line: Some(3),
            })
            .await
            .unwrap();

        assert_eq!(result, "line2\nline3");
    }

    #[tokio::test]
    async fn test_write_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("new_file.txt");

        let tool = WriteFileTool;
        let result = tool
            .call(WriteFileArgs {
                path: file_path.to_string_lossy().to_string(),
                content: "Hello, World!".to_string(),
                append: false,
                create_dirs: true,
            })
            .await
            .unwrap();

        assert!(result.contains("13 bytes"));
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "Hello, World!"
        );
    }

    #[tokio::test]
    async fn test_edit_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("edit.txt");
        std::fs::write(&file_path, "Hello, World!").unwrap();

        let tool = EditFileTool;
        let result = tool
            .call(EditFileArgs {
                path: file_path.to_string_lossy().to_string(),
                old_text: "World".to_string(),
                new_text: "Rust".to_string(),
                replace_all: false,
            })
            .await
            .unwrap();

        assert!(result.contains("1 occurrence"));
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "Hello, Rust!");
    }

    #[tokio::test]
    async fn test_list_dir() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("file1.txt"), "").unwrap();
        std::fs::write(temp.path().join("file2.txt"), "").unwrap();
        std::fs::create_dir(temp.path().join("subdir")).unwrap();

        let tool = ListDirTool::new();
        let result = tool
            .call(ListDirArgs {
                path: temp.path().to_string_lossy().to_string(),
                show_hidden: false,
                depth: None,
            })
            .await
            .unwrap();

        assert!(result.contains("subdir"));
        assert!(result.contains("file1.txt"));
        assert!(result.contains("file2.txt"));
    }
}
