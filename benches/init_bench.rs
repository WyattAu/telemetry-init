//! Criterion bench: one-shot init cost as a smoke signal.
//!
//! Measures subscriber construction (filter + fmt layer, with the OTLP
//! tracer layer when configured) — the bulk of `Telemetry::init` minus
//! the global install, which is once-per-process by nature and therefore
//! not repeatable inside a bench harness.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use telemetry_kit::{build_subscriber, LogFormat, TelemetryConfig};

fn bench_init_pretty(c: &mut Criterion) {
    let config = TelemetryConfig::new("bench-svc").log_format(LogFormat::Pretty);
    c.bench_function("init_subscriber_pretty", |b| {
        b.iter(|| black_box(build_subscriber(black_box(&config)).expect("build subscriber")));
    });
}

#[cfg(feature = "json")]
fn bench_init_json(c: &mut Criterion) {
    let config = TelemetryConfig::new("bench-svc"); // Json default
    c.bench_function("init_subscriber_json", |b| {
        b.iter(|| black_box(build_subscriber(black_box(&config)).expect("build subscriber")));
    });
}

#[cfg(feature = "json")]
criterion_group!(benches, bench_init_pretty, bench_init_json);
#[cfg(not(feature = "json"))]
criterion_group!(benches, bench_init_pretty);
criterion_main!(benches);
