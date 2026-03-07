use std::path::PathBuf;

pub const DEFAULT_HOOK_TIMEOUT_MS: u64 = 60_000;
pub const DEFAULT_HOOK_OUTPUT_LIMIT_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceHookKind {
    AfterCreate,
    BeforeRun,
    AfterRun,
    BeforeRemove,
}

impl WorkspaceHookKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AfterCreate => "after_create",
            Self::BeforeRun => "before_run",
            Self::AfterRun => "after_run",
            Self::BeforeRemove => "before_remove",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceHooks {
    pub after_create: Option<String>,
    pub before_run: Option<String>,
    pub after_run: Option<String>,
    pub before_remove: Option<String>,
    pub timeout_ms: u64,
    pub output_limit_bytes: usize,
}

impl Default for WorkspaceHooks {
    fn default() -> Self {
        Self {
            after_create: None,
            before_run: None,
            after_run: None,
            before_remove: None,
            timeout_ms: DEFAULT_HOOK_TIMEOUT_MS,
            output_limit_bytes: DEFAULT_HOOK_OUTPUT_LIMIT_BYTES,
        }
    }
}

impl WorkspaceHooks {
    pub fn command_for(&self, kind: WorkspaceHookKind) -> Option<&str> {
        match kind {
            WorkspaceHookKind::AfterCreate => self.after_create.as_deref(),
            WorkspaceHookKind::BeforeRun => self.before_run.as_deref(),
            WorkspaceHookKind::AfterRun => self.after_run.as_deref(),
            WorkspaceHookKind::BeforeRemove => self.before_remove.as_deref(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookRequest {
    pub kind: WorkspaceHookKind,
    pub command: String,
    pub workspace_path: PathBuf,
    pub timeout_ms: u64,
    pub output_limit_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookResult {
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub truncated: bool,
}

impl HookResult {
    pub fn success() -> Self {
        Self {
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
            exit_code: Some(0),
            truncated: false,
        }
    }

    pub fn with_output(stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            stdout: stdout.into(),
            stderr: stderr.into(),
            timed_out: false,
            exit_code: Some(0),
            truncated: false,
        }
    }

    pub fn with_status(
        exit_code: Option<i32>,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
    ) -> Self {
        Self {
            stdout: stdout.into(),
            stderr: stderr.into(),
            timed_out: false,
            exit_code,
            truncated: false,
        }
    }
}

pub trait HookExecutor {
    fn execute(&self, request: &HookRequest) -> Result<HookResult, String>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopHookExecutor;

impl HookExecutor for NoopHookExecutor {
    fn execute(&self, _request: &HookRequest) -> Result<HookResult, String> {
        Ok(HookResult::success())
    }
}

pub fn truncate_hook_result(mut result: HookResult, output_limit_bytes: usize) -> HookResult {
    let (stdout, stdout_truncated) = truncate_string(&result.stdout, output_limit_bytes);
    let (stderr, stderr_truncated) = truncate_string(&result.stderr, output_limit_bytes);
    result.stdout = stdout;
    result.stderr = stderr;
    result.truncated = result.truncated || stdout_truncated || stderr_truncated;
    result
}

fn truncate_string(value: &str, output_limit_bytes: usize) -> (String, bool) {
    if output_limit_bytes == 0 || value.len() <= output_limit_bytes {
        return (value.to_owned(), false);
    }

    let mut boundary = output_limit_bytes;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }

    if boundary == 0 {
        return (String::new(), true);
    }

    (value[..boundary].to_owned(), true)
}

#[cfg(test)]
mod tests {
    use super::{HookResult, truncate_hook_result};

    #[test]
    fn truncates_hook_output_to_byte_limit() {
        let result = HookResult::with_output("123456789", "abcdef");
        let truncated = truncate_hook_result(result, 5);

        assert_eq!(truncated.stdout, "12345");
        assert_eq!(truncated.stderr, "abcde");
        assert!(truncated.truncated);
    }

    #[test]
    fn truncation_preserves_utf8_boundaries() {
        let result = HookResult::with_output("ééé", "");
        let truncated = truncate_hook_result(result, 3);

        assert_eq!(truncated.stdout, "é");
        assert!(truncated.truncated);
    }
}
