use std::{iter::Peekable, str::Chars};

use serde_json::Value;

const REDACTED: &str = "[REDACTED]";
const SENSITIVE_KEYS: &[&str] = &[
    "api_key",
    "apikey",
    "authorization",
    "password",
    "secret",
    "token",
    "access_token",
    "refresh_token",
    "cookie",
    "set-cookie",
    "x-api-key",
    "client_secret",
];

/// Removes control bytes and ANSI escape sequences from observability text payloads.
#[must_use]
pub fn strip_control_bytes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{001b}' {
            skip_escape_sequence(&mut chars);
            continue;
        }

        if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
            continue;
        }

        output.push(ch);
    }

    output
}

#[must_use]
pub fn sanitize_event_text(input: &str) -> String {
    redact_secret_text(&strip_control_bytes(input))
}

#[must_use]
pub fn sanitize_message_text(input: &str) -> String {
    redact_secret_text(&strip_control_bytes(input))
}

#[must_use]
pub fn redact_secret_text(input: &str) -> String {
    let redacted_urls = redact_url_userinfo(input);
    let redacted_assignments = redact_sensitive_assignments(&redacted_urls);
    redact_bearer_tokens(&redacted_assignments)
}

#[must_use]
pub fn sanitize_json_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if is_sensitive_key(key) {
                        (key.clone(), Value::String(REDACTED.to_owned()))
                    } else {
                        (key.clone(), sanitize_json_value(value))
                    }
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(sanitize_json_value).collect()),
        Value::String(text) => Value::String(sanitize_message_text(text)),
        _ => value.clone(),
    }
}

#[must_use]
pub fn format_json_compact_sorted(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            let mut formatted = String::from("{");
            for (index, (key, value)) in entries.iter().enumerate() {
                if index > 0 {
                    formatted.push(',');
                }
                formatted.push_str(
                    &serde_json::to_string(key).unwrap_or_else(|_| "\"<invalid-key>\"".to_owned()),
                );
                formatted.push(':');
                formatted.push_str(&format_json_compact_sorted(value));
            }
            formatted.push('}');
            formatted
        }
        Value::Array(values) => {
            let mut formatted = String::from("[");
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    formatted.push(',');
                }
                formatted.push_str(&format_json_compact_sorted(value));
            }
            formatted.push(']');
            formatted
        }
        _ => serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned()),
    }
}

fn skip_escape_sequence(chars: &mut Peekable<Chars<'_>>) {
    match chars.peek().copied() {
        Some('[') => {
            chars.next();
            skip_csi_sequence(chars);
        }
        Some(']') => {
            chars.next();
            skip_osc_sequence(chars);
        }
        Some(_) => {
            chars.next();
        }
        None => {}
    }
}

fn skip_csi_sequence(chars: &mut Peekable<Chars<'_>>) {
    for ch in chars.by_ref() {
        let codepoint = ch as u32;
        if (0x40..=0x7E).contains(&codepoint) {
            break;
        }
    }
}

fn skip_osc_sequence(chars: &mut Peekable<Chars<'_>>) {
    let mut previous_was_escape = false;

    for ch in chars.by_ref() {
        if ch == '\u{0007}' {
            break;
        }

        if previous_was_escape && ch == '\\' {
            break;
        }

        previous_was_escape = ch == '\u{001b}';
    }
}

fn redact_bearer_tokens(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    let lowercase = input.to_ascii_lowercase();

    while let Some(offset) = lowercase[cursor..].find("bearer ") {
        let start = cursor + offset;
        let token_start = start + "bearer ".len();
        output.push_str(&input[cursor..token_start]);

        let token_end = find_unquoted_value_end(input, token_start);
        if token_end > token_start {
            output.push_str(REDACTED);
        }
        cursor = token_end;
    }

    output.push_str(&input[cursor..]);
    output
}

fn redact_sensitive_assignments(input: &str) -> String {
    let mut redacted = input.to_owned();
    for key in SENSITIVE_KEYS {
        redacted = redact_assignment_for_separator(&redacted, key, ':');
        redacted = redact_assignment_for_separator(&redacted, key, '=');
    }
    redacted
}

fn redact_assignment_for_separator(input: &str, key: &str, separator: char) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    let key_lower = key.to_ascii_lowercase();
    let lowercase = input.to_ascii_lowercase();
    let bytes = input.as_bytes();

    while let Some(offset) = lowercase[cursor..].find(&key_lower) {
        let start = cursor + offset;
        let end_key = start + key_lower.len();

        if !is_word_boundary(bytes, start, end_key) {
            output.push_str(&input[cursor..end_key]);
            cursor = end_key;
            continue;
        }

        let Some((value_start, value_end)) = assignment_value_range(input, end_key, separator)
        else {
            output.push_str(&input[cursor..end_key]);
            cursor = end_key;
            continue;
        };
        let value_end = if is_authorization_key(key)
            && input[value_start..]
                .get(..7)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Bearer "))
        {
            find_unquoted_value_end(input, value_start + "Bearer ".len())
        } else {
            value_end
        };

        output.push_str(&input[cursor..value_start]);
        output.push_str(REDACTED);
        cursor = value_end;
    }

    output.push_str(&input[cursor..]);
    output
}

fn assignment_value_range(
    input: &str,
    mut index: usize,
    separator: char,
) -> Option<(usize, usize)> {
    let bytes = input.as_bytes();

    if matches!(bytes.get(index).copied(), Some(b'"' | b'\'')) {
        index += 1;
    }

    while matches!(bytes.get(index).copied(), Some(b' ' | b'\t')) {
        index += 1;
    }

    if bytes.get(index).copied()? != separator as u8 {
        return None;
    }
    index += 1;

    while matches!(bytes.get(index).copied(), Some(b' ' | b'\t')) {
        index += 1;
    }

    match bytes.get(index).copied() {
        Some(b'"' | b'\'') => {
            let quote = bytes[index];
            let value_start = index + 1;
            let mut value_end = value_start;
            while let Some(byte) = bytes.get(value_end) {
                if *byte == quote {
                    return Some((value_start, value_end));
                }
                value_end += 1;
            }
            Some((value_start, value_end))
        }
        Some(_) => {
            let value_start = index;
            let value_end = find_unquoted_value_end(input, value_start);
            Some((value_start, value_end))
        }
        None => None,
    }
}

fn find_unquoted_value_end(input: &str, mut index: usize) -> usize {
    let bytes = input.as_bytes();
    while let Some(byte) = bytes.get(index) {
        if matches!(
            *byte,
            b' ' | b'\t' | b'\r' | b'\n' | b',' | b';' | b'&' | b'}' | b']' | b')' | b'"' | b'\''
        ) {
            break;
        }
        index += 1;
    }
    index
}

fn is_word_boundary(bytes: &[u8], start: usize, end: usize) -> bool {
    let preceding = start
        .checked_sub(1)
        .and_then(|index| bytes.get(index).copied());
    let following = bytes.get(end).copied();

    !matches!(preceding, Some(byte) if is_identifier_byte(byte))
        && !matches!(following, Some(byte) if is_identifier_byte(byte))
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn redact_url_userinfo(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(offset) = input[cursor..].find("://") {
        let scheme_sep = cursor + offset;
        let authority_start = scheme_sep + 3;
        let authority_end = input[authority_start..]
            .find(['/', '?', '#', ' ', '\t', '\r', '\n'])
            .map_or(input.len(), |offset| authority_start + offset);

        output.push_str(&input[cursor..authority_start]);
        let authority = &input[authority_start..authority_end];
        if let Some(at_index) = authority.find('@') {
            let userinfo = &authority[..at_index];
            if userinfo.contains(':') {
                output.push_str(REDACTED);
                output.push('@');
                output.push_str(&authority[at_index + 1..]);
            } else {
                output.push_str(authority);
            }
        } else {
            output.push_str(authority);
        }
        cursor = authority_end;
    }

    output.push_str(&input[cursor..]);
    output
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();

    SENSITIVE_KEYS
        .iter()
        .map(|key| {
            key.chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        })
        .any(|candidate| candidate == normalized)
}

fn is_authorization_key(key: &str) -> bool {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .eq("authorization".chars())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        format_json_compact_sorted, redact_secret_text, sanitize_event_text, sanitize_json_value,
        sanitize_message_text, strip_control_bytes,
    };

    #[test]
    fn strip_control_bytes_removes_embedded_controls() {
        let input = "ok\u{0000}\u{0008}\tline\nnext\r\u{007f}";
        let sanitized = strip_control_bytes(input);
        assert_eq!(sanitized, "ok\tline\nnext\r");
    }

    #[test]
    fn strip_control_bytes_removes_ansi_sequences() {
        let input = "\u{001b}[31merror\u{001b}[0m and \u{001b}]8;;https://example.com\u{0007}link\u{001b}]8;;\u{0007}";
        let sanitized = strip_control_bytes(input);
        assert_eq!(sanitized, "error and link");
    }

    #[test]
    fn sanitize_helpers_share_control_stripping_behavior() {
        let input = "warn\u{0000}\u{001b}[32m ok";
        assert_eq!(sanitize_event_text(input), "warn ok");
        assert_eq!(sanitize_message_text(input), "warn ok");
    }

    #[test]
    fn sanitize_helpers_redact_common_secret_patterns() {
        let input = concat!(
            "authorization: Bearer super-secret-token ",
            "api_key=linear-secret ",
            "url=https://user:pass@example.com/path"
        );

        let sanitized = sanitize_message_text(input);
        assert_eq!(
            sanitized,
            "authorization: [REDACTED] api_key=[REDACTED] url=https://[REDACTED]@example.com/path"
        );
        assert_eq!(sanitize_event_text(input), sanitized);
    }

    #[test]
    fn redact_secret_text_handles_json_like_assignments() {
        let input = r#"{"token":"abc123","nested":{"client_secret":"xyz"},"safe":"ok"}"#;
        let redacted = redact_secret_text(input);

        assert_eq!(
            redacted,
            r#"{"token":"[REDACTED]","nested":{"client_secret":"[REDACTED]"},"safe":"ok"}"#
        );
    }

    #[test]
    fn strip_control_bytes_preserves_unicode_text() {
        let input = "Línea válida ✔\u{0001}";
        assert_eq!(strip_control_bytes(input), "Línea válida ✔");
    }

    #[test]
    fn sanitize_json_value_redacts_nested_secret_keys_and_strings() {
        let value = json!({
            "safe": "status ok",
            "authorization": "Bearer abc123",
            "nested": {
                "api_key": "secret-key",
                "note": "cookie=session-123"
            }
        });

        let sanitized = sanitize_json_value(&value);
        assert_eq!(
            sanitized,
            json!({
                "safe": "status ok",
                "authorization": "[REDACTED]",
                "nested": {
                    "api_key": "[REDACTED]",
                    "note": "cookie=[REDACTED]"
                }
            })
        );
    }

    #[test]
    fn format_json_compact_sorted_orders_keys_deterministically() {
        let value = json!({
            "zeta": 1,
            "alpha": {
                "beta": true,
                "aardvark": false
            }
        });

        assert_eq!(
            format_json_compact_sorted(&value),
            r#"{"alpha":{"aardvark":false,"beta":true},"zeta":1}"#
        );
    }
}
