#![forbid(unsafe_code)]

pub mod worker;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use symphony_config::RuntimeConfig;
use symphony_domain::{
    Command, Event, InvariantError, IssueId, OrchestratorState, reduce, validate_invariants,
};
use symphony_observability::RuntimeSnapshot;
use symphony_tracker::{TrackerClient, TrackerError, TrackerIssue, TrackerState};
use symphony_workspace::{WorkspaceHooks, remove_workspace, remove_workspace_with_hooks};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use worker::{ShellHookExecutor, WorkerContext, WorkerLauncher, WorkerOutcome};

const DEFAULT_PROMPT_TEMPLATE: &str =
    "Issue {{issue_identifier}}: {{issue_title}}\n{{issue_description}}";

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("tracker request failed: {0}")]
    Tracker(#[from] TrackerError),
    #[error("orchestrator invariant violated: {0}")]
    Invariant(#[from] InvariantError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickResult {
    pub state: OrchestratorState,
    pub commands: Vec<Command>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryKind {
    Continuation,
    Failure,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetrySchedule {
    pub issue_id: IssueId,
    pub attempt: u32,
    pub kind: RetryKind,
    pub delay: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    pub continuation_delay: Duration,
    pub failure_base_delay: Duration,
    pub max_failure_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            continuation_delay: Duration::from_secs(5),
            failure_base_delay: Duration::from_secs(1),
            max_failure_backoff: Duration::from_secs(300),
        }
    }
}

impl RetryPolicy {
    pub fn schedule_retry(
        &self,
        issue_id: IssueId,
        attempt: u32,
        kind: RetryKind,
    ) -> RetrySchedule {
        let delay = match kind {
            RetryKind::Continuation => self.continuation_delay,
            RetryKind::Failure => self.failure_backoff(attempt),
        };
        RetrySchedule {
            issue_id,
            attempt,
            kind,
            delay,
        }
    }

    fn failure_backoff(&self, attempt: u32) -> Duration {
        let normalized_attempt = attempt.max(1);
        let shift = normalized_attempt.saturating_sub(1).min(127);
        let multiplier = 1_u128 << shift;
        let base_ms = self.failure_base_delay.as_millis();
        let capped_ms = base_ms
            .saturating_mul(multiplier)
            .min(self.max_failure_backoff.as_millis());
        duration_from_millis_u128(capped_ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerExit {
    Completed,
    Continuation,
    RetryableFailure,
    PermanentFailure,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerExitResult {
    pub state: OrchestratorState,
    pub commands: Vec<Command>,
    pub retry_schedule: Option<RetrySchedule>,
}

pub struct Runtime<T: TrackerClient> {
    tracker: Arc<T>,
    worker_launcher: Arc<dyn WorkerLauncher>,
    state: Mutex<OrchestratorState>,
    last_seen: Mutex<HashMap<IssueId, std::time::Instant>>,
    active_workers: StdMutex<HashMap<IssueId, JoinHandle<()>>>,
    scheduled_retries: StdMutex<HashMap<IssueId, JoinHandle<()>>>,
    pending_terminal_cleanup: StdMutex<HashMap<IssueId, String>>,
    prompt_template: RwLock<String>,
    snapshot_tx: tokio::sync::watch::Sender<RuntimeSnapshot>,
}

impl<T: TrackerClient + 'static> Runtime<T> {
    pub fn new(tracker: Arc<T>) -> Self {
        Self::with_worker_launcher(tracker, Arc::new(worker::ShellWorkerLauncher))
    }

    pub fn with_worker_launcher(tracker: Arc<T>, worker_launcher: Arc<dyn WorkerLauncher>) -> Self {
        let (snapshot_tx, _) = tokio::sync::watch::channel(RuntimeSnapshot {
            running: 0,
            retrying: 0,
        });
        Self {
            tracker,
            worker_launcher,
            state: Mutex::new(OrchestratorState::default()),
            last_seen: Mutex::new(HashMap::new()),
            active_workers: StdMutex::new(HashMap::new()),
            scheduled_retries: StdMutex::new(HashMap::new()),
            pending_terminal_cleanup: StdMutex::new(HashMap::new()),
            prompt_template: RwLock::new(DEFAULT_PROMPT_TEMPLATE.to_owned()),
            snapshot_tx,
        }
    }

    pub async fn set_prompt_template(&self, template: String) {
        let mut guard = self.prompt_template.write().await;
        *guard = template;
    }

    pub fn subscribe_snapshots(&self) -> tokio::sync::watch::Receiver<RuntimeSnapshot> {
        self.snapshot_tx.subscribe()
    }

    pub async fn report_activity(&self, issue_id: IssueId) {
        let mut last_seen = self.last_seen.lock().await;
        last_seen.insert(issue_id, std::time::Instant::now());
    }

    pub async fn run_tick(&self, config: &RuntimeConfig) -> Result<TickResult, RuntimeError> {
        let issues = self.tracker.fetch_candidates().await?;
        let mut candidate_issues = issues.clone();
        candidate_issues.sort_by(|left, right| {
            // 1. priority ascending (1..4 preferred, null/unknown sorts last)
            let left_prio = left.priority.unwrap_or(i32::MAX);
            let right_prio = right.priority.unwrap_or(i32::MAX);
            let res = left_prio.cmp(&right_prio);
            if res != std::cmp::Ordering::Equal {
                return res;
            }

            // 2. created_at oldest first (null sorts last)
            let left_created = left.created_at.unwrap_or(u64::MAX);
            let right_created = right.created_at.unwrap_or(u64::MAX);
            let res = left_created.cmp(&right_created);
            if res != std::cmp::Ordering::Equal {
                return res;
            }

            // 3. identifier lexicographic tie-breaker
            left.identifier.cmp(&right.identifier)
        });

        let candidate_ids: Vec<IssueId> = candidate_issues
            .iter()
            .map(|issue| issue.id.clone())
            .collect();
        let candidate_set: HashSet<IssueId> = candidate_ids.iter().cloned().collect();

        let mut state_guard = self.state.lock().await;
        let mut state = state_guard.clone();
        let mut commands = Vec::new();

        // 1. Stall Detection
        let now = std::time::Instant::now();
        let mut stalled_issues = Vec::new();
        if config.codex.stall_timeout_ms > 0 {
            let last_seen_guard = self.last_seen.lock().await;
            for issue_id in state.running.keys() {
                let last_active = last_seen_guard.get(issue_id).copied().unwrap_or(now);
                if now.duration_since(last_active)
                    > Duration::from_millis(config.codex.stall_timeout_ms as u64)
                {
                    stalled_issues.push(issue_id.clone());
                }
            }
        }
        for issue_id in stalled_issues {
            let attempt = next_retry_attempt(&state, &issue_id);
            state = apply_event(
                state,
                Event::QueueRetry {
                    issue_id: issue_id.clone(),
                    attempt,
                },
                &mut commands,
            )?;
            tracing::warn!("Worker stalled for issue {:?}", issue_id);
        }

        let active_states = tracker_states_from_names(&config.tracker.active_states);
        let terminal_states = tracker_states_from_names(&config.tracker.terminal_states);
        let running_ids: Vec<IssueId> = state.running.keys().cloned().collect();
        if !running_ids.is_empty() {
            match self.tracker.fetch_states_by_ids(&running_ids).await {
                Ok(states_by_id) => {
                    let mut terminal_running_ids = Vec::new();
                    let mut non_active_running_ids = Vec::new();

                    for issue_id in running_ids {
                        let Some(tracker_state) = states_by_id.get(&issue_id) else {
                            non_active_running_ids.push(issue_id);
                            continue;
                        };

                        if tracker_state.is_terminal(&terminal_states) {
                            terminal_running_ids.push(issue_id);
                            continue;
                        }

                        if !tracker_state_matches_any(tracker_state, &active_states) {
                            non_active_running_ids.push(issue_id);
                        }
                    }

                    sort_issue_ids(&mut terminal_running_ids);
                    sort_issue_ids(&mut non_active_running_ids);

                    for issue_id in terminal_running_ids {
                        if let Some(identifier) = state
                            .running
                            .get(&issue_id)
                            .and_then(|entry| entry.identifier.clone())
                        {
                            self.queue_terminal_cleanup(issue_id.clone(), identifier);
                        } else {
                            tracing::warn!(
                                issue_id = %issue_id.0,
                                "missing running issue identifier for terminal workspace cleanup"
                            );
                        }
                        state = apply_event(state, Event::Release(issue_id), &mut commands)?;
                    }

                    for issue_id in non_active_running_ids {
                        state = apply_event(state, Event::Release(issue_id), &mut commands)?;
                    }
                }
                Err(error) => {
                    tracing::warn!("running state refresh failed: {error}");
                }
            }
        }

        for issue_id in non_active_reconciliation_ids(&state, &candidate_set) {
            state = apply_event(state, Event::Release(issue_id), &mut commands)?;
        }

        let mut global_available =
            (config.agent.max_concurrent_agents as usize).saturating_sub(state.running.len());

        let mut running_by_state: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for issue in &issues {
            if state.running.contains_key(&issue.id)
                && let Some(state_key) = issue.state.normalized_key()
            {
                *running_by_state.entry(state_key).or_insert(0) += 1;
            }
        }

        for issue_id in candidate_ids {
            if global_available == 0 {
                break;
            }
            if state.claimed.contains(&issue_id)
                || state.running.contains_key(&issue_id)
                || state.retry_attempts.contains_key(&issue_id)
            {
                continue;
            }

            let issue = issues.iter().find(|i| i.id == issue_id).unwrap();
            if !dispatch_state_allows_issue(issue, &active_states, &terminal_states)
                || todo_issue_blocked_by_non_terminal(issue, &terminal_states)
            {
                continue;
            }
            if let Some(state_key) = issue.state.normalized_key()
                && let Some(&max_for_state) =
                    config.agent.max_concurrent_agents_by_state.get(&state_key)
            {
                let current_for_state = running_by_state.get(&state_key).copied().unwrap_or(0);
                if current_for_state >= max_for_state as usize {
                    continue;
                }
                *running_by_state.entry(state_key).or_insert(0) += 1;
            }

            let dispatch_issue_id = issue_id.clone();
            state = apply_event(
                state,
                Event::Claim(dispatch_issue_id.clone()),
                &mut commands,
            )?;
            state = apply_event(
                state,
                Event::MarkRunning(dispatch_issue_id.clone()),
                &mut commands,
            )?;
            if let Some(entry) = state.running.get_mut(&dispatch_issue_id) {
                entry.identifier = Some(issue.identifier.clone());
            }
            global_available = global_available.saturating_sub(1);
        }

        validate_invariants(&state)?;
        *state_guard = state.clone();

        let _ = self.snapshot_tx.send(RuntimeSnapshot {
            running: state.running.len(),
            retrying: state.retry_attempts.len(),
        });

        Ok(TickResult { state, commands })
    }

    pub async fn handle_worker_exit(
        &self,
        issue_id: IssueId,
        worker_exit: WorkerExit,
        retry_policy: &RetryPolicy,
    ) -> Result<WorkerExitResult, RuntimeError> {
        let mut state_guard = self.state.lock().await;
        let mut state = state_guard.clone();
        let mut commands = Vec::new();
        let mut retry_schedule = None;

        match worker_exit {
            WorkerExit::Completed | WorkerExit::PermanentFailure => {
                state = apply_event(state, Event::Release(issue_id), &mut commands)?;
            }
            WorkerExit::Continuation | WorkerExit::RetryableFailure => {
                let attempt = next_retry_attempt(&state, &issue_id);
                let kind = match worker_exit {
                    WorkerExit::Continuation => RetryKind::Continuation,
                    WorkerExit::RetryableFailure => RetryKind::Failure,
                    WorkerExit::Completed | WorkerExit::PermanentFailure => unreachable!(),
                };
                let planned_schedule = retry_policy.schedule_retry(issue_id.clone(), attempt, kind);
                state = apply_event(
                    state,
                    Event::QueueRetry {
                        issue_id: issue_id.clone(),
                        attempt,
                    },
                    &mut commands,
                )?;
                if state
                    .retry_attempts
                    .get(&issue_id)
                    .is_some_and(|retry_entry| retry_entry.attempt == attempt)
                {
                    retry_schedule = Some(planned_schedule);
                }
            }
        }

        validate_invariants(&state)?;
        *state_guard = state.clone();

        let _ = self.snapshot_tx.send(RuntimeSnapshot {
            running: state.running.len(),
            retrying: state.retry_attempts.len(),
        });

        Ok(WorkerExitResult {
            state,
            commands,
            retry_schedule,
        })
    }

    pub async fn update_agent(&self, event: Event) -> Result<(), RuntimeError> {
        let mut state_guard = self.state.lock().await;
        let mut state = state_guard.clone();
        let mut commands = Vec::new();

        state = apply_event(state, event, &mut commands)?;
        *state_guard = state.clone();

        let _ = self.snapshot_tx.send(RuntimeSnapshot {
            running: state.running.len(),
            retrying: state.retry_attempts.len(),
        });

        Ok(())
    }

    /// Handle agent protocol updates with typed parameters
    /// Integrates protocol messages into runtime state with thread-safe updates
    #[allow(clippy::too_many_arguments)]
    pub async fn handle_protocol_update(
        &self,
        issue_id: IssueId,
        session_id: Option<String>,
        thread_id: Option<String>,
        turn_id: Option<String>,
        pid: Option<u32>,
        event: Option<String>,
        timestamp: Option<u64>,
        message: Option<String>,
        usage: Option<symphony_domain::Usage>,
    ) -> Result<(), RuntimeError> {
        let update_event = Event::UpdateAgent {
            issue_id,
            session_id,
            thread_id,
            turn_id,
            pid,
            event,
            timestamp,
            message,
            usage,
        };

        self.update_agent(update_event).await
    }

    pub async fn snapshot(&self) -> RuntimeSnapshot {
        let state = self.state.lock().await;
        RuntimeSnapshot {
            running: state.running.len(),
            retrying: state.retry_attempts.len(),
        }
    }

    pub async fn state(&self) -> OrchestratorState {
        self.state.lock().await.clone()
    }

    /// Sets up an issue as claimed and running. For testing purposes.
    pub async fn setup_running(&self, issue_id: IssueId, entry: symphony_domain::RunningEntry) {
        let mut state = self.state.lock().await;
        state.claimed.insert(issue_id.clone());
        state.running.insert(issue_id, entry);
    }

    pub async fn run_poll_loop(
        self: Arc<Self>,
        config: std::sync::Arc<tokio::sync::RwLock<RuntimeConfig>>,
        mut refresh_rx: tokio::sync::mpsc::Receiver<()>,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) {
        let initial_config = config.read().await.clone();
        let mut current_version = initial_config.version;
        let mut interval = tokio::time::interval(Duration::from_millis(
            initial_config.polling.interval_ms.max(100),
        ));

        tracing::info!(
            poll_interval_ms = initial_config.polling.interval_ms,
            max_concurrent_agents = initial_config.agent.max_concurrent_agents,
            config_version = current_version,
            "Starting poll loop"
        );
        self.startup_terminal_workspace_cleanup(&initial_config)
            .await;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let new_config = config.read().await.clone();

                    // Detect config reload by version change
                    if new_config.version != current_version {
                        tracing::info!(
                            old_version = current_version,
                            new_version = new_config.version,
                            poll_interval_ms = new_config.polling.interval_ms,
                            max_concurrent_agents = new_config.agent.max_concurrent_agents,
                            "Config reloaded"
                        );
                        current_version = new_config.version;
                    }

                    match self.run_tick(&new_config).await {
                        Ok(tick) => self.process_tick_commands(tick.commands, new_config.clone()).await,
                        Err(error) => tracing::error!("Tick failed: {:?}", error),
                    }
                    interval = tokio::time::interval(Duration::from_millis(new_config.polling.interval_ms.max(100)));
                    interval.reset();
                }
                Some(_) = refresh_rx.recv() => {
                    let new_config = config.read().await.clone();

                    // Trigger immediate tick with potential config reload
                    if new_config.version != current_version {
                        tracing::info!(
                            old_version = current_version,
                            new_version = new_config.version,
                            "Refresh triggered config reload"
                        );
                        current_version = new_config.version;
                    }

                    match self.run_tick(&new_config).await {
                        Ok(tick) => self.process_tick_commands(tick.commands, new_config.clone()).await,
                        Err(error) => tracing::error!("Refresh tick failed: {:?}", error),
                    }
                    interval = tokio::time::interval(Duration::from_millis(new_config.polling.interval_ms.max(100)));
                    interval.reset();
                }
                _ = shutdown_rx.recv() => {
                    tracing::info!("Shutting down poll loop");
                    self.abort_background_tasks();
                    break;
                }
            }
        }
    }

    async fn process_tick_commands(
        self: &Arc<Self>,
        commands: Vec<Command>,
        config: RuntimeConfig,
    ) {
        for command in commands {
            match command {
                Command::Dispatch(issue_id) => {
                    self.spawn_worker_task(issue_id, 1, config.clone());
                }
                Command::ScheduleRetry { issue_id, attempt } => {
                    let retry_policy = retry_policy_for_config(&config);
                    let schedule =
                        retry_policy.schedule_retry(issue_id, attempt, RetryKind::Failure);
                    self.schedule_retry_task(schedule, config.clone());
                }
                Command::ReleaseClaim(issue_id) => {
                    self.cancel_retry_task(&issue_id);
                    self.abort_worker_task(&issue_id);
                    self.cleanup_terminal_workspace_if_pending(&issue_id, &config);
                }
                Command::TransitionRejected { issue_id, reason } => {
                    tracing::warn!(issue_id = %issue_id.0, reason = %reason, "transition rejected");
                }
            }
        }
    }

    fn spawn_worker_task(self: &Arc<Self>, issue_id: IssueId, attempt: u32, config: RuntimeConfig) {
        if self.has_active_worker(&issue_id) {
            return;
        }

        let runtime = Arc::clone(self);
        let issue_for_task = issue_id.clone();
        let handle = tokio::spawn(async move {
            runtime
                .execute_worker(issue_for_task.clone(), attempt, config)
                .await;
            let mut workers = lock_unpoisoned(&runtime.active_workers);
            workers.remove(&issue_for_task);
        });

        let mut workers = lock_unpoisoned(&self.active_workers);
        workers.insert(issue_id, handle);
    }

    fn has_active_worker(&self, issue_id: &IssueId) -> bool {
        let workers = lock_unpoisoned(&self.active_workers);
        workers
            .get(issue_id)
            .is_some_and(|handle| !handle.is_finished())
    }

    async fn execute_worker(
        self: &Arc<Self>,
        issue_id: IssueId,
        attempt: u32,
        config: RuntimeConfig,
    ) {
        let issue = match self.fetch_issue(&issue_id).await {
            Ok(Some(issue)) => issue,
            Ok(None) => {
                tracing::warn!(issue_id = %issue_id.0, "dispatch issue not found in tracker candidates");
                let retry_policy = retry_policy_for_config(&config);
                let _ = self
                    .handle_worker_exit(issue_id, WorkerExit::PermanentFailure, &retry_policy)
                    .await;
                return;
            }
            Err(error) => {
                tracing::error!(issue_id = %issue_id.0, "failed to fetch issue for worker launch: {error}");
                let retry_policy = retry_policy_for_config(&config);
                let _ = self
                    .handle_worker_exit(issue_id, WorkerExit::RetryableFailure, &retry_policy)
                    .await;
                return;
            }
        };

        let hooks = workspace_hooks_from_config(&config);
        self.record_running_identifier(&issue_id, &issue.identifier)
            .await;
        let prompt_template = self.prompt_template.read().await.clone();
        let worker_context = match WorkerContext::new(
            issue,
            attempt,
            config.workspace.root.clone(),
            hooks,
            config.tracker.clone(),
            config.codex.clone(),
            prompt_template,
        ) {
            Ok(context) => context,
            Err(error) => {
                tracing::error!(issue_id = %issue_id.0, "invalid worker context: {error}");
                let retry_policy = retry_policy_for_config(&config);
                let _ = self
                    .handle_worker_exit(issue_id, WorkerExit::PermanentFailure, &retry_policy)
                    .await;
                return;
            }
        };

        let worker_outcome = match self.worker_launcher.launch(worker_context).await {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::error!(issue_id = %issue_id.0, "worker launch failed: {error}");
                WorkerOutcome::RetryableFailure
            }
        };

        let worker_exit = map_worker_outcome(worker_outcome);
        let retry_policy = retry_policy_for_config(&config);
        match self
            .handle_worker_exit(issue_id.clone(), worker_exit, &retry_policy)
            .await
        {
            Ok(result) => {
                if let Some(schedule) = result.retry_schedule {
                    self.schedule_retry_task(schedule, config);
                }
            }
            Err(error) => {
                tracing::error!(issue_id = %issue_id.0, "failed to process worker exit: {error}");
            }
        }
    }

    async fn fetch_issue(&self, issue_id: &IssueId) -> Result<Option<TrackerIssue>, TrackerError> {
        let candidates = self.tracker.fetch_candidates().await?;
        Ok(candidates.into_iter().find(|issue| issue.id == *issue_id))
    }

    async fn record_running_identifier(&self, issue_id: &IssueId, identifier: &str) {
        let mut state = self.state.lock().await;
        if let Some(entry) = state.running.get_mut(issue_id) {
            entry.identifier = Some(identifier.to_owned());
        }
    }

    fn queue_terminal_cleanup(&self, issue_id: IssueId, identifier: String) {
        let mut pending = lock_unpoisoned(&self.pending_terminal_cleanup);
        pending.insert(issue_id, identifier);
    }

    fn cleanup_terminal_workspace_if_pending(&self, issue_id: &IssueId, config: &RuntimeConfig) {
        let identifier = {
            let mut pending = lock_unpoisoned(&self.pending_terminal_cleanup);
            pending.remove(issue_id)
        };

        if let Some(identifier) = identifier {
            self.cleanup_workspace_for_identifier(config, &identifier);
        }
    }

    fn cleanup_workspace_for_identifier(&self, config: &RuntimeConfig, issue_identifier: &str) {
        let hooks = workspace_hooks_from_config(config);
        let hook_executor = ShellHookExecutor;
        match remove_workspace_with_hooks(
            &config.workspace.root,
            issue_identifier,
            &hooks,
            &hook_executor,
        ) {
            Ok(true) => {
                tracing::info!(
                    issue_identifier,
                    "removed terminal workspace with before_remove hook"
                );
            }
            Ok(false) => {
                tracing::debug!(
                    issue_identifier,
                    "terminal workspace already absent during cleanup"
                );
            }
            Err(error) => {
                tracing::warn!(
                    issue_identifier,
                    "before_remove cleanup path failed ({error}); falling back to direct removal"
                );
                match remove_workspace(&config.workspace.root, issue_identifier) {
                    Ok(true) => {
                        tracing::info!(
                            issue_identifier,
                            "removed terminal workspace without hooks"
                        );
                    }
                    Ok(false) => {
                        tracing::debug!(
                            issue_identifier,
                            "terminal workspace already absent during cleanup fallback"
                        );
                    }
                    Err(remove_error) => {
                        tracing::warn!(
                            issue_identifier,
                            "terminal workspace cleanup failed: {remove_error}"
                        );
                    }
                }
            }
        }
    }

    async fn startup_terminal_workspace_cleanup(&self, config: &RuntimeConfig) {
        let terminal_states = tracker_states_from_names(&config.tracker.terminal_states);
        if terminal_states.is_empty() {
            return;
        }

        match self
            .tracker
            .fetch_terminal_candidates(&terminal_states)
            .await
        {
            Ok(terminal_issues) => {
                for issue in terminal_issues {
                    self.cleanup_workspace_for_identifier(config, &issue.identifier);
                }
            }
            Err(error) => {
                tracing::warn!("startup terminal cleanup skipped due to tracker error: {error}");
            }
        }
    }

    fn schedule_retry_task(self: &Arc<Self>, schedule: RetrySchedule, config: RuntimeConfig) {
        self.cancel_retry_task(&schedule.issue_id);

        let runtime = Arc::clone(self);
        let issue_id = schedule.issue_id.clone();
        let attempt = schedule.attempt;
        let delay = schedule.delay;
        let issue_for_task = issue_id.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            runtime
                .promote_retry_and_spawn(issue_for_task.clone(), attempt, config)
                .await;
            let mut retries = lock_unpoisoned(&runtime.scheduled_retries);
            retries.remove(&issue_for_task);
        });

        let mut retries = lock_unpoisoned(&self.scheduled_retries);
        retries.insert(issue_id, handle);
    }

    async fn promote_retry_and_spawn(
        self: &Arc<Self>,
        issue_id: IssueId,
        attempt: u32,
        config: RuntimeConfig,
    ) {
        let mut state_guard = self.state.lock().await;
        let mut state = state_guard.clone();
        let mut commands = Vec::new();

        let next_state =
            match apply_event(state, Event::MarkRunning(issue_id.clone()), &mut commands) {
                Ok(next_state) => next_state,
                Err(error) => {
                    tracing::warn!(issue_id = %issue_id.0, "retry promotion rejected: {error}");
                    return;
                }
            };

        state = next_state;
        *state_guard = state.clone();
        drop(state_guard);

        let _ = self.snapshot_tx.send(RuntimeSnapshot {
            running: state.running.len(),
            retrying: state.retry_attempts.len(),
        });

        if commands.iter().any(
            |command| matches!(command, Command::Dispatch(dispatched) if *dispatched == issue_id),
        ) {
            self.spawn_worker_task(issue_id, attempt, config);
        }
    }

    fn cancel_retry_task(&self, issue_id: &IssueId) {
        let mut retries = lock_unpoisoned(&self.scheduled_retries);
        if let Some(handle) = retries.remove(issue_id) {
            handle.abort();
        }
    }

    fn abort_worker_task(&self, issue_id: &IssueId) {
        let mut workers = lock_unpoisoned(&self.active_workers);
        if let Some(handle) = workers.remove(issue_id) {
            handle.abort();
        }
    }

    fn abort_background_tasks(&self) {
        let mut workers = lock_unpoisoned(&self.active_workers);
        for (_, handle) in workers.drain() {
            handle.abort();
        }
        drop(workers);

        let mut retries = lock_unpoisoned(&self.scheduled_retries);
        for (_, handle) in retries.drain() {
            handle.abort();
        }

        let mut pending_cleanup = lock_unpoisoned(&self.pending_terminal_cleanup);
        pending_cleanup.clear();
    }
}

fn lock_unpoisoned<T>(mutex: &StdMutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn map_worker_outcome(outcome: WorkerOutcome) -> WorkerExit {
    match outcome {
        WorkerOutcome::Completed => WorkerExit::Completed,
        WorkerOutcome::Continuation => WorkerExit::Continuation,
        WorkerOutcome::RetryableFailure => WorkerExit::RetryableFailure,
        WorkerOutcome::PermanentFailure => WorkerExit::PermanentFailure,
    }
}

fn retry_policy_for_config(config: &RuntimeConfig) -> RetryPolicy {
    let poll_ms = config.polling.interval_ms.max(10);
    RetryPolicy {
        continuation_delay: Duration::from_millis(poll_ms),
        failure_base_delay: Duration::from_millis(poll_ms),
        max_failure_backoff: Duration::from_millis(config.agent.max_retry_backoff_ms.max(poll_ms)),
    }
}

fn workspace_hooks_from_config(config: &RuntimeConfig) -> WorkspaceHooks {
    WorkspaceHooks {
        after_create: config.hooks.after_create.clone(),
        before_run: config.hooks.before_run.clone(),
        after_run: config.hooks.after_run.clone(),
        before_remove: config.hooks.before_remove.clone(),
        timeout_ms: config.hooks.timeout_ms,
        output_limit_bytes: symphony_workspace::DEFAULT_HOOK_OUTPUT_LIMIT_BYTES,
    }
}

pub fn non_active_reconciliation_ids(
    state: &OrchestratorState,
    active_ids: &HashSet<IssueId>,
) -> Vec<IssueId> {
    let mut stale: Vec<IssueId> = state
        .claimed
        .iter()
        .filter(|issue_id| {
            !state.running.contains_key(*issue_id)
                && !state.retry_attempts.contains_key(*issue_id)
                && !active_ids.contains(*issue_id)
        })
        .cloned()
        .collect();
    sort_issue_ids(&mut stale);
    stale
}

pub fn terminal_reconciliation_ids(
    state: &OrchestratorState,
    terminal_ids: &HashSet<IssueId>,
) -> Vec<IssueId> {
    let mut releases = Vec::new();
    releases.extend(
        state
            .claimed
            .iter()
            .filter(|issue_id| terminal_ids.contains(*issue_id))
            .cloned(),
    );
    releases.extend(
        state
            .running
            .keys()
            .filter(|issue_id| terminal_ids.contains(*issue_id))
            .cloned(),
    );
    releases.extend(
        state
            .retry_attempts
            .keys()
            .filter(|issue_id| terminal_ids.contains(*issue_id))
            .cloned(),
    );
    sort_issue_ids(&mut releases);
    releases
}

fn sort_issue_ids(issue_ids: &mut Vec<IssueId>) {
    issue_ids.sort_by(|left, right| left.0.cmp(&right.0));
    issue_ids.dedup();
}

fn tracker_states_from_names(states: &[String]) -> Vec<TrackerState> {
    states
        .iter()
        .map(|value| TrackerState::new(value.clone()))
        .collect()
}

fn tracker_state_matches_any(state: &TrackerState, allowed: &[TrackerState]) -> bool {
    let Some(state_key) = state.normalized_key() else {
        return false;
    };

    allowed
        .iter()
        .filter_map(TrackerState::normalized_key)
        .any(|candidate| candidate == state_key)
}

fn dispatch_state_allows_issue(
    issue: &TrackerIssue,
    active_states: &[TrackerState],
    terminal_states: &[TrackerState],
) -> bool {
    tracker_state_matches_any(&issue.state, active_states)
        && !issue.state.is_terminal(terminal_states)
        && !issue.id.0.trim().is_empty()
        && !issue.identifier.trim().is_empty()
        && !issue.title.trim().is_empty()
}

fn todo_issue_blocked_by_non_terminal(
    issue: &TrackerIssue,
    terminal_states: &[TrackerState],
) -> bool {
    if issue.state.normalized_key().as_deref() != Some("todo") {
        return false;
    }

    issue.blocked_by.iter().any(|blocker| {
        blocker.state.as_ref().is_none_or(|state_name| {
            !TrackerState::new(state_name.clone()).is_terminal(terminal_states)
        })
    })
}

fn next_retry_attempt(state: &OrchestratorState, issue_id: &IssueId) -> u32 {
    state
        .retry_attempts
        .get(issue_id)
        .map_or(1, |retry_entry| retry_entry.attempt.saturating_add(1))
}

fn duration_from_millis_u128(millis: u128) -> Duration {
    Duration::from_millis(millis.min(u128::from(u64::MAX)) as u64)
}

fn apply_event(
    state: OrchestratorState,
    event: Event,
    commands: &mut Vec<Command>,
) -> Result<OrchestratorState, InvariantError> {
    let (next_state, mut emitted) = reduce(state, event);
    validate_invariants(&next_state)?;

    // Check for rejected transitions and convert to error
    for cmd in &emitted {
        if let Command::TransitionRejected { reason, .. } = cmd {
            return Err(InvariantError::from(reason.clone()));
        }
    }

    commands.append(&mut emitted);
    Ok(next_state)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use async_trait::async_trait;
    use symphony_domain::{Command, IssueId, validate_invariants};
    use symphony_tracker::{
        TrackerBlockerRef, TrackerClient, TrackerError, TrackerIssue, TrackerState,
    };
    use tokio::sync::Mutex;

    use super::*;
    use crate::worker::{WorkerError, WorkerOutcome};

    #[derive(Default)]
    struct MutableTracker {
        issues: Mutex<Vec<TrackerIssue>>,
    }

    impl MutableTracker {
        async fn set_candidates(&self, issues: Vec<TrackerIssue>) {
            let mut guard = self.issues.lock().await;
            *guard = issues;
        }
    }

    #[async_trait]
    impl TrackerClient for MutableTracker {
        async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
            Ok(self.issues.lock().await.clone())
        }
    }

    #[derive(Default)]
    struct MockWorkerLauncher {
        outcomes: Mutex<VecDeque<WorkerOutcome>>,
        launch_count: AtomicUsize,
    }

    impl MockWorkerLauncher {
        fn with_outcomes(outcomes: Vec<WorkerOutcome>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes.into()),
                launch_count: AtomicUsize::new(0),
            }
        }

        fn launch_count(&self) -> usize {
            self.launch_count.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl WorkerLauncher for MockWorkerLauncher {
        async fn launch(&self, _ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            self.launch_count.fetch_add(1, Ordering::Relaxed);
            let mut outcomes = self.outcomes.lock().await;
            Ok(outcomes.pop_front().unwrap_or(WorkerOutcome::Completed))
        }
    }

    fn issue(id: &str) -> IssueId {
        IssueId(id.to_owned())
    }

    fn tracker_issue(id: &str) -> TrackerIssue {
        TrackerIssue {
            id: issue(id),
            identifier: id.to_owned(),
            title: format!("Issue {id}"),
            description: None,
            priority: None,
            state: TrackerState::new("Todo"),
            branch_name: None,
            url: None,
            labels: Vec::new(),
            blocked_by: Vec::new(),
            created_at: None,
            updated_at: None,
        }
    }

    #[tokio::test]
    async fn runtime_reports_zero_when_no_candidates() {
        let tracker = Arc::new(MutableTracker::default());
        let runtime = Runtime::new(tracker);
        let snapshot = runtime.snapshot().await;
        assert_eq!(snapshot.running, 0);
        assert_eq!(snapshot.retrying, 0);
    }

    #[tokio::test]
    async fn scheduler_never_runs_without_claim() {
        let issue_id = issue("SYM-100");
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![tracker_issue(&issue_id.0)])
            .await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        let mut config = RuntimeConfig::default();
        config.agent.max_concurrent_agents = 1;

        let first_tick = runtime
            .run_tick(&config)
            .await
            .expect("dispatch tick should succeed");
        assert_eq!(
            first_tick.commands,
            vec![Command::Dispatch(issue_id.clone())],
        );

        let state = runtime.state().await;
        assert!(state.claimed.contains(&issue_id));
        assert!(state.running.contains_key(&issue_id));
        assert_eq!(validate_invariants(&state), Ok(()));

        let second_tick = runtime
            .run_tick(&config)
            .await
            .expect("already running issue should not be re-dispatched");
        assert!(second_tick.commands.is_empty());
    }

    #[tokio::test]
    async fn scheduler_releases_stale_claims_in_sorted_order() {
        let tracker = Arc::new(MutableTracker::default());
        let running_issue = issue("SYM-200");
        tracker
            .set_candidates(vec![tracker_issue(&running_issue.0)])
            .await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue("SYM-20"));
            state.claimed.insert(issue("SYM-10"));
            state.claimed.insert(running_issue.clone());
            state.running.insert(
                running_issue.clone(),
                symphony_domain::RunningEntry::default(),
            );
        }

        let mut config = RuntimeConfig::default();
        config.agent.max_concurrent_agents = 3;

        let tick = runtime
            .run_tick(&config)
            .await
            .expect("release tick should succeed");
        assert_eq!(
            tick.commands,
            vec![
                Command::ReleaseClaim(issue("SYM-10")),
                Command::ReleaseClaim(issue("SYM-20")),
            ],
        );

        let state = runtime.state().await;
        assert!(state.claimed.contains(&running_issue));
        assert!(state.running.contains_key(&running_issue));
        assert_eq!(validate_invariants(&state), Ok(()));
    }

    #[test]
    fn retry_policy_continuation_schedule_uses_constant_delay() {
        let policy = RetryPolicy {
            continuation_delay: Duration::from_secs(7),
            failure_base_delay: Duration::from_secs(2),
            max_failure_backoff: Duration::from_secs(60),
        };
        let schedule = policy.schedule_retry(issue("SYM-13"), 3, RetryKind::Continuation);

        assert_eq!(schedule.attempt, 3);
        assert_eq!(schedule.kind, RetryKind::Continuation);
        assert_eq!(schedule.delay, Duration::from_secs(7));
    }

    #[test]
    fn retry_policy_failure_schedule_uses_exponential_backoff_with_cap() {
        let policy = RetryPolicy {
            continuation_delay: Duration::from_millis(10),
            failure_base_delay: Duration::from_secs(2),
            max_failure_backoff: Duration::from_secs(10),
        };

        let uncapped = policy.schedule_retry(issue("SYM-14"), 2, RetryKind::Failure);
        let capped = policy.schedule_retry(issue("SYM-14"), 8, RetryKind::Failure);

        assert_eq!(uncapped.delay, Duration::from_secs(4));
        assert_eq!(capped.delay, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn worker_exit_retryable_failure_queues_retry_and_schedules_backoff() {
        let issue_id = issue("SYM-301");
        let tracker = Arc::new(MutableTracker::default());
        let runtime = Runtime::new(Arc::clone(&tracker));
        let policy = RetryPolicy {
            continuation_delay: Duration::from_secs(3),
            failure_base_delay: Duration::from_secs(2),
            max_failure_backoff: Duration::from_secs(20),
        };

        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
            state
                .running
                .insert(issue_id.clone(), symphony_domain::RunningEntry::default());
        }

        let result = runtime
            .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
            .await
            .expect("worker exit mapping should succeed");

        assert_eq!(
            result.commands,
            vec![Command::ScheduleRetry {
                issue_id: issue_id.clone(),
                attempt: 1,
            }],
        );
        assert_eq!(
            result.retry_schedule,
            Some(RetrySchedule {
                issue_id: issue_id.clone(),
                attempt: 1,
                kind: RetryKind::Failure,
                delay: Duration::from_secs(2),
            }),
        );
        assert!(!result.state.running.contains_key(&issue_id));
        assert_eq!(
            result
                .state
                .retry_attempts
                .get(&issue_id)
                .map(|entry| entry.attempt),
            Some(1)
        );
    }

    #[tokio::test]
    async fn worker_exit_continuation_increments_retry_attempt() {
        let issue_id = issue("SYM-302");
        let tracker = Arc::new(MutableTracker::default());
        let runtime = Runtime::new(Arc::clone(&tracker));
        let policy = RetryPolicy::default();

        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
            state
                .running
                .insert(issue_id.clone(), symphony_domain::RunningEntry::default());
            state
                .retry_attempts
                .insert(issue_id.clone(), symphony_domain::RetryEntry { attempt: 2 });
        }

        let result = runtime
            .handle_worker_exit(issue_id.clone(), WorkerExit::Continuation, &policy)
            .await
            .expect("continuation path should succeed");

        assert_eq!(
            result.commands,
            vec![Command::ScheduleRetry {
                issue_id: issue_id.clone(),
                attempt: 3,
            }],
        );
        assert_eq!(
            result.retry_schedule,
            Some(RetrySchedule {
                issue_id: issue_id.clone(),
                attempt: 3,
                kind: RetryKind::Continuation,
                delay: policy.continuation_delay,
            }),
        );
    }

    #[tokio::test]
    async fn worker_exit_completed_releases_claim() {
        let issue_id = issue("SYM-303");
        let tracker = Arc::new(MutableTracker::default());
        let runtime = Runtime::new(Arc::clone(&tracker));
        let policy = RetryPolicy::default();

        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
            state
                .running
                .insert(issue_id.clone(), symphony_domain::RunningEntry::default());
        }

        let result = runtime
            .handle_worker_exit(issue_id.clone(), WorkerExit::Completed, &policy)
            .await
            .expect("completion should release");

        assert_eq!(result.retry_schedule, None);
        assert_eq!(
            result.commands,
            vec![Command::ReleaseClaim(issue_id.clone())]
        );
        assert!(!result.state.claimed.contains(&issue_id));
        assert!(!result.state.running.contains_key(&issue_id));
    }

    #[test]
    fn reconciliation_helpers_handle_terminal_and_non_active_sets() {
        let issue_terminal = issue("SYM-401");
        let issue_non_active = issue("SYM-402");
        let issue_active = issue("SYM-403");

        let mut state = OrchestratorState::default();
        state.claimed.insert(issue_non_active.clone());
        state.claimed.insert(issue_terminal.clone());
        state.claimed.insert(issue_active.clone());
        state.running.insert(
            issue_active.clone(),
            symphony_domain::RunningEntry::default(),
        );

        let terminal = HashSet::from([issue_terminal.clone(), issue("SYM-999")]);
        let active = HashSet::from([issue_active.clone()]);

        assert_eq!(
            terminal_reconciliation_ids(&state, &terminal),
            vec![issue_terminal.clone()],
        );
        assert_eq!(
            non_active_reconciliation_ids(&state, &active),
            vec![issue_terminal, issue_non_active],
        );
    }

    #[tokio::test]
    async fn run_tick_dispatch_order_is_deterministic() {
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![
                tracker_issue("SYM-9"),
                tracker_issue("SYM-1"),
                tracker_issue("SYM-3"),
            ])
            .await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        let mut config = RuntimeConfig::default();
        config.agent.max_concurrent_agents = 3;

        let tick = runtime
            .run_tick(&config)
            .await
            .expect("dispatch tick should succeed");
        assert_eq!(
            tick.commands,
            vec![
                Command::Dispatch(issue("SYM-1")),
                Command::Dispatch(issue("SYM-3")),
                Command::Dispatch(issue("SYM-9")),
            ],
        );
    }

    #[tokio::test]
    async fn run_tick_enforces_per_state_concurrency() {
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![
                tracker_issue("SYM-1"), // Todo
                tracker_issue("SYM-2"), // Todo
                tracker_issue("SYM-3"), // Todo
                TrackerIssue {
                    id: issue("SYM-4"),
                    identifier: "SYM-4".to_owned(),
                    title: "Issue SYM-4".to_owned(),
                    description: None,
                    priority: None,
                    state: TrackerState::new("In Progress"),
                    branch_name: None,
                    url: None,
                    labels: Vec::new(),
                    blocked_by: Vec::new(),
                    created_at: None,
                    updated_at: None,
                },
                TrackerIssue {
                    id: issue("SYM-5"),
                    identifier: "SYM-5".to_owned(),
                    title: "Issue SYM-5".to_owned(),
                    description: None,
                    priority: None,
                    state: TrackerState::new("In Progress"),
                    branch_name: None,
                    url: None,
                    labels: Vec::new(),
                    blocked_by: Vec::new(),
                    created_at: None,
                    updated_at: None,
                },
            ])
            .await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        let mut config = RuntimeConfig::default();
        config.agent.max_concurrent_agents = 4;
        config
            .agent
            .max_concurrent_agents_by_state
            .insert("todo".to_owned(), 2);
        config
            .agent
            .max_concurrent_agents_by_state
            .insert("in progress".to_owned(), 1);

        let tick = runtime
            .run_tick(&config)
            .await
            .expect("dispatch tick should succeed");

        // Should dispatch 2 Todo and 1 In Progress
        assert_eq!(tick.commands.len(), 3);
        assert!(tick.commands.contains(&Command::Dispatch(issue("SYM-1"))));
        assert!(tick.commands.contains(&Command::Dispatch(issue("SYM-2"))));
        assert!(tick.commands.contains(&Command::Dispatch(issue("SYM-4"))));
    }

    #[tokio::test]
    async fn run_tick_stalls_and_queues_retry() {
        let issue_id = issue("SYM-99");
        let tracker = Arc::new(MutableTracker::default());
        tracker.set_candidates(vec![tracker_issue("SYM-99")]).await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        // Mark it as running and explicitly set its last seen to the past
        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
            state
                .running
                .insert(issue_id.clone(), symphony_domain::RunningEntry::default());
            let mut last_seen = runtime.last_seen.lock().await;
            last_seen.insert(
                issue_id.clone(),
                std::time::Instant::now() - Duration::from_secs(400),
            );
        }

        let mut config = RuntimeConfig::default();
        config.codex.stall_timeout_ms = 300_000; // 5 minutes

        let tick = runtime
            .run_tick(&config)
            .await
            .expect("dispatch tick should succeed");

        assert_eq!(
            tick.commands,
            vec![Command::ScheduleRetry {
                issue_id: issue_id.clone(),
                attempt: 1,
            }]
        );

        let state = runtime.state().await;
        assert!(!state.running.contains_key(&issue_id));
        assert!(state.retry_attempts.contains_key(&issue_id));
    }

    #[tokio::test]
    async fn run_tick_skips_todo_issues_blocked_by_non_terminal_issues() {
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![
                TrackerIssue::new(issue("SYM-1"), "SYM-1", TrackerState::new("Todo"))
                    .with_title("Blocked todo")
                    .with_blocked_by(vec![TrackerBlockerRef {
                        id: Some("SYM-9".to_owned()),
                        identifier: Some("SYM-9".to_owned()),
                        state: Some("In Progress".to_owned()),
                    }]),
                TrackerIssue::new(issue("SYM-2"), "SYM-2", TrackerState::new("Todo"))
                    .with_title("Ready todo"),
            ])
            .await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        let mut config = RuntimeConfig::default();
        config.agent.max_concurrent_agents = 2;

        let tick = runtime
            .run_tick(&config)
            .await
            .expect("dispatch tick should succeed");

        assert_eq!(tick.commands, vec![Command::Dispatch(issue("SYM-2"))]);
    }

    #[tokio::test]
    async fn run_tick_allows_todo_issues_when_all_blockers_are_terminal() {
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![
                TrackerIssue::new(issue("SYM-1"), "SYM-1", TrackerState::new("Todo"))
                    .with_title("Terminally blocked")
                    .with_blocked_by(vec![TrackerBlockerRef {
                        id: Some("SYM-9".to_owned()),
                        identifier: Some("SYM-9".to_owned()),
                        state: Some("Done".to_owned()),
                    }]),
            ])
            .await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        let mut config = RuntimeConfig::default();
        config.agent.max_concurrent_agents = 1;

        let tick = runtime
            .run_tick(&config)
            .await
            .expect("dispatch tick should succeed");

        assert_eq!(tick.commands, vec![Command::Dispatch(issue("SYM-1"))]);
    }

    #[tokio::test]
    async fn dispatch_command_launches_worker_and_releases_claim_on_completion() {
        let issue_id = issue("SYM-600");
        let tracker = Arc::new(MutableTracker::default());
        tracker.set_candidates(vec![tracker_issue("SYM-600")]).await;
        let launcher = Arc::new(MockWorkerLauncher::with_outcomes(vec![
            WorkerOutcome::Completed,
        ]));
        let runtime = Arc::new(Runtime::with_worker_launcher(
            Arc::clone(&tracker),
            launcher.clone(),
        ));

        let mut config = RuntimeConfig::default();
        config.polling.interval_ms = 10;
        config.codex.command = "true".to_owned();

        let tick = runtime
            .run_tick(&config)
            .await
            .expect("tick should succeed");
        runtime.process_tick_commands(tick.commands, config).await;
        tokio::time::sleep(Duration::from_millis(25)).await;

        let state = runtime.state().await;
        assert!(!state.claimed.contains(&issue_id));
        assert!(!state.running.contains_key(&issue_id));
        assert_eq!(launcher.launch_count(), 1);
    }

    #[tokio::test]
    async fn retryable_failure_schedules_retry_and_relaunches_worker() {
        let issue_id = issue("SYM-601");
        let tracker = Arc::new(MutableTracker::default());
        tracker.set_candidates(vec![tracker_issue("SYM-601")]).await;
        let launcher = Arc::new(MockWorkerLauncher::with_outcomes(vec![
            WorkerOutcome::RetryableFailure,
            WorkerOutcome::Completed,
        ]));
        let runtime = Arc::new(Runtime::with_worker_launcher(
            Arc::clone(&tracker),
            launcher.clone(),
        ));

        let mut config = RuntimeConfig::default();
        config.polling.interval_ms = 10;
        config.agent.max_retry_backoff_ms = 20;
        config.codex.command = "true".to_owned();

        let tick = runtime
            .run_tick(&config)
            .await
            .expect("tick should succeed");
        runtime
            .process_tick_commands(tick.commands, config.clone())
            .await;

        tokio::time::sleep(Duration::from_millis(80)).await;

        let state = runtime.state().await;
        assert!(!state.claimed.contains(&issue_id));
        assert!(!state.running.contains_key(&issue_id));
        assert!(!state.retry_attempts.contains_key(&issue_id));
        assert_eq!(launcher.launch_count(), 2);
    }

    #[test]
    fn runtime_config_version_increments() {
        let config = RuntimeConfig::default();
        assert_eq!(config.version, 0);

        let incremented = config.increment_version();
        assert_eq!(incremented.version, 1);
        assert_eq!(config.version, 0); // Original unchanged
    }

    #[tokio::test]
    async fn handle_protocol_update_updates_running_entry() {
        let issue_id = issue("SYM-500");
        let tracker = Arc::new(MutableTracker::default());
        let runtime = Runtime::new(Arc::clone(&tracker));

        // First, mark the issue as running
        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
            state
                .running
                .insert(issue_id.clone(), symphony_domain::RunningEntry::default());
        }

        // Then update with protocol information
        let usage = symphony_domain::Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
        };

        runtime
            .handle_protocol_update(
                issue_id.clone(),
                Some("session-123".to_owned()),
                Some("thread-456".to_owned()),
                Some("turn-789".to_owned()),
                Some(12345),
                Some("turn.start".to_owned()),
                Some(1700000000),
                Some("Processing issue".to_owned()),
                Some(usage.clone()),
            )
            .await
            .expect("protocol update should succeed");

        let state = runtime.state().await;
        let entry = state
            .running
            .get(&issue_id)
            .expect("issue should be running");
        assert_eq!(entry.session_id, Some("session-123".to_owned()));
        assert_eq!(entry.thread_id, Some("thread-456".to_owned()));
        assert_eq!(entry.turn_id, Some("turn-789".to_owned()));
        assert_eq!(entry.codex_app_server_pid, Some(12345));
        assert_eq!(entry.last_codex_event, Some("turn.start".to_owned()));
        assert_eq!(entry.last_codex_timestamp, Some(1700000000));
        assert_eq!(
            entry.last_codex_message,
            Some("Processing issue".to_owned())
        );
        assert_eq!(entry.usage, usage);

        // Check global totals updated
        assert_eq!(state.codex_totals.input_tokens, 100);
        assert_eq!(state.codex_totals.output_tokens, 50);
        assert_eq!(state.codex_totals.total_tokens, 150);
    }

    #[tokio::test]
    async fn handle_protocol_update_rejects_non_running_issue() {
        let issue_id = issue("SYM-501");
        let tracker = Arc::new(MutableTracker::default());
        let runtime = Runtime::new(Arc::clone(&tracker));

        // Don't mark as running, just try to update
        let result = runtime
            .handle_protocol_update(
                issue_id.clone(),
                Some("session-123".to_owned()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await;

        // Should fail with MissingClaim (which is the invariant error for non-running issues)
        assert!(result.is_err());
    }
}
