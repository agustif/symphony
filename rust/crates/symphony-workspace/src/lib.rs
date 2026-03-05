#![forbid(unsafe_code)]

mod error;
mod hooks;
mod lifecycle;

pub use error::WorkspaceError;
pub use hooks::{
    DEFAULT_HOOK_OUTPUT_LIMIT_BYTES, DEFAULT_HOOK_TIMEOUT_MS, HookExecutor, HookRequest,
    HookResult, NoopHookExecutor, WorkspaceHookKind, WorkspaceHooks, truncate_hook_result,
};
pub use lifecycle::{
    PreparedWorkspace, ensure_within_root, prepare_workspace, prepare_workspace_with_hooks,
    remove_workspace, remove_workspace_with_hooks, run_after_run_hook, run_before_run_hook,
    sanitize_workspace_key, validate_worker_cwd, workspace_path,
};
