use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use symphony_domain::{reduce, Event, IssueId, OrchestratorState};

fn generate_events(count: usize) -> Vec<Event> {
    (0..count)
        .map(|i| {
            let issue_id = IssueId(format!("SYM-{}", i));
            Event::Claim(issue_id)
        })
        .collect()
}

fn benchmark_reducer_claim(c: &mut Criterion) {
    let mut group = c.benchmark_group("reducer_claim");

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let events = generate_events(size);
            b.iter(|| {
                let mut state = OrchestratorState::default();
                for event in &events {
                    let (new_state, _) = reduce(black_box(state), black_box(event.clone()));
                    state = new_state;
                }
                state
            });
        });
    }

    group.finish();
}

fn benchmark_reducer_full_lifecycle(c: &mut Criterion) {
    c.bench_function("reducer_full_lifecycle", |b| {
        b.iter(|| {
            let mut state = OrchestratorState::default();
            let issue_id = IssueId("SYM-1".into());

            // Claim
            let (new_state, _) = reduce(state, Event::Claim(issue_id.clone()));
            state = new_state;

            // Mark running
            let (new_state, _) = reduce(state, Event::MarkRunning(issue_id.clone()));
            state = new_state;

            // Release
            let (new_state, _) = reduce(state, Event::Release(issue_id));
            state = new_state;

            state
        });
    });
}

criterion_group!(
    benches,
    benchmark_reducer_claim,
    benchmark_reducer_full_lifecycle
);
criterion_main!(benches);
