use criterion::{black_box, criterion_group, criterion_main, Criterion};
use symphony_workspace::sanitize_workspace_key;

fn benchmark_sanitize_key(c: &mut Criterion) {
    let inputs = vec![
        "SYM-123",
        "issue-with-many-words",
        "Issue_With_Underscores",
        "issue.with.dots",
        "issue/with/slashes",
        "issue:with:colons",
    ];

    c.bench_function("sanitize_workspace_key", |b| {
        b.iter(|| {
            for input in &inputs {
                black_box(sanitize_workspace_key(input));
            }
        });
    });
}

criterion_group!(benches, benchmark_sanitize_key);
criterion_main!(benches);
