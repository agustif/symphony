use criterion::{black_box, criterion_group, criterion_main, Criterion};

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
                black_box(sanitize_key(input));
            }
        });
    });
}

fn sanitize_key(identifier: &str) -> String {
    let mut sanitized = String::with_capacity(identifier.len().saturating_mul(4));

    for byte in identifier.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' => {
                sanitized.push(byte as char);
            }
            b'_' => sanitized.push_str("__"),
            _ => {
                sanitized.push('_');
                sanitized.push_str(&format!("{:02X}", byte));
                sanitized.push('_');
            }
        }
    }

    if sanitized.is_empty() {
        "_".to_owned()
    } else {
        sanitized
    }
}

criterion_group!(benches, benchmark_sanitize_key);
criterion_main!(benches);
