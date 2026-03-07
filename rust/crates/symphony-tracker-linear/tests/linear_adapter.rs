#![forbid(unsafe_code)]

use std::{
    env, fs,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use reqwest::Client;
use serde_json::json;
use symphony_domain::IssueId;
use symphony_tracker::{TrackerClient, TrackerError, TrackerState};
use symphony_tracker_linear::LinearTracker;
use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
    matchers::{body_partial_json, header, method, path},
};

const API_KEY: &str = "linear-api-key";
const PROJECT_SLUG: &str = "symphony";
const REAL_LINEAR_ENDPOINT: &str = "https://api.linear.app/graphql";
const REAL_LINEAR_ENV_FILE: &str = "/Users/af/symphony/.env.local";
const REAL_LINEAR_WORKFLOW_FILE: &str = "/Users/af/symphony/elixir/WORKFLOW.md";
const REAL_LINEAR_PROJECT_SLUG_ENV: &str = "LINEAR_PROJECT_SLUG";

fn build_tracker(server: &MockServer) -> LinearTracker {
    LinearTracker::new(format!("{}/graphql", server.uri()), API_KEY, PROJECT_SLUG)
}

fn build_tracker_with_client(server: &MockServer, client: Client) -> LinearTracker {
    LinearTracker::with_client(
        client,
        format!("{}/graphql", server.uri()),
        API_KEY,
        PROJECT_SLUG,
    )
}

struct SequenceResponder {
    attempts: Arc<AtomicUsize>,
    responses: Vec<ResponseTemplate>,
}

impl Respond for SequenceResponder {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let index = self.attempts.fetch_add(1, Ordering::SeqCst);
        self.responses
            .get(index)
            .cloned()
            .or_else(|| self.responses.last().cloned())
            .expect("sequence responder requires at least one response")
    }
}

fn read_env_file_value(path: &str, key: &str) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;

    contents.lines().find_map(|line| {
        let trimmed = line.trim();
        let trimmed = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let (candidate_key, raw_value) = trimmed.split_once('=')?;
        if candidate_key.trim() != key {
            return None;
        }

        let value = raw_value.trim().trim_matches('"').trim_matches('\'').trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_owned())
        }
    })
}

fn read_workflow_project_slug(path: &str) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let mut frontmatter_lines = contents.lines();
    if frontmatter_lines.next()?.trim() != "---" {
        return None;
    }

    frontmatter_lines
        .take_while(|line| line.trim() != "---")
        .find_map(|line| {
            let (key, raw_value) = line.split_once(':')?;
            if key.trim() != "project_slug" {
                return None;
            }

            let value = raw_value.trim().trim_matches('"').trim_matches('\'').trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_owned())
            }
        })
}

fn live_linear_api_key() -> Option<String> {
    env::var("LINEAR_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| read_env_file_value(REAL_LINEAR_ENV_FILE, "LINEAR_API_KEY"))
}

fn live_linear_project_slug() -> Option<String> {
    env::var(REAL_LINEAR_PROJECT_SLUG_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| read_env_file_value(REAL_LINEAR_ENV_FILE, REAL_LINEAR_PROJECT_SLUG_ENV))
        .or_else(|| read_workflow_project_slug(REAL_LINEAR_WORKFLOW_FILE))
}

fn skip_live_linear_test(reason: impl AsRef<str>) {
    eprintln!("skipped live Linear smoke test: {}", reason.as_ref());
}

fn should_skip_live_linear_error(error: &TrackerError) -> bool {
    match error {
        TrackerError::Transport { .. } => true,
        TrackerError::Status { status, .. } => *status == 408 || *status == 429 || *status >= 500,
        _ => false,
    }
}

#[tokio::test]
async fn fetch_candidates_by_states_normalizes_payload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(header("authorization", "linear-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": " lin_1 ",
                            "identifier": " SYM-101 ",
                            "title": " Harden tracker parity ",
                            "description": " normalize linear payload ",
                            "priority": 2,
                            "state": { "name": " Todo " }
                            ,
                            "branchName": " feature/sym-101 ",
                            "url": " https://linear.app/symphony/issue/SYM-101 ",
                            "labels": {
                                "nodes": [
                                    { "name": "Bug" },
                                    { "name": "backend" },
                                    { "name": "BUG" }
                                ]
                            },
                            "inverseRelations": {
                                "nodes": [
                                    {
                                        "type": " blocks ",
                                        "issue": {
                                            "id": " lin_999 ",
                                            "identifier": " SYM-999 ",
                                            "state": { "name": " In Progress " }
                                        }
                                    },
                                    {
                                        "type": "relates",
                                        "issue": {
                                            "id": "lin_ignored",
                                            "identifier": "SYM-ignored",
                                            "state": { "name": "Todo" }
                                        }
                                    }
                                ]
                            },
                            "createdAt": "1970-01-01T00:00:10Z",
                            "updatedAt": "1970-01-01T00:00:20Z"
                        },
                        {
                            "id": "lin_2",
                            "identifier": "SYM-102",
                            "title": "Second candidate",
                            "state": { "name": "In Progress" }
                        }
                    ],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let issues = tracker
        .fetch_candidates_by_states(&[TrackerState::new("Todo"), TrackerState::new("In Progress")])
        .await
        .expect("state-filtered candidates request should succeed");

    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].id, IssueId("lin_1".to_owned()));
    assert_eq!(issues[0].identifier, "SYM-101");
    assert_eq!(issues[0].title, "Harden tracker parity");
    assert_eq!(
        issues[0].description.as_deref(),
        Some("normalize linear payload")
    );
    assert_eq!(issues[0].priority, Some(2));
    assert_eq!(issues[0].state, TrackerState::new("Todo"));
    assert_eq!(issues[0].branch_name.as_deref(), Some("feature/sym-101"));
    assert_eq!(
        issues[0].url.as_deref(),
        Some("https://linear.app/symphony/issue/SYM-101")
    );
    assert_eq!(
        issues[0].labels,
        vec!["bug".to_owned(), "backend".to_owned()]
    );
    assert_eq!(issues[0].blocked_by.len(), 1);
    assert_eq!(issues[0].blocked_by[0].id.as_deref(), Some("lin_999"));
    assert_eq!(
        issues[0].blocked_by[0].identifier.as_deref(),
        Some("SYM-999")
    );
    assert_eq!(
        issues[0].blocked_by[0].state.as_deref(),
        Some("In Progress")
    );
    assert_eq!(issues[0].created_at, Some(10));
    assert_eq!(issues[0].updated_at, Some(20));

    let requests = server
        .received_requests()
        .await
        .expect("wiremock should capture requests");
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("request should be valid json");
    assert_eq!(
        body["variables"]["states"],
        json!(["Todo", "In Progress"]),
        "graphql request should carry state filter variables"
    );
    assert_eq!(body["variables"]["projectSlug"], json!(PROJECT_SLUG));
    assert_eq!(body["variables"]["first"], json!(50));
    assert_eq!(body["variables"]["relationFirst"], json!(50));
    assert_eq!(body["variables"]["after"], serde_json::Value::Null);
}

#[tokio::test]
async fn fetch_candidates_accepts_api_keys_with_bearer_prefix_but_sends_raw_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(header("authorization", "linear-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let tracker = LinearTracker::new(
        format!("{}/graphql", server.uri()),
        "Bearer linear-api-key",
        PROJECT_SLUG,
    );
    let issues = tracker
        .fetch_candidates()
        .await
        .expect("bearer-prefixed api key should be normalized");

    assert!(issues.is_empty());
}

#[tokio::test]
async fn fetch_candidates_by_states_paginates_until_terminal_page() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "after": null
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": "lin_1",
                            "identifier": "SYM-101",
                            "title": "First page",
                            "state": { "name": "Todo" }
                        }
                    ],
                    "pageInfo": {
                        "hasNextPage": true,
                        "endCursor": "cursor-1"
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "after": "cursor-1"
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": "lin_2",
                            "identifier": "SYM-102",
                            "title": "Second page",
                            "state": { "name": "In Progress" }
                        }
                    ],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let issues = tracker
        .fetch_candidates_by_states(&[TrackerState::new("Todo"), TrackerState::new("In Progress")])
        .await
        .expect("state-filtered candidates request should paginate");

    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].id, IssueId("lin_1".to_owned()));
    assert_eq!(issues[1].id, IssueId("lin_2".to_owned()));
}

#[tokio::test]
async fn fetch_candidates_skips_null_issue_and_nested_nodes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        null,
                        {
                            "id": "lin_3",
                            "identifier": "SYM-103",
                            "title": "Null nodes are ignored",
                            "state": { "name": "Todo" },
                            "labels": {
                                "nodes": [
                                    null,
                                    { "name": "Bug" },
                                    { "name": " bug " }
                                ]
                            },
                            "inverseRelations": {
                                "nodes": [
                                    null,
                                    { "type": "blocks", "issue": null },
                                    {
                                        "type": "blocks",
                                        "issue": {
                                            "id": " blocker-1 ",
                                            "identifier": " SYM-900 ",
                                            "state": null
                                        }
                                    }
                                ]
                            }
                        }
                    ],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let issues = tracker
        .fetch_candidates()
        .await
        .expect("null connection nodes should be ignored");

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].id, IssueId("lin_3".to_owned()));
    assert_eq!(issues[0].labels, vec!["bug".to_owned()]);
    assert_eq!(issues[0].blocked_by.len(), 1);
    assert_eq!(issues[0].blocked_by[0].id.as_deref(), Some("blocker-1"));
    assert_eq!(
        issues[0].blocked_by[0].identifier.as_deref(),
        Some("SYM-900")
    );
    assert_eq!(issues[0].blocked_by[0].state, None);
}

#[tokio::test]
async fn fetch_candidates_surfaces_status_errors() {
    let server = MockServer::start().await;
    let attempts = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(SequenceResponder {
            attempts: Arc::clone(&attempts),
            responses: vec![
                ResponseTemplate::new(503).set_body_string("upstream unavailable"),
                ResponseTemplate::new(503).set_body_string("upstream unavailable"),
                ResponseTemplate::new(503).set_body_string("upstream unavailable"),
            ],
        })
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let error = tracker
        .fetch_candidates()
        .await
        .expect_err("non-success status should map to status error");

    assert_eq!(
        error,
        TrackerError::status(503, "upstream unavailable".to_owned())
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn fetch_candidates_surfaces_graphql_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "errors": [{ "message": "permission denied" }]
        })))
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let error = tracker
        .fetch_candidates()
        .await
        .expect_err("graphql errors should map to graphql taxonomy");

    assert_eq!(
        error,
        TrackerError::graphql(vec!["permission denied".to_owned()])
    );
    let requests = server
        .received_requests()
        .await
        .expect("wiremock should capture requests");
    assert_eq!(requests.len(), 1, "graphql errors should not be retried");
}

#[tokio::test]
async fn fetch_candidates_surfaces_graphql_errors_with_blank_messages() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "errors": [{ "message": "   " }]
        })))
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let error = tracker
        .fetch_candidates()
        .await
        .expect_err("blank graphql messages should still map to graphql taxonomy");

    assert_eq!(
        error,
        TrackerError::graphql(vec!["unknown graphql error".to_owned()])
    );
}

#[tokio::test]
async fn fetch_candidates_surfaces_payload_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": "",
                            "identifier": "SYM-101",
                            "title": "Broken candidate",
                            "state": { "name": "Todo" }
                        }
                    ],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let error = tracker
        .fetch_candidates()
        .await
        .expect_err("invalid payload should map to payload taxonomy");

    assert!(
        matches!(error, TrackerError::Payload { .. }),
        "expected payload taxonomy variant"
    );
}

#[tokio::test]
async fn fetch_candidates_surfaces_payload_errors_for_missing_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [],
                    "pageInfo": {
                        "hasNextPage": true,
                        "endCursor": null
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let error = tracker
        .fetch_candidates()
        .await
        .expect_err("pagination payload missing cursor should map to payload taxonomy");

    assert!(
        matches!(error, TrackerError::Payload { .. }),
        "expected payload taxonomy variant"
    );
}

#[tokio::test]
async fn fetch_candidates_surfaces_payload_errors_for_repeated_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "after": null
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": "lin_cursor_1",
                            "identifier": "SYM-CURSOR-1",
                            "title": "First page",
                            "state": { "name": "Todo" }
                        }
                    ],
                    "pageInfo": {
                        "hasNextPage": true,
                        "endCursor": "cursor-1"
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "after": "cursor-1"
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": "lin_cursor_2",
                            "identifier": "SYM-CURSOR-2",
                            "title": "Repeated cursor",
                            "state": { "name": "In Progress" }
                        }
                    ],
                    "pageInfo": {
                        "hasNextPage": true,
                        "endCursor": "cursor-1"
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let error = tracker
        .fetch_candidates()
        .await
        .expect_err("repeated cursor should map to payload taxonomy");

    assert!(
        matches!(error, TrackerError::Payload { .. }),
        "expected payload taxonomy variant"
    );
}

#[tokio::test]
async fn fetch_candidates_surfaces_transport_errors() {
    let tracker = LinearTracker::new("http://127.0.0.1:9/graphql", API_KEY, PROJECT_SLUG);
    let error = tracker
        .fetch_candidates()
        .await
        .expect_err("unreachable endpoint should map to transport error");
    assert!(
        matches!(error, TrackerError::Transport { .. }),
        "expected transport taxonomy variant"
    );
}

#[tokio::test]
async fn fetch_candidates_retries_transient_status_errors_and_succeeds() {
    let server = MockServer::start().await;
    let attempts = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(SequenceResponder {
            attempts: Arc::clone(&attempts),
            responses: vec![
                ResponseTemplate::new(503).set_body_string("upstream unavailable"),
                ResponseTemplate::new(503).set_body_string("upstream unavailable"),
                ResponseTemplate::new(200).set_body_json(json!({
                    "data": {
                        "issues": {
                            "nodes": [
                                {
                                    "id": "lin_retry_1",
                                    "identifier": "SYM-RETRY-1",
                                    "title": "Recovered issue",
                                    "state": { "name": "Todo" }
                                }
                            ],
                            "pageInfo": {
                                "hasNextPage": false,
                                "endCursor": null
                            }
                        }
                    }
                })),
            ],
        })
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let issues = tracker
        .fetch_candidates()
        .await
        .expect("transient status failures should eventually recover");

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].id, IssueId("lin_retry_1".to_owned()));
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn fetch_candidates_retries_timeouts_and_succeeds() {
    let server = MockServer::start().await;
    let attempts = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(SequenceResponder {
            attempts: Arc::clone(&attempts),
            responses: vec![
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(100))
                    .set_body_json(json!({
                        "data": {
                            "issues": {
                                "nodes": [
                                    {
                                        "id": "lin_timeout_1",
                                        "identifier": "SYM-TIMEOUT-1",
                                        "title": "Recovered after timeout",
                                        "state": { "name": "Todo" }
                                    }
                                ],
                                "pageInfo": {
                                    "hasNextPage": false,
                                    "endCursor": null
                                }
                            }
                        }
                    })),
                ResponseTemplate::new(200).set_body_json(json!({
                    "data": {
                        "issues": {
                            "nodes": [
                                {
                                    "id": "lin_timeout_1",
                                    "identifier": "SYM-TIMEOUT-1",
                                    "title": "Recovered after timeout",
                                    "state": { "name": "Todo" }
                                }
                            ],
                            "pageInfo": {
                                "hasNextPage": false,
                                "endCursor": null
                            }
                        }
                    }
                })),
            ],
        })
        .mount(&server)
        .await;

    let client = Client::builder()
        .timeout(Duration::from_millis(50))
        .build()
        .expect("client should build");
    let tracker = build_tracker_with_client(&server, client);
    let issues = tracker
        .fetch_candidates()
        .await
        .expect("transient timeout should be retried");

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].identifier, "SYM-TIMEOUT-1");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn fetch_states_by_ids_returns_state_map_across_pages() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "after": null
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        { "id": "lin_7", "state": { "name": "Done" } },
                        { "id": "lin_ignored", "state": { "name": "Backlog" } }
                    ],
                    "pageInfo": {
                        "hasNextPage": true,
                        "endCursor": "cursor-1"
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "after": "cursor-1"
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        { "id": "lin_8", "state": { "name": "Backlog" } }
                    ],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let states = tracker
        .fetch_states_by_ids(&[IssueId("lin_8".to_owned()), IssueId("lin_7".to_owned())])
        .await
        .expect("state map query should succeed");

    assert_eq!(states.len(), 2);
    assert_eq!(
        states.get(&IssueId("lin_7".to_owned())),
        Some(&TrackerState::new("Done"))
    );
    assert_eq!(
        states.get(&IssueId("lin_8".to_owned())),
        Some(&TrackerState::new("Backlog"))
    );

    let requests = server
        .received_requests()
        .await
        .expect("wiremock should capture requests");
    assert_eq!(requests.len(), 2);
    let body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("request should be valid json");
    assert_eq!(body["variables"]["ids"], json!(["lin_8", "lin_7"]));
    assert_eq!(body["variables"]["first"], json!(50));
}

#[tokio::test]
async fn fetch_states_by_ids_surfaces_payload_errors_for_duplicate_ids() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        { "id": "lin_7", "state": { "name": "Done" } },
                        { "id": "lin_7", "state": { "name": "Backlog" } }
                    ],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let error = tracker
        .fetch_states_by_ids(&[IssueId("lin_7".to_owned())])
        .await
        .expect_err("duplicate ids in payload should map to payload taxonomy");
    assert!(
        matches!(error, TrackerError::Payload { .. }),
        "expected payload taxonomy variant"
    );
}

#[tokio::test]
async fn fetch_states_by_ids_skips_null_nodes_and_missing_states() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        null,
                        { "id": "lin_7", "state": null },
                        { "id": "lin_8", "state": { "name": "Done" } },
                        { "id": "lin_ignored", "state": { "name": "Backlog" } }
                    ],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let states = tracker
        .fetch_states_by_ids(&[IssueId("lin_7".to_owned()), IssueId("lin_8".to_owned())])
        .await
        .expect("missing nodes and missing state payloads should be ignored");

    assert_eq!(states.len(), 1);
    assert_eq!(
        states.get(&IssueId("lin_8".to_owned())),
        Some(&TrackerState::new("Done"))
    );
    assert!(!states.contains_key(&IssueId("lin_7".to_owned())));
}

#[tokio::test]
#[ignore = "requires live Linear credentials and network access"]
async fn live_linear_smoke_test_reports_explicit_skip_when_dependencies_are_unavailable() {
    let Some(api_key) = live_linear_api_key() else {
        skip_live_linear_test(format!(
            "missing LINEAR_API_KEY in env or {}",
            REAL_LINEAR_ENV_FILE
        ));
        return;
    };
    let Some(project_slug) = live_linear_project_slug() else {
        skip_live_linear_test(format!(
            "missing {} and no workflow project_slug fallback",
            REAL_LINEAR_PROJECT_SLUG_ENV
        ));
        return;
    };

    let tracker = LinearTracker::new(REAL_LINEAR_ENDPOINT, api_key, project_slug);
    let issues = match tracker.fetch_candidates().await {
        Ok(issues) => issues,
        Err(error) if should_skip_live_linear_error(&error) => {
            skip_live_linear_test(error.to_string());
            return;
        }
        Err(error) => panic!("live Linear candidate fetch failed: {error}"),
    };

    if let Some(issue) = issues.first() {
        match tracker
            .fetch_states_by_ids(std::slice::from_ref(&issue.id))
            .await
        {
            Ok(states) => assert_eq!(states.get(&issue.id), Some(&issue.state)),
            Err(error) if should_skip_live_linear_error(&error) => {
                skip_live_linear_test(error.to_string());
            }
            Err(error) => panic!("live Linear state refresh failed: {error}"),
        }
    } else {
        eprintln!(
            "live Linear smoke test fetched zero visible issues; candidate fetch coverage only"
        );
    }
}

// ============================================================================
// C3.1.3: Pagination Edge Case Tests
// ============================================================================

/// C3.1.3: Pagination handles empty page gracefully (hasNextPage but no nodes)
#[tokio::test]
async fn fetch_candidates_handles_empty_page_with_has_next_page() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "after": null
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [],
                    "pageInfo": {
                        "hasNextPage": true,
                        "endCursor": "cursor-empty"
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "after": "cursor-empty"
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": "lin_1",
                            "identifier": "SYM-200",
                            "title": "After empty page",
                            "state": { "name": "Todo" }
                        }
                    ],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let issues = tracker
        .fetch_candidates_by_states(&[TrackerState::new("Todo")])
        .await
        .expect("should handle empty page gracefully");

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].identifier, "SYM-200");
}

/// C3.1.3: Pagination handles error mid-way through pages
#[tokio::test]
async fn fetch_candidates_surfaces_error_mid_pagination() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "after": null
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": "lin_1",
                            "identifier": "SYM-301",
                            "title": "First page",
                            "state": { "name": "Todo" }
                        }
                    ],
                    "pageInfo": {
                        "hasNextPage": true,
                        "endCursor": "cursor-1"
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(json!({
            "variables": {
                "after": "cursor-1"
            }
        })))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "errors": [{ "message": "Internal server error during pagination" }]
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let result = tracker
        .fetch_candidates_by_states(&[TrackerState::new("Todo")])
        .await;

    assert!(result.is_err(), "should surface mid-pagination error");
}

/// C3.1.3: Pagination handles missing pageInfo as payload error
#[tokio::test]
async fn fetch_candidates_rejects_missing_page_info() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": "lin_1",
                            "identifier": "SYM-400",
                            "title": "No page info",
                            "state": { "name": "Todo" }
                        }
                    ]
                    // pageInfo is missing entirely - should error
                }
            }
        })))
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let result = tracker
        .fetch_candidates_by_states(&[TrackerState::new("Todo")])
        .await;

    // Missing pageInfo should be a payload error
    assert!(result.is_err(), "missing pageInfo should cause error");
    let err = result.unwrap_err();
    assert!(matches!(err, TrackerError::Payload { .. }));
}

/// C3.1.3: Pagination handles single large page (no pagination needed)
#[tokio::test]
async fn fetch_candidates_single_page_without_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": "lin_1",
                            "identifier": "SYM-500",
                            "title": "Only issue",
                            "state": { "name": "Todo" }
                        }
                    ],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let issues = tracker
        .fetch_candidates_by_states(&[TrackerState::new("Todo")])
        .await
        .expect("should handle single page");

    assert_eq!(issues.len(), 1);
}

/// C3.1.3: Pagination rejects partial node data (missing required fields)
#[tokio::test]
async fn fetch_candidates_rejects_partial_node_data() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": "lin_1",
                            "identifier": "SYM-600"
                            // title is missing - should error
                        }
                    ],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let tracker = build_tracker(&server);
    let result = tracker
        .fetch_candidates_by_states(&[TrackerState::new("Todo")])
        .await;

    // Missing required fields should be a payload error
    assert!(result.is_err(), "missing title should cause error");
    let err = result.unwrap_err();
    assert!(matches!(err, TrackerError::Payload { .. }));
}
