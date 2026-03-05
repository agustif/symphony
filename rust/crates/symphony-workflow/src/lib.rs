#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowDocument {
    pub front_matter: String,
    pub prompt_body: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkflowError {
    #[error("workflow body is empty")]
    EmptyBody,
}

pub fn parse(markdown: &str) -> Result<WorkflowDocument, WorkflowError> {
    let prompt_body = markdown.trim().to_owned();
    if prompt_body.is_empty() {
        return Err(WorkflowError::EmptyBody);
    }

    Ok(WorkflowDocument {
        front_matter: String::new(),
        prompt_body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_empty_body() {
        assert_eq!(parse("\n\n"), Err(WorkflowError::EmptyBody));
    }
}
