#![forbid(unsafe_code)]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use symphony_workflow::{DEFAULT_WORKFLOW_FILE_NAME, WorkflowError, load_workflow, parse};

static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn missing_workflow_file_returns_typed_error() {
    let root = fresh_temp_path("symphony-conformance-missing-workflow");

    let error = load_workflow(&root, None).expect_err("missing workflow should fail");
    assert_eq!(
        error,
        WorkflowError::MissingWorkflowFile(root.join(DEFAULT_WORKFLOW_FILE_NAME))
    );
}

#[test]
fn invalid_workflow_front_matter_is_rejected() {
    let root = fresh_temp_path("symphony-conformance-invalid-workflow");
    let workflow_path = root.join(DEFAULT_WORKFLOW_FILE_NAME);
    fs::create_dir_all(&root).expect("temp root should be creatable");
    fs::write(&workflow_path, "---\ntracker: [\n---\nPrompt")
        .expect("workflow file should be writable");

    let error = load_workflow(&root, None).expect_err("invalid front matter should fail");
    assert!(matches!(error, WorkflowError::InvalidFrontMatter(_)));

    cleanup(&root);
}

#[test]
fn non_map_front_matter_is_rejected() {
    let error = parse("---\n- not\n- a\n- map\n---\nPrompt").expect_err("must fail");
    assert_eq!(error, WorkflowError::FrontMatterNotMap);
}

#[test]
fn prompt_without_front_matter_is_accepted_as_body() {
    let workflow = parse("Run migration and report status.").expect("parse should succeed");
    assert!(workflow.front_matter.is_empty());
    assert_eq!(workflow.prompt_body, "Run migration and report status.");
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
