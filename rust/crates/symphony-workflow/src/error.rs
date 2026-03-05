use std::path::PathBuf;

use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum WorkflowError {
    #[error("workflow file is missing: `{0}`")]
    MissingWorkflowFile(PathBuf),
    #[error("failed to read workflow file `{path}`: {reason}")]
    ReadWorkflow { path: PathBuf, reason: String },
    #[error("workflow body is empty")]
    EmptyBody,
    #[error("workflow front matter is not terminated")]
    UnterminatedFrontMatter,
    #[error("workflow front matter must decode to a YAML map")]
    FrontMatterNotMap,
    #[error("invalid workflow front matter: {0}")]
    InvalidFrontMatter(String),
}
