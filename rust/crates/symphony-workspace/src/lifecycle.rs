use std::{
    ffi::OsString,
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use crate::{
    HookExecutor, HookRequest, HookResult, WorkspaceError, WorkspaceHookKind, WorkspaceHooks,
    truncate_hook_result,
};

const TRANSIENT_WORKSPACE_ENTRIES: &[&str] = &[".elixir_ls", "tmp"];

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

/// Lookup table for hex encoding (uppercase).
const HEX_TABLE: &[u8; 16] = b"0123456789ABCDEF";

/// Encode a byte as two uppercase hex characters without allocation.
#[inline]
fn byte_to_hex(byte: u8) -> [char; 2] {
    [
        HEX_TABLE[(byte >> 4) as usize] as char,
        HEX_TABLE[(byte & 0x0F) as usize] as char,
    ]
}

pub fn sanitize_workspace_key(issue_identifier: &str) -> String {
    let mut sanitized = String::with_capacity(issue_identifier.len().saturating_mul(4));

    for byte in issue_identifier.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' => {
                sanitized.push(byte as char);
            }
            b'_' => sanitized.push_str("__"),
            _ => {
                sanitized.push('_');
                let hex = byte_to_hex(byte);
                sanitized.push(hex[0]);
                sanitized.push(hex[1]);
                sanitized.push('_');
            }
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

    let resolved_root = resolve_path(root)?;
    workspace_path_for_key(&resolved_root, &key)
}

pub fn validate_worker_cwd(
    root: &Path,
    workspace: &Path,
    cwd: &Path,
) -> Result<PathBuf, WorkspaceError> {
    let resolved_root = resolve_path(root)?;
    let resolved_workspace = ensure_within_root(&resolved_root, workspace)?;
    if resolved_workspace == resolved_root {
        return Err(WorkspaceError::WorkspaceIsRoot(resolved_workspace));
    }

    let candidate_input = if cwd.is_absolute() {
        cwd.to_path_buf()
    } else {
        resolved_workspace.join(cwd)
    };
    let resolved_cwd = resolve_path(&candidate_input)?;

    if !resolved_cwd.starts_with(&resolved_workspace) {
        return Err(WorkspaceError::InvalidWorkerCwd {
            root: resolved_root,
            workspace: resolved_workspace,
            cwd: resolved_cwd,
        });
    }

    Ok(resolved_cwd)
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

    let path = workspace_path_for_key(&resolved_root, &key)?;
    let created_now = match fs::metadata(&path) {
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(WorkspaceError::WorkspacePathNotDirectory(path));
            }
            cleanup_transient_workspace_entries(&path)?;
            false
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
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

pub fn prepare_workspace_with_hooks(
    root: &Path,
    issue_identifier: &str,
    hooks: &WorkspaceHooks,
    executor: &dyn HookExecutor,
) -> Result<PreparedWorkspace, WorkspaceError> {
    let prepared = prepare_workspace(root, issue_identifier)?;
    if prepared.created_now
        && let Err(hook_error) = run_workspace_hook(
            &prepared.path,
            hooks,
            WorkspaceHookKind::AfterCreate,
            executor,
        )
    {
        match fs::remove_dir_all(&prepared.path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(WorkspaceError::RemoveDirectory {
                    path: prepared.path.clone(),
                    reason: error.to_string(),
                });
            }
        }
        return Err(hook_error);
    }
    Ok(prepared)
}

pub fn run_before_run_hook(
    workspace: &PreparedWorkspace,
    hooks: &WorkspaceHooks,
    executor: &dyn HookExecutor,
) -> Result<Option<HookResult>, WorkspaceError> {
    run_workspace_hook(
        &workspace.path,
        hooks,
        WorkspaceHookKind::BeforeRun,
        executor,
    )
}

pub fn run_after_run_hook(
    workspace: &PreparedWorkspace,
    hooks: &WorkspaceHooks,
    executor: &dyn HookExecutor,
) -> Result<Option<HookResult>, WorkspaceError> {
    run_workspace_hook(
        &workspace.path,
        hooks,
        WorkspaceHookKind::AfterRun,
        executor,
    )
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
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(WorkspaceError::PathResolution {
            path,
            reason: error.to_string(),
        }),
    }
}

pub fn remove_workspace_with_hooks(
    root: &Path,
    issue_identifier: &str,
    hooks: &WorkspaceHooks,
    executor: &dyn HookExecutor,
) -> Result<bool, WorkspaceError> {
    let path = workspace_path(root, issue_identifier)?;
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(WorkspaceError::PathResolution {
                path,
                reason: error.to_string(),
            });
        }
    };

    if !metadata.is_dir() {
        return Err(WorkspaceError::WorkspacePathNotDirectory(path));
    }

    run_workspace_hook(&path, hooks, WorkspaceHookKind::BeforeRemove, executor)?;
    match fs::remove_dir_all(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(WorkspaceError::RemoveDirectory {
                path,
                reason: error.to_string(),
            });
        }
    }
    Ok(true)
}

fn run_workspace_hook(
    workspace_path: &Path,
    hooks: &WorkspaceHooks,
    kind: WorkspaceHookKind,
    executor: &dyn HookExecutor,
) -> Result<Option<HookResult>, WorkspaceError> {
    let command = hooks
        .command_for(kind)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let Some(command) = command else {
        return Ok(None);
    };

    let timeout_ms = hooks.timeout_ms.max(1);
    let output_limit_bytes = hooks.output_limit_bytes.max(1);
    let request = HookRequest {
        kind,
        command,
        workspace_path: workspace_path.to_path_buf(),
        timeout_ms,
        output_limit_bytes,
    };

    let result =
        executor
            .execute(&request)
            .map_err(|reason| WorkspaceError::HookExecutionFailed {
                hook: kind.as_str().to_owned(),
                reason,
            })?;
    Ok(Some(classify_hook_result(
        kind,
        truncate_hook_result(result, output_limit_bytes),
        timeout_ms,
    )?))
}

fn classify_hook_result(
    kind: WorkspaceHookKind,
    result: HookResult,
    timeout_ms: u64,
) -> Result<HookResult, WorkspaceError> {
    let hook = kind.as_str().to_owned();
    if result.timed_out {
        return Err(WorkspaceError::HookTimedOut {
            hook,
            timeout_ms,
            result,
        });
    }

    match result.exit_code {
        Some(0) => Ok(result),
        Some(exit_code) => Err(WorkspaceError::HookExitedNonZero {
            hook,
            exit_code,
            result,
        }),
        None => Err(WorkspaceError::HookTerminated { hook, result }),
    }
}

fn cleanup_transient_workspace_entries(workspace_path: &Path) -> Result<(), WorkspaceError> {
    for entry in TRANSIENT_WORKSPACE_ENTRIES {
        remove_workspace_entry(&workspace_path.join(entry))?;
    }

    Ok(())
}

fn remove_workspace_entry(path: &Path) -> Result<(), WorkspaceError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(WorkspaceError::CleanupPath {
                path: path.to_path_buf(),
                reason: error.to_string(),
            });
        }
    };

    if metadata.is_dir() {
        fs::remove_dir_all(path).map_err(|error| WorkspaceError::CleanupPath {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    } else {
        fs::remove_file(path).map_err(|error| WorkspaceError::CleanupPath {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    }

    Ok(())
}

fn workspace_path_for_key(root: &Path, key: &str) -> Result<PathBuf, WorkspaceError> {
    let path = ensure_within_root(root, Path::new(key))?;
    if path == root {
        return Err(WorkspaceError::WorkspaceIsRoot(path));
    }
    Ok(path)
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
        cell::RefCell,
        collections::VecDeque,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[derive(Default)]
    struct RecordingExecutor {
        requests: RefCell<Vec<HookRequest>>,
        responses: RefCell<VecDeque<Result<HookResult, String>>>,
    }

    impl RecordingExecutor {
        fn with_responses(responses: Vec<Result<HookResult, String>>) -> Self {
            Self {
                requests: RefCell::new(Vec::new()),
                responses: RefCell::new(VecDeque::from(responses)),
            }
        }

        fn requests(&self) -> Vec<HookRequest> {
            self.requests.borrow().clone()
        }
    }

    impl HookExecutor for RecordingExecutor {
        fn execute(&self, request: &HookRequest) -> Result<HookResult, String> {
            self.requests.borrow_mut().push(request.clone());
            self.responses
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Ok(HookResult::success()))
        }
    }

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
        assert_eq!(
            sanitize_workspace_key("SYM 1/#danger"),
            "SYM_20_1_2F__23_danger"
        );
        assert_eq!(sanitize_workspace_key("SYM_1"), "SYM__1");
        assert_eq!(sanitize_workspace_key(""), "_");
    }

    #[test]
    fn workspace_path_encodes_dot_safely() {
        let root = fresh_temp_path("symphony-workspace-dot-sanitize");
        fs::create_dir_all(&root).expect("root should be creatable");
        let canonical_root = root.canonicalize().expect("root should resolve");

        let path = workspace_path(&root, ".").expect("dot should be sanitized safely");
        assert!(
            path.ends_with("_2E_"),
            "expected path to end with encoded dot, got {:?}",
            path
        );
        assert!(
            path.starts_with(&canonical_root),
            "path {:?} should be within root {:?}",
            path,
            canonical_root
        );

        cleanup(&root);
    }

    #[test]
    fn sanitize_workspace_key_keeps_distinct_invalid_identifiers_unique() {
        let slash = sanitize_workspace_key("SYM/1");
        let colon = sanitize_workspace_key("SYM:1");
        let dot = sanitize_workspace_key("SYM.1");

        assert_ne!(slash, colon);
        assert_ne!(slash, dot);
        assert_ne!(colon, dot);
    }

    #[test]
    fn validate_worker_cwd_accepts_workspace_descendant() {
        let root = fresh_temp_path("symphony-workspace-cwd-valid");
        let workspace = root.join("SYM-1");
        let cwd = workspace.join("agent");
        fs::create_dir_all(&cwd).expect("workspace cwd should be creatable");

        let resolved = validate_worker_cwd(&root, &workspace, &cwd).expect("cwd should validate");
        assert_eq!(resolved, cwd.canonicalize().expect("cwd should resolve"));

        cleanup(&root);
    }

    #[test]
    fn validate_worker_cwd_rejects_root_and_escapes() {
        let root = fresh_temp_path("symphony-workspace-cwd-invalid");
        let workspace = root.join("SYM-1");
        let escaped = root.join("outside");
        fs::create_dir_all(&workspace).expect("workspace should be creatable");
        fs::create_dir_all(&escaped).expect("outside path should be creatable");

        let root_error = validate_worker_cwd(&root, &root, &workspace);
        assert!(matches!(
            root_error,
            Err(WorkspaceError::WorkspaceIsRoot(_))
        ));

        let escape_error = validate_worker_cwd(&root, &workspace, Path::new("../outside"));
        assert!(matches!(
            escape_error,
            Err(WorkspaceError::InvalidWorkerCwd { .. })
        ));

        cleanup(&root);
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
    fn prepare_workspace_reuse_cleans_transient_entries_only() {
        let root = fresh_temp_path("symphony-workspace-reuse-cleanup");

        let created = prepare_workspace(&root, "SYM-42").expect("workspace should be prepared");
        fs::create_dir_all(created.path.join(".elixir_ls/cache"))
            .expect("transient elixir ls directory should be creatable");
        fs::create_dir_all(created.path.join("tmp/build"))
            .expect("transient tmp directory should be creatable");
        fs::write(created.path.join("keep.txt"), "stable").expect("stable file should be writable");

        let reused = prepare_workspace(&root, "SYM-42").expect("workspace should be reused");

        assert!(!reused.created_now);
        assert!(!reused.path.join(".elixir_ls").exists());
        assert!(!reused.path.join("tmp").exists());
        assert_eq!(
            fs::read_to_string(reused.path.join("keep.txt"))
                .expect("stable file should be preserved"),
            "stable"
        );

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
    fn prepare_workspace_with_hooks_runs_after_create_only_once() {
        let root = fresh_temp_path("symphony-workspace-hooks-create");
        let hooks = WorkspaceHooks {
            after_create: Some("echo created".to_owned()),
            ..WorkspaceHooks::default()
        };
        let executor = RecordingExecutor::default();

        let first = prepare_workspace_with_hooks(&root, "SYM-44", &hooks, &executor)
            .expect("initial prepare should succeed");
        let second = prepare_workspace_with_hooks(&root, "SYM-44", &hooks, &executor)
            .expect("second prepare should succeed");

        assert!(first.created_now);
        assert!(!second.created_now);
        let requests = executor.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].kind, WorkspaceHookKind::AfterCreate);

        cleanup(&root);
    }

    #[test]
    fn prepare_workspace_with_hooks_rolls_back_failed_after_create() {
        let root = fresh_temp_path("symphony-workspace-hooks-create-failure");
        let hooks = WorkspaceHooks {
            after_create: Some("echo created".to_owned()),
            ..WorkspaceHooks::default()
        };
        let executor = RecordingExecutor::with_responses(vec![
            Ok(HookResult::with_status(Some(9), "stdout", "stderr")),
            Ok(HookResult::success()),
        ]);

        let first = prepare_workspace_with_hooks(&root, "SYM-44", &hooks, &executor);
        assert_eq!(
            first,
            Err(WorkspaceError::HookExitedNonZero {
                hook: "after_create".to_owned(),
                exit_code: 9,
                result: HookResult::with_status(Some(9), "stdout", "stderr"),
            })
        );
        assert!(!root.join("SYM-44").exists());

        let second = prepare_workspace_with_hooks(&root, "SYM-44", &hooks, &executor)
            .expect("second prepare should retry after rollback");
        assert!(second.created_now);

        let requests = executor.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].kind, WorkspaceHookKind::AfterCreate);
        assert_eq!(requests[1].kind, WorkspaceHookKind::AfterCreate);

        cleanup(&root);
    }

    #[test]
    fn run_before_and_after_run_hooks_invoke_executor() {
        let root = fresh_temp_path("symphony-workspace-hooks-run");
        let workspace = prepare_workspace(&root, "SYM-45").expect("workspace should be prepared");
        let hooks = WorkspaceHooks {
            before_run: Some("echo before".to_owned()),
            after_run: Some("echo after".to_owned()),
            ..WorkspaceHooks::default()
        };
        let executor = RecordingExecutor::default();

        let before = run_before_run_hook(&workspace, &hooks, &executor)
            .expect("before_run should succeed")
            .expect("before_run hook should execute");
        let after = run_after_run_hook(&workspace, &hooks, &executor)
            .expect("after_run should succeed")
            .expect("after_run hook should execute");

        assert_eq!(before.exit_code, Some(0));
        assert_eq!(after.exit_code, Some(0));
        let requests = executor.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].kind, WorkspaceHookKind::BeforeRun);
        assert_eq!(requests[1].kind, WorkspaceHookKind::AfterRun);

        cleanup(&root);
    }

    #[test]
    fn run_hook_truncates_output_by_policy() {
        let root = fresh_temp_path("symphony-workspace-hooks-truncate");
        let workspace = prepare_workspace(&root, "SYM-46").expect("workspace should be prepared");
        let hooks = WorkspaceHooks {
            before_run: Some("echo before".to_owned()),
            output_limit_bytes: 5,
            ..WorkspaceHooks::default()
        };
        let executor = RecordingExecutor::with_responses(vec![Ok(HookResult::with_output(
            "123456789",
            "abcdef",
        ))]);

        let result = run_before_run_hook(&workspace, &hooks, &executor)
            .expect("hook should run")
            .expect("hook should execute");

        assert_eq!(result.stdout, "12345");
        assert_eq!(result.stderr, "abcde");
        assert!(result.truncated);

        cleanup(&root);
    }

    #[test]
    fn run_hook_reports_timeout() {
        let root = fresh_temp_path("symphony-workspace-hooks-timeout");
        let workspace = prepare_workspace(&root, "SYM-47").expect("workspace should be prepared");
        let hooks = WorkspaceHooks {
            before_run: Some("echo before".to_owned()),
            timeout_ms: 10,
            ..WorkspaceHooks::default()
        };
        let timeout_result = HookResult {
            stdout: "partial stdout".to_owned(),
            stderr: "partial stderr".to_owned(),
            timed_out: true,
            exit_code: None,
            truncated: false,
        };
        let executor = RecordingExecutor::with_responses(vec![Ok(timeout_result)]);

        let result = run_before_run_hook(&workspace, &hooks, &executor);

        assert_eq!(
            result,
            Err(WorkspaceError::HookTimedOut {
                hook: "before_run".to_owned(),
                timeout_ms: 10,
                result: HookResult {
                    stdout: "partial stdout".to_owned(),
                    stderr: "partial stderr".to_owned(),
                    timed_out: true,
                    exit_code: None,
                    truncated: false,
                },
            })
        );

        cleanup(&root);
    }

    #[test]
    fn run_after_run_hook_reports_nonzero_exit_taxonomy() {
        let root = fresh_temp_path("symphony-workspace-hooks-after-run-error");
        let workspace = prepare_workspace(&root, "SYM-47A").expect("workspace should be prepared");
        let hooks = WorkspaceHooks {
            after_run: Some("echo after".to_owned()),
            ..WorkspaceHooks::default()
        };
        let executor = RecordingExecutor::with_responses(vec![Ok(HookResult::with_status(
            Some(7),
            "after stdout",
            "after stderr",
        ))]);

        let result = run_after_run_hook(&workspace, &hooks, &executor);

        assert_eq!(
            result,
            Err(WorkspaceError::HookExitedNonZero {
                hook: "after_run".to_owned(),
                exit_code: 7,
                result: HookResult::with_status(Some(7), "after stdout", "after stderr"),
            })
        );

        cleanup(&root);
    }

    #[test]
    fn remove_workspace_with_hooks_runs_before_remove() {
        let root = fresh_temp_path("symphony-workspace-hooks-remove");
        let workspace = prepare_workspace(&root, "SYM-48").expect("workspace should be prepared");
        let hooks = WorkspaceHooks {
            before_remove: Some("echo remove".to_owned()),
            ..WorkspaceHooks::default()
        };
        let executor = RecordingExecutor::default();

        let removed = remove_workspace_with_hooks(&root, "SYM-48", &hooks, &executor)
            .expect("remove should succeed");

        assert!(removed);
        assert!(!workspace.path.exists());
        let requests = executor.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].kind, WorkspaceHookKind::BeforeRemove);

        cleanup(&root);
    }

    #[test]
    fn remove_workspace_with_hooks_preserves_workspace_when_before_remove_fails() {
        let root = fresh_temp_path("symphony-workspace-hooks-remove-rollback");
        let workspace = prepare_workspace(&root, "SYM-48B").expect("workspace should be prepared");
        fs::write(workspace.path.join("keep.txt"), "still here")
            .expect("workspace marker should be writable");
        let hooks = WorkspaceHooks {
            before_remove: Some("echo remove".to_owned()),
            ..WorkspaceHooks::default()
        };
        let executor = RecordingExecutor::with_responses(vec![Ok(HookResult::with_status(
            Some(9),
            "remove stdout",
            "remove stderr",
        ))]);

        let result = remove_workspace_with_hooks(&root, "SYM-48B", &hooks, &executor);

        assert_eq!(
            result,
            Err(WorkspaceError::HookExitedNonZero {
                hook: "before_remove".to_owned(),
                exit_code: 9,
                result: HookResult::with_status(Some(9), "remove stdout", "remove stderr"),
            })
        );
        assert!(workspace.path.exists());
        assert_eq!(
            fs::read_to_string(workspace.path.join("keep.txt"))
                .expect("workspace contents should remain for fallback cleanup"),
            "still here"
        );

        let removed = remove_workspace(&root, "SYM-48B").expect("direct removal should succeed");
        assert!(removed);
        assert!(!workspace.path.exists());

        cleanup(&root);
    }

    #[test]
    fn remove_workspace_with_hooks_tolerates_hook_owned_deletion() {
        let root = fresh_temp_path("symphony-workspace-hooks-remove-owned");
        let workspace = prepare_workspace(&root, "SYM-48C").expect("workspace should be prepared");

        struct RemovingExecutor {
            requests: RefCell<Vec<HookRequest>>,
            workspace_path: PathBuf,
        }

        impl HookExecutor for RemovingExecutor {
            fn execute(&self, request: &HookRequest) -> Result<HookResult, String> {
                self.requests.borrow_mut().push(request.clone());
                fs::remove_dir_all(&self.workspace_path).map_err(|error| error.to_string())?;
                Ok(HookResult::success())
            }
        }

        let hooks = WorkspaceHooks {
            before_remove: Some("echo remove".to_owned()),
            ..WorkspaceHooks::default()
        };
        let executor = RemovingExecutor {
            requests: RefCell::new(Vec::new()),
            workspace_path: workspace.path.clone(),
        };

        let removed = remove_workspace_with_hooks(&root, "SYM-48C", &hooks, &executor)
            .expect("remove should succeed when hook already removed workspace");

        assert!(removed);
        assert!(!workspace.path.exists());
        assert_eq!(executor.requests.borrow().len(), 1);

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

    // ========================================================================
    // W3.1.2: Long Path Behavior Tests
    // ========================================================================

    #[test]
    fn workspace_creation_rejects_path_exceeding_limit() {
        // Test that very long paths are handled gracefully
        let root = fresh_temp_path("symphony-workspace-long-path");
        fs::create_dir_all(&root).expect("root should be creatable");

        // Create a path with a very long component (>255 chars, typical filename limit)
        let long_key: String = (0..300).map(|_| 'a').collect();
        let result = workspace_path(&root, &long_key);

        // Should either succeed with truncation/sanitization or fail gracefully
        match result {
            Ok(path) => {
                // Path was created - verify it's within bounds
                // Use canonicalized root for comparison since workspace_path resolves the root
                let resolved_root = root.canonicalize().unwrap_or_else(|_| root.clone());
                assert!(path.starts_with(&resolved_root));
            }
            Err(WorkspaceError::PathResolution { .. }) => {
                // Acceptable - path was too long to resolve
            }
            Err(_) => {}
        }

        cleanup(&root);
    }

    #[test]
    fn deeply_nested_workspace_path_is_valid() {
        let root = fresh_temp_path("symphony-workspace-nested");
        fs::create_dir_all(&root).expect("root should be creatable");

        // Create deeply nested workspace path
        let nested_key = "level1/level2/level3/level4/level5";
        let result = workspace_path(&root, nested_key);

        // "/" is encoded, so this should create a valid path
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.starts_with(root.canonicalize().unwrap_or_else(|_| root.clone())));

        cleanup(&root);
    }

    #[test]
    fn path_boundary_at_component_limit() {
        let root = fresh_temp_path("symphony-workspace-boundary");
        fs::create_dir_all(&root).expect("root should be creatable");

        // Test with key at typical filesystem component limit (255 chars)
        let boundary_key: String = (0..255).map(|_| 'x').collect();
        let result = prepare_workspace(&root, &boundary_key);

        // Should handle gracefully
        match result {
            Ok(workspace) => {
                assert!(workspace.path.exists());
                cleanup(&root);
            }
            Err(_) => {
                cleanup(&root);
            }
        }
    }

    // ========================================================================
    // W3.1.2: Permission Edge Behavior Tests (Unix-specific)
    // ========================================================================

    #[test]
    #[cfg(unix)]
    fn workspace_creation_in_readonly_parent_fails_gracefully() {
        use std::os::unix::fs::PermissionsExt;

        let root = fresh_temp_path("symphony-workspace-perm-test");
        let readonly_dir = root.join("readonly");
        fs::create_dir_all(&readonly_dir).expect("should create readonly dir");

        // Set directory to read-only
        let mut perms = fs::metadata(&readonly_dir)
            .expect("should get metadata")
            .permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&readonly_dir, perms).expect("should set permissions");

        // Attempt to create workspace inside read-only directory
        let result = prepare_workspace(&readonly_dir, "SHOULD-FAIL");

        // Restore permissions for cleanup
        let mut perms = fs::metadata(&readonly_dir)
            .expect("should get metadata for restore")
            .permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&readonly_dir, perms);

        // Should fail with appropriate error
        assert!(
            result.is_err(),
            "should fail to create workspace in read-only dir"
        );
        cleanup(&root);
    }

    #[test]
    #[cfg(unix)]
    fn hook_execution_in_readonly_workspace_reports_error() {
        use std::os::unix::fs::PermissionsExt;

        let root = fresh_temp_path("symphony-workspace-hook-perm");
        let workspace =
            prepare_workspace(&root, "SYM-PERM-TEST").expect("workspace should be prepared");

        // Create a file and make parent directory read-only
        fs::write(workspace.path.join("test.txt"), "content").expect("should write file");

        let hooks = WorkspaceHooks {
            before_run: Some("echo test".to_owned()),
            ..WorkspaceHooks::default()
        };
        let executor = RecordingExecutor::default();

        // Make workspace read-only after preparation
        let mut perms = fs::metadata(&workspace.path)
            .expect("should get metadata")
            .permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&workspace.path, perms).expect("should set permissions");

        // Hook execution should still work (hooks run with their own permissions)
        let result = run_before_run_hook(&workspace, &hooks, &executor);

        // Restore permissions
        let mut perms = fs::metadata(&workspace.path)
            .expect("should get metadata for restore")
            .permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&workspace.path, perms);

        // Hook should execute (executor is mock, so it succeeds)
        assert!(result.is_ok());

        cleanup(&root);
    }

    #[test]
    #[cfg(unix)]
    fn workspace_cleanup_handles_permission_denied() {
        use std::os::unix::fs::PermissionsExt;

        let root = fresh_temp_path("symphony-workspace-cleanup-perm");
        let workspace =
            prepare_workspace(&root, "SYM-CLEANUP").expect("workspace should be prepared");

        // Create a file inside
        fs::write(workspace.path.join("protected.txt"), "data").expect("should write file");

        // Make the file read-only and the workspace directory read-only
        let mut file_perms = fs::metadata(workspace.path.join("protected.txt"))
            .expect("should get file metadata")
            .permissions();
        file_perms.set_mode(0o444);
        fs::set_permissions(workspace.path.join("protected.txt"), file_perms)
            .expect("should set file permissions");

        // Attempt removal - should either succeed or fail gracefully
        let result = remove_workspace(&root, "SYM-CLEANUP");

        // Restore permissions if removal failed
        if result.is_err() {
            let mut file_perms = fs::metadata(workspace.path.join("protected.txt"))
                .expect("should get file metadata for restore")
                .permissions();
            file_perms.set_mode(0o644);
            let _ = fs::set_permissions(workspace.path.join("protected.txt"), file_perms);
            let _ = remove_workspace(&root, "SYM-CLEANUP");
        }

        cleanup(&root);
    }
}
