use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{WorkflowDocument, WorkflowError, parse};

pub const DEFAULT_WORKFLOW_FILE_NAME: &str = "WORKFLOW.md";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowChangeStamp {
    pub content_hash: u64,
    pub content_bytes: usize,
    pub modified_unix_ms: Option<u128>,
}

impl WorkflowChangeStamp {
    fn from_markdown(markdown: &str, modified_at: Option<SystemTime>) -> Self {
        let mut hasher = DefaultHasher::new();
        markdown.hash(&mut hasher);

        let modified_unix_ms = modified_at
            .and_then(|modified_at| modified_at.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis());

        Self {
            content_hash: hasher.finish(),
            content_bytes: markdown.len(),
            modified_unix_ms,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedWorkflow {
    pub path: PathBuf,
    pub markdown: String,
    pub document: WorkflowDocument,
    pub change_stamp: WorkflowChangeStamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowReloadOutcome {
    Unchanged {
        stamp: WorkflowChangeStamp,
    },
    Updated {
        previous: WorkflowChangeStamp,
        current: WorkflowChangeStamp,
    },
    Retained {
        retained: WorkflowChangeStamp,
        error: WorkflowError,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowReloader {
    path: PathBuf,
    current: LoadedWorkflow,
}

impl WorkflowReloader {
    pub fn load(root: &Path, workflow_path: Option<&Path>) -> Result<Self, WorkflowError> {
        let resolved_path = resolve_workflow_path(root, workflow_path);
        Self::load_from_path(resolved_path)
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, WorkflowError> {
        let path = path.as_ref().to_path_buf();
        let current = load_workflow_from_path(&path)?;
        Ok(Self { path, current })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn current(&self) -> &LoadedWorkflow {
        &self.current
    }

    pub fn reload(&mut self) -> WorkflowReloadOutcome {
        match load_workflow_from_path(&self.path) {
            Ok(reloaded) => {
                if reloaded.change_stamp == self.current.change_stamp {
                    WorkflowReloadOutcome::Unchanged {
                        stamp: self.current.change_stamp.clone(),
                    }
                } else {
                    let previous = self.current.change_stamp.clone();
                    self.current = reloaded;

                    WorkflowReloadOutcome::Updated {
                        previous,
                        current: self.current.change_stamp.clone(),
                    }
                }
            }
            Err(error) => WorkflowReloadOutcome::Retained {
                retained: self.current.change_stamp.clone(),
                error,
            },
        }
    }
}

pub fn resolve_workflow_path(root: &Path, workflow_path: Option<&Path>) -> PathBuf {
    match workflow_path {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => root.join(path),
        None => root.join(DEFAULT_WORKFLOW_FILE_NAME),
    }
}

pub fn load_workflow(
    root: &Path,
    workflow_path: Option<&Path>,
) -> Result<LoadedWorkflow, WorkflowError> {
    let path = resolve_workflow_path(root, workflow_path);
    load_workflow_from_path(path)
}

pub fn load_workflow_from_path(path: impl AsRef<Path>) -> Result<LoadedWorkflow, WorkflowError> {
    let path = path.as_ref().to_path_buf();
    let modified_at = read_modified_time(&path)?;
    let markdown = read_markdown(&path)?;
    let document = parse(&markdown)?;
    let change_stamp = WorkflowChangeStamp::from_markdown(&markdown, modified_at);

    Ok(LoadedWorkflow {
        path,
        markdown,
        document,
        change_stamp,
    })
}

fn read_modified_time(path: &Path) -> Result<Option<SystemTime>, WorkflowError> {
    let metadata = fs::metadata(path).map_err(|error| map_file_error(path, error))?;
    Ok(metadata.modified().ok())
}

fn read_markdown(path: &Path) -> Result<String, WorkflowError> {
    fs::read_to_string(path).map_err(|error| map_file_error(path, error))
}

fn map_file_error(path: &Path, error: std::io::Error) -> WorkflowError {
    if error.kind() == ErrorKind::NotFound {
        WorkflowError::MissingWorkflowFile(path.to_path_buf())
    } else {
        WorkflowError::ReadWorkflow {
            path: path.to_path_buf(),
            reason: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::WorkflowError;

    static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn resolves_default_workflow_path() {
        let root = Path::new("/tmp/symphony-test");

        let resolved = resolve_workflow_path(root, None);

        assert_eq!(resolved, root.join(DEFAULT_WORKFLOW_FILE_NAME));
    }

    #[test]
    fn resolves_relative_explicit_workflow_path() {
        let root = Path::new("/tmp/symphony-test");

        let resolved = resolve_workflow_path(root, Some(Path::new("nested/WORKFLOW.custom.md")));

        assert_eq!(resolved, root.join("nested/WORKFLOW.custom.md"));
    }

    #[test]
    fn resolves_absolute_explicit_workflow_path() {
        let root = Path::new("/tmp/symphony-test");
        let explicit = Path::new("/var/tmp/custom-workflow.md");

        let resolved = resolve_workflow_path(root, Some(explicit));

        assert_eq!(resolved, explicit);
    }

    #[test]
    fn returns_typed_error_when_workflow_is_missing() {
        let root = fresh_temp_path("symphony-workflow-missing");

        let error = load_workflow(&root, None).expect_err("workflow should be missing");
        assert_eq!(
            error,
            WorkflowError::MissingWorkflowFile(root.join(DEFAULT_WORKFLOW_FILE_NAME))
        );
    }

    #[test]
    fn reload_returns_unchanged_when_stamp_is_identical() {
        let root = fresh_temp_path("symphony-workflow-unchanged");
        let workflow_path = root.join(DEFAULT_WORKFLOW_FILE_NAME);
        fs::create_dir_all(&root).expect("root should be creatable");
        fs::write(&workflow_path, "Prompt body").expect("workflow file should be writable");

        let mut reloader = WorkflowReloader::load(&root, None).expect("initial load should work");
        let initial_stamp = reloader.current().change_stamp.clone();
        let outcome = reloader.reload();

        assert_eq!(
            outcome,
            WorkflowReloadOutcome::Unchanged {
                stamp: initial_stamp.clone()
            }
        );
        assert_eq!(reloader.current().change_stamp, initial_stamp);

        cleanup(&root);
    }

    #[test]
    fn reload_returns_updated_when_workflow_changes() {
        let root = fresh_temp_path("symphony-workflow-updated");
        let workflow_path = root.join(DEFAULT_WORKFLOW_FILE_NAME);
        fs::create_dir_all(&root).expect("root should be creatable");
        fs::write(&workflow_path, "Prompt body").expect("workflow file should be writable");

        let mut reloader = WorkflowReloader::load(&root, None).expect("initial load should work");
        let previous_stamp = reloader.current().change_stamp.clone();
        fs::write(&workflow_path, "Prompt body\n\nUpdated").expect("workflow file should update");

        let outcome = reloader.reload();

        match outcome {
            WorkflowReloadOutcome::Updated { previous, current } => {
                assert_eq!(previous, previous_stamp);
                assert_ne!(current, previous);
            }
            _ => panic!("expected updated outcome"),
        }
        assert_eq!(
            reloader.current().document.prompt_body,
            "Prompt body\n\nUpdated"
        );

        cleanup(&root);
    }

    #[test]
    fn reload_retains_last_good_when_new_content_is_invalid() {
        let root = fresh_temp_path("symphony-workflow-retained");
        let workflow_path = root.join(DEFAULT_WORKFLOW_FILE_NAME);
        fs::create_dir_all(&root).expect("root should be creatable");
        fs::write(&workflow_path, "Original prompt").expect("workflow file should be writable");

        let mut reloader = WorkflowReloader::load(&root, None).expect("initial load should work");
        let retained_stamp = reloader.current().change_stamp.clone();
        fs::write(&workflow_path, "---\ntracker: [\n---\nPrompt").expect("workflow should update");

        let outcome = reloader.reload();

        match outcome {
            WorkflowReloadOutcome::Retained { retained, error } => {
                assert_eq!(retained, retained_stamp);
                assert!(matches!(error, WorkflowError::InvalidFrontMatter(_)));
            }
            _ => panic!("expected retained outcome"),
        }
        assert_eq!(reloader.current().document.prompt_body, "Original prompt");
        assert_eq!(reloader.current().change_stamp, retained_stamp);

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
