#![forbid(unsafe_code)]

pub mod worker;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use symphony_config::RuntimeConfig;
use symphony_domain::{
    AgentUpdate, CodexTotals, Command, Event, InvariantError, IssueId, OrchestratorState, reduce,
    validate_invariants,
};
use symphony_observability::{
    IssueSnapshot, RuntimeActivitySnapshot, RuntimeSnapshot, StateSnapshot, ThroughputSnapshot,
};
use symphony_tracker::{TrackerClient, TrackerError, TrackerIssue, TrackerState};
use symphony_workspace::{WorkspaceHooks, remove_workspace, remove_workspace_with_hooks};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use worker::{
    ShellHookExecutor, WorkerContext, WorkerExecutionConfig, WorkerLauncher, WorkerOutcome,
    WorkerProtocolUpdate, WorkerRuntimeHandles,
};

const DEFAULT_PROMPT_TEMPLATE: &str =
    "Issue {{issue_identifier}}: {{issue_title}}\n{{issue_description}}";
const THROUGHPUT_WINDOW_SECONDS: u64 = 5;
static NEXT_WORKER_ID: AtomicU64 = AtomicU64::new(1);

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
            continuation_delay: Duration::from_secs(1),
            failure_base_delay: Duration::from_secs(10),
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

    /// Calculate exponential backoff delay for failure retries.
    ///
    /// Uses capped exponential growth: delay = base * 2^(attempt-1), capped at max_failure_backoff.
    ///
    /// # Overflow Safety
    /// The shift amount is capped at 127 (via `.min(127)`) to prevent overflow in `1_u128 << shift`.
    /// This allows up to 128 bits of shift, which is the maximum for u128. For any attempt >= 128,
    /// the multiplier becomes 2^127, which when multiplied by any reasonable base delay will hit
    /// the max_failure_backoff cap immediately.
    fn failure_backoff(&self, attempt: u32) -> Duration {
        let normalized_attempt = attempt.max(1);
        // Cap shift at 127 to prevent overflow: 1_u128 << 127 is the largest safe shift.
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
    retry_metadata: StdMutex<HashMap<IssueId, RetryMetadata>>,
    running_attempts: StdMutex<HashMap<IssueId, u32>>,
    pending_dispatch_attempts: StdMutex<HashMap<IssueId, u32>>,
    active_workers: StdMutex<HashMap<IssueId, ActiveWorkerHandle>>,
    scheduled_retries: StdMutex<HashMap<IssueId, JoinHandle<()>>>,
    pending_terminal_cleanup: StdMutex<HashMap<IssueId, String>>,
    telemetry: StdMutex<RuntimeTelemetry>,
    prompt_template: RwLock<String>,
    snapshot_tx: tokio::sync::watch::Sender<RuntimeSnapshot>,
}

struct ActiveWorkerHandle {
    worker_id: u64,
    join: JoinHandle<()>,
    stop_tx: tokio::sync::watch::Sender<bool>,
}

#[derive(Clone, Debug, Default)]
struct RetryMetadata {
    identifier: Option<String>,
    due_at: Option<u64>,
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct StalledIssue {
    issue_id: IssueId,
    attempt: u32,
    identifier: String,
    error: String,
    elapsed_ms: u64,
}

#[derive(Clone, Debug, Default)]
struct RuntimeTelemetry {
    poll_in_progress: bool,
    last_poll_started_at: Option<u64>,
    last_poll_completed_at: Option<u64>,
    next_poll_due_at: Option<u64>,
    last_runtime_activity_at: Option<u64>,
    token_samples: VecDeque<TokenSample>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TokenSample {
    at: u64,
    totals: CodexTotals,
}

impl<T: TrackerClient + 'static> Runtime<T> {
    pub fn new(tracker: Arc<T>) -> Self {
        Self::with_worker_launcher(tracker, Arc::new(worker::ShellWorkerLauncher))
    }

    pub fn with_worker_launcher(tracker: Arc<T>, worker_launcher: Arc<dyn WorkerLauncher>) -> Self {
        let (snapshot_tx, _) = tokio::sync::watch::channel(RuntimeSnapshot {
            running: 0,
            retrying: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            seconds_running: 0.0,
            rate_limits: None,
            activity: RuntimeActivitySnapshot::default(),
        });
        Self {
            tracker,
            worker_launcher,
            state: Mutex::new(OrchestratorState::default()),
            retry_metadata: StdMutex::new(HashMap::new()),
            running_attempts: StdMutex::new(HashMap::new()),
            pending_dispatch_attempts: StdMutex::new(HashMap::new()),
            active_workers: StdMutex::new(HashMap::new()),
            scheduled_retries: StdMutex::new(HashMap::new()),
            pending_terminal_cleanup: StdMutex::new(HashMap::new()),
            telemetry: StdMutex::new(RuntimeTelemetry::default()),
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

    fn publish_snapshot(&self, state: &OrchestratorState) {
        let _ = self.snapshot_tx.send(self.snapshot_from_state(state));
    }

    fn record_protocol_metrics(&self, state: &OrchestratorState, timestamp_secs: u64) {
        let mut telemetry = lock_unpoisoned(&self.telemetry);
        telemetry.last_runtime_activity_at = Some(timestamp_secs);
        record_token_sample(
            &mut telemetry.token_samples,
            timestamp_secs,
            &state.codex_totals,
        );
    }

    fn mark_poll_started(&self, timestamp_secs: u64) {
        let mut telemetry = lock_unpoisoned(&self.telemetry);
        telemetry.poll_in_progress = true;
        telemetry.last_poll_started_at = Some(timestamp_secs);
        telemetry.last_runtime_activity_at = Some(timestamp_secs);
        telemetry.next_poll_due_at = None;
    }

    fn mark_poll_completed(&self, timestamp_secs: u64, next_interval_ms: u64) {
        let mut telemetry = lock_unpoisoned(&self.telemetry);
        telemetry.poll_in_progress = false;
        telemetry.last_poll_completed_at = Some(timestamp_secs);
        telemetry.last_runtime_activity_at = Some(timestamp_secs);
        telemetry.next_poll_due_at =
            Some(timestamp_secs.saturating_add(duration_to_rounded_up_seconds(next_interval_ms)));
    }

    fn mark_next_poll_due(&self, timestamp_secs: u64) {
        let mut telemetry = lock_unpoisoned(&self.telemetry);
        telemetry.next_poll_due_at = Some(timestamp_secs);
    }

    fn remember_running_attempt(&self, issue_id: IssueId, attempt: u32) {
        let mut running_attempts = lock_unpoisoned(&self.running_attempts);
        running_attempts.insert(issue_id, attempt);
    }

    fn take_running_attempt(&self, issue_id: &IssueId) -> Option<u32> {
        let mut running_attempts = lock_unpoisoned(&self.running_attempts);
        running_attempts.remove(issue_id)
    }

    fn running_attempt(&self, issue_id: &IssueId) -> Option<u32> {
        let running_attempts = lock_unpoisoned(&self.running_attempts);
        running_attempts.get(issue_id).copied()
    }

    fn set_pending_dispatch_attempt(&self, issue_id: IssueId, attempt: u32) {
        let mut pending_dispatch_attempts = lock_unpoisoned(&self.pending_dispatch_attempts);
        pending_dispatch_attempts.insert(issue_id, attempt.max(1));
    }

    fn take_pending_dispatch_attempt(&self, issue_id: &IssueId) -> Option<u32> {
        let mut pending_dispatch_attempts = lock_unpoisoned(&self.pending_dispatch_attempts);
        pending_dispatch_attempts.remove(issue_id)
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
        let now_secs = current_unix_timestamp_secs();
        let stall_timeout_ms = config.codex.stall_timeout_ms.max(0) as u64;
        let stalled_issues = if stall_timeout_ms == 0 {
            Vec::new()
        } else {
            state
                .running
                .iter()
                .filter_map(|(issue_id, entry)| {
                    let elapsed_ms = stall_elapsed_ms(entry, now_secs)?;
                    if elapsed_ms <= stall_timeout_ms {
                        return None;
                    }

                    Some(StalledIssue {
                        issue_id: issue_id.clone(),
                        attempt: next_running_retry_attempt(
                            &state,
                            self.running_attempt(issue_id),
                            issue_id,
                        ),
                        identifier: entry
                            .identifier
                            .clone()
                            .unwrap_or_else(|| issue_id.0.clone()),
                        error: format!("stalled for {elapsed_ms}ms without codex activity"),
                        elapsed_ms,
                    })
                })
                .collect::<Vec<_>>()
        };
        for stalled in &stalled_issues {
            state = apply_event(
                state,
                Event::QueueRetry {
                    issue_id: stalled.issue_id.clone(),
                    attempt: stalled.attempt,
                },
                &mut commands,
            )?;
            tracing::warn!(
                issue_id = %stalled.issue_id.0,
                issue_identifier = %stalled.identifier,
                elapsed_ms = stalled.elapsed_ms,
                "worker stalled; scheduling retry"
            );
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

                        if let Some(entry) = state.running.get_mut(&issue_id) {
                            entry.tracker_state = Some(tracker_state.as_str().to_owned());
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

        let mut running_by_state = running_state_counts(&state, &issues);

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
        drop(state_guard);

        if !stalled_issues.is_empty() {
            let mut retry_metadata = lock_unpoisoned(&self.retry_metadata);
            for stalled in &stalled_issues {
                retry_metadata.insert(
                    stalled.issue_id.clone(),
                    RetryMetadata {
                        identifier: Some(stalled.identifier.clone()),
                        due_at: None,
                        error: Some(stalled.error.clone()),
                    },
                );
                let _ = self.take_running_attempt(&stalled.issue_id);
            }
        }

        self.publish_snapshot(&state);

        Ok(TickResult { state, commands })
    }

    pub async fn handle_worker_exit(
        &self,
        issue_id: IssueId,
        worker_exit: WorkerExit,
        retry_policy: &RetryPolicy,
    ) -> Result<WorkerExitResult, RuntimeError> {
        let running_attempt = self.take_running_attempt(&issue_id);
        let mut state_guard = self.state.lock().await;
        let mut state = state_guard.clone();

        // Accumulate runtime for completed sessions (SPEC Section 13.5)
        if let Some(entry) = state.running.get(&issue_id)
            && let Some(started_at) = entry.started_at
        {
            let session_runtime = current_unix_timestamp_secs().saturating_sub(started_at) as f64;
            state.codex_totals.completed_seconds_running += session_runtime;
        }

        let mut commands = Vec::new();
        let mut retry_schedule = None;
        let mut next_retry_metadata = match worker_exit {
            WorkerExit::Completed | WorkerExit::PermanentFailure => None,
            WorkerExit::Continuation | WorkerExit::RetryableFailure => {
                Some(build_retry_metadata(&state, &issue_id))
            }
        };

        match worker_exit {
            WorkerExit::Completed | WorkerExit::PermanentFailure => {
                state = apply_event(state, Event::Release(issue_id.clone()), &mut commands)?;
            }
            WorkerExit::Continuation | WorkerExit::RetryableFailure => {
                let attempt = next_running_retry_attempt(&state, running_attempt, &issue_id);
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
                    if let Some(metadata) = next_retry_metadata.as_mut() {
                        metadata.due_at = Some(
                            current_unix_timestamp_secs()
                                .saturating_add(planned_schedule.delay.as_secs()),
                        );
                    }
                    retry_schedule = Some(planned_schedule);
                }
            }
        }

        validate_invariants(&state)?;
        *state_guard = state.clone();
        drop(state_guard);

        match next_retry_metadata {
            Some(metadata) => {
                let mut retry_metadata = lock_unpoisoned(&self.retry_metadata);
                retry_metadata.insert(issue_id.clone(), metadata);
            }
            None => {
                let mut retry_metadata = lock_unpoisoned(&self.retry_metadata);
                retry_metadata.remove(&issue_id);
            }
        }

        self.publish_snapshot(&state);

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

        self.publish_snapshot(&state);

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
        rate_limits: Option<serde_json::Value>,
    ) -> Result<(), RuntimeError> {
        let update_event = Event::UpdateAgent(Box::new(AgentUpdate {
            issue_id,
            session_id,
            thread_id,
            turn_id,
            pid,
            event,
            timestamp,
            message,
            usage,
            rate_limits,
        }));
        let activity_timestamp = timestamp.unwrap_or_else(current_unix_timestamp_secs);
        let mut state_guard = self.state.lock().await;
        let mut state = state_guard.clone();
        let mut commands = Vec::new();

        state = apply_event(state, update_event, &mut commands)?;
        *state_guard = state.clone();
        drop(state_guard);

        self.record_protocol_metrics(&state, activity_timestamp);
        self.publish_snapshot(&state);

        Ok(())
    }

    pub async fn snapshot(&self) -> RuntimeSnapshot {
        let state = self.state.lock().await;
        self.snapshot_from_state(&state)
    }

    pub async fn state(&self) -> OrchestratorState {
        self.state.lock().await.clone()
    }

    pub async fn state_snapshot(&self) -> StateSnapshot {
        let state = self.state.lock().await.clone();
        let retry_metadata = lock_unpoisoned(&self.retry_metadata).clone();
        StateSnapshot {
            runtime: self.snapshot_from_state(&state),
            issues: build_issue_snapshots(&state, &retry_metadata),
        }
    }

    /// Sets up an issue as claimed and running. For testing purposes.
    pub async fn setup_running(&self, issue_id: IssueId, entry: symphony_domain::RunningEntry) {
        let mut state = self.state.lock().await;
        state.claimed.insert(issue_id.clone());
        state.running.insert(issue_id, entry);
    }

    /// Sets up an issue as claimed and in retry queue. For testing purposes.
    pub async fn setup_retry(&self, issue_id: IssueId, entry: symphony_domain::RetryEntry) {
        let mut state = self.state.lock().await;
        state.claimed.insert(issue_id.clone());
        state.retry_attempts.insert(issue_id, entry);
    }

    fn snapshot_from_state(&self, state: &OrchestratorState) -> RuntimeSnapshot {
        let telemetry = lock_unpoisoned(&self.telemetry);
        RuntimeSnapshot {
            running: state.running.len(),
            retrying: state.retry_attempts.len(),
            input_tokens: state.codex_totals.input_tokens,
            output_tokens: state.codex_totals.output_tokens,
            total_tokens: state.codex_totals.total_tokens,
            seconds_running: aggregate_running_seconds(state),
            rate_limits: state.codex_rate_limits.clone(),
            activity: runtime_activity_snapshot(&telemetry),
        }
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
        self.mark_next_poll_due(current_unix_timestamp_secs());

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let new_config = config.read().await.clone();
                    self.mark_poll_started(current_unix_timestamp_secs());

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
                    self.mark_poll_completed(
                        current_unix_timestamp_secs(),
                        new_config.polling.interval_ms.max(100),
                    );
                    interval = tokio::time::interval(Duration::from_millis(new_config.polling.interval_ms.max(100)));
                    interval.reset();
                }
                Some(_) = refresh_rx.recv() => {
                    let new_config = config.read().await.clone();
                    self.mark_poll_started(current_unix_timestamp_secs());

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
                    self.mark_poll_completed(
                        current_unix_timestamp_secs(),
                        new_config.polling.interval_ms.max(100),
                    );
                    interval = tokio::time::interval(Duration::from_millis(new_config.polling.interval_ms.max(100)));
                    interval.reset();
                }
                _ = shutdown_rx.recv() => {
                    tracing::info!("Shutting down poll loop");
                    let shutdown_config = config.read().await.clone();
                    self.abort_background_tasks_with_cleanup(&shutdown_config);
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
                    let attempt = self.take_pending_dispatch_attempt(&issue_id).unwrap_or(0);
                    self.spawn_worker_task(issue_id, attempt, config.clone());
                }
                Command::ScheduleRetry { issue_id, attempt } => {
                    self.abort_worker_task(&issue_id);
                    let retry_policy = retry_policy_for_config(&config);
                    let schedule =
                        retry_policy.schedule_retry(issue_id, attempt, RetryKind::Failure);
                    self.schedule_retry_task(schedule, config.clone());
                }
                Command::ReleaseClaim(issue_id) => {
                    self.cancel_retry_task(&issue_id);
                    self.abort_worker_task(&issue_id);
                    self.take_running_attempt(&issue_id);
                    self.take_pending_dispatch_attempt(&issue_id);
                    let mut retry_metadata = lock_unpoisoned(&self.retry_metadata);
                    retry_metadata.remove(&issue_id);
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

        let worker_id = NEXT_WORKER_ID.fetch_add(1, Ordering::Relaxed);
        self.remember_running_attempt(issue_id.clone(), attempt);
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        let runtime = Arc::clone(self);
        let issue_for_task = issue_id.clone();
        let join = tokio::spawn(async move {
            runtime
                .execute_worker(issue_for_task.clone(), worker_id, attempt, config, stop_rx)
                .await;
            let mut workers = lock_unpoisoned(&runtime.active_workers);
            if workers
                .get(&issue_for_task)
                .is_some_and(|handle| handle.worker_id == worker_id)
            {
                workers.remove(&issue_for_task);
            }
        });

        let mut workers = lock_unpoisoned(&self.active_workers);
        workers.insert(
            issue_id,
            ActiveWorkerHandle {
                worker_id,
                join,
                stop_tx,
            },
        );
    }

    fn has_active_worker(&self, issue_id: &IssueId) -> bool {
        let workers = lock_unpoisoned(&self.active_workers);
        workers
            .get(issue_id)
            .is_some_and(|handle| !handle.join.is_finished())
    }

    async fn execute_worker(
        self: &Arc<Self>,
        issue_id: IssueId,
        worker_id: u64,
        attempt: u32,
        config: RuntimeConfig,
        stop_rx: tokio::sync::watch::Receiver<bool>,
    ) {
        let issue = match self.revalidate_issue_for_dispatch(&issue_id, &config).await {
            Ok(Some(issue)) => issue,
            Ok(None) => {
                tracing::info!(issue_id = %issue_id.0, "skipping stale dispatch; issue is no longer an active candidate");
                let retry_policy = retry_policy_for_config(&config);
                let _ = self
                    .handle_worker_exit(issue_id, WorkerExit::Completed, &retry_policy)
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
        self.record_running_issue_metadata(&issue_id, &issue).await;
        let prompt_template = self.prompt_template.read().await.clone();
        let active_states = tracker_states_from_names(&config.tracker.active_states);
        let (protocol_update_tx, mut protocol_update_rx) =
            tokio::sync::mpsc::unbounded_channel::<WorkerProtocolUpdate>();
        let runtime_for_updates = Arc::clone(self);
        let update_issue_id = issue_id.clone();
        let protocol_update_task = tokio::spawn(async move {
            while let Some(update) = protocol_update_rx.recv().await {
                let _ = runtime_for_updates
                    .handle_protocol_update(
                        update_issue_id.clone(),
                        update.session_id,
                        update.thread_id,
                        update.turn_id,
                        update.pid,
                        update.event,
                        update.timestamp,
                        update.message,
                        update.usage,
                        update.rate_limits,
                    )
                    .await;
            }
        });
        let tracker_client: Arc<dyn TrackerClient> = self.tracker.clone();
        let worker_context = match WorkerContext::new(
            issue,
            attempt,
            WorkerExecutionConfig {
                root: config.workspace.root.clone(),
                hooks,
                tracker_config: config.tracker.clone(),
                codex_config: config.codex.clone(),
                prompt_template,
            },
            WorkerRuntimeHandles {
                tracker_client,
                active_states,
                max_turns: config.agent.max_turns,
                protocol_updates: Some(protocol_update_tx),
                stop_rx,
            },
        ) {
            Ok(context) => context,
            Err(error) => {
                tracing::error!(issue_id = %issue_id.0, "invalid worker context: {error}");
                protocol_update_task.abort();
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
        let _ = protocol_update_task.await;
        {
            let mut workers = lock_unpoisoned(&self.active_workers);
            if workers
                .get(&issue_id)
                .is_some_and(|handle| handle.worker_id == worker_id)
            {
                workers.remove(&issue_id);
            }
        }

        if matches!(worker_outcome, WorkerOutcome::Stopped) {
            return;
        }

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

    async fn revalidate_issue_for_dispatch(
        &self,
        issue_id: &IssueId,
        config: &RuntimeConfig,
    ) -> Result<Option<TrackerIssue>, TrackerError> {
        let candidates = self.tracker.fetch_candidates().await?;
        let active_states = tracker_states_from_names(&config.tracker.active_states);
        let terminal_states = tracker_states_from_names(&config.tracker.terminal_states);
        Ok(candidates.into_iter().find(|issue| {
            issue.id == *issue_id
                && dispatch_state_allows_issue(issue, &active_states, &terminal_states)
                && !todo_issue_blocked_by_non_terminal(issue, &terminal_states)
        }))
    }

    async fn record_running_issue_metadata(&self, issue_id: &IssueId, issue: &TrackerIssue) {
        let mut state = self.state.lock().await;
        if let Some(entry) = state.running.get_mut(issue_id) {
            entry.identifier = Some(issue.identifier.clone());
            entry.tracker_state = Some(issue.state.as_str().to_owned());
            if entry.started_at.is_none() {
                entry.started_at = Some(current_unix_timestamp_secs());
            }
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
        {
            let mut retry_metadata = lock_unpoisoned(&self.retry_metadata);
            let metadata = retry_metadata.entry(schedule.issue_id.clone()).or_default();
            metadata.due_at =
                Some(current_unix_timestamp_secs().saturating_add(schedule.delay.as_secs()));
        }

        let runtime = Arc::clone(self);
        let issue_id = schedule.issue_id.clone();
        let attempt = schedule.attempt;
        let delay = schedule.delay;
        let issue_for_task = issue_id.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            {
                let mut retries = lock_unpoisoned(&runtime.scheduled_retries);
                retries.remove(&issue_for_task);
            }
            runtime
                .handle_ready_retry(issue_for_task.clone(), attempt, config)
                .await;
        });

        let mut retries = lock_unpoisoned(&self.scheduled_retries);
        retries.insert(issue_id, handle);
    }

    async fn handle_ready_retry(
        self: &Arc<Self>,
        issue_id: IssueId,
        attempt: u32,
        config: RuntimeConfig,
    ) {
        let issues = match self.tracker.fetch_candidates().await {
            Ok(issues) => issues,
            Err(error) => {
                tracing::warn!(issue_id = %issue_id.0, "retry candidate refresh failed: {error}");
                self.requeue_retry(issue_id, attempt.saturating_add(1), config.clone())
                    .await;
                return;
            }
        };

        let active_states = tracker_states_from_names(&config.tracker.active_states);
        let terminal_states = tracker_states_from_names(&config.tracker.terminal_states);
        let issue = issues
            .iter()
            .find(|candidate| candidate.id == issue_id)
            .cloned();

        let mut state_guard = self.state.lock().await;
        let mut state = state_guard.clone();
        if state
            .retry_attempts
            .get(&issue_id)
            .map(|entry| entry.attempt)
            != Some(attempt)
        {
            return;
        }
        let mut commands = Vec::new();

        match issue {
            None => {
                state = match apply_event(state, Event::Release(issue_id.clone()), &mut commands) {
                    Ok(next_state) => next_state,
                    Err(error) => {
                        tracing::warn!(issue_id = %issue_id.0, "retry release rejected: {error}");
                        return;
                    }
                };
            }
            Some(issue)
                if !dispatch_state_allows_issue(&issue, &active_states, &terminal_states)
                    || todo_issue_blocked_by_non_terminal(&issue, &terminal_states) =>
            {
                state = match apply_event(state, Event::Release(issue_id.clone()), &mut commands) {
                    Ok(next_state) => next_state,
                    Err(error) => {
                        tracing::warn!(issue_id = %issue_id.0, "retry stale release rejected: {error}");
                        return;
                    }
                };
            }
            Some(issue) => {
                if !dispatch_slots_available_for_issue(&state, issues.as_slice(), &issue, &config) {
                    drop(state_guard);
                    self.requeue_retry(issue_id, attempt.saturating_add(1), config.clone())
                        .await;
                    return;
                }

                state = match apply_event(
                    state,
                    Event::MarkRunning(issue_id.clone()),
                    &mut commands,
                ) {
                    Ok(next_state) => next_state,
                    Err(error) => {
                        tracing::warn!(issue_id = %issue_id.0, "retry promotion rejected: {error}");
                        return;
                    }
                };
                if let Some(entry) = state.running.get_mut(&issue_id) {
                    entry.identifier = Some(issue.identifier);
                    entry.tracker_state = Some(issue.state.as_str().to_owned());
                    if entry.started_at.is_none() {
                        entry.started_at = Some(current_unix_timestamp_secs());
                    }
                }
                self.set_pending_dispatch_attempt(issue_id.clone(), attempt);
            }
        }

        *state_guard = state.clone();
        drop(state_guard);

        {
            let mut retry_metadata = lock_unpoisoned(&self.retry_metadata);
            retry_metadata.remove(&issue_id);
        }

        self.publish_snapshot(&state);

        self.process_tick_commands(commands, config).await;
    }

    async fn requeue_retry(
        self: &Arc<Self>,
        issue_id: IssueId,
        attempt: u32,
        config: RuntimeConfig,
    ) {
        let mut state_guard = self.state.lock().await;
        let mut state = state_guard.clone();
        if !state.claimed.contains(&issue_id) {
            return;
        }
        if !state.retry_attempts.contains_key(&issue_id) {
            return;
        }

        let mut commands = Vec::new();
        state = match apply_event(
            state,
            Event::QueueRetry {
                issue_id: issue_id.clone(),
                attempt,
            },
            &mut commands,
        ) {
            Ok(next_state) => next_state,
            Err(error) => {
                tracing::warn!(issue_id = %issue_id.0, "retry requeue rejected: {error}");
                return;
            }
        };
        *state_guard = state.clone();
        drop(state_guard);

        self.publish_snapshot(&state);

        for command in commands {
            if let Command::ScheduleRetry {
                issue_id,
                attempt: scheduled_attempt,
            } = command
            {
                let retry_policy = retry_policy_for_config(&config);
                let schedule =
                    retry_policy.schedule_retry(issue_id, scheduled_attempt, RetryKind::Failure);
                self.schedule_retry_task(schedule, config.clone());
            }
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
        if let Some(worker) = workers.remove(issue_id) {
            self.take_running_attempt(issue_id);
            request_worker_stop(worker);
        }
    }

    #[cfg(test)]
    fn abort_background_tasks(&self) {
        self.abort_background_tasks_internal(None);
    }

    fn abort_background_tasks_with_cleanup(&self, config: &RuntimeConfig) {
        self.abort_background_tasks_internal(Some(config));
    }

    fn abort_background_tasks_internal(&self, config: Option<&RuntimeConfig>) {
        let mut workers = lock_unpoisoned(&self.active_workers);
        for (_, worker) in workers.drain() {
            request_worker_stop(worker);
        }
        drop(workers);

        let mut running_attempts = lock_unpoisoned(&self.running_attempts);
        running_attempts.clear();

        let mut pending_dispatch_attempts = lock_unpoisoned(&self.pending_dispatch_attempts);
        pending_dispatch_attempts.clear();

        let mut retries = lock_unpoisoned(&self.scheduled_retries);
        for (_, handle) in retries.drain() {
            handle.abort();
        }

        let pending_cleanup = {
            let mut pending_cleanup = lock_unpoisoned(&self.pending_terminal_cleanup);
            pending_cleanup
                .drain()
                .map(|(_, identifier)| identifier)
                .collect::<Vec<_>>()
        };
        if let Some(config) = config {
            for identifier in pending_cleanup {
                self.cleanup_workspace_for_identifier(config, &identifier);
            }
        }
    }
}

fn request_worker_stop(worker: ActiveWorkerHandle) {
    let _ = worker.stop_tx.send(true);
    tokio::spawn(async move {
        let mut join = worker.join;
        if tokio::time::timeout(Duration::from_secs(2), &mut join)
            .await
            .is_err()
        {
            join.abort();
            let _ = join.await;
        }
    });
}

fn build_issue_snapshots(
    state: &OrchestratorState,
    retry_metadata: &HashMap<IssueId, RetryMetadata>,
) -> Vec<IssueSnapshot> {
    let mut issues = Vec::with_capacity(state.running.len() + state.retry_attempts.len());

    for (issue_id, entry) in &state.running {
        issues.push(IssueSnapshot {
            id: issue_id.clone(),
            identifier: entry
                .identifier
                .clone()
                .unwrap_or_else(|| issue_id.0.clone()),
            state: entry
                .tracker_state
                .clone()
                .unwrap_or_else(|| "Running".to_owned()),
            retry_attempts: 0,
            workspace_path: None,
            session_id: entry.session_id.clone(),
            turn_count: entry.turn_count,
            last_event: entry.last_codex_event.clone(),
            last_message: entry.last_codex_message.clone(),
            started_at: entry.started_at,
            last_event_at: entry.last_codex_timestamp,
            codex_app_server_pid: entry.codex_app_server_pid,
            input_tokens: entry.usage.input_tokens,
            output_tokens: entry.usage.output_tokens,
            total_tokens: entry.usage.total_tokens,
            retry_due_at: None,
            retry_error: None,
        });
    }

    for (issue_id, entry) in &state.retry_attempts {
        let metadata = retry_metadata.get(issue_id);
        issues.push(IssueSnapshot {
            id: issue_id.clone(),
            identifier: metadata
                .and_then(|details| details.identifier.clone())
                .unwrap_or_else(|| issue_id.0.clone()),
            state: "Retrying".to_owned(),
            retry_attempts: entry.attempt,
            workspace_path: None,
            session_id: None,
            turn_count: 0,
            last_event: None,
            last_message: None,
            started_at: None,
            last_event_at: None,
            codex_app_server_pid: None,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            retry_due_at: metadata.and_then(|details| details.due_at),
            retry_error: metadata.and_then(|details| details.error.clone()),
        });
    }

    issues.sort_by(|left, right| left.identifier.cmp(&right.identifier));
    issues
}

fn build_retry_metadata(state: &OrchestratorState, issue_id: &IssueId) -> RetryMetadata {
    let mut metadata = RetryMetadata::default();
    if let Some(entry) = state.running.get(issue_id) {
        metadata.identifier = entry.identifier.clone();
        metadata.error = entry.last_codex_message.clone();
    }
    metadata
}

fn aggregate_running_seconds(state: &OrchestratorState) -> f64 {
    let now = current_unix_timestamp_secs();
    let active_seconds: f64 = state
        .running
        .values()
        .filter_map(|entry| entry.started_at)
        .map(|started_at| now.saturating_sub(started_at) as f64)
        .sum();

    // Add cumulative runtime from completed sessions (SPEC Section 13.5)
    active_seconds + state.codex_totals.completed_seconds_running
}

fn runtime_activity_snapshot(telemetry: &RuntimeTelemetry) -> RuntimeActivitySnapshot {
    RuntimeActivitySnapshot {
        poll_in_progress: telemetry.poll_in_progress,
        last_poll_started_at: telemetry.last_poll_started_at,
        last_poll_completed_at: telemetry.last_poll_completed_at,
        next_poll_due_at: telemetry.next_poll_due_at,
        last_runtime_activity_at: telemetry.last_runtime_activity_at,
        throughput: throughput_snapshot(&telemetry.token_samples),
    }
}

fn throughput_snapshot(samples: &VecDeque<TokenSample>) -> ThroughputSnapshot {
    let Some(latest) = samples.back() else {
        return ThroughputSnapshot {
            window_seconds: THROUGHPUT_WINDOW_SECONDS as f64,
            ..ThroughputSnapshot::default()
        };
    };
    let cutoff = latest.at.saturating_sub(THROUGHPUT_WINDOW_SECONDS);
    let baseline = samples
        .iter()
        .rev()
        .find(|sample| sample.at <= cutoff)
        .cloned()
        .unwrap_or_else(|| samples.front().cloned().unwrap_or_else(|| latest.clone()));
    let elapsed_seconds = latest.at.saturating_sub(baseline.at).max(1) as f64;
    let delta_input = latest
        .totals
        .input_tokens
        .saturating_sub(baseline.totals.input_tokens);
    let delta_output = latest
        .totals
        .output_tokens
        .saturating_sub(baseline.totals.output_tokens);
    let delta_total = latest
        .totals
        .total_tokens
        .saturating_sub(baseline.totals.total_tokens);

    ThroughputSnapshot {
        window_seconds: elapsed_seconds,
        input_tokens_per_second: delta_input as f64 / elapsed_seconds,
        output_tokens_per_second: delta_output as f64 / elapsed_seconds,
        total_tokens_per_second: delta_total as f64 / elapsed_seconds,
    }
}

fn record_token_sample(
    samples: &mut VecDeque<TokenSample>,
    timestamp_secs: u64,
    totals: &CodexTotals,
) {
    let normalized_at = samples
        .back()
        .map_or(timestamp_secs, |sample| sample.at.max(timestamp_secs));
    let next_sample = TokenSample {
        at: normalized_at,
        totals: totals.clone(),
    };

    if samples
        .back()
        .is_some_and(|sample| sample.totals == next_sample.totals)
    {
        return;
    }

    samples.push_back(next_sample);
    trim_token_samples(samples);
}

fn trim_token_samples(samples: &mut VecDeque<TokenSample>) {
    let Some(latest) = samples.back().cloned() else {
        return;
    };
    let cutoff = latest.at.saturating_sub(THROUGHPUT_WINDOW_SECONDS);
    while samples.len() > 2 && samples.get(1).is_some_and(|sample| sample.at < cutoff) {
        samples.pop_front();
    }
}

fn duration_to_rounded_up_seconds(duration_ms: u64) -> u64 {
    let seconds = duration_ms / 1_000;
    if duration_ms.is_multiple_of(1_000) {
        seconds
    } else {
        seconds.saturating_add(1)
    }
}

fn current_unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
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
        WorkerOutcome::Stopped => WorkerExit::Completed,
    }
}

fn retry_policy_for_config(config: &RuntimeConfig) -> RetryPolicy {
    let failure_base_ms = 10_000_u64;
    RetryPolicy {
        continuation_delay: Duration::from_secs(1),
        failure_base_delay: Duration::from_millis(failure_base_ms),
        max_failure_backoff: Duration::from_millis(
            config.agent.max_retry_backoff_ms.max(failure_base_ms),
        ),
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

fn dispatch_slots_available_for_issue(
    state: &OrchestratorState,
    issues: &[TrackerIssue],
    issue: &TrackerIssue,
    config: &RuntimeConfig,
) -> bool {
    let global_available =
        (config.agent.max_concurrent_agents as usize).saturating_sub(state.running.len());
    if global_available == 0 {
        return false;
    }

    let running_by_state = running_state_counts(state, issues);

    let Some(state_key) = issue.state.normalized_key() else {
        return false;
    };

    if let Some(&max_for_state) = config.agent.max_concurrent_agents_by_state.get(&state_key) {
        let current_for_state = running_by_state.get(&state_key).copied().unwrap_or(0);
        return current_for_state < max_for_state as usize;
    }

    true
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

fn running_state_counts(
    state: &OrchestratorState,
    issues: &[TrackerIssue],
) -> HashMap<String, usize> {
    let issue_states = issues
        .iter()
        .filter_map(|issue| {
            issue
                .state
                .normalized_key()
                .map(|key| (issue.id.clone(), key))
        })
        .collect::<HashMap<_, _>>();
    let mut running_by_state = HashMap::new();

    for (issue_id, running_entry) in &state.running {
        let state_key = running_entry
            .tracker_state
            .as_deref()
            .and_then(|state_name| TrackerState::new(state_name).normalized_key())
            .or_else(|| issue_states.get(issue_id).cloned());
        if let Some(state_key) = state_key {
            *running_by_state.entry(state_key).or_insert(0) += 1;
        }
    }

    running_by_state
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

fn next_running_retry_attempt(
    state: &OrchestratorState,
    running_attempt: Option<u32>,
    issue_id: &IssueId,
) -> u32 {
    running_attempt
        .filter(|attempt| *attempt > 0)
        .map(|attempt| attempt.saturating_add(1))
        .unwrap_or_else(|| next_retry_attempt(state, issue_id))
}

fn stall_elapsed_ms(entry: &symphony_domain::RunningEntry, now_secs: u64) -> Option<u64> {
    let last_activity_secs = entry.last_codex_timestamp.or(entry.started_at)?;
    Some(
        now_secs
            .saturating_sub(last_activity_secs)
            .saturating_mul(1_000),
    )
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
        fs,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
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

    struct ErrorTracker;

    #[async_trait]
    impl TrackerClient for ErrorTracker {
        async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
            Err(TrackerError::transport("tracker unavailable"))
        }
    }

    #[derive(Default)]
    struct MockWorkerLauncher {
        outcomes: Mutex<VecDeque<WorkerOutcome>>,
        launch_count: AtomicUsize,
        attempts: Mutex<Vec<u32>>,
    }

    #[cfg(unix)]
    #[derive(Default)]
    struct HangingChildWorkerLauncher;

    impl MockWorkerLauncher {
        fn with_outcomes(outcomes: Vec<WorkerOutcome>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes.into()),
                launch_count: AtomicUsize::new(0),
                attempts: Mutex::new(Vec::new()),
            }
        }

        fn launch_count(&self) -> usize {
            self.launch_count.load(Ordering::Relaxed)
        }

        async fn launched_attempts(&self) -> Vec<u32> {
            self.attempts.lock().await.clone()
        }
    }

    #[async_trait]
    impl WorkerLauncher for MockWorkerLauncher {
        async fn launch(&self, ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            self.launch_count.fetch_add(1, Ordering::Relaxed);
            self.attempts.lock().await.push(ctx.attempt);
            let mut outcomes = self.outcomes.lock().await;
            Ok(outcomes.pop_front().unwrap_or(WorkerOutcome::Completed))
        }
    }

    #[cfg(unix)]
    #[async_trait]
    impl WorkerLauncher for HangingChildWorkerLauncher {
        async fn launch(&self, ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
            let mut child = tokio::process::Command::new("sh");
            child.arg("-lc").arg("sleep 30");
            child.kill_on_drop(true);
            child.stdin(std::process::Stdio::null());
            child.stdout(std::process::Stdio::null());
            child.stderr(std::process::Stdio::null());

            let child = child
                .spawn()
                .map_err(|error| WorkerError::LaunchFailed(error.to_string()))?;

            if let Some(protocol_updates) = &ctx.protocol_updates {
                let _ = protocol_updates.send(WorkerProtocolUpdate {
                    pid: child.id(),
                    event: Some("worker/started".to_owned()),
                    timestamp: Some(current_unix_timestamp_secs()),
                    ..WorkerProtocolUpdate::default()
                });
            }

            tokio::time::sleep(Duration::from_secs(30)).await;
            drop(child);
            Ok(WorkerOutcome::Completed)
        }
    }

    fn issue(id: &str) -> IssueId {
        IssueId(id.to_owned())
    }

    fn unique_temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "symphony-runtime-{label}-{}-{nonce}",
            std::process::id()
        ))
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

    #[cfg(unix)]
    fn sleeping_app_server_command(thread_id: &str, turn_id: &str) -> String {
        let thread_start = serde_json::json!({
            "result": {
                "thread": {
                    "id": thread_id,
                }
            }
        });
        let turn_start = serde_json::json!({
            "result": {
                "turn": {
                    "id": turn_id,
                }
            }
        });

        format!(
            "printf '%s\\n' '{thread_start}'; sleep 0.05; printf '%s\\n' '{turn_start}'; sleep 30 # app-server"
        )
    }

    #[cfg(unix)]
    fn process_is_running(pid: u32) -> bool {
        std::process::Command::new("sh")
            .arg("-lc")
            .arg(format!("kill -0 {pid} >/dev/null 2>&1"))
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(unix)]
    async fn wait_for_worker_pid<T: TrackerClient + 'static>(
        runtime: &Runtime<T>,
        issue_id: &IssueId,
    ) -> u32 {
        for _ in 0..80 {
            let state = runtime.state().await;
            if let Some(pid) = state
                .running
                .get(issue_id)
                .and_then(|entry| entry.codex_app_server_pid)
            {
                return pid;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        panic!("worker pid for {} was not published", issue_id.0);
    }

    #[cfg(unix)]
    async fn assert_process_exits(pid: u32, reason: &str) {
        for _ in 0..120 {
            if !process_is_running(pid) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        panic!("child process {pid} still appears to be running after {reason}");
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

    #[test]
    fn next_running_retry_attempt_prefers_running_attempt_over_retry_entry() {
        let issue_id = issue("SYM-300");
        let mut state = OrchestratorState::default();
        state.retry_attempts.insert(
            issue_id.clone(),
            symphony_domain::RetryEntry {
                attempt: 2,
                ..symphony_domain::RetryEntry::default()
            },
        );

        assert_eq!(next_running_retry_attempt(&state, Some(4), &issue_id), 5);
        assert_eq!(next_running_retry_attempt(&state, Some(0), &issue_id), 3);
        assert_eq!(next_running_retry_attempt(&state, None, &issue_id), 3);
    }

    #[test]
    fn next_running_retry_attempt_treats_initial_dispatch_as_first_retry() {
        let issue_id = issue("SYM-300A");
        let state = OrchestratorState::default();

        assert_eq!(next_running_retry_attempt(&state, Some(0), &issue_id), 1);
    }

    #[test]
    fn stall_elapsed_ms_prefers_last_codex_timestamp_over_started_at() {
        let now_secs: u64 = 1_700_000_600;
        let entry = symphony_domain::RunningEntry {
            started_at: Some(now_secs.saturating_sub(500)),
            last_codex_timestamp: Some(now_secs.saturating_sub(30)),
            ..symphony_domain::RunningEntry::default()
        };

        assert_eq!(stall_elapsed_ms(&entry, now_secs), Some(30_000));
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
        }
        {
            let mut running_attempts = lock_unpoisoned(&runtime.running_attempts);
            running_attempts.insert(issue_id.clone(), 2);
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
    async fn run_tick_dispatch_order_uses_priority_then_created_at() {
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![
                // High priority (1) but newer - should be first
                TrackerIssue {
                    id: issue("SYM-HIGH"),
                    identifier: "SYM-HIGH".to_owned(),
                    title: "High priority newer".to_owned(),
                    priority: Some(1),
                    created_at: Some(2000),
                    ..tracker_issue("SYM-HIGH")
                },
                // Medium priority (2) - should be second
                TrackerIssue {
                    id: issue("SYM-MED"),
                    identifier: "SYM-MED".to_owned(),
                    title: "Medium priority".to_owned(),
                    priority: Some(2),
                    created_at: Some(1000),
                    ..tracker_issue("SYM-MED")
                },
                // Low priority (3) but oldest - should be third (priority beats created_at)
                TrackerIssue {
                    id: issue("SYM-LOW"),
                    identifier: "SYM-LOW".to_owned(),
                    title: "Low priority oldest".to_owned(),
                    priority: Some(3),
                    created_at: Some(500),
                    ..tracker_issue("SYM-LOW")
                },
                // No priority (sorts last) but oldest created_at
                TrackerIssue {
                    id: issue("SYM-NONE"),
                    identifier: "SYM-NONE".to_owned(),
                    title: "No priority oldest".to_owned(),
                    priority: None,
                    created_at: Some(100),
                    ..tracker_issue("SYM-NONE")
                },
            ])
            .await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        let mut config = RuntimeConfig::default();
        config.agent.max_concurrent_agents = 4;

        let tick = runtime
            .run_tick(&config)
            .await
            .expect("dispatch tick should succeed");

        // Priority ordering: HIGH(1) < MED(2) < LOW(3) < NONE(max)
        assert_eq!(
            tick.commands,
            vec![
                Command::Dispatch(issue("SYM-HIGH")),
                Command::Dispatch(issue("SYM-MED")),
                Command::Dispatch(issue("SYM-LOW")),
                Command::Dispatch(issue("SYM-NONE")),
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
            state.running.insert(
                issue_id.clone(),
                symphony_domain::RunningEntry {
                    identifier: Some("SYM-99".to_owned()),
                    started_at: Some(current_unix_timestamp_secs().saturating_sub(400)),
                    ..symphony_domain::RunningEntry::default()
                },
            );
        }
        {
            let mut running_attempts = lock_unpoisoned(&runtime.running_attempts);
            running_attempts.insert(issue_id.clone(), 3);
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
                attempt: 4,
            }]
        );

        let state = runtime.state().await;
        assert!(!state.running.contains_key(&issue_id));
        assert!(state.retry_attempts.contains_key(&issue_id));
        assert_eq!(
            lock_unpoisoned(&runtime.retry_metadata)
                .get(&issue_id)
                .and_then(|metadata| metadata.error.clone())
                .as_deref(),
            Some("stalled for 400000ms without codex activity")
        );
    }

    #[tokio::test]
    async fn worker_exit_retryable_failure_uses_running_attempt_for_backoff() {
        let issue_id = issue("SYM-305");
        let tracker = Arc::new(MutableTracker::default());
        let runtime = Runtime::new(Arc::clone(&tracker));
        let policy = RetryPolicy {
            continuation_delay: Duration::from_secs(1),
            failure_base_delay: Duration::from_secs(10),
            max_failure_backoff: Duration::from_secs(300),
        };

        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
            state
                .running
                .insert(issue_id.clone(), symphony_domain::RunningEntry::default());
        }
        {
            let mut running_attempts = lock_unpoisoned(&runtime.running_attempts);
            running_attempts.insert(issue_id.clone(), 3);
        }

        let result = runtime
            .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
            .await
            .expect("worker exit mapping should succeed");

        assert_eq!(
            result.retry_schedule,
            Some(RetrySchedule {
                issue_id: issue_id.clone(),
                attempt: 4,
                kind: RetryKind::Failure,
                delay: Duration::from_secs(80),
            }),
        );
        assert_eq!(
            result
                .state
                .retry_attempts
                .get(&issue_id)
                .map(|entry| entry.attempt),
            Some(4)
        );
    }

    #[tokio::test]
    async fn handle_ready_retry_dispatches_worker_with_retry_attempt() {
        let issue_id = issue("SYM-703A");
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![tracker_issue(&issue_id.0)])
            .await;
        let launcher = Arc::new(MockWorkerLauncher::with_outcomes(vec![
            WorkerOutcome::Completed,
        ]));
        let runtime = Arc::new(Runtime::with_worker_launcher(
            Arc::clone(&tracker),
            launcher.clone(),
        ));

        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
            state.retry_attempts.insert(
                issue_id.clone(),
                symphony_domain::RetryEntry {
                    attempt: 3,
                    ..symphony_domain::RetryEntry::default()
                },
            );
        }

        let mut config = RuntimeConfig::default();
        config.codex.command = "true".to_owned();

        runtime
            .handle_ready_retry(issue_id.clone(), 3, config)
            .await;
        tokio::time::sleep(Duration::from_millis(25)).await;

        assert_eq!(launcher.launch_count(), 1);
        assert_eq!(launcher.launched_attempts().await, vec![3]);
    }

    #[tokio::test]
    async fn execute_worker_releases_dispatch_when_issue_becomes_blocked() {
        let issue_id = issue("SYM-704B");
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![
                TrackerIssue::new(issue_id.clone(), "SYM-704B", TrackerState::new("Todo"))
                    .with_title("Blocked after claim")
                    .with_blocked_by(vec![TrackerBlockerRef {
                        id: Some("SYM-999".to_owned()),
                        identifier: Some("SYM-999".to_owned()),
                        state: Some("In Progress".to_owned()),
                    }]),
            ])
            .await;
        let launcher = Arc::new(MockWorkerLauncher::default());
        let runtime = Arc::new(Runtime::with_worker_launcher(
            Arc::clone(&tracker),
            launcher.clone(),
        ));

        runtime
            .setup_running(issue_id.clone(), symphony_domain::RunningEntry::default())
            .await;

        {
            let mut running_attempts = lock_unpoisoned(&runtime.running_attempts);
            running_attempts.insert(issue_id.clone(), 2);
        }

        let config = RuntimeConfig::default();
        let (_stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        runtime
            .execute_worker(issue_id.clone(), 0, 2, config, stop_rx)
            .await;

        let state = runtime.state().await;
        assert!(!state.claimed.contains(&issue_id));
        assert!(!state.running.contains_key(&issue_id));
        assert!(!state.retry_attempts.contains_key(&issue_id));
        assert_eq!(launcher.launch_count(), 0);
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
    async fn retryable_failure_relaunches_worker_once_retry_is_ready() {
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

        tokio::time::sleep(Duration::from_millis(25)).await;

        let state = runtime.state().await;
        assert!(state.claimed.contains(&issue_id));
        assert!(!state.running.contains_key(&issue_id));
        assert_eq!(
            state
                .retry_attempts
                .get(&issue_id)
                .map(|entry| entry.attempt),
            Some(1)
        );

        runtime.cancel_retry_task(&issue_id);
        runtime
            .handle_ready_retry(issue_id.clone(), 1, config.clone())
            .await;
        tokio::time::sleep(Duration::from_millis(25)).await;

        let state = runtime.state().await;

        assert!(!state.claimed.contains(&issue_id));
        assert!(!state.running.contains_key(&issue_id));
        assert!(!state.retry_attempts.contains_key(&issue_id));
        assert_eq!(launcher.launch_count(), 2);
    }

    #[tokio::test]
    async fn handle_ready_retry_releases_claim_when_issue_is_no_longer_active() {
        let issue_id = issue("SYM-700");
        let tracker = Arc::new(MutableTracker::default());
        tracker.set_candidates(Vec::new()).await;
        let runtime = Arc::new(Runtime::new(Arc::clone(&tracker)));

        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
            state.retry_attempts.insert(
                issue_id.clone(),
                symphony_domain::RetryEntry {
                    attempt: 1,
                    ..symphony_domain::RetryEntry::default()
                },
            );
        }

        runtime
            .handle_ready_retry(issue_id.clone(), 1, RuntimeConfig::default())
            .await;

        let state = runtime.state().await;
        assert!(!state.claimed.contains(&issue_id));
        assert!(!state.retry_attempts.contains_key(&issue_id));
    }

    #[tokio::test]
    async fn handle_ready_retry_requeues_when_tracker_refresh_fails() {
        let issue_id = issue("SYM-701");
        let runtime = Arc::new(Runtime::new(Arc::new(ErrorTracker)));

        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
            state.retry_attempts.insert(
                issue_id.clone(),
                symphony_domain::RetryEntry {
                    attempt: 1,
                    ..symphony_domain::RetryEntry::default()
                },
            );
        }

        let mut config = RuntimeConfig::default();
        config.polling.interval_ms = 60_000;

        runtime
            .handle_ready_retry(issue_id.clone(), 1, config.clone())
            .await;

        let state = runtime.state().await;
        assert!(state.claimed.contains(&issue_id));
        assert_eq!(
            state
                .retry_attempts
                .get(&issue_id)
                .map(|entry| entry.attempt),
            Some(2)
        );

        runtime.cancel_retry_task(&issue_id);
    }

    #[tokio::test]
    async fn handle_ready_retry_requeues_when_no_dispatch_slots_are_available() {
        let issue_id = issue("SYM-702");
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![
                TrackerIssue::new(issue_id.clone(), "SYM-702", TrackerState::new("Todo"))
                    .with_title("Retry issue"),
                TrackerIssue::new(issue("SYM-703"), "SYM-703", TrackerState::new("Todo"))
                    .with_title("Already running"),
            ])
            .await;
        let runtime = Arc::new(Runtime::new(Arc::clone(&tracker)));

        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
            state.retry_attempts.insert(
                issue_id.clone(),
                symphony_domain::RetryEntry {
                    attempt: 1,
                    ..symphony_domain::RetryEntry::default()
                },
            );
            state.claimed.insert(issue("SYM-703"));
            state
                .running
                .insert(issue("SYM-703"), symphony_domain::RunningEntry::default());
        }

        let mut config = RuntimeConfig::default();
        config.polling.interval_ms = 60_000;
        config.agent.max_concurrent_agents = 1;

        runtime
            .handle_ready_retry(issue_id.clone(), 1, config.clone())
            .await;

        let state = runtime.state().await;
        assert!(state.claimed.contains(&issue_id));
        assert_eq!(
            state
                .retry_attempts
                .get(&issue_id)
                .map(|entry| entry.attempt),
            Some(2)
        );
        assert!(!state.running.contains_key(&issue_id));

        runtime.cancel_retry_task(&issue_id);
    }

    #[tokio::test]
    async fn execute_worker_releases_stale_dispatch_without_retry() {
        let issue_id = issue("SYM-704");
        let tracker = Arc::new(MutableTracker::default());
        tracker.set_candidates(Vec::new()).await;
        let launcher = Arc::new(MockWorkerLauncher::default());
        let runtime = Arc::new(Runtime::with_worker_launcher(
            Arc::clone(&tracker),
            launcher.clone(),
        ));

        runtime
            .setup_running(issue_id.clone(), symphony_domain::RunningEntry::default())
            .await;

        let config = RuntimeConfig::default();
        let (_stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        runtime
            .execute_worker(issue_id.clone(), 0, 1, config, stop_rx)
            .await;

        let state = runtime.state().await;
        assert!(!state.claimed.contains(&issue_id));
        assert!(!state.running.contains_key(&issue_id));
        assert!(!state.retry_attempts.contains_key(&issue_id));
        assert_eq!(launcher.launch_count(), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn release_claim_stop_path_kills_active_app_server_child_process() {
        let issue_id = issue("SYM-705");
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![tracker_issue(&issue_id.0)])
            .await;
        let runtime = Arc::new(Runtime::new(Arc::clone(&tracker)));

        let root = unique_temp_root("release-claim-stop");
        let mut config = RuntimeConfig::default();
        config.workspace.root = root.clone();
        config.codex.command =
            sleeping_app_server_command("thread-release-stop", "turn-release-stop");
        config.codex.turn_timeout_ms = 5_000;
        config.codex.read_timeout_ms = 500;

        let dispatch_tick = runtime
            .run_tick(&config)
            .await
            .expect("dispatch tick should succeed");
        assert_eq!(
            dispatch_tick.commands,
            vec![Command::Dispatch(issue_id.clone())]
        );
        runtime
            .process_tick_commands(dispatch_tick.commands, config.clone())
            .await;

        let pid = wait_for_worker_pid(runtime.as_ref(), &issue_id).await;
        assert!(process_is_running(pid));

        tracker.set_candidates(Vec::new()).await;
        let release_tick = runtime
            .run_tick(&config)
            .await
            .expect("release tick should succeed");
        assert_eq!(
            release_tick.commands,
            vec![Command::ReleaseClaim(issue_id.clone())]
        );
        runtime
            .process_tick_commands(release_tick.commands, config)
            .await;

        assert_process_exits(pid, "release claim stop path").await;

        let state = runtime.state().await;
        assert!(!state.claimed.contains(&issue_id));
        assert!(!state.running.contains_key(&issue_id));
        assert!(!runtime.has_active_worker(&issue_id));

        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn abort_background_tasks_kills_child_process_when_worker_ignores_stop() {
        let issue_id = issue("SYM-706");
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![tracker_issue(&issue_id.0)])
            .await;
        let runtime = Arc::new(Runtime::with_worker_launcher(
            Arc::clone(&tracker),
            Arc::new(HangingChildWorkerLauncher),
        ));

        let dispatch_tick = runtime
            .run_tick(&RuntimeConfig::default())
            .await
            .expect("dispatch tick should succeed");
        assert_eq!(
            dispatch_tick.commands,
            vec![Command::Dispatch(issue_id.clone())]
        );
        runtime
            .process_tick_commands(dispatch_tick.commands, RuntimeConfig::default())
            .await;

        let pid = wait_for_worker_pid(runtime.as_ref(), &issue_id).await;
        assert!(process_is_running(pid));

        runtime.abort_background_tasks();

        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(
            process_is_running(pid),
            "child exited before the abort fallback path could run"
        );

        assert_process_exits(pid, "runtime abort fallback").await;
        assert!(!runtime.has_active_worker(&issue_id));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stalled_retry_schedule_stops_existing_child_process() {
        let issue_id = issue("SYM-706A");
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![tracker_issue(&issue_id.0)])
            .await;
        let runtime = Arc::new(Runtime::new(Arc::clone(&tracker)));

        let root = unique_temp_root("stall-retry-stop");
        let mut config = RuntimeConfig::default();
        config.workspace.root = root.clone();
        config.codex.command = sleeping_app_server_command("thread-stall-stop", "turn-stall-stop");
        config.codex.turn_timeout_ms = 5_000;
        config.codex.read_timeout_ms = 500;
        config.codex.stall_timeout_ms = 300_000;

        let dispatch_tick = runtime
            .run_tick(&config)
            .await
            .expect("dispatch tick should succeed");
        assert_eq!(
            dispatch_tick.commands,
            vec![Command::Dispatch(issue_id.clone())]
        );
        runtime
            .process_tick_commands(dispatch_tick.commands, config.clone())
            .await;

        let pid = wait_for_worker_pid(runtime.as_ref(), &issue_id).await;
        assert!(process_is_running(pid));

        {
            let mut state = runtime.state.lock().await;
            let entry = state
                .running
                .get_mut(&issue_id)
                .expect("issue should still be running");
            entry.last_codex_timestamp = Some(current_unix_timestamp_secs().saturating_sub(400));
        }

        let stalled_tick = runtime
            .run_tick(&config)
            .await
            .expect("stall tick should succeed");
        assert_eq!(
            stalled_tick.commands,
            vec![Command::ScheduleRetry {
                issue_id: issue_id.clone(),
                attempt: 1,
            }]
        );
        runtime
            .process_tick_commands(stalled_tick.commands, config)
            .await;

        assert_process_exits(pid, "stalled retry stop path").await;

        let state = runtime.state().await;
        assert_eq!(
            state
                .retry_attempts
                .get(&issue_id)
                .map(|entry| entry.attempt),
            Some(1)
        );
        assert!(!runtime.has_active_worker(&issue_id));

        runtime.cancel_retry_task(&issue_id);
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn terminal_reconciliation_stops_child_and_cleans_workspace() {
        let issue_id = issue("SYM-707");
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![tracker_issue(&issue_id.0)])
            .await;
        let runtime = Arc::new(Runtime::new(Arc::clone(&tracker)));

        let root = unique_temp_root("terminal-reconciliation-stop");
        let workspace_path = root.join("SYM-707");
        fs::create_dir_all(&workspace_path).expect("workspace should be created");

        let mut config = RuntimeConfig::default();
        config.workspace.root = root.clone();
        config.codex.command =
            sleeping_app_server_command("thread-terminal-stop", "turn-terminal-stop");
        config.codex.turn_timeout_ms = 5_000;
        config.codex.read_timeout_ms = 500;

        let dispatch_tick = runtime
            .run_tick(&config)
            .await
            .expect("dispatch tick should succeed");
        assert_eq!(
            dispatch_tick.commands,
            vec![Command::Dispatch(issue_id.clone())]
        );
        runtime
            .process_tick_commands(dispatch_tick.commands, config.clone())
            .await;

        let pid = wait_for_worker_pid(runtime.as_ref(), &issue_id).await;
        assert!(process_is_running(pid));

        tracker
            .set_candidates(vec![
                TrackerIssue::new(issue_id.clone(), "SYM-707", TrackerState::new("Done"))
                    .with_title("Terminal issue"),
            ])
            .await;

        let release_tick = runtime
            .run_tick(&config)
            .await
            .expect("release tick should succeed");
        assert_eq!(
            release_tick.commands,
            vec![Command::ReleaseClaim(issue_id.clone())]
        );
        runtime
            .process_tick_commands(release_tick.commands, config)
            .await;

        assert_process_exits(pid, "terminal reconciliation stop path").await;
        assert!(
            !workspace_path.exists(),
            "terminal workspace should be removed"
        );
        assert!(!runtime.has_active_worker(&issue_id));

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn shutdown_cleanup_drains_pending_terminal_workspaces() {
        let runtime = Runtime::new(Arc::new(MutableTracker::default()));
        let root = unique_temp_root("shutdown-pending-cleanup");
        let workspace_path = root.join("SYM-SHUTDOWN");
        fs::create_dir_all(&workspace_path).expect("workspace should be created");

        let mut config = RuntimeConfig::default();
        config.workspace.root = root.clone();

        runtime.queue_terminal_cleanup(issue("lin-shutdown"), "SYM-SHUTDOWN".to_owned());
        runtime.abort_background_tasks_with_cleanup(&config);

        assert!(
            !workspace_path.exists(),
            "shutdown cleanup should remove pending terminal workspace"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn startup_terminal_workspace_cleanup_removes_terminal_workspaces_only() {
        let tracker = Arc::new(MutableTracker::default());
        tracker
            .set_candidates(vec![
                TrackerIssue::new(issue("lin-term"), "SYM-TERM", TrackerState::new("Done"))
                    .with_title("Terminal issue"),
                TrackerIssue::new(issue("lin-active"), "SYM-ACTIVE", TrackerState::new("Todo"))
                    .with_title("Active issue"),
            ])
            .await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        let root = unique_temp_root("startup-cleanup");
        fs::create_dir_all(root.join("SYM-TERM")).expect("terminal workspace should be created");
        fs::create_dir_all(root.join("SYM-ACTIVE")).expect("active workspace should be created");

        let mut config = RuntimeConfig::default();
        config.workspace.root = root.clone();

        runtime.startup_terminal_workspace_cleanup(&config).await;

        assert!(!root.join("SYM-TERM").exists());
        assert!(root.join("SYM-ACTIVE").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn startup_terminal_workspace_cleanup_ignores_tracker_failures() {
        let runtime = Runtime::new(Arc::new(ErrorTracker));

        let root = unique_temp_root("startup-cleanup-error");
        fs::create_dir_all(root.join("SYM-TERM")).expect("workspace should be created");

        let mut config = RuntimeConfig::default();
        config.workspace.root = root.clone();

        runtime.startup_terminal_workspace_cleanup(&config).await;

        assert!(root.join("SYM-TERM").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn cleanup_workspace_for_identifier_falls_back_after_before_remove_failure() {
        let runtime = Runtime::new(Arc::new(MutableTracker::default()));

        let root = unique_temp_root("cleanup-fallback");
        fs::create_dir_all(root.join("SYM-TERM")).expect("workspace should be created");

        let mut config = RuntimeConfig::default();
        config.workspace.root = root.clone();
        config.hooks.before_remove = Some("definitely-not-a-real-command-symphony".to_owned());

        runtime.cleanup_workspace_for_identifier(&config, "SYM-TERM");

        assert!(!root.join("SYM-TERM").exists());

        let _ = fs::remove_dir_all(&root);
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
                Some(serde_json::json!({
                    "primary": {
                        "remaining": 11
                    }
                })),
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
        assert_eq!(
            state.codex_rate_limits,
            Some(serde_json::json!({
                "primary": {
                    "remaining": 11
                }
            }))
        );

        let snapshot = runtime.snapshot().await;
        assert_eq!(snapshot.input_tokens, 100);
        assert_eq!(snapshot.output_tokens, 50);
        assert_eq!(snapshot.total_tokens, 150);
        assert_eq!(snapshot.activity.last_runtime_activity_at, Some(1700000000));
        assert_eq!(snapshot.activity.throughput.window_seconds, 1.0);
        assert_eq!(snapshot.activity.throughput.total_tokens_per_second, 0.0);
    }

    #[tokio::test]
    async fn handle_protocol_update_tracks_rolling_throughput_from_absolute_totals() {
        let issue_id = issue("SYM-502");
        let tracker = Arc::new(MutableTracker::default());
        let runtime = Runtime::new(Arc::clone(&tracker));

        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
            state
                .running
                .insert(issue_id.clone(), symphony_domain::RunningEntry::default());
        }

        runtime
            .handle_protocol_update(
                issue_id.clone(),
                Some("session-1".to_owned()),
                Some("thread-1".to_owned()),
                Some("turn-1".to_owned()),
                None,
                Some("turn.start".to_owned()),
                Some(1_700_000_000),
                None,
                Some(symphony_domain::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: 15,
                }),
                None,
            )
            .await
            .expect("first protocol update should succeed");

        runtime
            .handle_protocol_update(
                issue_id.clone(),
                Some("session-1".to_owned()),
                Some("thread-1".to_owned()),
                Some("turn-2".to_owned()),
                None,
                Some("turn.completed".to_owned()),
                Some(1_700_000_005),
                None,
                Some(symphony_domain::Usage {
                    input_tokens: 30,
                    output_tokens: 20,
                    total_tokens: 50,
                }),
                None,
            )
            .await
            .expect("second protocol update should succeed");

        let snapshot = runtime.snapshot().await;
        assert_eq!(
            snapshot.activity.last_runtime_activity_at,
            Some(1_700_000_005)
        );
        assert_eq!(snapshot.activity.throughput.window_seconds, 5.0);
        assert_eq!(snapshot.activity.throughput.input_tokens_per_second, 4.0);
        assert_eq!(snapshot.activity.throughput.output_tokens_per_second, 3.0);
        assert_eq!(snapshot.activity.throughput.total_tokens_per_second, 7.0);
    }

    #[test]
    fn poll_activity_snapshot_tracks_progress_and_next_due_time() {
        let runtime = Runtime::new(Arc::new(MutableTracker::default()));
        let state = OrchestratorState::default();

        runtime.mark_next_poll_due(1_700_000_000);
        runtime.mark_poll_started(1_700_000_001);
        runtime.mark_poll_completed(1_700_000_006, 30_000);

        let snapshot = runtime.snapshot_from_state(&state);
        assert!(!snapshot.activity.poll_in_progress);
        assert_eq!(snapshot.activity.last_poll_started_at, Some(1_700_000_001));
        assert_eq!(
            snapshot.activity.last_poll_completed_at,
            Some(1_700_000_006)
        );
        assert_eq!(snapshot.activity.next_poll_due_at, Some(1_700_000_036));
        assert_eq!(
            snapshot.activity.last_runtime_activity_at,
            Some(1_700_000_006)
        );
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
                None,
            )
            .await;

        // Should fail with MissingClaim (which is the invariant error for non-running issues)
        assert!(result.is_err());
    }

    /// R1.1.3: Verify dispatch-time config revalidation rejects stale candidates
    /// When an issue is queued for dispatch but is no longer a valid candidate
    /// (e.g., changed state in tracker), the dispatch should be skipped gracefully.
    #[tokio::test]
    async fn dispatch_time_revalidation_skips_stale_candidates() {
        let issue_id = issue("SYM-STALE");
        let tracker = Arc::new(MutableTracker::default());

        // Initially the issue exists in the tracker
        tracker
            .set_candidates(vec![
                TrackerIssue::new(
                    issue_id.clone(),
                    "SYM-STALE",
                    TrackerState::new("In Progress"),
                )
                .with_title("Stale issue"),
            ])
            .await;

        let runtime = Runtime::new(Arc::clone(&tracker));
        let config = RuntimeConfig::default();

        // Claim the issue (simulating tick processing)
        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
        }

        // Now remove the issue from tracker (simulating external state change)
        tracker.set_candidates(vec![]).await;

        // Attempt to revalidate for dispatch
        let result = runtime
            .revalidate_issue_for_dispatch(&issue_id, &config)
            .await
            .expect("revalidation should not error");

        // Should return None because issue is no longer a candidate
        assert!(
            result.is_none(),
            "stale issue should not be valid for dispatch"
        );

        // State should still have the claim (cleanup happens via handle_worker_exit)
        let state = runtime.state().await;
        assert!(state.claimed.contains(&issue_id));
    }

    /// R1.1.3: Verify dispatch-time revalidation succeeds for valid candidates
    #[tokio::test]
    async fn dispatch_time_revalidation_accepts_valid_candidates() {
        let issue_id = issue("SYM-VALID");
        let tracker = Arc::new(MutableTracker::default());

        let valid_issue = TrackerIssue::new(
            issue_id.clone(),
            "SYM-VALID",
            TrackerState::new("In Progress"),
        )
        .with_title("Valid issue");

        tracker.set_candidates(vec![valid_issue.clone()]).await;

        let runtime = Runtime::new(Arc::clone(&tracker));
        let config = RuntimeConfig::default();

        // Claim the issue
        {
            let mut state = runtime.state.lock().await;
            state.claimed.insert(issue_id.clone());
        }

        // Revalidate for dispatch
        let result = runtime
            .revalidate_issue_for_dispatch(&issue_id, &config)
            .await
            .expect("revalidation should not error");

        // Should return Some with the issue
        assert!(result.is_some(), "valid issue should pass revalidation");
        let revalidated = result.unwrap();
        assert_eq!(revalidated.id, issue_id);
        assert_eq!(revalidated.identifier, "SYM-VALID");
    }

    /// R1.1.3: Verify dispatch-time revalidation handles tracker errors gracefully
    #[tokio::test]
    async fn dispatch_time_revalidation_handles_tracker_errors() {
        let issue_id = issue("SYM-ERROR");
        let tracker = Arc::new(ErrorTracker);
        let runtime = Runtime::new(Arc::clone(&tracker));
        let config = RuntimeConfig::default();

        // Attempt to revalidate with failing tracker
        let result = runtime
            .revalidate_issue_for_dispatch(&issue_id, &config)
            .await;

        // Should return an error
        assert!(
            result.is_err(),
            "tracker error should propagate as revalidation failure"
        );
    }
}
