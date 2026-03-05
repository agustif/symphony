#![forbid(unsafe_code)]

use std::process::ExitCode;

use symphony_cli::{CliStartupError, build_validated_startup_config_from_args};

fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!("failed to resolve current working directory: {error}");
            return ExitCode::from(1);
        }
    };

    match build_validated_startup_config_from_args(std::env::args_os(), &cwd) {
        Ok(startup_config) => {
            println!(
                "symphony-rust bootstrap ready (workflow: {}, config: {})",
                startup_config.workflow_path.display(),
                startup_config.config_path.display(),
            );
            ExitCode::SUCCESS
        }
        Err(CliStartupError::CliArgs(error)) => {
            let _ = error.print();
            ExitCode::from(2)
        }
        Err(error) => {
            eprintln!("startup validation failed: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
