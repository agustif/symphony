# Test fixture for workflow state

This fixture demonstrates variable substitution in fixtures.

Variables:
- ISSUE_ID: The issue ID
- ISSUE_STATE: The issue state

Example usage:
```rust
use symphony_testkit::{load_json_fixture_with_vars, FixtureBuilder};

let mut vars = std::collections::HashMap::new();
vars.insert("ISSUE_ID".to_string(), "123".to_string());
vars.insert("ISSUE_STATE".to_string(), "Running".to_string());

let data: serde_json::Value = load_json_fixture_with_vars("fixtures/test_issues.json", &vars)
    .expect("Failed to load fixture");
```
