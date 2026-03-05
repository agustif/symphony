use crate::{AppServerEvent, LineOrigin, ParsedLine, ProtocolError, StderrLine};

pub fn decode_stdout_line(line: &str) -> Result<AppServerEvent, ProtocolError> {
    let normalized = normalize_line(line);
    if normalized.is_empty() {
        return Err(ProtocolError::EmptyLine);
    }

    let event = serde_json::from_str::<AppServerEvent>(normalized).map_err(|error| {
        ProtocolError::InvalidStdoutLine(format!("line `{normalized}`: {error}"))
    })?;

    if event.method.trim().is_empty() {
        return Err(ProtocolError::MissingMethod);
    }

    Ok(event)
}

pub fn decode_stderr_line(line: &str) -> StderrLine {
    StderrLine {
        message: normalize_line(line).to_owned(),
    }
}

pub fn decode_line(origin: LineOrigin, line: &str) -> Result<ParsedLine, ProtocolError> {
    match origin {
        LineOrigin::Stdout => decode_stdout_line(line).map(ParsedLine::StdoutEvent),
        LineOrigin::Stderr => Ok(ParsedLine::StderrLine(decode_stderr_line(line))),
    }
}

fn normalize_line(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
}
