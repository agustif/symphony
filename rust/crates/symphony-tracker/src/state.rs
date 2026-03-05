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
