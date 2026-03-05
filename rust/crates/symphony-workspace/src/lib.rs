#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkspaceError {
    #[error("workspace path escaped root")]
    EscapedRoot,
}

pub fn ensure_within_root(root: &Path, candidate: &Path) -> Result<PathBuf, WorkspaceError> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let candidate = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.to_path_buf());

    if candidate.starts_with(&root) {
        Ok(candidate)
    } else {
        Err(WorkspaceError::EscapedRoot)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn accepts_paths_within_root() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be monotonic enough for test")
            .as_nanos();
        let root: PathBuf = std::env::temp_dir().join(format!("symphony-workspace-test-{suffix}"));
        let candidate = root.join("issue-SYM-1");
        fs::create_dir_all(&candidate).expect("candidate path should be creatable");

        let result = ensure_within_root(&root, &candidate);
        assert!(result.is_ok());
    }
}
