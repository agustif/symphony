use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkspaceError {
    #[error("workspace path escaped root: root={root:?} candidate={candidate:?}")]
    EscapedRoot { root: PathBuf, candidate: PathBuf },
    #[error("workspace key is empty after sanitization")]
    EmptyWorkspaceKey,
    #[error("failed to resolve path `{path}`: {reason}")]
    PathResolution { path: PathBuf, reason: String },
    #[error("workspace path is not a directory: `{0}`")]
    WorkspacePathNotDirectory(PathBuf),
    #[error("failed to create directory `{path}`: {reason}")]
    CreateDirectory { path: PathBuf, reason: String },
    #[error("failed to remove directory `{path}`: {reason}")]
    RemoveDirectory { path: PathBuf, reason: String },
}
