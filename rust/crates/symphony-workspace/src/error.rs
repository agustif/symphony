use std::path::PathBuf;

use thiserror::Error;

use crate::HookResult;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkspaceError {
    #[error("workspace path escaped root: root={root:?} candidate={candidate:?}")]
    EscapedRoot { root: PathBuf, candidate: PathBuf },
    #[error("workspace key is empty after sanitization")]
    EmptyWorkspaceKey,
    #[error("workspace path resolves to root and cannot be used as a workspace: `{0}`")]
    WorkspaceIsRoot(PathBuf),
    #[error("failed to resolve path `{path}`: {reason}")]
    PathResolution { path: PathBuf, reason: String },
    #[error("workspace path is not a directory: `{0}`")]
    WorkspacePathNotDirectory(PathBuf),
    #[error("failed to create directory `{path}`: {reason}")]
    CreateDirectory { path: PathBuf, reason: String },
    #[error("failed to clean workspace path `{path}`: {reason}")]
    CleanupPath { path: PathBuf, reason: String },
    #[error("failed to remove directory `{path}`: {reason}")]
    RemoveDirectory { path: PathBuf, reason: String },
    #[error(
        "worker cwd must stay inside workspace: root={root:?} workspace={workspace:?} cwd={cwd:?}"
    )]
    InvalidWorkerCwd {
        root: PathBuf,
        workspace: PathBuf,
        cwd: PathBuf,
    },
    #[error("workspace hook `{hook}` failed: {reason}")]
    HookExecutionFailed { hook: String, reason: String },
    #[error("workspace hook `{hook}` exited with status {exit_code}")]
    HookExitedNonZero {
        hook: String,
        exit_code: i32,
        result: HookResult,
    },
    #[error("workspace hook `{hook}` terminated without an exit status")]
    HookTerminated { hook: String, result: HookResult },
    #[error("workspace hook `{hook}` timed out after {timeout_ms}ms")]
    HookTimedOut {
        hook: String,
        timeout_ms: u64,
        result: HookResult,
    },
}
