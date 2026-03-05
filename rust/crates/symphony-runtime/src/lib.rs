#![forbid(unsafe_code)]

pub mod worker;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use symphony_config::RuntimeConfig;
use symphony_domain::{
    Command, Event, InvariantError, IssueId, OrchestratorState, reduce, validate_invariants,
};
use symphony_observability::RuntimeSnapshot;
use symphony_tracker::{TrackerClient, TrackerError};
use thiserror::Error;
use tokio::sync::Mutex;

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
    state: Mutex<OrchestratorState>,
    last_seen: Mutex<std::collections::HashMap<IssueId, std::time::Instant>>,
    snapshot_tx: tokio::sync::watch::Sender<RuntimeSnapshot>,
}

impl<T: TrackerClient> Runtime<T> {
    pub fn new(tracker: Arc<T>) -> Self {
        let (snapshot_tx, _) = tokio::sync::watch::channel(RuntimeSnapshot {
            running: 0,
            retrying: 0,
        });
        Self {
            tracker,
            state: Mutex::new(OrchestratorState::default()),
            last_seen: Mutex::new(std::collections::HashMap::new()),
            snapshot_tx,
        }
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

            state = apply_event(state, Event::Claim(issue_id.clone()), &mut commands)?;
            state = apply_event(state, Event::MarkRunning(issue_id), &mut commands)?;
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
        &self,
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

                    if let Err(e) = self.run_tick(&new_config).await {
                        tracing::error!("Tick failed: {:?}", e);
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

                    if let Err(e) = self.run_tick(&new_config).await {
                        tracing::error!("Refresh tick failed: {:?}", e);
                    }
                    interval = tokio::time::interval(Duration::from_millis(new_config.polling.interval_ms.max(100)));
                    interval.reset();
                }
                _ = shutdown_rx.recv() => {
                    tracing::info!("Shutting down poll loop");
                    break;
                }
            }
        }
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
    use std::sync::Arc;

    use async_trait::async_trait;
    use symphony_domain::{Command, IssueId, validate_invariants};
    use symphony_tracker::{TrackerClient, TrackerError, TrackerIssue, TrackerState};
    use tokio::sync::Mutex;

    use super::*;

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

    fn issue(id: &str) -> IssueId {
        IssueId(id.to_owned())
    }

    fn tracker_issue(id: &str) -> TrackerIssue {
        TrackerIssue {
            id: issue(id),
            identifier: id.to_owned(),
            title: String::new(),
            state: TrackerState::new("Todo"),
            priority: None,
            description: None,
            created_at: None,
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
        tracker.set_candidates(Vec::new()).await;
        let runtime = Runtime::new(Arc::clone(&tracker));

        let running_issue = issue("SYM-200");
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
                    title: String::new(),
                    state: TrackerState::new("In Progress"),
                    priority: None,
                    description: None,
                    created_at: None,
                },
                TrackerIssue {
                    id: issue("SYM-5"),
                    identifier: "SYM-5".to_owned(),
                    title: String::new(),
                    state: TrackerState::new("In Progress"),
                    priority: None,
                    description: None,
                    created_at: None,
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
