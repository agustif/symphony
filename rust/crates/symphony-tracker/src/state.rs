use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TrackerState(pub String);

impl TrackerState {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn normalized(&self) -> String {
        self.as_str().trim().to_owned()
    }

    pub fn normalized_key(&self) -> Option<String> {
        normalize_state_key(self.as_str())
    }

    pub fn is_terminal(&self, terminal_states: &[TrackerState]) -> bool {
        let Some(state_key) = self.normalized_key() else {
            return false;
        };
        terminal_states
            .iter()
            .filter_map(TrackerState::normalized_key)
            .any(|terminal_key| terminal_key == state_key)
    }
}

fn normalize_state_key(value: &str) -> Option<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return None;
    }
    Some(normalized.to_ascii_lowercase())
}

impl fmt::Display for TrackerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for TrackerState {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for TrackerState {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}
