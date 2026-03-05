use crate::{AppServerEvent, StderrLine};

#[derive(Clone, Debug, PartialEq)]
pub enum ParsedLine {
    StdoutEvent(AppServerEvent),
    StderrLine(StderrLine),
}

impl ParsedLine {
    pub fn stdout_event(&self) -> Option<&AppServerEvent> {
        match self {
            Self::StdoutEvent(event) => Some(event),
            Self::StderrLine(_) => None,
        }
    }

    pub fn stderr_message(&self) -> Option<&str> {
        match self {
            Self::StdoutEvent(_) => None,
            Self::StderrLine(stderr) => Some(stderr.message.as_str()),
        }
    }
}

pub fn stdout_event(line: &ParsedLine) -> Option<&AppServerEvent> {
    line.stdout_event()
}

pub fn stderr_message(line: &ParsedLine) -> Option<&str> {
    line.stderr_message()
}
