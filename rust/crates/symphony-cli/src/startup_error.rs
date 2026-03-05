use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub enum StartupConfigError {
    MissingConfigFile { path: PathBuf },
    ConfigPathNotFile { path: PathBuf },
    MissingWorkflowFile { path: PathBuf, explicit: bool },
    WorkflowPathNotFile { path: PathBuf, explicit: bool },
}

impl Display for StartupConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingConfigFile { path } => {
                write!(f, "runtime config file does not exist: {}", path.display())
            }
            Self::ConfigPathNotFile { path } => {
                write!(f, "runtime config path is not a file: {}", path.display())
            }
            Self::MissingWorkflowFile { path, .. } => {
                write!(f, "workflow file does not exist: {}", path.display())
            }
            Self::WorkflowPathNotFile { path, .. } => {
                write!(f, "workflow path is not a file: {}", path.display())
            }
        }
    }
}

impl Error for StartupConfigError {}

#[derive(Debug)]
pub enum CliStartupError {
    CliArgs(clap::Error),
    StartupConfig(StartupConfigError),
}

impl Display for CliStartupError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CliArgs(error) => Display::fmt(error, f),
            Self::StartupConfig(error) => Display::fmt(error, f),
        }
    }
}

impl Error for CliStartupError {}

impl From<clap::Error> for CliStartupError {
    fn from(value: clap::Error) -> Self {
        Self::CliArgs(value)
    }
}

impl From<StartupConfigError> for CliStartupError {
    fn from(value: StartupConfigError) -> Self {
        Self::StartupConfig(value)
    }
}

impl CliStartupError {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::CliArgs(_) => 2,
            Self::StartupConfig(_) => 1,
        }
    }
}
