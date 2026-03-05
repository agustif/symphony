use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::Parser;

use crate::{CliArgs, CliStartupError, StartupConfigError};

const DEFAULT_WORKFLOW_FILE: &str = "WORKFLOW.md";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeStartupConfig {
    pub config_path: PathBuf,
    pub workflow_path: PathBuf,
    pub workflow_path_explicit: bool,
}

pub fn resolve_config_path(config_path: &Path, cwd: &Path) -> PathBuf {
    if config_path.is_absolute() {
        config_path.to_path_buf()
    } else {
        cwd.join(config_path)
    }
}

pub fn resolve_workflow_path(workflow_path: Option<&Path>, cwd: &Path) -> PathBuf {
    match workflow_path {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => cwd.join(path),
        None => cwd.join(DEFAULT_WORKFLOW_FILE),
    }
}

pub fn startup_config_from_cli(cli: CliArgs, cwd: &Path) -> RuntimeStartupConfig {
    RuntimeStartupConfig {
        config_path: resolve_config_path(&cli.config, cwd),
        workflow_path: resolve_workflow_path(cli.workflow.as_deref(), cwd),
        workflow_path_explicit: cli.workflow.is_some(),
    }
}

pub fn validate_startup_config(config: &RuntimeStartupConfig) -> Result<(), StartupConfigError> {
    validate_config_path(&config.config_path)?;
    validate_workflow_path(&config.workflow_path, config.workflow_path_explicit)?;
    Ok(())
}

pub fn build_startup_config_from_args<I, T>(
    args: I,
    cwd: &Path,
) -> Result<RuntimeStartupConfig, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = CliArgs::try_parse_from(args)?;
    Ok(startup_config_from_cli(cli, cwd))
}

pub fn build_validated_startup_config_from_args<I, T>(
    args: I,
    cwd: &Path,
) -> Result<RuntimeStartupConfig, CliStartupError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let startup = build_startup_config_from_args(args, cwd)?;
    validate_startup_config(&startup)?;
    Ok(startup)
}

fn validate_config_path(config_path: &Path) -> Result<(), StartupConfigError> {
    if !config_path.exists() {
        return Err(StartupConfigError::MissingConfigFile {
            path: config_path.to_path_buf(),
        });
    }

    if !config_path.is_file() {
        return Err(StartupConfigError::ConfigPathNotFile {
            path: config_path.to_path_buf(),
        });
    }

    Ok(())
}

fn validate_workflow_path(workflow_path: &Path, explicit: bool) -> Result<(), StartupConfigError> {
    if !workflow_path.exists() {
        return Err(StartupConfigError::MissingWorkflowFile {
            path: workflow_path.to_path_buf(),
            explicit,
        });
    }

    if !workflow_path.is_file() {
        return Err(StartupConfigError::WorkflowPathNotFile {
            path: workflow_path.to_path_buf(),
            explicit,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn resolves_default_workflow_path_relative_to_cwd() {
        let cwd = PathBuf::from("/srv/symphony");
        let args = CliArgs {
            workflow: None,
            config: PathBuf::from("symphony.runtime.json"),
        };

        let startup = startup_config_from_cli(args, &cwd);
        assert_eq!(startup.workflow_path, cwd.join("WORKFLOW.md"));
        assert!(!startup.workflow_path_explicit);
    }

    #[test]
    fn validated_builder_accepts_existing_config_and_workflow() {
        let cwd = temp_dir("validated_builder_accepts_existing_config_and_workflow");
        write_file(cwd.join("symphony.runtime.json"));
        write_file(cwd.join("WORKFLOW.md"));

        let startup = build_validated_startup_config_from_args(["symphony"], &cwd)
            .expect("existing config and workflow should validate");

        assert_eq!(startup.config_path, cwd.join("symphony.runtime.json"));
        assert_eq!(startup.workflow_path, cwd.join("WORKFLOW.md"));
    }

    #[test]
    fn validated_builder_rejects_missing_default_workflow() {
        let cwd = temp_dir("validated_builder_rejects_missing_default_workflow");
        write_file(cwd.join("symphony.runtime.json"));

        let error = build_validated_startup_config_from_args(["symphony"], &cwd)
            .expect_err("missing default workflow should fail validation");

        assert!(matches!(
            error,
            CliStartupError::StartupConfig(StartupConfigError::MissingWorkflowFile {
                explicit: false,
                ..
            })
        ));
    }

    #[test]
    fn validated_builder_rejects_missing_explicit_workflow() {
        let cwd = temp_dir("validated_builder_rejects_missing_explicit_workflow");
        write_file(cwd.join("symphony.runtime.json"));

        let error =
            build_validated_startup_config_from_args(["symphony", "relative/WORKFLOW.md"], &cwd)
                .expect_err("missing explicit workflow should fail validation");

        assert!(matches!(
            error,
            CliStartupError::StartupConfig(StartupConfigError::MissingWorkflowFile {
                explicit: true,
                ..
            })
        ));
    }

    #[test]
    fn validated_builder_rejects_missing_config() {
        let cwd = temp_dir("validated_builder_rejects_missing_config");
        write_file(cwd.join("WORKFLOW.md"));

        let error = build_validated_startup_config_from_args(["symphony"], &cwd)
            .expect_err("missing config should fail validation");

        assert!(matches!(
            error,
            CliStartupError::StartupConfig(StartupConfigError::MissingConfigFile { .. })
        ));
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("symphony-cli-{name}-{nonce}"));
        fs::create_dir_all(&path).expect("temp directory should be creatable");
        path
    }

    fn write_file(path: impl AsRef<Path>) {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).expect("parent directories should be creatable");
        }
        fs::write(path, "{}\n").expect("file should be writable");
    }
}
