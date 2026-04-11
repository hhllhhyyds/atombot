//! File system tools — read, write, edit, and list files.
//!
//! All file operations are sandboxed via [`AllowedDirectoriesConfig`](super::AllowedDirectoriesConfig)
//! to prevent access outside the configured workspace.

pub mod edit_file;
pub mod list_dir;
pub mod read_file;
pub mod write_file;
