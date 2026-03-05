use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkflowError {
    #[error("workflow body is empty")]
    EmptyBody,
    #[error("workflow front matter is not terminated")]
    UnterminatedFrontMatter,
    #[error("workflow front matter must decode to a YAML map")]
    FrontMatterNotMap,
    #[error("invalid workflow front matter: {0}")]
    InvalidFrontMatter(String),
}
