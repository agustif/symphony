#![forbid(unsafe_code)]

use serde_json::json;
use symphony_domain::IssueId;
use symphony_tracker::{TrackerClient, TrackerError, TrackerState};
use symphony_tracker_linear::LinearTracker;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, header, method, path},
};

const API_KEY: &str = "linear-api-key";

fn build_tracker(server: &MockServer) -> LinearTracker {
    LinearTracker::new(format!("{}/graphql", server.uri()), API_KEY)
}

#[tokio::test]
async fn fetch_candidates_by_states_normalizes_payload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(header("authorization", "Bearer linear-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "issues": {
                    "nodes": [
                        {
                            "id": " lin_1 ",
                            "identifier": " SYM-101 ",
                            "state": { "name": " Todo " }
                        },
                        {
                            "id": "lin_2",
                            "identifier": "SYM-102",
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
    assert_eq!(issues[0].state, TrackerState::new("Todo"));

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
    assert_eq!(body["variables"]["first"], json!(100));
    assert_eq!(body["variables"]["after"], serde_json::Value::Null);
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
async fn fetch_candidates_surfaces_status_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream unavailable"))
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
async fn fetch_candidates_surfaces_transport_errors() {
    let tracker = LinearTracker::new("http://127.0.0.1:9/graphql", API_KEY);
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
    assert_eq!(body["variables"]["first"], json!(100));
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
