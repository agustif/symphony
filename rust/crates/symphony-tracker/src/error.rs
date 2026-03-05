use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TrackerError {
    #[error("tracker transport error: {message}")]
    Transport { message: String },

    #[error("tracker status error ({status}): {body}")]
    Status { status: u16, body: String },

    #[error("tracker graphql error(s): {messages:?}")]
    GraphQl { messages: Vec<String> },

    #[error("tracker payload error: {message}")]
    Payload { message: String },
}

impl TrackerError {
    pub fn transport(message: impl Into<String>) -> Self {
        Self::Transport {
            message: message.into(),
        }
    }

    pub fn status(status: u16, body: impl Into<String>) -> Self {
        Self::Status {
            status,
            body: body.into(),
        }
    }

    pub fn graphql(messages: Vec<String>) -> Self {
        Self::GraphQl { messages }
    }

    pub fn payload(message: impl Into<String>) -> Self {
        Self::Payload {
            message: message.into(),
        }
    }
}
