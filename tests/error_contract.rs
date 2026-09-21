#![allow(clippy::unwrap_used, clippy::expect_used)]
//! API-contract checks from the downstream-crate perspective: these only
//! hold in a separate compilation unit (e.g. `#[non_exhaustive]` is
//! invisible inside the defining crate).

use telemetry_init::TelemetryError;

#[test]
fn non_exhaustive_error_forces_a_downstream_wildcard() {
    // Compiles only because `TelemetryError` is `#[non_exhaustive]` and
    // this file is a separate crate: future variants cannot break the
    // wildcard arm.
    fn describe(e: &TelemetryError) -> &'static str {
        match e {
            TelemetryError::InitFailed(_) => "init",
            TelemetryError::AlreadyInitialized => "already",
            _ => "future variant",
        }
    }
    assert_eq!(describe(&TelemetryError::AlreadyInitialized), "already");
    assert_eq!(describe(&TelemetryError::InitFailed("x".into())), "init");
}
