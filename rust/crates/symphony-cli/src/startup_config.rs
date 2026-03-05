use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::Parser;

use crate::CliArgs;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeStartupConfig {
    pub config_path: PathBuf,
}

pub fn resolve_config_path(config_path: &Path, cwd: &Path) -> PathBuf {
    if config_path.is_absolute() {
        config_path.to_path_buf()
    } else {
        cwd.join(config_path)
    }
}

pub fn startup_config_from_cli(cli: CliArgs, cwd: &Path) -> RuntimeStartupConfig {
    RuntimeStartupConfig {
        config_path: resolve_config_path(&cli.config, cwd),
    }
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
