use serde_json::Value;

use crate::{AppServerEvent, ProtocolMethodKind, canonical_method_name};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolFailureReason {
    TurnFailed,
    TurnCancelled,
    TurnTimeout,
    TurnInputRequired,
    ApprovalRequired,
    CodexNotFound,
    InvalidWorkspaceCwd,
    ResponseTimeout,
    ResponseError,
    PortExit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolPolicyOutcome {
    RetryableFailure(ProtocolFailureReason),
    PermanentFailure(ProtocolFailureReason),
}

pub fn classify_policy_outcome(event: &AppServerEvent) -> Option<ProtocolPolicyOutcome> {
    if event_signals_input_required(event) {
        return Some(ProtocolPolicyOutcome::PermanentFailure(
            ProtocolFailureReason::TurnInputRequired,
        ));
    }

    if event_signals_approval_required(event) {
        return Some(ProtocolPolicyOutcome::PermanentFailure(
            ProtocolFailureReason::ApprovalRequired,
        ));
    }

    if event_signals_error_code(event, &["codex_not_found"]) {
        return Some(ProtocolPolicyOutcome::PermanentFailure(
            ProtocolFailureReason::CodexNotFound,
        ));
    }

    if event_signals_error_code(event, &["invalid_workspace_cwd"]) {
        return Some(ProtocolPolicyOutcome::PermanentFailure(
            ProtocolFailureReason::InvalidWorkspaceCwd,
        ));
    }

    if event_signals_error_code(event, &["port_exit"]) {
        return Some(ProtocolPolicyOutcome::RetryableFailure(
            ProtocolFailureReason::PortExit,
        ));
    }

    if event_signals_error_code(
        event,
        &[
            "response_timeout",
            "read_timeout",
            "startup_timeout",
            "handshake_timeout",
            "startup/read_timeout",
            "startup/response_timeout",
            "response_timed_out",
        ],
    ) {
        return Some(ProtocolPolicyOutcome::RetryableFailure(
            ProtocolFailureReason::ResponseTimeout,
        ));
    }

    if event_signals_error_code(event, &["response_error"]) {
        return Some(ProtocolPolicyOutcome::RetryableFailure(
            ProtocolFailureReason::ResponseError,
        ));
    }

    match event.method_kind() {
        ProtocolMethodKind::TurnCancelled => Some(ProtocolPolicyOutcome::RetryableFailure(
            ProtocolFailureReason::TurnCancelled,
        )),
        ProtocolMethodKind::TurnFailed => {
            let reason = if event_signals_turn_timeout(event) {
                ProtocolFailureReason::TurnTimeout
            } else {
                ProtocolFailureReason::TurnFailed
            };
            Some(ProtocolPolicyOutcome::RetryableFailure(reason))
        }
        _ => None,
    }
}

fn event_signals_input_required(event: &AppServerEvent) -> bool {
    matches!(event.method_kind(), ProtocolMethodKind::InputRequired)
        || event_payloads(event).iter().flatten().any(|payload| {
            payload_has_true_flag(payload, &["input_required", "requires_input"])
                || payload_has_string_marker(
                    payload,
                    &["type", "event", "method", "code", "reason", "name"],
                    &[
                        "input_required",
                        "turn_input_required",
                        "turn/input_required",
                        "item/tool/requestUserInput",
                    ],
                )
        })
}

fn event_signals_approval_required(event: &AppServerEvent) -> bool {
    matches!(event.method_kind(), ProtocolMethodKind::ApprovalRequested)
        || event_payloads(event).iter().flatten().any(|payload| {
            payload_has_true_flag(payload, &["approval_required", "requires_approval"])
                || payload_has_string_marker(
                    payload,
                    &["type", "event", "method", "code", "reason", "name"],
                    &[
                        "approval_required",
                        "approval/requested",
                        "approval/required",
                    ],
                )
        })
}

fn event_signals_turn_timeout(event: &AppServerEvent) -> bool {
    event_payloads(event).iter().flatten().any(|payload| {
        payload_has_string_marker(
            payload,
            &["type", "event", "method", "code", "reason", "name"],
            &[
                "turn_timeout",
                "turn/timeout",
                "response_timeout",
                "read_timeout",
                "timeout",
            ],
        )
    })
}

fn event_signals_error_code(event: &AppServerEvent, markers: &[&str]) -> bool {
    event_payloads(event).iter().flatten().any(|payload| {
        payload_has_string_marker(
            payload,
            &["type", "event", "method", "code", "reason", "name"],
            markers,
        )
    })
}

fn event_payloads(event: &AppServerEvent) -> [Option<&Value>; 3] {
    [
        Some(&event.params),
        event.result.as_ref(),
        event.error.as_ref(),
    ]
}

fn payload_has_true_flag(value: &Value, keys: &[&str]) -> bool {
    let normalized_keys = normalize_tokens(keys);
    payload_has_true_flag_with_keys(value, &normalized_keys)
}

fn payload_has_true_flag_with_keys(value: &Value, keys: &[String]) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, nested)| {
            let normalized_key = normalize_token(key);
            if keys.contains(&normalized_key) && nested.as_bool() == Some(true) {
                return true;
            }
            payload_has_true_flag_with_keys(nested, keys)
        }),
        Value::Array(values) => values
            .iter()
            .any(|nested| payload_has_true_flag_with_keys(nested, keys)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

fn payload_has_string_marker(value: &Value, keys: &[&str], markers: &[&str]) -> bool {
    let normalized_keys = normalize_tokens(keys);
    let normalized_markers = normalize_tokens(markers);
    payload_has_string_marker_with_sets(value, &normalized_keys, &normalized_markers)
}

fn payload_has_string_marker_with_sets(value: &Value, keys: &[String], markers: &[String]) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, nested)| {
            let normalized_key = normalize_token(key);
            let direct_match = keys.contains(&normalized_key)
                && nested
                    .as_str()
                    .is_some_and(|raw| markers.contains(&normalize_token(raw)));
            if direct_match {
                return true;
            }
            payload_has_string_marker_with_sets(nested, keys, markers)
        }),
        Value::Array(values) => values
            .iter()
            .any(|nested| payload_has_string_marker_with_sets(nested, keys, markers)),
        Value::String(raw) => markers.contains(&normalize_token(raw)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn normalize_tokens(values: &[&str]) -> Vec<String> {
    values.iter().copied().map(normalize_token).collect()
}

fn normalize_token(value: &str) -> String {
    canonical_method_name(value).replace(['_', '-'], "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        method: &str,
        params: serde_json::Value,
        result: Option<serde_json::Value>,
        error: Option<serde_json::Value>,
    ) -> AppServerEvent {
        AppServerEvent {
            id: None,
            method: method.to_owned(),
            params,
            result,
            error,
        }
    }

    #[test]
    fn classifies_method_alias_input_required_as_permanent_failure() {
        let outcome = classify_policy_outcome(&event(
            "item/tool/requestUserInput",
            serde_json::json!({}),
            None,
            None,
        ));
        assert_eq!(
            outcome,
            Some(ProtocolPolicyOutcome::PermanentFailure(
                ProtocolFailureReason::TurnInputRequired
            ))
        );
    }

    #[test]
    fn classifies_nested_approval_required_flag_as_permanent_failure() {
        let outcome = classify_policy_outcome(&event(
            "notification",
            serde_json::json!({"event": {"approval_required": true}}),
            None,
            None,
        ));
        assert_eq!(
            outcome,
            Some(ProtocolPolicyOutcome::PermanentFailure(
                ProtocolFailureReason::ApprovalRequired
            ))
        );
    }

    #[test]
    fn classifies_turn_failed_timeout_as_retryable_timeout() {
        let outcome = classify_policy_outcome(&event(
            "turn/failed",
            serde_json::json!({"error": {"code": "turn_timeout"}}),
            None,
            None,
        ));
        assert_eq!(
            outcome,
            Some(ProtocolPolicyOutcome::RetryableFailure(
                ProtocolFailureReason::TurnTimeout
            ))
        );
    }

    #[test]
    fn classifies_turn_failed_without_timeout_as_retryable_failure() {
        let outcome = classify_policy_outcome(&event(
            "turn/failed",
            serde_json::json!({"error": {"code": "turn_failed"}}),
            None,
            None,
        ));
        assert_eq!(
            outcome,
            Some(ProtocolPolicyOutcome::RetryableFailure(
                ProtocolFailureReason::TurnFailed
            ))
        );
    }

    #[test]
    fn classifies_turn_cancelled_as_retryable_failure() {
        let outcome =
            classify_policy_outcome(&event("turn/cancelled", serde_json::json!({}), None, None));
        assert_eq!(
            outcome,
            Some(ProtocolPolicyOutcome::RetryableFailure(
                ProtocolFailureReason::TurnCancelled
            ))
        );
    }

    #[test]
    fn ignores_non_terminal_non_policy_events() {
        let outcome = classify_policy_outcome(&event(
            "turn/delta",
            serde_json::json!({"delta": "hello"}),
            None,
            None,
        ));
        assert_eq!(outcome, None);
    }

    #[test]
    fn classifies_codex_not_found_as_permanent_failure() {
        let outcome = classify_policy_outcome(&event(
            "turn/failed",
            serde_json::json!({"error": {"code": "codex_not_found"}}),
            None,
            None,
        ));
        assert_eq!(
            outcome,
            Some(ProtocolPolicyOutcome::PermanentFailure(
                ProtocolFailureReason::CodexNotFound
            ))
        );
    }

    #[test]
    fn classifies_invalid_workspace_cwd_as_permanent_failure() {
        let outcome = classify_policy_outcome(&event(
            "startup/failed",
            serde_json::json!({"code": "invalid_workspace_cwd"}),
            None,
            None,
        ));
        assert_eq!(
            outcome,
            Some(ProtocolPolicyOutcome::PermanentFailure(
                ProtocolFailureReason::InvalidWorkspaceCwd
            ))
        );
    }

    #[test]
    fn classifies_response_timeout_as_retryable_failure() {
        let outcome = classify_policy_outcome(&event(
            "startup/failed",
            serde_json::json!({"error": {"code": "response_timeout"}}),
            None,
            None,
        ));
        assert_eq!(
            outcome,
            Some(ProtocolPolicyOutcome::RetryableFailure(
                ProtocolFailureReason::ResponseTimeout
            ))
        );
    }

    #[test]
    fn classifies_startup_timeout_aliases_as_retryable_failure() {
        let outcome = classify_policy_outcome(&event(
            "startup/failed",
            serde_json::json!({"details": {"reason": "handshake_timeout"}}),
            None,
            None,
        ));
        assert_eq!(
            outcome,
            Some(ProtocolPolicyOutcome::RetryableFailure(
                ProtocolFailureReason::ResponseTimeout
            ))
        );
    }

    #[test]
    fn classifies_response_error_as_retryable_failure() {
        let outcome = classify_policy_outcome(&event(
            "startup/failed",
            serde_json::json!({"error": {"code": "response_error"}}),
            None,
            None,
        ));
        assert_eq!(
            outcome,
            Some(ProtocolPolicyOutcome::RetryableFailure(
                ProtocolFailureReason::ResponseError
            ))
        );
    }

    #[test]
    fn classifies_port_exit_as_retryable_failure() {
        let outcome = classify_policy_outcome(&event(
            "startup/failed",
            serde_json::json!({"error": {"reason": "port_exit"}}),
            None,
            None,
        ));
        assert_eq!(
            outcome,
            Some(ProtocolPolicyOutcome::RetryableFailure(
                ProtocolFailureReason::PortExit
            ))
        );
    }

    #[test]
    fn does_not_hard_fail_on_unsupported_tool_call_marker_event() {
        let outcome = classify_policy_outcome(&event(
            "unsupported_tool_call",
            serde_json::json!({}),
            None,
            None,
        ));
        assert_eq!(outcome, None);
    }
}
