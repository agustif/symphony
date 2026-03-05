#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

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
}

impl<T: TrackerClient> Runtime<T> {
    pub fn new(tracker: Arc<T>) -> Self {
        Self {
            tracker,
            state: Mutex::new(OrchestratorState::default()),
        }
    }

    pub async fn run_tick(&self, max_concurrent_agents: usize) -> Result<TickResult, RuntimeError> {
        let issues = self.tracker.fetch_candidates().await?;
        let mut candidate_ids: Vec<IssueId> = issues.into_iter().map(|issue| issue.id).collect();
        candidate_ids.sort_by(|left, right| left.0.cmp(&right.0));
        let candidate_set: HashSet<IssueId> = candidate_ids.iter().cloned().collect();

        let mut state_guard = self.state.lock().await;
        let mut state = state_guard.clone();
        let mut commands = Vec::new();

        for issue_id in non_active_reconciliation_ids(&state, &candidate_set) {
            state = apply_event(state, Event::Release(issue_id), &mut commands)?;
        }

        let mut available_slots = max_concurrent_agents.saturating_sub(state.running.len());
        for issue_id in candidate_ids {
            if available_slots == 0 {
                break;
            }
            if state.claimed.contains(&issue_id)
                || state.running.contains(&issue_id)
                || state.retry_attempts.contains_key(&issue_id)
            {
                continue;
            }
            state = apply_event(state, Event::Claim(issue_id.clone()), &mut commands)?;
            state = apply_event(state, Event::MarkRunning(issue_id), &mut commands)?;
            available_slots = available_slots.saturating_sub(1);
        }

        validate_invariants(&state)?;
        *state_guard = state.clone();

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

        Ok(WorkerExitResult {
            state,
            commands,
            retry_schedule,
        })
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
}

pub fn non_active_reconciliation_ids(
    state: &OrchestratorState,
    active_ids: &HashSet<IssueId>,
) -> Vec<IssueId> {
    let mut stale: Vec<IssueId> = state
        .claimed
        .iter()
        .filter(|issue_id| {
            !state.running.contains(*issue_id)
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
            .iter()
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
            state: TrackerState::new("Todo"),
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

        let first_tick = runtime
            .run_tick(1)
            .await
            .expect("dispatch tick should succeed");
        assert_eq!(
            first_tick.commands,
            vec![Command::Dispatch(issue_id.clone())],
        );

        let state = runtime.state().await;
        assert!(state.claimed.contains(&issue_id));
        assert!(state.running.contains(&issue_id));
        assert_eq!(validate_invariants(&state), Ok(()));

        let second_tick = runtime
            .run_tick(1)
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
            state.running.insert(running_issue.clone());
        }

        let tick = runtime
            .run_tick(3)
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
        assert!(state.running.contains(&running_issue));
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
            state.running.insert(issue_id.clone());
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
        assert!(!result.state.running.contains(&issue_id));
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
            state.running.insert(issue_id.clone());
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
        assert!(!result.state.running.contains(&issue_id));
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
        state.running.insert(issue_active.clone());

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

        let tick = runtime
            .run_tick(3)
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
}
