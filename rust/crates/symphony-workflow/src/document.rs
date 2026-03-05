use serde_yaml::Mapping;

#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowDocument {
    pub front_matter: Mapping,
    pub prompt_body: String,
}

impl WorkflowDocument {
    pub fn new(front_matter: Mapping, prompt_body: String) -> Self {
        Self {
            front_matter,
            prompt_body,
        }
    }
}
