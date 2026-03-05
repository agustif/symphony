use thiserror::Error;

use crate::{AppServerEvent, ProtocolMethodKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupPhase {
    AwaitInitialize,
    AwaitInitialized,
    AwaitSessionStart,
    AwaitFirstTurnStart,
    Ready,
}

impl StartupPhase {
    fn expected_method(self) -> Option<&'static str> {
        match self {
            Self::AwaitInitialize => Some("initialize"),
            Self::AwaitInitialized => Some("initialized"),
            Self::AwaitSessionStart => Some("thread/start or session/new"),
            Self::AwaitFirstTurnStart => Some("turn/start"),
            Self::Ready => None,
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProtocolSequenceError {
    #[error("invalid startup sequence: expected `{expected}` but observed `{observed}`")]
    UnexpectedStartupMethod {
        expected: &'static str,
        observed: String,
    },
    #[error("invalid turn sequence: `{observed}` started while a turn is already active")]
    TurnAlreadyActive { observed: String },
    #[error("invalid turn sequence: terminal `{observed}` observed without an active turn")]
    TurnTerminalWithoutActiveTurn { observed: String },
    #[error("invalid startup sequence: observed `{observed}` after startup completed")]
    StartupAlreadyComplete { observed: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolSequenceValidator {
    startup_phase: StartupPhase,
    turn_active: bool,
}

impl Default for ProtocolSequenceValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolSequenceValidator {
    pub fn new() -> Self {
        Self {
            startup_phase: StartupPhase::AwaitInitialize,
            turn_active: false,
        }
    }

    pub fn startup_phase(&self) -> StartupPhase {
        self.startup_phase
    }

    pub fn startup_complete(&self) -> bool {
        matches!(self.startup_phase, StartupPhase::Ready)
    }

    pub fn turn_active(&self) -> bool {
        self.turn_active
    }

    pub fn observe_event(
        &mut self,
        event: &AppServerEvent,
    ) -> Result<ProtocolMethodKind, ProtocolSequenceError> {
        self.observe_method(event.method.as_str())
    }

    pub fn observe_method(
        &mut self,
        method: &str,
    ) -> Result<ProtocolMethodKind, ProtocolSequenceError> {
        let method_kind = ProtocolMethodKind::from_method(method);
        self.observe_method_kind(method_kind)?;
        Ok(method_kind)
    }

    pub fn observe_method_kind(
        &mut self,
        method_kind: ProtocolMethodKind,
    ) -> Result<(), ProtocolSequenceError> {
        match method_kind {
            ProtocolMethodKind::Initialize
            | ProtocolMethodKind::Initialized
            | ProtocolMethodKind::ThreadStart
            | ProtocolMethodKind::SessionNew => self.observe_startup_method(method_kind),
            ProtocolMethodKind::TurnStart => self.observe_turn_start(),
            method if method.is_turn_terminal() => self.observe_turn_terminal(method),
            _ => Ok(()),
        }
    }

    fn observe_startup_method(
        &mut self,
        method_kind: ProtocolMethodKind,
    ) -> Result<(), ProtocolSequenceError> {
        let observed = method_kind.canonical_name().to_owned();

        match self.startup_phase {
            StartupPhase::AwaitInitialize => {
                if method_kind == ProtocolMethodKind::Initialize {
                    self.startup_phase = StartupPhase::AwaitInitialized;
                    Ok(())
                } else {
                    Err(ProtocolSequenceError::UnexpectedStartupMethod {
                        expected: StartupPhase::AwaitInitialize
                            .expected_method()
                            .expect("expected startup method"),
                        observed,
                    })
                }
            }
            StartupPhase::AwaitInitialized => {
                if method_kind == ProtocolMethodKind::Initialized {
                    self.startup_phase = StartupPhase::AwaitSessionStart;
                    Ok(())
                } else {
                    Err(ProtocolSequenceError::UnexpectedStartupMethod {
                        expected: StartupPhase::AwaitInitialized
                            .expected_method()
                            .expect("expected startup method"),
                        observed,
                    })
                }
            }
            StartupPhase::AwaitSessionStart => {
                if matches!(
                    method_kind,
                    ProtocolMethodKind::ThreadStart | ProtocolMethodKind::SessionNew
                ) {
                    self.startup_phase = StartupPhase::AwaitFirstTurnStart;
                    Ok(())
                } else {
                    Err(ProtocolSequenceError::UnexpectedStartupMethod {
                        expected: StartupPhase::AwaitSessionStart
                            .expected_method()
                            .expect("expected startup method"),
                        observed,
                    })
                }
            }
            StartupPhase::AwaitFirstTurnStart => {
                Err(ProtocolSequenceError::UnexpectedStartupMethod {
                    expected: StartupPhase::AwaitFirstTurnStart
                        .expected_method()
                        .expect("expected startup method"),
                    observed,
                })
            }
            StartupPhase::Ready => Err(ProtocolSequenceError::StartupAlreadyComplete { observed }),
        }
    }

    fn observe_turn_start(&mut self) -> Result<(), ProtocolSequenceError> {
        if self.turn_active {
            return Err(ProtocolSequenceError::TurnAlreadyActive {
                observed: ProtocolMethodKind::TurnStart.canonical_name().to_owned(),
            });
        }

        match self.startup_phase {
            StartupPhase::AwaitFirstTurnStart => {
                self.startup_phase = StartupPhase::Ready;
                self.turn_active = true;
                Ok(())
            }
            StartupPhase::Ready => {
                self.turn_active = true;
                Ok(())
            }
            phase => Err(ProtocolSequenceError::UnexpectedStartupMethod {
                expected: phase.expected_method().expect("expected startup method"),
                observed: ProtocolMethodKind::TurnStart.canonical_name().to_owned(),
            }),
        }
    }

    fn observe_turn_terminal(
        &mut self,
        method_kind: ProtocolMethodKind,
    ) -> Result<(), ProtocolSequenceError> {
        if !self.turn_active {
            return Err(ProtocolSequenceError::TurnTerminalWithoutActiveTurn {
                observed: method_kind.canonical_name().to_owned(),
            });
        }

        self.turn_active = false;
        Ok(())
    }
}

pub fn validate_startup_turn_sequence<'a, I>(methods: I) -> Result<(), ProtocolSequenceError>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut validator = ProtocolSequenceValidator::new();
    for method in methods {
        validator.observe_method(method)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_stdout_line;

    #[test]
    fn accepts_valid_startup_and_turn_sequence_with_session_new_alias() {
        let methods = [
            "initialize",
            "initialized",
            "session/new",
            "turn/start",
            "turn/completed",
            "turn/start",
            "turn/failed",
        ];
        assert!(validate_startup_turn_sequence(methods).is_ok());
    }

    #[test]
    fn accepts_dot_variant_methods() {
        let methods = [
            "initialize",
            "initialized",
            "thread.start",
            "turn.start",
            "turn.end",
        ];
        assert!(validate_startup_turn_sequence(methods).is_ok());
    }

    #[test]
    fn rejects_invalid_startup_order() {
        let error = validate_startup_turn_sequence(["initialized"]).expect_err("must fail");
        assert_eq!(
            error,
            ProtocolSequenceError::UnexpectedStartupMethod {
                expected: "initialize",
                observed: "initialized".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_turn_start_before_session_start() {
        let methods = ["initialize", "initialized", "turn/start"];
        let error = validate_startup_turn_sequence(methods).expect_err("must fail");
        assert_eq!(
            error,
            ProtocolSequenceError::UnexpectedStartupMethod {
                expected: "thread/start or session/new",
                observed: "turn/start".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_turn_terminal_without_active_turn() {
        let methods = [
            "initialize",
            "initialized",
            "thread/start",
            "turn/completed",
        ];
        let error = validate_startup_turn_sequence(methods).expect_err("must fail");
        assert_eq!(
            error,
            ProtocolSequenceError::TurnTerminalWithoutActiveTurn {
                observed: "turn/completed".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_starting_new_turn_while_active() {
        let methods = [
            "initialize",
            "initialized",
            "thread/start",
            "turn/start",
            "turn/start",
        ];
        let error = validate_startup_turn_sequence(methods).expect_err("must fail");
        assert_eq!(
            error,
            ProtocolSequenceError::TurnAlreadyActive {
                observed: "turn/start".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_restarting_handshake_while_waiting_for_first_turn() {
        let methods = ["initialize", "initialized", "thread/start", "initialized"];
        let error = validate_startup_turn_sequence(methods).expect_err("must fail");
        assert_eq!(
            error,
            ProtocolSequenceError::UnexpectedStartupMethod {
                expected: "turn/start",
                observed: "initialized".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_restarting_handshake_after_ready() {
        let methods = [
            "initialize",
            "initialized",
            "thread/start",
            "turn/start",
            "turn/completed",
            "initialized",
        ];
        let error = validate_startup_turn_sequence(methods).expect_err("must fail");
        assert_eq!(
            error,
            ProtocolSequenceError::StartupAlreadyComplete {
                observed: "initialized".to_owned(),
            }
        );
    }

    #[test]
    fn observes_decoded_event_methods() {
        let mut validator = ProtocolSequenceValidator::new();
        let startup = decode_stdout_line(r#"{"method":"initialize"}"#).expect("decode");
        assert_eq!(
            validator.observe_event(&startup).expect("observe"),
            ProtocolMethodKind::Initialize
        );
        assert_eq!(validator.startup_phase(), StartupPhase::AwaitInitialized);
    }
}
