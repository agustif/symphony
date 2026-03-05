#![forbid(unsafe_code)]

use anyhow::Result;
use symphony_cli::{CliArgs, Parser, startup_config_from_cli};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = CliArgs::parse();
    let cwd = std::env::current_dir()?;
    let startup_config = startup_config_from_cli(args, &cwd);
    println!(
        "symphony-rust bootstrap ready (config: {})",
        startup_config.config_path.display()
    );
    Ok(())
}
