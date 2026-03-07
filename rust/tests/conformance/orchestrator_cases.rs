use std::collections::HashSet;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::json;
use symphony_config::RuntimeConfig;
use symphony_domain::{
    Command, Event, IssueId, OrchestratorState, RetryEntry, RunningEntry, Usage,
};
use symphony_runtime::{
    RetryPolicy, Runtime, WorkerExit, non_active_reconciliation_ids, terminal_reconciliation_ids,
    worker::{WorkerContext, WorkerError, WorkerLauncher, WorkerOutcome, WorkerProtocolUpdate},
};
use symphony_testkit::{FakeTracker, FakeTrackerResponse};
use symphony_tracker::{TrackerIssue, TrackerState};
use tokio::sync::Mutex;

#[derive(Default)]
struct MockWorkerLauncher {
    outcomes: Mutex<std::collections::VecDeque<WorkerOutcome>>,
    protocol_message: Option<String>,
}

impl MockWorkerLauncher {
    fn with_outcomes(outcomes: Vec<WorkerOutcome>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes.into()),
            protocol_message: None,
        }
    }

    fn with_outcomes_and_protocol_message(
        outcomes: Vec<WorkerOutcome>,
        protocol_message: impl Into<String>,
    ) -> Self {
        Self {
            outcomes: Mutex::new(outcomes.into()),
            protocol_message: Some(protocol_message.into()),
        }
    }
}

#[async_trait]
impl WorkerLauncher for MockWorkerLauncher {
    async fn launch(&self, ctx: WorkerContext) -> Result<WorkerOutcome, WorkerError> {
        if let Some(message) = self.protocol_message.clone()
            && let Some(protocol_updates) = &ctx.protocol_updates
        {
            let _ = protocol_updates.send(WorkerProtocolUpdate {
                event: Some("turn/failed".to_owned()),
                timestamp: Some(1_700_000_200),
                message: Some(message),
                ..WorkerProtocolUpdate::default()
            });
        }
        let mut outcomes = self.outcomes.lock().await;
        Ok(outcomes.pop_front().unwrap_or(WorkerOutcome::Completed))
    }
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("symphony-runtime-conformance-{label}-{nonce}"));
    fs::create_dir_all(&path).expect("temporary root should be creatable");
    path
}

#[tokio::test]
async fn dispatch_sorting_priority_and_recency() {
    let issue_1 = TrackerIssue::new(IssueId("1".into()), "SYM-1", TrackerState::new("Todo"))
        .with_priority(3)
        .with_created_at(1000);
    let issue_2 = TrackerIssue::new(IssueId("2".into()), "SYM-2", TrackerState::new("Todo"))
        .with_priority(1) // Higher priority
        .with_created_at(2000);
    let issue_3 = TrackerIssue::new(IssueId("3".into()), "SYM-3", TrackerState::new("Todo"))
        .with_priority(3)
        .with_created_at(500); // Older than issue 1

    let tracker = Arc::new(FakeTracker::with_default_issues(vec![
        issue_1.clone(),
        issue_2.clone(),
        issue_3.clone(),
    ]));
    let runtime = Runtime::new(tracker);

    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 3;

    let tick = runtime
        .run_tick(&config)
        .await
        .expect("tick should succeed");

    // Expected order: SYM-2 (prio 1), SYM-3 (prio 3, older), SYM-1 (prio 3, newer)
    assert_eq!(
        tick.commands,
        vec![
            Command::Dispatch(issue_2.id),
            Command::Dispatch(issue_3.id),
            Command::Dispatch(issue_1.id),
        ]
    );
}

#[tokio::test]
async fn exponential_backoff_conformance() {
    let issue_id = IssueId("SYM-1".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-1",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);
    let policy = RetryPolicy {
        continuation_delay: Duration::from_secs(1),
        failure_base_delay: Duration::from_secs(10),
        max_failure_backoff: Duration::from_secs(100),
    };

    // Set up issue as claimed and running before handling worker exit
    runtime
        .setup_running(issue_id.clone(), RunningEntry::default())
        .await;

    // First failure: 10s
    let res1 = runtime
        .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
        .await
        .unwrap();
    assert_eq!(res1.retry_schedule.unwrap().delay, Duration::from_secs(10));

    // Set up for second failure
    runtime
        .setup_running(issue_id.clone(), RunningEntry::default())
        .await;

    // Second failure: 20s
    let res2 = runtime
        .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
        .await
        .unwrap();
    assert_eq!(res2.retry_schedule.unwrap().delay, Duration::from_secs(20));

    // Capped: 100s
    for _ in 0..5 {
        runtime
            .setup_running(issue_id.clone(), RunningEntry::default())
            .await;
        let _ = runtime
            .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
            .await
            .unwrap();
    }
    runtime
        .setup_running(issue_id.clone(), RunningEntry::default())
        .await;
    let res_capped = runtime
        .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
        .await
        .unwrap();
    assert_eq!(
        res_capped.retry_schedule.unwrap().delay,
        Duration::from_secs(100)
    );
}

#[tokio::test]
async fn candidate_selection_skips_claimed_running_and_retrying_issues() {
    let issue_claimed = TrackerIssue::new(IssueId("1".into()), "SYM-1", TrackerState::new("Todo"));
    let issue_running = TrackerIssue::new(IssueId("2".into()), "SYM-2", TrackerState::new("Todo"));
    let issue_retrying = TrackerIssue::new(IssueId("3".into()), "SYM-3", TrackerState::new("Todo"));
    let issue_available =
        TrackerIssue::new(IssueId("4".into()), "SYM-4", TrackerState::new("Todo"));

    let tracker = Arc::new(FakeTracker::with_default_issues(vec![
        issue_claimed.clone(),
        issue_running.clone(),
        issue_retrying.clone(),
        issue_available.clone(),
    ]));
    let runtime = Runtime::new(tracker);

    runtime
        .update_agent(Event::Claim(issue_claimed.id.clone()))
        .await
        .expect("claim should apply");
    runtime
        .setup_running(issue_running.id.clone(), RunningEntry::default())
        .await;
    runtime
        .setup_running(issue_retrying.id.clone(), RunningEntry::default())
        .await;
    runtime
        .handle_worker_exit(
            issue_retrying.id.clone(),
            WorkerExit::Continuation,
            &RetryPolicy::default(),
        )
        .await
        .expect("retry should apply");

    let mut config = RuntimeConfig::default();
    config.agent.max_concurrent_agents = 10;

    let tick = runtime
        .run_tick(&config)
        .await
        .expect("tick should succeed");
    assert_eq!(tick.commands, vec![Command::Dispatch(issue_available.id)]);
}

#[test]
fn terminal_reconciliation_releases_claimed_running_and_retrying() {
    let issue_terminal = IssueId("SYM-10".into());
    let issue_other = IssueId("SYM-20".into());

    let mut state = OrchestratorState::default();
    state.claimed.insert(issue_terminal.clone());
    state.claimed.insert(issue_other.clone());
    state
        .running
        .insert(issue_terminal.clone(), RunningEntry::default());
    state.retry_attempts.insert(
        issue_terminal.clone(),
        RetryEntry {
            attempt: 2,
            ..RetryEntry::default()
        },
    );

    let terminal_ids = HashSet::from([issue_terminal.clone()]);
    let releases = terminal_reconciliation_ids(&state, &terminal_ids);

    assert_eq!(releases, vec![issue_terminal]);
}

#[test]
fn non_active_reconciliation_preserves_running_retrying_and_active_candidates() {
    let issue_stale = IssueId("SYM-11".into());
    let issue_running = IssueId("SYM-12".into());
    let issue_retrying = IssueId("SYM-13".into());
    let issue_active_candidate = IssueId("SYM-14".into());

    let mut state = OrchestratorState::default();
    state.claimed.insert(issue_stale.clone());
    state.claimed.insert(issue_running.clone());
    state.claimed.insert(issue_retrying.clone());
    state.claimed.insert(issue_active_candidate.clone());
    state.running.insert(issue_running, RunningEntry::default());
    state.retry_attempts.insert(
        issue_retrying,
        RetryEntry {
            attempt: 1,
            ..RetryEntry::default()
        },
    );

    let active_ids = HashSet::from([issue_active_candidate]);
    let stale = non_active_reconciliation_ids(&state, &active_ids);

    assert_eq!(stale, vec![issue_stale]);
}

#[tokio::test]
async fn stall_detection_uses_last_protocol_activity_timestamp() {
    let issue_id = IssueId("SYM-15".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-15",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_secs();

    runtime
        .setup_running(
            issue_id.clone(),
            RunningEntry {
                identifier: Some("SYM-15".to_owned()),
                started_at: Some(now_secs.saturating_sub(600)),
                ..RunningEntry::default()
            },
        )
        .await;
    runtime
        .handle_protocol_update(
            issue_id.clone(),
            Some("thread-SYM-15:turn-1".to_owned()),
            Some("thread-SYM-15".to_owned()),
            Some("turn-1".to_owned()),
            None,
            Some("turn/update".to_owned()),
            Some(now_secs.saturating_sub(30)),
            Some("recent activity".to_owned()),
            None,
            None,
        )
        .await
        .expect("protocol update should apply");

    let mut config = RuntimeConfig::default();
    config.codex.stall_timeout_ms = 300_000;

    let tick = runtime
        .run_tick(&config)
        .await
        .expect("tick should succeed");

    assert!(
        tick.commands.is_empty(),
        "recent protocol activity should suppress stale restart"
    );
}

#[tokio::test]
async fn stale_protocol_activity_triggers_retry_restart() {
    let issue_id = IssueId("SYM-16".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-16",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_secs();

    runtime
        .setup_running(
            issue_id.clone(),
            RunningEntry {
                identifier: Some("SYM-16".to_owned()),
                started_at: Some(now_secs.saturating_sub(900)),
                ..RunningEntry::default()
            },
        )
        .await;
    runtime
        .handle_protocol_update(
            issue_id.clone(),
            Some("thread-SYM-16:turn-4".to_owned()),
            Some("thread-SYM-16".to_owned()),
            Some("turn-4".to_owned()),
            None,
            Some("turn/update".to_owned()),
            Some(now_secs.saturating_sub(360)),
            Some("turn 4 active".to_owned()),
            None,
            None,
        )
        .await
        .expect("protocol update should apply");

    let mut config = RuntimeConfig::default();
    config.codex.stall_timeout_ms = 300_000;

    let tick = runtime
        .run_tick(&config)
        .await
        .expect("tick should succeed");

    assert_eq!(
        tick.commands,
        vec![Command::ScheduleRetry {
            issue_id,
            attempt: 1,
        }]
    );
}

#[tokio::test]
async fn worker_exit_continuation_uses_continuation_retry_policy() {
    let issue_id = IssueId("SYM-17".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-17",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);
    let policy = RetryPolicy {
        continuation_delay: Duration::from_secs(3),
        failure_base_delay: Duration::from_secs(20),
        max_failure_backoff: Duration::from_secs(60),
    };

    runtime
        .setup_running(issue_id.clone(), RunningEntry::default())
        .await;

    let result = runtime
        .handle_worker_exit(issue_id.clone(), WorkerExit::Continuation, &policy)
        .await
        .expect("continuation exit should succeed");

    assert_eq!(
        result.commands,
        vec![Command::ScheduleRetry {
            issue_id: issue_id.clone(),
            attempt: 1,
        }]
    );
    assert_eq!(
        result.retry_schedule,
        Some(symphony_runtime::RetrySchedule {
            issue_id: issue_id.clone(),
            attempt: 1,
            kind: symphony_runtime::RetryKind::Continuation,
            delay: Duration::from_secs(3),
        })
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
async fn permanent_failure_releases_claim_without_retrying() {
    let issue_id = IssueId("SYM-18".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-18",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    runtime
        .setup_running(issue_id.clone(), RunningEntry::default())
        .await;

    let result = runtime
        .handle_worker_exit(
            issue_id.clone(),
            WorkerExit::PermanentFailure,
            &RetryPolicy::default(),
        )
        .await
        .expect("permanent failure should release claim");

    assert_eq!(result.retry_schedule, None);
    assert_eq!(
        result.commands,
        vec![Command::ReleaseClaim(issue_id.clone())]
    );
    assert!(!result.state.claimed.contains(&issue_id));
    assert!(!result.state.running.contains_key(&issue_id));
    assert!(!result.state.retry_attempts.contains_key(&issue_id));
}

#[tokio::test]
async fn retryable_failure_surfaces_retry_metadata_in_state_snapshot() {
    let issue_id = IssueId("SYM-19".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-19",
        TrackerState::new("Todo"),
    )]));
    let runtime = Arc::new(Runtime::with_worker_launcher(
        tracker,
        Arc::new(MockWorkerLauncher::with_outcomes_and_protocol_message(
            vec![WorkerOutcome::RetryableFailure],
            "worker crashed",
        )),
    ));
    let config = Arc::new(tokio::sync::RwLock::new(RuntimeConfig::default()));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
    let runtime_task = tokio::spawn(Arc::clone(&runtime).run_poll_loop(
        config,
        refresh_rx,
        shutdown_tx.subscribe(),
    ));

    tokio::time::sleep(Duration::from_millis(50)).await;

    let snapshot = runtime.state_snapshot().await;
    assert_eq!(snapshot.runtime.running, 0);
    assert_eq!(snapshot.runtime.retrying, 1);
    let issue = snapshot
        .issues
        .iter()
        .find(|issue| issue.id == issue_id)
        .expect("retrying issue should appear in snapshot");
    assert_eq!(issue.identifier, "SYM-19");
    assert_eq!(issue.state, "Retrying");
    assert_eq!(issue.retry_attempts, 1);
    assert_eq!(issue.retry_error.as_deref(), Some("worker crashed"));
    assert!(issue.retry_due_at.is_some());

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");
}

#[tokio::test]
async fn protocol_updates_flow_into_runtime_state_snapshot_surface() {
    let issue_id = IssueId("SYM-20".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-20",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    runtime
        .setup_running(
            issue_id.clone(),
            RunningEntry {
                identifier: Some("SYM-20".to_owned()),
                started_at: Some(1_700_000_000),
                ..RunningEntry::default()
            },
        )
        .await;

    runtime
        .handle_protocol_update(
            issue_id.clone(),
            Some("thread-20-turn-1".to_owned()),
            Some("thread-20".to_owned()),
            Some("turn-1".to_owned()),
            Some(4242),
            Some("turn/start".to_owned()),
            Some(1_700_000_100),
            Some("processing".to_owned()),
            Some(Usage {
                input_tokens: 12,
                output_tokens: 8,
                total_tokens: 20,
            }),
            Some(json!({
                "primary": {
                    "remaining": 7
                }
            })),
        )
        .await
        .expect("protocol update should populate state snapshot");

    let snapshot = runtime.state_snapshot().await;
    assert_eq!(snapshot.runtime.running, 1);
    assert_eq!(snapshot.runtime.total_tokens, 20);
    assert_eq!(
        snapshot.runtime.rate_limits,
        Some(json!({
            "primary": {
                "remaining": 7
            }
        }))
    );
    assert_eq!(
        snapshot.runtime.activity.last_runtime_activity_at,
        Some(1_700_000_100)
    );

    let issue = snapshot
        .issues
        .iter()
        .find(|issue| issue.id == issue_id)
        .expect("running issue should appear in state snapshot");
    assert_eq!(issue.session_id.as_deref(), Some("thread-20-turn-1"));
    assert_eq!(issue.turn_count, 1);
    assert_eq!(issue.last_event.as_deref(), Some("turn/start"));
    assert_eq!(issue.last_message.as_deref(), Some("processing"));
    assert_eq!(issue.total_tokens, 20);
}

#[tokio::test]
async fn run_tick_keeps_running_issue_when_state_refresh_fails() {
    let issue = TrackerIssue::new(
        IssueId("SYM-21".into()),
        "SYM-21",
        TrackerState::new("Todo"),
    );
    let tracker = Arc::new(FakeTracker::scripted(vec![
        FakeTrackerResponse::issues(vec![issue.clone()]),
        FakeTrackerResponse::error("refresh timeout"),
    ]));
    let runtime = Runtime::new(tracker.clone());

    runtime
        .setup_running(
            issue.id.clone(),
            RunningEntry {
                identifier: Some("SYM-21".to_owned()),
                ..RunningEntry::default()
            },
        )
        .await;

    let tick = runtime
        .run_tick(&RuntimeConfig::default())
        .await
        .expect("tick should preserve running state when refresh fails");

    assert!(tick.commands.is_empty());
    let state = runtime.state().await;
    assert!(state.claimed.contains(&issue.id));
    assert!(state.running.contains_key(&issue.id));
    assert_eq!(tracker.fetch_count(), 2);
}

#[tokio::test]
async fn startup_cleanup_removes_only_terminal_workspaces_on_poll_loop_boot() {
    let terminal_issue = TrackerIssue::new(
        IssueId("SYM-23".into()),
        "SYM-TERM",
        TrackerState::new("Done"),
    );
    let active_issue = TrackerIssue::new(
        IssueId("SYM-24".into()),
        "SYM-ACTIVE",
        TrackerState::new("Todo"),
    );
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![
        terminal_issue,
        active_issue,
    ]));
    let runtime = Arc::new(Runtime::new(tracker));
    let root = temp_root("startup-cleanup");
    fs::create_dir_all(root.join("SYM-TERM")).expect("terminal workspace should be created");
    fs::create_dir_all(root.join("SYM-ACTIVE")).expect("active workspace should be created");

    let mut config = RuntimeConfig::default();
    config.workspace.root = root.clone();
    config.polling.interval_ms = 60_000;
    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
    let runtime_task = tokio::spawn(Arc::clone(&runtime).run_poll_loop(
        config,
        refresh_rx,
        shutdown_tx.subscribe(),
    ));

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        !root.join("SYM-TERM").exists(),
        "startup cleanup should remove terminal workspaces"
    );
    assert!(
        root.join("SYM-ACTIVE").exists(),
        "startup cleanup should preserve active workspaces"
    );

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn startup_cleanup_ignores_tracker_failures_and_preserves_workspaces() {
    let tracker = Arc::new(FakeTracker::scripted(vec![FakeTrackerResponse::error(
        "tracker unavailable",
    )]));
    let runtime = Arc::new(Runtime::new(tracker));
    let root = temp_root("startup-cleanup-error");
    fs::create_dir_all(root.join("SYM-TERM")).expect("workspace should be created");

    let mut config = RuntimeConfig::default();
    config.workspace.root = root.clone();
    config.polling.interval_ms = 60_000;
    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
    let runtime_task = tokio::spawn(Arc::clone(&runtime).run_poll_loop(
        config,
        refresh_rx,
        shutdown_tx.subscribe(),
    ));

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        root.join("SYM-TERM").exists(),
        "startup cleanup should preserve workspaces when tracker refresh fails"
    );

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn terminal_reconciliation_cleanup_falls_back_after_before_remove_failure() {
    let issue_id = IssueId("SYM-25".into());
    // The poll loop calls startup_terminal_workspace_cleanup first (consuming one fetch),
    // then run_tick calls fetch_candidates (consuming another), then fetch_states_by_ids
    // (consuming one more). We need three scripted responses:
    // 1. Startup cleanup fetch → issue with Todo state (not terminal, so not cleaned up)
    // 2. Tick candidate fetch → issue with Todo state (shows as running)
    // 3. State refresh → issue with Done state (terminal → triggers terminal cleanup)
    let tracker = Arc::new(FakeTracker::scripted(vec![
        FakeTrackerResponse::issues(vec![TrackerIssue::new(
            issue_id.clone(),
            "SYM-TERM-FALLBACK",
            TrackerState::new("Todo"),
        )]),
        FakeTrackerResponse::issues(vec![TrackerIssue::new(
            issue_id.clone(),
            "SYM-TERM-FALLBACK",
            TrackerState::new("Todo"),
        )]),
        FakeTrackerResponse::issues(vec![TrackerIssue::new(
            issue_id.clone(),
            "SYM-TERM-FALLBACK",
            TrackerState::new("Done"),
        )]),
    ]));
    let runtime = Arc::new(Runtime::new(tracker));
    let root = temp_root("terminal-cleanup-fallback");
    fs::create_dir_all(root.join("SYM-TERM-FALLBACK")).expect("workspace should be created");

    runtime
        .setup_running(
            issue_id.clone(),
            RunningEntry {
                identifier: Some("SYM-TERM-FALLBACK".to_owned()),
                tracker_state: Some("Todo".to_owned()),
                ..RunningEntry::default()
            },
        )
        .await;

    let mut config = RuntimeConfig::default();
    config.workspace.root = root.clone();
    config.polling.interval_ms = 60_000;
    config.hooks.before_remove = Some("definitely-not-a-real-command-symphony".to_owned());
    config.hooks.timeout_ms = 2_000;
    let config = Arc::new(tokio::sync::RwLock::new(config));
    let (_refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
    let runtime_task = tokio::spawn(Arc::clone(&runtime).run_poll_loop(
        config,
        refresh_rx,
        shutdown_tx.subscribe(),
    ));

    tokio::time::sleep(Duration::from_millis(500)).await;

    assert!(
        !root.join("SYM-TERM-FALLBACK").exists(),
        "terminal reconciliation should remove the workspace even when before_remove fails"
    );
    let state = runtime.state().await;
    assert!(!state.claimed.contains(&issue_id));
    assert!(!state.running.contains_key(&issue_id));
    assert!(!state.retry_attempts.contains_key(&issue_id));

    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn real_worker_launcher_profile_is_opt_in_only() {
    if std::env::var(super::REAL_CONFORMANCE_OPT_IN_ENV)
        .ok()
        .is_none_or(|value| !matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
    {
        super::skip_real_integration(
            "real_worker_launcher_profile_is_opt_in_only",
            format!(
                "set {}=1 to enable worker-launcher smoke runs",
                super::REAL_CONFORMANCE_OPT_IN_ENV
            ),
        );
        return;
    }

    let issue_id = IssueId("SYM-22".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-22",
        TrackerState::new("Todo"),
    )]));
    let runtime = Arc::new(Runtime::with_worker_launcher(
        tracker,
        Arc::new(MockWorkerLauncher::with_outcomes(vec![
            WorkerOutcome::Completed,
        ])),
    ));
    let config = Arc::new(tokio::sync::RwLock::new(RuntimeConfig::default()));
    let (refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(1);
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
    let runtime_task = tokio::spawn(Arc::clone(&runtime).run_poll_loop(
        config,
        refresh_rx,
        shutdown_tx.subscribe(),
    ));

    refresh_tx
        .send(())
        .await
        .expect("opted-in real worker launcher smoke run should enqueue refresh");
    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = shutdown_tx.send(());
    runtime_task
        .await
        .expect("poll loop should shut down cleanly");

    let state = runtime.state().await;
    assert!(!state.claimed.contains(&issue_id));
    assert!(!state.running.contains_key(&issue_id));
}

// =============================================================================
// C2.2.2: Session reuse, backoff parity, and child-stop conformance
// =============================================================================

#[tokio::test]
async fn continuation_uses_short_fixed_delay_not_exponential_backoff() {
    let issue_id = IssueId("SYM-C221".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-C221",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    let policy = RetryPolicy {
        continuation_delay: Duration::from_secs(1),
        failure_base_delay: Duration::from_secs(10),
        max_failure_backoff: Duration::from_secs(300),
    };

    runtime
        .setup_running(issue_id.clone(), RunningEntry::default())
        .await;

    let continuation_result = runtime
        .handle_worker_exit(issue_id.clone(), WorkerExit::Continuation, &policy)
        .await
        .expect("continuation exit should succeed");

    assert_eq!(
        continuation_result.retry_schedule.unwrap().delay,
        Duration::from_secs(1),
        "continuation retry should use short fixed delay, not exponential backoff"
    );
}

#[tokio::test]
async fn retryable_failure_uses_exponential_backoff_not_continuation_delay() {
    let issue_id = IssueId("SYM-C222".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-C222",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    let policy = RetryPolicy {
        continuation_delay: Duration::from_secs(1),
        failure_base_delay: Duration::from_secs(10),
        max_failure_backoff: Duration::from_secs(300),
    };

    runtime
        .setup_running(issue_id.clone(), RunningEntry::default())
        .await;

    let failure_result = runtime
        .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
        .await
        .expect("failure exit should succeed");

    assert_eq!(
        failure_result.retry_schedule.unwrap().delay,
        Duration::from_secs(10),
        "failure retry should use exponential backoff base, not continuation delay"
    );
}

#[tokio::test]
async fn backoff_caps_at_max_failure_backoff() {
    let issue_id = IssueId("SYM-C223".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-C223",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    let policy = RetryPolicy {
        continuation_delay: Duration::from_secs(1),
        failure_base_delay: Duration::from_secs(10),
        max_failure_backoff: Duration::from_secs(50),
    };

    for attempt in 1..=10 {
        runtime
            .setup_running(issue_id.clone(), RunningEntry::default())
            .await;

        let result = runtime
            .handle_worker_exit(issue_id.clone(), WorkerExit::RetryableFailure, &policy)
            .await
            .expect("failure exit should succeed");

        let delay = result.retry_schedule.unwrap().delay;
        assert!(
            delay <= Duration::from_secs(50),
            "backoff delay {}s at attempt {} should be capped at max_failure_backoff",
            delay.as_secs(),
            attempt
        );
    }
}

#[tokio::test]
async fn continuation_turns_preserve_claim_and_issue_context() {
    let issue_id = IssueId("SYM-C224".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-C224",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    runtime
        .update_agent(Event::Claim(issue_id.clone()))
        .await
        .expect("claim should apply");

    runtime
        .setup_running(
            issue_id.clone(),
            RunningEntry {
                identifier: Some("SYM-C224".to_owned()),
                ..RunningEntry::default()
            },
        )
        .await;

    let result = runtime
        .handle_worker_exit(
            issue_id.clone(),
            WorkerExit::Continuation,
            &RetryPolicy::default(),
        )
        .await
        .expect("continuation should preserve context");

    assert!(
        result.state.claimed.contains(&issue_id),
        "continuation should preserve claim"
    );
    assert!(
        result.state.retry_attempts.contains_key(&issue_id),
        "continuation should schedule retry"
    );
    assert!(
        !result.state.running.contains_key(&issue_id),
        "continuation should remove from running"
    );
}

#[tokio::test]
async fn permanent_failure_releases_claim_and_removes_from_retry_queue() {
    let issue_id = IssueId("SYM-C225".into());
    let tracker = Arc::new(FakeTracker::with_default_issues(vec![TrackerIssue::new(
        issue_id.clone(),
        "SYM-C225",
        TrackerState::new("Todo"),
    )]));
    let runtime = Runtime::new(tracker);

    runtime
        .update_agent(Event::Claim(issue_id.clone()))
        .await
        .expect("claim should apply");

    runtime
        .setup_retry(
            issue_id.clone(),
            RetryEntry {
                attempt: 3,
                ..RetryEntry::default()
            },
        )
        .await;

    runtime
        .setup_running(issue_id.clone(), RunningEntry::default())
        .await;

    let result = runtime
        .handle_worker_exit(
            issue_id.clone(),
            WorkerExit::PermanentFailure,
            &RetryPolicy::default(),
        )
        .await
        .expect("permanent failure should release claim");

    assert!(
        !result.state.claimed.contains(&issue_id),
        "permanent failure should release claim"
    );
    assert!(
        !result.state.running.contains_key(&issue_id),
        "permanent failure should remove from running"
    );
    assert!(
        !result.state.retry_attempts.contains_key(&issue_id),
        "permanent failure should remove from retry queue"
    );
    assert!(
        result
            .commands
            .contains(&Command::ReleaseClaim(issue_id.clone())),
        "permanent failure should emit ReleaseClaim command"
    );
}
