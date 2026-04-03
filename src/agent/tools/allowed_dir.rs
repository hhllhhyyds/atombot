//! Path security and sandboxing for tool execution.
//!
//! This module ensures that file operations are restricted to a set of
//! allowed directories, preventing accidental or malicious access outside
//! the configured boundaries.

use std::io;
use std::path::{Path, PathBuf};

/// Checks if `path` is under `directory` after canonicalization.
///
/// Returns `false` if either path cannot be canonicalized (e.g., doesn't exist).
fn is_under(path: impl AsRef<Path>, directory: impl AsRef<Path>) -> bool {
    let path = path.as_ref().canonicalize().ok();
    let directory = directory.as_ref().canonicalize().ok();
    match (path, directory) {
        (Some(p), Some(d)) => p.starts_with(d),
        _ => false,
    }
}

/// Configuration for allowed directory access.
///
/// This struct controls which directories file operations are permitted to access.
/// All paths are checked against this configuration before any file operation.
#[derive(Debug, Clone, Default)]
pub struct AllowedDirectoriesConfig {
    workspace: Option<PathBuf>,
    allowed_dir: Option<PathBuf>,
    extra_allowed_dirs: Option<Vec<PathBuf>>,
}

impl AllowedDirectoriesConfig {
    /// Creates a new empty configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the workspace directory.
    ///
    /// The workspace is typically the root directory of the project being worked on.
    pub fn with_workspace(mut self, workspace: impl AsRef<Path>) -> Self {
        self.workspace = Some(workspace.as_ref().to_path_buf());
        self
    }

    /// Sets the primary allowed directory.
    ///
    /// This is typically used for user-specified directories.
    pub fn with_allowed_dir(mut self, allowed_dir: impl AsRef<Path>) -> Self {
        self.allowed_dir = Some(allowed_dir.as_ref().to_path_buf());
        self
    }

    /// Adds additional allowed directories.
    pub fn with_extra_allowed_dirs(mut self, extra_dirs: Vec<PathBuf>) -> Self {
        self.extra_allowed_dirs = Some(extra_dirs);
        self
    }

    /// Canonicalizes and validates that `path` falls under an allowed directory.
    ///
    /// # Errors
    ///
    /// Returns `PermissionDenied` if the path is not under any allowed directory。
    /// Returns standard `Io::Error` if the path cannot be canonicalized.
    pub fn canonicalize_under_allowed(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
        let path = path.as_ref().canonicalize()?;

        if let Some(workspace) = &self.workspace {
            if is_under(&path, workspace) {
                return Ok(path);
            }
        }
        if let Some(allowed_dir) = &self.allowed_dir {
            if is_under(&path, allowed_dir) {
                return Ok(path);
            }
        }
        if let Some(extra_dirs) = &self.extra_allowed_dirs {
            for extra in extra_dirs {
                if is_under(&path, extra) {
                    return Ok(path);
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Path is not under any allowed directory",
        ))
    }
}
