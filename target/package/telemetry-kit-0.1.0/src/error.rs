//! Errors produced by telemetry initialization and shutdown.

use thiserror::Error;

/// Initialization and shutdown failures.
///
/// Documented failure modes are part of the API contract; every variant
/// lists the condition that produces it.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TelemetryError {
    /// A configuration or construction step failed before (or after) the
    /// subscriber could be installed. The payload carries the underlying
    /// message — an invalid `env-filter` directive, an unbuildable OTLP
    /// exporter, or a failed shutdown flush.
    #[error("telemetry init failed: {0}")]
    InitFailed(String),
    /// The global tracing subscriber is a once-per-process resource:
    /// [`tracing::subscriber::set_global_default`] was already called by
    /// this crate, another crate, or an earlier `Telemetry::init`. A
    /// second bootstrap returns this error instead of panicking.
    #[error("telemetry already initialized: the global subscriber is installed once per process")]
    AlreadyInitialized,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_informative() {
        let e = TelemetryError::InitFailed("invalid directive".into());
        assert_eq!(e.to_string(), "telemetry init failed: invalid directive");
        assert_eq!(
            TelemetryError::AlreadyInitialized.to_string(),
            "telemetry already initialized: the global subscriber is installed once per process"
        );
    }

    #[test]
    fn is_std_error_with_source_none() {
        let e = TelemetryError::InitFailed("boom".into());
        let std_err: &dyn std::error::Error = &e;
        assert!(std_err.to_string().contains("boom"));
        assert!(std_err.source().is_none());
    }

    #[test]
    fn debug_and_clone_round_trip() {
        let e = TelemetryError::InitFailed("a".into());
        let cloned = e.clone();
        assert_eq!(format!("{e:?}"), r#"InitFailed("a")"#);
        assert_eq!(e, cloned);
        assert_ne!(e, TelemetryError::AlreadyInitialized);
    }
}
