use std::path::PathBuf;

use clap::Parser;

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
#[command(name = "symphony", about = "Symphony runtime CLI")]
pub struct CliArgs {
    #[arg(value_name = "WORKFLOW_PATH")]
    pub workflow: Option<PathBuf>,

    #[arg(
        long,
        short = 'c',
        value_name = "PATH",
        default_value = "symphony.runtime.json"
    )]
    pub config: PathBuf,
}
