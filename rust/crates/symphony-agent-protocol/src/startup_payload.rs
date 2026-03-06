use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SupportedToolSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_input_schema")]
    pub input_schema: Value,
}

impl SupportedToolSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: default_input_schema(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_input_schema(mut self, input_schema: Value) -> Self {
        self.input_schema = input_schema;
        self
    }
}

pub fn build_initialize_request(
    id: Value,
    client_name: &str,
    client_version: &str,
    supported_tools: &[SupportedToolSpec],
) -> Value {
    let mut params = Map::new();
    params.insert(
        "client".to_owned(),
        json!({
            "name": client_name,
            "version": client_version
        }),
    );

    if !supported_tools.is_empty() {
        params.insert(
            "capabilities".to_owned(),
            json!({
                "tools": {
                    "supported": supported_tools
                }
            }),
        );
    }

    json!({
        "id": id,
        "method": "initialize",
        "params": params
    })
}

pub fn build_initialized_notification() -> Value {
    json!({
        "method": "initialized",
        "params": {}
    })
}

pub fn build_thread_start_request(
    id: Value,
    cwd: &Path,
    approval_policy: Option<&Value>,
    sandbox: Option<&str>,
) -> Value {
    let mut params = Map::new();
    params.insert("cwd".to_owned(), json!(cwd.to_string_lossy()));
    if let Some(approval_policy) = approval_policy {
        params.insert("approvalPolicy".to_owned(), approval_policy.clone());
    }
    if let Some(sandbox) = sandbox {
        params.insert("sandbox".to_owned(), json!(sandbox));
    }

    json!({
        "id": id,
        "method": "thread/start",
        "params": params
    })
}

pub fn build_turn_start_request(
    id: Value,
    thread_id: &str,
    prompt_text: &str,
    cwd: &Path,
    title: &str,
    approval_policy: Option<&Value>,
    sandbox_policy: Option<&Value>,
) -> Value {
    let mut params = Map::new();
    params.insert("threadId".to_owned(), json!(thread_id));
    params.insert(
        "input".to_owned(),
        json!([
            {
                "type": "text",
                "text": prompt_text
            }
        ]),
    );
    params.insert("cwd".to_owned(), json!(cwd.to_string_lossy()));
    params.insert("title".to_owned(), json!(title));
    if let Some(approval_policy) = approval_policy {
        params.insert("approvalPolicy".to_owned(), approval_policy.clone());
    }
    if let Some(sandbox_policy) = sandbox_policy {
        params.insert(
            "sandboxPolicy".to_owned(),
            normalized_sandbox_policy(sandbox_policy),
        );
    }

    json!({
        "id": id,
        "method": "turn/start",
        "params": params
    })
}

fn normalized_sandbox_policy(value: &Value) -> Value {
    match value {
        Value::String(policy_type) => json!({ "type": policy_type }),
        other => other.clone(),
    }
}

fn default_input_schema() -> Value {
    json!({
        "type": "object"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_request_omits_tool_advertisement_when_empty() {
        let request = build_initialize_request(
            json!(1),
            "symphony-rust",
            "0.1.0",
            &[] as &[SupportedToolSpec],
        );

        assert_eq!(request["method"], "initialize");
        assert_eq!(request["params"]["client"]["name"], "symphony-rust");
        assert!(
            request["params"]
                .as_object()
                .is_some_and(|params| !params.contains_key("capabilities"))
        );
    }

    #[test]
    fn initialize_request_includes_supported_tools_when_present() {
        let request = build_initialize_request(
            json!(1),
            "symphony-rust",
            "0.1.0",
            &[SupportedToolSpec::new("linear_graphql")
                .with_description("Execute Linear GraphQL queries")
                .with_input_schema(json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    },
                    "required": ["query"]
                }))],
        );

        let supported = request["params"]["capabilities"]["tools"]["supported"]
            .as_array()
            .expect("supported tools list should be present");
        assert_eq!(supported.len(), 1);
        assert_eq!(supported[0]["name"], "linear_graphql");
        assert_eq!(
            supported[0]["input_schema"]["required"][0],
            serde_json::json!("query")
        );
    }

    #[test]
    fn thread_start_request_contains_policy_and_workspace_context() {
        let request = build_thread_start_request(
            json!(2),
            Path::new("/tmp/workspace"),
            Some(&json!({"mode": "auto"})),
            Some("workspace-write"),
        );

        assert_eq!(request["method"], "thread/start");
        assert_eq!(request["params"]["approvalPolicy"]["mode"], "auto");
        assert_eq!(request["params"]["sandbox"], "workspace-write");
        assert_eq!(request["params"]["cwd"], "/tmp/workspace");
    }

    #[test]
    fn turn_start_request_contains_prompt_context_and_policy() {
        let request = build_turn_start_request(
            json!(3),
            "thread-1",
            "Fix issue SYM-1",
            Path::new("/tmp/workspace"),
            "SYM-1: Fix issue",
            Some(&json!({"reject": {"sandbox_approval": true}})),
            Some(&json!({"type": "workspaceWrite"})),
        );

        assert_eq!(request["method"], "turn/start");
        assert_eq!(request["params"]["threadId"], "thread-1");
        assert_eq!(request["params"]["input"][0]["text"], "Fix issue SYM-1");
        assert_eq!(request["params"]["title"], "SYM-1: Fix issue");
        assert_eq!(
            request["params"]["approvalPolicy"]["reject"]["sandbox_approval"],
            serde_json::json!(true)
        );
        assert_eq!(
            request["params"]["sandboxPolicy"]["type"],
            serde_json::json!("workspaceWrite")
        );
    }

    #[test]
    fn turn_start_request_promotes_legacy_string_sandbox_policy() {
        let request = build_turn_start_request(
            json!(3),
            "thread-1",
            "Fix issue SYM-1",
            Path::new("/tmp/workspace"),
            "SYM-1: Fix issue",
            Some(&json!("auto")),
            Some(&json!("workspace-write")),
        );

        assert_eq!(request["params"]["approvalPolicy"], "auto");
        assert_eq!(
            request["params"]["sandboxPolicy"],
            serde_json::json!({"type": "workspace-write"})
        );
    }

    #[test]
    fn thread_and_turn_payloads_omit_optional_policy_fields_when_missing() {
        let thread = build_thread_start_request(json!(2), Path::new("/tmp/workspace"), None, None);
        assert!(
            thread["params"]
                .as_object()
                .is_some_and(|params| !params.contains_key("approvalPolicy"))
        );
        assert!(
            thread["params"]
                .as_object()
                .is_some_and(|params| !params.contains_key("sandbox"))
        );

        let turn = build_turn_start_request(
            json!(3),
            "thread-1",
            "Prompt",
            Path::new("/tmp/workspace"),
            "Title",
            None,
            None,
        );
        assert!(
            turn["params"]
                .as_object()
                .is_some_and(|params| !params.contains_key("approvalPolicy"))
        );
        assert!(
            turn["params"]
                .as_object()
                .is_some_and(|params| !params.contains_key("sandboxPolicy"))
        );
    }
}
