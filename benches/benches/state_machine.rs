use criterion::{black_box, criterion_group, criterion_main, Criterion};
use symphony_domain::{IssueId, OrchestratorState};

fn benchmark_state_clone(c: &mut Criterion) {
    let mut state = OrchestratorState::default();

    // Populate with some data
    for i in 0..100 {
        state.claimed.insert(IssueId(format!("SYM-{}", i)));
    }

    c.bench_function("state_clone_100_issues", |b| {
        b.iter(|| black_box(state.clone()));
    });
}

fn benchmark_state_serialize(c: &mut Criterion) {
    let mut state = OrchestratorState::default();

    for i in 0..100 {
        state.claimed.insert(IssueId(format!("SYM-{}", i)));
    }

    c.bench_function("state_serialize_100_issues", |b| {
        b.iter(|| serde_json::to_string(black_box(&state)).unwrap());
    });
}

fn benchmark_state_deserialize(c: &mut Criterion) {
    let mut state = OrchestratorState::default();

    for i in 0..100 {
        state.claimed.insert(IssueId(format!("SYM-{}", i)));
    }

    let json = serde_json::to_string(&state).unwrap();

    c.bench_function("state_deserialize_100_issues", |b| {
        b.iter(|| serde_json::from_str::<OrchestratorState>(black_box(&json)).unwrap());
    });
}

criterion_group!(
    benches,
    benchmark_state_clone,
    benchmark_state_serialize,
    benchmark_state_deserialize
);
criterion_main!(benches);
