#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolMethodCategory {
    Startup,
    Turn,
    Approval,
    Input,
    Tooling,
    Notification,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolMethodKind {
    Initialize,
    Initialized,
    ThreadStart,
    SessionNew,
    TurnStart,
    TurnCompleted,
    TurnFailed,
    TurnCancelled,
    TurnEnded,
    TurnDelta,
    ApprovalRequested,
    InputRequired,
    ToolCall,
    UnsupportedToolCall,
    Notification,
    Other,
}

impl ProtocolMethodKind {
    pub fn from_method(method: &str) -> Self {
        match canonical_method_name(method).as_str() {
            "initialize" => Self::Initialize,
            "initialized" => Self::Initialized,
            "thread/start" => Self::ThreadStart,
            "session/new" => Self::SessionNew,
            "turn/start" => Self::TurnStart,
            "turn/completed" => Self::TurnCompleted,
            "turn/failed" => Self::TurnFailed,
            "turn/cancelled" => Self::TurnCancelled,
            "turn/end" => Self::TurnEnded,
            "turn/delta" | "turn/stream" | "turn/update" => Self::TurnDelta,
            "approval/requested"
            | "approval/required"
            | "turn/approval_required"
            | "turn/approval-required" => Self::ApprovalRequested,
            "turn/input_required"
            | "turn/input-required"
            | "input/required"
            | "item/tool/requestuserinput" => Self::InputRequired,
            "item/tool/call" => Self::ToolCall,
            "unsupported_tool_call" | "unsupported/tool_call" => Self::UnsupportedToolCall,
            "notification" => Self::Notification,
            _ => Self::Other,
        }
    }

    pub fn category(self) -> ProtocolMethodCategory {
        match self {
            Self::Initialize | Self::Initialized | Self::ThreadStart | Self::SessionNew => {
                ProtocolMethodCategory::Startup
            }
            Self::TurnStart
            | Self::TurnCompleted
            | Self::TurnFailed
            | Self::TurnCancelled
            | Self::TurnEnded
            | Self::TurnDelta => ProtocolMethodCategory::Turn,
            Self::ApprovalRequested => ProtocolMethodCategory::Approval,
            Self::InputRequired => ProtocolMethodCategory::Input,
            Self::ToolCall | Self::UnsupportedToolCall => ProtocolMethodCategory::Tooling,
            Self::Notification => ProtocolMethodCategory::Notification,
            Self::Other => ProtocolMethodCategory::Other,
        }
    }

    pub fn canonical_name(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::Initialized => "initialized",
            Self::ThreadStart => "thread/start",
            Self::SessionNew => "session/new",
            Self::TurnStart => "turn/start",
            Self::TurnCompleted => "turn/completed",
            Self::TurnFailed => "turn/failed",
            Self::TurnCancelled => "turn/cancelled",
            Self::TurnEnded => "turn/end",
            Self::TurnDelta => "turn/delta",
            Self::ApprovalRequested => "approval/requested",
            Self::InputRequired => "turn/input_required",
            Self::ToolCall => "item/tool/call",
            Self::UnsupportedToolCall => "unsupported/tool_call",
            Self::Notification => "notification",
            Self::Other => "other",
        }
    }

    pub fn is_turn_terminal(self) -> bool {
        matches!(
            self,
            Self::TurnCompleted | Self::TurnFailed | Self::TurnCancelled | Self::TurnEnded
        )
    }
}

pub fn canonical_method_name(method: &str) -> String {
    method
        .trim()
        .chars()
        .map(|character| match character {
            '.' => '/',
            character => character.to_ascii_lowercase(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_dot_and_slash_variants() {
        assert_eq!(
            ProtocolMethodKind::from_method("turn.start"),
            ProtocolMethodKind::TurnStart
        );
        assert_eq!(
            ProtocolMethodKind::from_method("turn/start"),
            ProtocolMethodKind::TurnStart
        );
        assert_eq!(
            ProtocolMethodKind::from_method("session.new"),
            ProtocolMethodKind::SessionNew
        );
        assert_eq!(
            ProtocolMethodKind::from_method("turn/approval_required"),
            ProtocolMethodKind::ApprovalRequested
        );
        assert_eq!(
            ProtocolMethodKind::from_method("item/tool/requestUserInput"),
            ProtocolMethodKind::InputRequired
        );
        assert_eq!(
            ProtocolMethodKind::from_method("item/tool/call"),
            ProtocolMethodKind::ToolCall
        );
    }

    #[test]
    fn assigns_categories() {
        assert_eq!(
            ProtocolMethodKind::Initialize.category(),
            ProtocolMethodCategory::Startup
        );
        assert_eq!(
            ProtocolMethodKind::TurnCompleted.category(),
            ProtocolMethodCategory::Turn
        );
        assert_eq!(
            ProtocolMethodKind::UnsupportedToolCall.category(),
            ProtocolMethodCategory::Tooling
        );
    }

    #[test]
    fn canonicalizes_mixed_case_method_names() {
        assert_eq!(canonical_method_name(" Turn.Start "), "turn/start");
    }
}
