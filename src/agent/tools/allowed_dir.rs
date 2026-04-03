use std::io;
use std::path::{Path, PathBuf};

fn is_under(path: impl AsRef<Path>, directory: impl AsRef<Path>) -> bool {
    let path = path.as_ref().canonicalize().ok();
    let directory = directory.as_ref().canonicalize().ok();
    match (path, directory) {
        (Some(p), Some(d)) => p.starts_with(d),
        _ => false,
    }
}

#[derive(Debug, Clone, Default)]
pub struct AllowedDirectoriesConfig {
    workspace: Option<PathBuf>,
    allowed_dir: Option<PathBuf>,
    extra_allowed_dirs: Option<Vec<PathBuf>>,
}

impl AllowedDirectoriesConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_workspace(mut self, workspace: impl AsRef<Path>) -> Self {
        self.workspace = Some(workspace.as_ref().to_path_buf());
        self
    }

    pub fn with_allowed_dir(mut self, allowed_dir: impl AsRef<Path>) -> Self {
        self.allowed_dir = Some(allowed_dir.as_ref().to_path_buf());
        self
    }

    pub fn with_extra_allowed_dirs(mut self, extra_dirs: Vec<PathBuf>) -> Self {
        self.extra_allowed_dirs = Some(extra_dirs);
        self
    }

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
