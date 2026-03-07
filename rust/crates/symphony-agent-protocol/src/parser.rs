use crate::{AppServerEvent, LineOrigin, ParsedLine, ProtocolError, StderrLine};

pub fn decode_stdout_line(line: &str) -> Result<AppServerEvent, ProtocolError> {
    let normalized = normalize_line(line);
    if normalized.is_empty() {
        return Err(ProtocolError::EmptyLine);
    }

    let mut deserializer = serde_json::Deserializer::from_str(normalized);
    let event: AppServerEvent =
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            let path = error.path().to_string();
            let detail = if path.is_empty() {
                error.inner().to_string()
            } else {
                format!("{} at `{path}`", error.inner())
            };
            ProtocolError::InvalidStdoutLine(format!("line `{normalized}`: {detail}"))
        })?;

    let missing_method = event.method.trim().is_empty();
    let has_response_payload = event.result.is_some() || event.error.is_some();
    if missing_method && !has_response_payload {
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
