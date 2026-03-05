use std::path::PathBuf;

use symphony_config::CodexConfig;
use symphony_tracker::TrackerIssue;
use symphony_workspace::{
    HookExecutor, PreparedWorkspace, WorkspaceError, WorkspaceHooks, ensure_within_root, workspace_path,
};
use symphony_domain::IssueId;

/// Context for spawning a worker with full workspace and prompt information
#[derive(Clone)]
pub struct WorkerContext {
    pub issue: TrackerIssue,
    pub attempt: u32,
    pub root: PathBuf,
    pub hooks: WorkspaceHooks,
    pub codex_config: CodexConfig,
    pub prompt_template: String,
}

impl WorkerContext {
    /// Create a new WorkerContext ensuring workspace path safety
    pub fn new(
        issue: TrackerIssue,
        attempt: u32,
        root: PathBuf,
        hooks: WorkspaceHooks,
        codex_config: CodexConfig,
        prompt_template: String,
    ) -> Result<Self, WorkspaceError> {
        // Validate workspace root is safe
        ensure_within_root(&root, &issue.identifier)?;

        Ok(Self {
            issue,
            attempt,
            root,
            hooks,
            codex_config,
            prompt_template,
        })
    }

    /// Get the issue ID for this worker context
    pub fn issue_id(&self) -> IssueId {
        self.issue.id.clone()
    }

    /// Get the workspace path for this issue
    pub fn workspace_path(&self) -> PathBuf {
        workspace_path(&self.root, &self.issue.identifier)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("prompt rendering failed")]
    PromptError,
    #[error("agent failure")]
    AgentFailure,
}

#[async_trait::async_trait]
pub trait WorkerLauncher: Send + Sync {
    async fn launch(&self, ctx: WorkerContext) -> Result<(), WorkerError>;
}

/// Prepare workspace for a worker with hooks
/// Ensures workspace path safety and executes before-run hooks
pub fn prepare_worker_workspace(
    ctx: &WorkerContext,
    executor: &impl HookExecutor,
) -> Result<PreparedWorkspace, WorkspaceError> {
    prepare_workspace_with_hooks(&ctx.root, &ctx.issue.identifier, &ctx.hooks, executor)
}

/// Render prompt template with issue context
/// Replaces common placeholders with actual issue data
pub fn render_prompt(ctx: &WorkerContext) -> Result<String, WorkerError> {
    // Simple placeholder replacement for now
    // Future enhancement: Use proper templating engine (e.g., handlebars, tera)
    let description = ctx.issue.description.as_deref().unwrap_or(&String::new());
    let prompt = ctx
        .prompt_template
        .replace("{{issue_id}}", &ctx.issue.id.0)
        .replace("{{issue_identifier}}", &ctx.issue.identifier)
        .replace("{{issue_title}}", &ctx.issue.title)
        .replace("{{issue_description}}", description)
        .replace("{{attempt}}", &ctx.attempt.to_string());

    Ok(prompt)
}

// In the future: Render the prompt here, launch the codex command, parse stdout/stderr streams.

#[cfg(test)]
mod tests {
    use super::*;
    use symphony_tracker::TrackerState;

    fn create_test_context() -> WorkerContext {
        let issue = TrackerIssue {
            id: IssueId("SYM-123".to_owned()),
            identifier: "SYM-123".to_owned(),
            title: "Test Issue".to_owned(),
            state: TrackerState::new("Todo"),
            description: Some("Test description".to_owned()),
        };
        let root = std::env::temp_dir().join("test-workspaces");
        let hooks = WorkspaceHooks::default();
        let codex_config = CodexConfig::default();
        let prompt_template =
            "Issue {{issue_id}}: {{issue_title}}\n{{issue_description}}\nAttempt: {{attempt}}"
                .to_owned();

        WorkerContext::new(issue, 1, root, hooks, codex_config, prompt_template)
            .expect("Failed to create context")
    }

    #[test]
    fn worker_context_creates_with_safety_checks() {
        let ctx = create_test_context();
        assert_eq!(ctx.issue.id.0, "SYM-123");
        assert_eq!(ctx.attempt, 1);
        assert_eq!(ctx.issue.title, "Test Issue");
    }

    #[test]
    fn worker_context_provides_issue_id() {
        let ctx = create_test_context();
        assert_eq!(ctx.issue_id().0, "SYM-123");
    }

    #[test]
    fn worker_context_provides_workspace_path() {
        let ctx = create_test_context();
        let ws_path = ctx.workspace_path();
        assert!(ws_path.ends_with("SYM-123"));
    }

    #[test]
    fn render_prompt_replaces_placeholders() {
        let ctx = create_test_context();
        let prompt = render_prompt(&ctx).expect("Failed to render prompt");
        assert!(prompt.contains("Issue SYM-123"));
        assert!(prompt.contains("Test Issue"));
        assert!(prompt.contains("Test description"));
        assert!(prompt.contains("Attempt: 1"));
    }

    #[test]
    fn render_prompt_handles_missing_description() {
        let mut ctx = create_test_context();
        ctx.issue.description = None;
        let prompt = render_prompt(&ctx).expect("Failed to render prompt");
        // Should not panic with None description
        assert!(prompt.contains("Issue SYM-123"));
    }
}
