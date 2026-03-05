use serde_yaml::{Mapping, Value};

use crate::{WorkflowDocument, WorkflowError};

pub fn parse(markdown: &str) -> Result<WorkflowDocument, WorkflowError> {
    let content = strip_utf8_bom(markdown);
    let (raw_front_matter, raw_body) = split_front_matter(content)?;
    let front_matter = parse_front_matter(raw_front_matter)?;

    let prompt_body = raw_body.trim().to_owned();
    if prompt_body.is_empty() {
        return Err(WorkflowError::EmptyBody);
    }

    Ok(WorkflowDocument::new(front_matter, prompt_body))
}

fn strip_utf8_bom(content: &str) -> &str {
    content.strip_prefix('\u{feff}').unwrap_or(content)
}

fn split_front_matter(content: &str) -> Result<(Option<String>, String), WorkflowError> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.first().copied() != Some("---") {
        return Ok((None, content.to_owned()));
    }

    let Some(closing_index) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (*line == "---").then_some(index))
    else {
        return Err(WorkflowError::UnterminatedFrontMatter);
    };

    let front_matter = lines[1..closing_index].join("\n");
    let prompt_body = lines[(closing_index + 1)..].join("\n");
    Ok((Some(front_matter), prompt_body))
}

fn parse_front_matter(raw_front_matter: Option<String>) -> Result<Mapping, WorkflowError> {
    let Some(raw_front_matter) = raw_front_matter else {
        return Ok(Mapping::new());
    };

    if raw_front_matter.trim().is_empty() {
        return Ok(Mapping::new());
    }

    let value: Value = serde_yaml::from_str(&raw_front_matter)
        .map_err(|error| WorkflowError::InvalidFrontMatter(error.to_string()))?;

    match value {
        Value::Mapping(mapping) => Ok(mapping),
        _ => Err(WorkflowError::FrontMatterNotMap),
    }
}

#[cfg(test)]
mod tests {
    use serde_yaml::Value;

    use super::*;

    #[test]
    fn parses_markdown_without_front_matter() {
        let workflow = parse("## Prompt\n\nImplement feature X.").expect("parse should succeed");

        assert!(workflow.front_matter.is_empty());
        assert_eq!(workflow.prompt_body, "## Prompt\n\nImplement feature X.");
    }

    #[test]
    fn parses_yaml_front_matter_and_prompt_body() {
        let workflow = parse(
            "---\ntracker:\n  kind: linear\n  project_slug: symphony\n---\n\nShip release notes.",
        )
        .expect("parse should succeed");

        let tracker = workflow
            .front_matter
            .get(Value::String("tracker".to_owned()))
            .expect("tracker key should exist");
        assert!(tracker.is_mapping());
        assert_eq!(workflow.prompt_body, "Ship release notes.");
    }

    #[test]
    fn parses_front_matter_with_windows_newlines() {
        let workflow = parse("---\r\ntracker:\r\n  kind: linear\r\n---\r\n\r\nPrompt\r\n")
            .expect("parse should succeed");

        assert_eq!(workflow.prompt_body, "Prompt");
    }

    #[test]
    fn rejects_unterminated_front_matter() {
        assert_eq!(
            parse("---\ntracker:\n  kind: linear\n"),
            Err(WorkflowError::UnterminatedFrontMatter)
        );
    }

    #[test]
    fn rejects_non_map_front_matter() {
        assert_eq!(
            parse("---\n- not\n- a\n- map\n---\nPrompt"),
            Err(WorkflowError::FrontMatterNotMap)
        );
    }

    #[test]
    fn rejects_invalid_front_matter_yaml() {
        let error = parse("---\ntracker: [\n---\nPrompt").expect_err("parse should fail");
        assert!(matches!(error, WorkflowError::InvalidFrontMatter(_)));
    }

    #[test]
    fn rejects_empty_body() {
        assert_eq!(parse("\n\n"), Err(WorkflowError::EmptyBody));
        assert_eq!(
            parse("---\ntracker: {}\n---\n\n"),
            Err(WorkflowError::EmptyBody)
        );
    }
}
