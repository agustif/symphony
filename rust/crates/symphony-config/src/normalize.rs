use std::collections::HashSet;

pub fn normalize_state_name(value: &str) -> Option<String> {
    let normalized = value
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub fn normalize_state_list(states: Vec<String>) -> Vec<String> {
    let mut normalized_states = Vec::new();
    let mut seen = HashSet::new();

    for state in states {
        let Some(normalized_state) = normalize_state_name(&state) else {
            continue;
        };

        if seen.insert(normalized_state.clone()) {
            normalized_states.push(normalized_state);
        }
    }

    normalized_states
}

#[cfg(test)]
mod tests {
    use super::{normalize_state_list, normalize_state_name};

    #[test]
    fn normalizes_state_names_consistently() {
        assert_eq!(
            normalize_state_name("  In   Progress "),
            Some("in progress".to_owned())
        );
        assert_eq!(normalize_state_name(""), None);
    }

    #[test]
    fn normalizes_and_deduplicates_state_lists() {
        let states = vec![
            " In Progress ".to_owned(),
            "in    progress".to_owned(),
            "Blocked".to_owned(),
        ];

        assert_eq!(
            normalize_state_list(states),
            vec!["in progress".to_owned(), "blocked".to_owned()]
        );
    }
}
