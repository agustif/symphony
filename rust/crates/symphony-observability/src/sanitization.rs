use std::{iter::Peekable, str::Chars};

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
    strip_control_bytes(input)
}

#[must_use]
pub fn sanitize_message_text(input: &str) -> String {
    strip_control_bytes(input)
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

#[cfg(test)]
mod tests {
    use super::{sanitize_event_text, sanitize_message_text, strip_control_bytes};

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
    fn strip_control_bytes_preserves_unicode_text() {
        let input = "Línea válida ✔\u{0001}";
        assert_eq!(strip_control_bytes(input), "Línea válida ✔");
    }
}
