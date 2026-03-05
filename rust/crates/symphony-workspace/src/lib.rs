#![forbid(unsafe_code)]

mod error;
mod lifecycle;

pub use error::WorkspaceError;
pub use lifecycle::{
    PreparedWorkspace, ensure_within_root, prepare_workspace, remove_workspace,
    sanitize_workspace_key, workspace_path,
};
