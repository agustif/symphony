use std::{
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
};

use crate::WorkspaceError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedWorkspace {
    pub root: PathBuf,
    pub key: String,
    pub path: PathBuf,
    pub created_now: bool,
}

pub fn ensure_within_root(root: &Path, candidate: &Path) -> Result<PathBuf, WorkspaceError> {
    let resolved_root = resolve_path(root)?;
    let candidate_input = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        resolved_root.join(candidate)
    };
    let resolved_candidate = resolve_path(&candidate_input)?;

    if resolved_candidate.starts_with(&resolved_root) {
        Ok(resolved_candidate)
    } else {
        Err(WorkspaceError::EscapedRoot {
            root: resolved_root,
            candidate: resolved_candidate,
        })
    }
}

pub fn sanitize_workspace_key(issue_identifier: &str) -> String {
    let mut sanitized = String::with_capacity(issue_identifier.len());

    for character in issue_identifier.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
            sanitized.push(character);
        } else {
            sanitized.push('_');
        }
    }

    if sanitized.is_empty() {
        "_".to_owned()
    } else {
        sanitized
    }
}

pub fn workspace_path(root: &Path, issue_identifier: &str) -> Result<PathBuf, WorkspaceError> {
    let key = sanitize_workspace_key(issue_identifier);
    if key.is_empty() {
        return Err(WorkspaceError::EmptyWorkspaceKey);
    }

    ensure_within_root(root, Path::new(&key))
}

pub fn prepare_workspace(
    root: &Path,
    issue_identifier: &str,
) -> Result<PreparedWorkspace, WorkspaceError> {
    fs::create_dir_all(root).map_err(|error| WorkspaceError::CreateDirectory {
        path: root.to_path_buf(),
        reason: error.to_string(),
    })?;

    let resolved_root = resolve_path(root)?;
    let key = sanitize_workspace_key(issue_identifier);
    if key.is_empty() {
        return Err(WorkspaceError::EmptyWorkspaceKey);
    }

    let path = ensure_within_root(&resolved_root, Path::new(&key))?;
    let created_now = match fs::metadata(&path) {
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(WorkspaceError::WorkspacePathNotDirectory(path));
            }
            false
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(&path).map_err(|create_error| WorkspaceError::CreateDirectory {
                path: path.clone(),
                reason: create_error.to_string(),
            })?;
            true
        }
        Err(error) => {
            return Err(WorkspaceError::PathResolution {
                path: path.clone(),
                reason: error.to_string(),
            });
        }
    };

    Ok(PreparedWorkspace {
        root: resolved_root,
        key,
        path,
        created_now,
    })
}

pub fn remove_workspace(root: &Path, issue_identifier: &str) -> Result<bool, WorkspaceError> {
    let path = workspace_path(root, issue_identifier)?;

    match fs::metadata(&path) {
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(WorkspaceError::WorkspacePathNotDirectory(path));
            }

            fs::remove_dir_all(&path).map_err(|error| WorkspaceError::RemoveDirectory {
                path,
                reason: error.to_string(),
            })?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(WorkspaceError::PathResolution {
            path,
            reason: error.to_string(),
        }),
    }
}

fn resolve_path(path: &Path) -> Result<PathBuf, WorkspaceError> {
    let absolute = absolute_path(path)?;

    let mut existing_ancestor = absolute.as_path();
    let mut suffix = Vec::<OsString>::new();

    while !existing_ancestor.exists() {
        let file_name =
            existing_ancestor
                .file_name()
                .ok_or_else(|| WorkspaceError::PathResolution {
                    path: absolute.clone(),
                    reason: "path has no existing ancestor".to_owned(),
                })?;
        suffix.push(file_name.to_os_string());
        existing_ancestor =
            existing_ancestor
                .parent()
                .ok_or_else(|| WorkspaceError::PathResolution {
                    path: absolute.clone(),
                    reason: "path has no parent".to_owned(),
                })?;
    }

    let mut resolved =
        existing_ancestor
            .canonicalize()
            .map_err(|error| WorkspaceError::PathResolution {
                path: existing_ancestor.to_path_buf(),
                reason: error.to_string(),
            })?;

    for component in suffix.iter().rev() {
        resolved.push(component);
    }

    Ok(normalize_path(&resolved))
}

fn absolute_path(path: &Path) -> Result<PathBuf, WorkspaceError> {
    if path.is_absolute() {
        return Ok(normalize_path(path));
    }

    let current_directory =
        std::env::current_dir().map_err(|error| WorkspaceError::PathResolution {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;

    Ok(normalize_path(&current_directory.join(path)))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let has_root = path.has_root();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && !has_root {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn ensure_within_root_accepts_nested_paths() {
        let root = fresh_temp_path("symphony-workspace-within");
        let nested = root.join("project").join("issue-SYM-1");
        fs::create_dir_all(&nested).expect("nested path should be creatable");

        let resolved = ensure_within_root(&root, &nested).expect("path should be accepted");

        assert!(resolved.starts_with(root.canonicalize().expect("root should resolve")));
        cleanup(&root);
    }

    #[test]
    fn ensure_within_root_rejects_relative_escape() {
        let root = fresh_temp_path("symphony-workspace-escape");
        fs::create_dir_all(&root).expect("root should be creatable");

        let result = ensure_within_root(&root, Path::new("../outside"));
        assert!(matches!(result, Err(WorkspaceError::EscapedRoot { .. })));

        cleanup(&root);
    }

    #[test]
    fn sanitize_workspace_key_replaces_invalid_characters() {
        assert_eq!(sanitize_workspace_key("SYM-1"), "SYM-1");
        assert_eq!(sanitize_workspace_key("SYM 1/#danger"), "SYM_1__danger");
        assert_eq!(sanitize_workspace_key(""), "_");
    }

    #[test]
    fn prepare_workspace_creates_and_then_reuses_directory() {
        let root = fresh_temp_path("symphony-workspace-prepare");

        let created = prepare_workspace(&root, "SYM-42").expect("workspace should be prepared");
        assert!(created.created_now);
        assert!(created.path.is_dir());

        let reused = prepare_workspace(&root, "SYM-42").expect("workspace should be reused");
        assert!(!reused.created_now);
        assert_eq!(created.path, reused.path);

        cleanup(&root);
    }

    #[test]
    fn prepare_workspace_rejects_non_directory_path() {
        let root = fresh_temp_path("symphony-workspace-file");
        fs::create_dir_all(&root).expect("root should be creatable");

        let workspace_file = root.join(sanitize_workspace_key("SYM-13"));
        fs::write(&workspace_file, "not a directory").expect("file should be writable");

        let result = prepare_workspace(&root, "SYM-13");
        assert!(matches!(
            result,
            Err(WorkspaceError::WorkspacePathNotDirectory(ref path))
                if path.file_name().is_some_and(|file_name| file_name == "SYM-13")
        ));

        cleanup(&root);
    }

    #[test]
    fn remove_workspace_handles_present_and_missing_directories() {
        let root = fresh_temp_path("symphony-workspace-remove");
        let prepared = prepare_workspace(&root, "SYM-99").expect("workspace should be created");

        let removed = remove_workspace(&root, "SYM-99").expect("remove should succeed");
        assert!(removed);
        assert!(!prepared.path.exists());

        let removed_again = remove_workspace(&root, "SYM-99").expect("remove should succeed");
        assert!(!removed_again);

        cleanup(&root);
    }

    fn fresh_temp_path(prefix: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be monotonic enough")
            .as_nanos();
        let counter = UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);

        std::env::temp_dir().join(format!("{prefix}-{now}-{counter}"))
    }

    fn cleanup(path: &Path) {
        if path.exists() {
            let _ = fs::remove_dir_all(path);
        }
    }
}
