#![cfg(feature = "metrics")]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Global-subscriber lifecycle: everything that touches
//! `Telemetry::init` lives in ONE test function.
//!
//! The global tracing subscriber is a once-per-process resource, so two
//! tests that both call `init` would race under cargo's default
//! multi-threaded harness (one succeeds, the other gets
//! `AlreadyInitialized` — or worse, the reverse, depending on order).
//! Scoping all global-state assertions into a single deterministic
//! function avoids both the race and cross-test log pollution; the
//! process dies with the test, so no other binary is affected.

use telemetry_init::{LogFormat, Telemetry, TelemetryConfig, TelemetryError};

/// The full bootstrap → double-init → metrics surface → shutdown → drop
/// lifecycle, in the exact order a service would exercise it.
#[test]
fn full_lifecycle_in_one_deterministic_test() {
    // 1. First init succeeds and hands out the metrics registry.
    let telemetry = Telemetry::init(
        TelemetryConfig::new("it-svc")
            .version("0.1.0")
            .log_format(LogFormat::Json)
            .log_level("info")
            .metrics_budget(4),
    )
    .expect("first init must succeed");

    // 2. Second init: AlreadyInitialized, not a panic.
    let err = Telemetry::init(TelemetryConfig::new("second-svc")).unwrap_err();
    assert!(
        matches!(err, TelemetryError::AlreadyInitialized),
        "expected AlreadyInitialized, got {err:?}"
    );

    // 3. Metrics surface: register + record + render through
    //    Telemetry::metrics().
    let registry = telemetry.metrics();
    let requests = registry
        .counter("it_requests_total", "Total integration requests.", &[])
        .expect("register counter");
    requests.inc();
    let text = registry.render();
    assert!(text.contains("it_requests_total 1"), "got: {text}");

    // 4. The configured cardinality budget is honored: 4-series budget
    //    means exactly 4 series land and everything after is rejected.
    for i in 0..8 {
        let name = format!("it_budget_{i}_total");
        let _ = registry.counter(&name, "Budget filler.", &[]);
    }
    assert_eq!(registry.series_count(), 4);

    // 5. Events flow through the installed global subscriber (smoke: the
    //    stack is live and must not panic on the hot path).
    tracing::info!("telemetry-init integration event");

    // 6. Shutdown is idempotent: Ok on the first call, Ok on every later
    //    call.
    telemetry.shutdown().expect("first shutdown");
    telemetry.shutdown().expect("second shutdown");

    // 7. Drop after explicit shutdown: best-effort flush short-circuits,
    //    no panic.
    drop(telemetry);
}

/// Pure config checks that do not need the global subscriber — kept here
/// so the no-global contract of this file stays documented in one place.
#[test]
fn public_builder_chain_is_fully_chainable() {
    let cfg = TelemetryConfig::new("cfg-svc")
        .version("9.9.9")
        .log_format(LogFormat::Pretty)
        .log_level("debug")
        .metrics_budget(64)
        .sample_rate(0.5);
    // Construction only: every init call lives in the single lifecycle
    // test above so the global subscriber is never raced.
    let _ = cfg;
}
