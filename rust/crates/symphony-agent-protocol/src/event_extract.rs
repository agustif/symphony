use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AppServerEvent;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

pub type ProtocolRateLimits = Value;

pub fn extract_thread_id(event: &AppServerEvent) -> Option<String> {
    extract_first_string(event, &[&["thread", "id"], &["thread_id"], &["threadId"]])
}

pub fn extract_turn_id(event: &AppServerEvent) -> Option<String> {
    extract_first_string(event, &[&["turn", "id"], &["turn_id"], &["turnId"]])
}

pub fn build_session_id(thread_id: Option<&str>, turn_id: Option<&str>) -> Option<String> {
    match (thread_id, turn_id) {
        (Some(thread), Some(turn)) if !thread.is_empty() && !turn.is_empty() => {
            Some(format!("{thread}-{turn}"))
        }
        _ => None,
    }
}

pub fn extract_usage(event: &AppServerEvent) -> Option<ProtocolUsage> {
    let usage_candidates = [
        value_at_path(
            &event.params,
            &["msg", "payload", "info", "total_token_usage"],
        ),
        value_at_path(
            &event.params,
            &["msg", "payload", "info", "totalTokenUsage"],
        ),
        value_at_path(&event.params, &["msg", "info", "total_token_usage"]),
        value_at_path(&event.params, &["msg", "info", "totalTokenUsage"]),
        value_at_path(&event.params, &["tokenUsage", "total"]),
        value_at_path(&event.params, &["token_usage", "total"]),
        event.result.as_ref().and_then(|value| {
            value_at_path(value, &["msg", "payload", "info", "total_token_usage"])
        }),
        event
            .result
            .as_ref()
            .and_then(|value| value_at_path(value, &["msg", "payload", "info", "totalTokenUsage"])),
        event
            .result
            .as_ref()
            .and_then(|value| value_at_path(value, &["msg", "info", "total_token_usage"])),
        event
            .result
            .as_ref()
            .and_then(|value| value_at_path(value, &["msg", "info", "totalTokenUsage"])),
        event
            .result
            .as_ref()
            .and_then(|value| value_at_path(value, &["tokenUsage", "total"])),
        event
            .result
            .as_ref()
            .and_then(|value| value_at_path(value, &["token_usage", "total"])),
    ];

    for candidate in usage_candidates.into_iter().flatten() {
        if let Some(usage) = parse_usage(candidate) {
            return Some(usage);
        }
    }

    None
}

pub fn extract_rate_limits(event: &AppServerEvent) -> Option<ProtocolRateLimits> {
    let payloads = [
        Some(&event.params),
        event.result.as_ref(),
        event.error.as_ref(),
    ];

    for payload in payloads.into_iter().flatten() {
        if let Some(rate_limits) = rate_limits_from_payload(payload) {
            return Some(rate_limits);
        }
    }

    None
}

pub fn extract_tool_call_id(event: &AppServerEvent) -> Option<Value> {
    if let Some(id) = event.id.clone() {
        return Some(id);
    }

    let payloads = [
        Some(&event.params),
        event.result.as_ref(),
        event.error.as_ref(),
    ];
    let id_paths: [&[&str]; 5] = [
        &["id"],
        &["tool_call_id"],
        &["toolCallId"],
        &["call", "id"],
        &["tool_call", "id"],
    ];

    for payload in payloads.into_iter().flatten() {
        for path in id_paths {
            if let Some(id) = value_at_path(payload, path).cloned() {
                return Some(id);
            }
        }
    }

    None
}

pub fn extract_tool_name(event: &AppServerEvent) -> Option<String> {
    extract_first_string(
        event,
        &[
            &["name"],
            &["tool_name"],
            &["toolName"],
            &["tool", "name"],
            &["call", "name"],
        ],
    )
}

fn extract_first_string(event: &AppServerEvent, paths: &[&[&str]]) -> Option<String> {
    let payloads = [
        event.result.as_ref(),
        Some(&event.params),
        event.error.as_ref(),
    ];
    for payload in payloads.into_iter().flatten() {
        for path in paths {
            if let Some(value) = value_at_path(payload, path).and_then(Value::as_str)
                && !value.is_empty()
            {
                return Some(value.to_owned());
            }
        }
    }
    None
}

fn parse_usage(value: &Value) -> Option<ProtocolUsage> {
    let input_tokens = extract_u64(
        value,
        &[
            "input_tokens",
            "inputTokens",
            "prompt_tokens",
            "promptTokens",
        ],
    );
    let output_tokens = extract_u64(
        value,
        &[
            "output_tokens",
            "outputTokens",
            "completion_tokens",
            "completionTokens",
        ],
    );
    let total_tokens = extract_u64(value, &["total_tokens", "totalTokens"]);

    let has_any = input_tokens.is_some() || output_tokens.is_some() || total_tokens.is_some();
    if !has_any {
        return None;
    }

    let input = input_tokens.unwrap_or(0);
    let output = output_tokens.unwrap_or(0);
    let total = total_tokens.unwrap_or(input.saturating_add(output));

    Some(ProtocolUsage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: total,
    })
}

fn extract_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|candidate| match candidate {
            Value::Number(number) => number.as_u64(),
            Value::String(text) => text.trim().parse::<u64>().ok(),
            _ => None,
        })
    })
}

fn rate_limits_from_payload(value: &Value) -> Option<Value> {
    if let Some(direct) = value
        .get("rate_limits")
        .or_else(|| value.get("rateLimits"))
        .filter(|candidate| candidate.is_object())
    {
        return Some(direct.clone());
    }

    if rate_limits_map(value) {
        return Some(value.clone());
    }

    match value {
        Value::Object(map) => map.values().find_map(rate_limits_from_payload),
        Value::Array(values) => values.iter().find_map(rate_limits_from_payload),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
    }
}

fn rate_limits_map(value: &Value) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    map.contains_key("primary")
        || map.contains_key("secondary")
        || map.contains_key("credits")
        || map.contains_key("limit_id")
        || map.contains_key("limit_name")
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(params: Value, result: Option<Value>) -> AppServerEvent {
        AppServerEvent {
            id: None,
            method: "notification".to_owned(),
            params,
            result,
            error: None,
        }
    }

    #[test]
    fn extracts_nested_thread_and_turn_ids_from_result_payload() {
        let event = event(
            serde_json::json!({}),
            Some(serde_json::json!({
                "thread": { "id": "thread-123" },
                "turn": { "id": "turn-456" }
            })),
        );
        assert_eq!(extract_thread_id(&event), Some("thread-123".to_owned()));
        assert_eq!(extract_turn_id(&event), Some("turn-456".to_owned()));
    }

    #[test]
    fn extracts_alias_thread_and_turn_ids_from_params_payload() {
        let event = event(
            serde_json::json!({
                "threadId": "thread-camel",
                "turn_id": "turn-snake"
            }),
            None,
        );
        assert_eq!(extract_thread_id(&event), Some("thread-camel".to_owned()));
        assert_eq!(extract_turn_id(&event), Some("turn-snake".to_owned()));
    }

    #[test]
    fn builds_session_id_when_thread_and_turn_are_present() {
        assert_eq!(
            build_session_id(Some("thread-1"), Some("turn-2")),
            Some("thread-1-turn-2".to_owned())
        );
        assert_eq!(build_session_id(Some("thread-1"), None), None);
    }

    #[test]
    fn extracts_usage_from_turn_nested_usage_payload() {
        let event = event(
            serde_json::json!({}),
            Some(serde_json::json!({
                "msg": {
                    "payload": {
                        "info": {
                            "total_token_usage": {
                                "inputTokens": 120,
                                "completionTokens": 30
                            }
                        }
                    }
                }
            })),
        );
        assert_eq!(
            extract_usage(&event),
            Some(ProtocolUsage {
                input_tokens: 120,
                output_tokens: 30,
                total_tokens: 150,
            })
        );
    }

    #[test]
    fn extracts_usage_from_flat_snake_case_payload_with_explicit_total() {
        let event = event(
            serde_json::json!({
                "tokenUsage": {
                    "total": {
                        "input_tokens": "200",
                        "output_tokens": 50,
                        "total_tokens": 260
                    }
                }
            }),
            None,
        );
        assert_eq!(
            extract_usage(&event),
            Some(ProtocolUsage {
                input_tokens: 200,
                output_tokens: 50,
                total_tokens: 260,
            })
        );
    }

    #[test]
    fn usage_extraction_returns_none_without_usage_fields() {
        let event = event(serde_json::json!({"message":"no usage"}), None);
        assert_eq!(extract_usage(&event), None);
    }

    #[test]
    fn usage_extraction_ignores_generic_turn_usage_payloads() {
        let event = event(
            serde_json::json!({}),
            Some(serde_json::json!({
                "turn": {
                    "usage": {
                        "inputTokens": 120,
                        "completionTokens": 30
                    }
                }
            })),
        );
        assert_eq!(extract_usage(&event), None);
    }

    #[test]
    fn extracts_nested_rate_limits_from_payload() {
        let event = event(
            serde_json::json!({
                "msg": {
                    "payload": {
                        "rate_limits": {
                            "primary": {"remaining": 9}
                        }
                    }
                }
            }),
            None,
        );
        assert_eq!(
            extract_rate_limits(&event),
            Some(serde_json::json!({
                "primary": {"remaining": 9}
            }))
        );
    }

    #[test]
    fn extracts_tool_call_id_from_envelope_or_nested_fields() {
        let with_envelope_id = AppServerEvent {
            id: Some(serde_json::json!("call-1")),
            method: "item/tool/call".to_owned(),
            params: serde_json::json!({}),
            result: None,
            error: None,
        };
        assert_eq!(
            extract_tool_call_id(&with_envelope_id),
            Some(serde_json::json!("call-1"))
        );

        let nested = AppServerEvent {
            id: None,
            method: "item/tool/call".to_owned(),
            params: serde_json::json!({"call": {"id": 42}}),
            result: None,
            error: None,
        };
        assert_eq!(extract_tool_call_id(&nested), Some(serde_json::json!(42)));
    }

    #[test]
    fn extracts_tool_name_from_common_payload_shapes() {
        let direct = AppServerEvent {
            id: None,
            method: "item/tool/call".to_owned(),
            params: serde_json::json!({"name": "linear_graphql"}),
            result: None,
            error: None,
        };
        assert_eq!(
            extract_tool_name(&direct),
            Some("linear_graphql".to_owned())
        );

        let nested = AppServerEvent {
            id: None,
            method: "item/tool/call".to_owned(),
            params: serde_json::json!({"tool": {"name": "other_tool"}}),
            result: None,
            error: None,
        };
        assert_eq!(extract_tool_name(&nested), Some("other_tool".to_owned()));
    }
}
