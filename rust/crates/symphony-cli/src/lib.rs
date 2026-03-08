#![forbid(unsafe_code)]

mod cli_args;
mod reload_classification;
mod startup_config;
mod startup_error;
mod terminal_dashboard;

pub use clap::Parser;
pub use cli_args::CliArgs;
pub use reload_classification::{
    HostOwnedConfigChange, WorkflowReloadDisposition, classify_workflow_reload,
};
pub use startup_config::{
    RuntimeStartupConfig, build_startup_config_from_args, build_validated_startup_config_from_args,
    resolve_config_path, resolve_logs_root, resolve_workflow_path, startup_config_from_cli,
    validate_startup_config,
};
pub use startup_error::{CliStartupError, StartupConfigError};
pub use terminal_dashboard::{
    DashboardCodexTotals, DashboardPollingStatus, DashboardRetryEntry, DashboardRunningEntry,
    RenderContext as TerminalDashboardRenderContext, TerminalDashboardSnapshot, dashboard_url,
    render_content_to_terminal, render_offline_status_to_terminal,
    render_snapshot_content_for_test, render_state_snapshot_content_for_test,
    stdout_supports_dashboard,
};
