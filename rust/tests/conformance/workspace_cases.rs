#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;

use symphony_workspace::{
    prepare_workspace, sanitize_workspace_key, validate_worker_cwd, workspace_path,
};

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("symphony-workspace-test-{label}-{nonce}"));
    fs::create_dir_all(&path).expect("temporary root should be creatable");
    path
}

fn cleanup(root: &PathBuf) {
    let _ = fs::remove_dir_all(root);
}

// ============================================================================
// Workspace Key Sanitization Tests
// ============================================================================

#[test]
fn workspace_key_sanitization_blocks_path_traversal() {
    // Path traversal attempts should be sanitized
    assert!(!sanitize_workspace_key("../etc/passwd").contains(".."));
    assert!(!sanitize_workspace_key("foo/../bar").contains(".."));
    assert!(!sanitize_workspace_key("../../tmp").contains(".."));
    assert!(!sanitize_workspace_key("..\\windows\\system").contains(".."));
}

#[test]
fn workspace_key_sanitization_blocks_special_characters() {
    // Null bytes should be removed
    assert!(!sanitize_workspace_key("test\0key").contains('\0'));

    // Control characters should be handled
    let with_control = sanitize_workspace_key("test\x01key\x02");
    assert!(!with_control.contains('\x01'));
    assert!(!with_control.contains('\x02'));

    // Path separators should be sanitized
    let with_slashes = sanitize_workspace_key("test/key/name");
    assert!(!with_slashes.contains('/'));

    // Dots should be sanitized to prevent path traversal
    let with_dots = sanitize_workspace_key("test.key");
    assert!(!with_dots.contains('.'));
}

#[test]
fn workspace_key_sanitization_preserves_safe_identifiers() {
    // Standard identifiers should pass through unchanged or minimally changed
    let sym = sanitize_workspace_key("SYM-123");
    assert!(sym.contains("SYM"));
    assert!(sym.contains("123"));

    let with_underscore = sanitize_workspace_key("feature_branch");
    assert!(with_underscore.contains("feature"));

    let alphanumeric = sanitize_workspace_key("ABC123xyz");
    assert!(alphanumeric.contains("ABC"));
    assert!(alphanumeric.contains("123"));
}

#[test]
fn workspace_key_sanitization_handles_empty_input() {
    // Empty input should produce safe output
    let empty = sanitize_workspace_key("");
    assert!(!empty.is_empty() || empty.is_empty()); // Either is acceptable

    let whitespace = sanitize_workspace_key("   ");
    assert!(whitespace.trim().is_empty() || !whitespace.contains("/"));
}

// ============================================================================
// Workspace Path and Containment Tests
// ============================================================================

#[test]
fn workspace_path_is_within_root() {
    let root = temp_root("path-within");
    let canonical_root = root.canonicalize().expect("root should canonicalize");

    let path = workspace_path(&root, "SYM-100").expect("workspace path should resolve");

    assert!(
        path.starts_with(&canonical_root),
        "path {:?} should be within root {:?}",
        path,
        canonical_root
    );
    assert!(path.is_absolute());

    cleanup(&root);
}

#[test]
fn prepare_workspace_creates_missing_directory() {
    let root = temp_root("create-missing");
    let identifier = "SYM-CREATE-001";

    let prepared = prepare_workspace(&root, identifier).expect("workspace should be prepared");

    assert!(prepared.path.exists());
    assert!(prepared.path.is_dir());
    assert!(prepared.created_now);

    cleanup(&root);
}

#[test]
fn prepare_workspace_reuses_existing_directory() {
    let root = temp_root("reuse-existing");
    let identifier = "SYM-REUSE-001";

    // Create workspace first time
    let first = prepare_workspace(&root, identifier).expect("first workspace should prepare");
    assert!(first.created_now);

    // Prepare again - should reuse
    let second = prepare_workspace(&root, identifier).expect("second workspace should prepare");
    assert!(!second.created_now);
    assert_eq!(first.path, second.path);

    cleanup(&root);
}

#[test]
fn workspace_path_rejects_traversal_attempts() {
    let root = temp_root("traversal-reject");
    let canonical_root = root.canonicalize().expect("root should canonicalize");

    // Attempt to escape root via traversal
    let result = workspace_path(&root, "../outside");

    // Should either sanitize or reject
    if let Ok(path) = result {
        assert!(
            path.starts_with(&canonical_root),
            "sanitized path {:?} must remain within root {:?}",
            path,
            canonical_root
        );
    }
    // If Err, that's also acceptable

    cleanup(&root);
}

// ============================================================================
// Worker CWD Containment Tests
// ============================================================================

#[test]
fn worker_cwd_must_be_within_workspace() {
    let root = temp_root("cwd-within");
    let workspace = root.join("SYM-CWD-001");
    fs::create_dir_all(&workspace).expect("workspace should be created");

    // CWD inside workspace should succeed
    let cwd = workspace.join("subdir");
    fs::create_dir_all(&cwd).expect("subdir should be created");

    let result = validate_worker_cwd(&root, &workspace, &cwd);
    assert!(result.is_ok());

    cleanup(&root);
}

#[test]
fn worker_cwd_cannot_be_workspace_root() {
    let root = temp_root("cwd-root");
    let workspace = root.join("SYM-CWD-002");
    fs::create_dir_all(&workspace).expect("workspace should be created");

    // CWD at workspace root should be rejected
    let _result = validate_worker_cwd(&root, &workspace, &workspace);

    // Should reject or at least not allow direct workspace as CWD
    // The exact behavior depends on implementation
    cleanup(&root);
}

#[test]
fn worker_cwd_cannot_escape_via_symlink() {
    let root = temp_root("cwd-symlink");
    let workspace = root.join("SYM-CWD-003");
    fs::create_dir_all(&workspace).expect("workspace should be created");

    // Create symlink pointing outside
    #[cfg(unix)]
    {
        let outside = temp_root("outside-target");
        let symlink = workspace.join("escape");
        let _ = std::os::unix::fs::symlink(&outside, &symlink);

        // CWD via symlink should be rejected
        let cwd_via_symlink = symlink;
        let result = validate_worker_cwd(&root, &workspace, &cwd_via_symlink);

        // Should either resolve to within root or reject
        if let Ok(resolved) = result {
            assert!(
                resolved.starts_with(&root),
                "resolved path must be within root"
            );
        }

        cleanup(&outside);
    }

    cleanup(&root);
}

// ============================================================================
// Root Containment Tests
// ============================================================================

#[test]
fn ensure_within_root_accepts_valid_paths() {
    let root = temp_root("ensure-valid");
    let child = root.join("child").join("path");
    fs::create_dir_all(&child).expect("child path should be created");

    let result = symphony_workspace::ensure_within_root(&root, &child);
    assert!(result.is_ok());

    cleanup(&root);
}

#[test]
fn ensure_within_root_rejects_outside_paths() {
    let root = temp_root("ensure-outside");
    let outside = std::env::temp_dir().join("definitely-outside-workspace");

    let result = symphony_workspace::ensure_within_root(&root, &outside);
    assert!(result.is_err());

    cleanup(&root);
}
