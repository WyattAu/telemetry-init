//! [`TelemetryConfig`] — the builder that captures every estate default.

/// Default series budget handed to the metrics-kit registry.
#[cfg(feature = "metrics")]
pub const DEFAULT_METRICS_BUDGET: usize = 8192;

/// Default service version recorded when none is supplied.
pub const DEFAULT_SERVICE_VERSION: &str = "0.0.0";

/// Default `env-filter` directive when neither `RUST_LOG` nor an explicit
/// `log_level` override applies.
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// Default trace sample rate: record every span.
pub const DEFAULT_SAMPLE_RATE: f32 = 1.0;

/// Log output format for the subscriber's `fmt` layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum LogFormat {
    /// Single-line JSON objects — the estate default for services, so
    /// collectors can parse without regex. Requires the `json` feature
    /// (enabled by default); requesting it without the feature is a
    /// configuration error, never a silent fallback.
    #[default]
    Json,
    /// Human-oriented multi-line format with ANSI color when stderr is a
    /// TTY — for local development.
    Pretty,
}

/// Bootstrap configuration for the estate telemetry stack.
///
/// Built with [`TelemetryConfig::new`] and consumed by
/// [`Telemetry::init`](crate::Telemetry::init). Every setter is
/// chainable; unset fields keep the documented defaults.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    #[cfg_attr(not(feature = "otlp"), allow(dead_code))]
    pub(crate) service_name: String,
    pub(crate) service_version: String,
    pub(crate) log_level: String,
    pub(crate) log_format: LogFormat,
    #[cfg(feature = "metrics")]
    pub(crate) metrics_budget: usize,
    #[cfg(feature = "otlp")]
    pub(crate) otlp_endpoint: Option<String>,
    pub(crate) sample_rate: f32,
}

impl TelemetryConfig {
    /// Create a config for `service_name` with estate defaults:
    /// version `"0.0.0"`, `env-filter` `"info"` (overridden by `RUST_LOG`),
    /// JSON logs, a 8192-series metrics budget, no OTLP endpoint (no trace
    /// export), and a 1.0 sample rate.
    #[must_use]
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            service_version: DEFAULT_SERVICE_VERSION.to_owned(),
            log_level: DEFAULT_LOG_LEVEL.to_owned(),
            log_format: LogFormat::default(),
            #[cfg(feature = "metrics")]
            metrics_budget: DEFAULT_METRICS_BUDGET,
            #[cfg(feature = "otlp")]
            otlp_endpoint: None,
            sample_rate: DEFAULT_SAMPLE_RATE,
        }
    }

    /// Set the service version attached to OTLP resource attributes.
    #[must_use]
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.service_version = version.into();
        self
    }

    /// Set the log output format.
    #[must_use]
    pub fn log_format(mut self, format: LogFormat) -> Self {
        self.log_format = format;
        self
    }

    /// Set the `env-filter` directive string (e.g. `"info"`,
    /// `"debug,hyper=warn"`).
    ///
    /// `RUST_LOG`, when set and non-empty, always wins over this value.
    #[must_use]
    pub fn log_level(mut self, level: impl Into<String>) -> Self {
        self.log_level = level.into();
        self
    }

    /// Set the metrics-kit series budget (cardinality guard).
    #[cfg(feature = "metrics")]
    #[must_use]
    pub fn metrics_budget(mut self, budget: usize) -> Self {
        self.metrics_budget = budget;
        self
    }

    /// Set the OTLP endpoint (e.g. `"http://localhost:4317"`). Requires the
    /// `otlp` feature. With no endpoint configured, spans are never
    /// exported.
    #[cfg(feature = "otlp")]
    #[must_use]
    pub fn otlp_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.otlp_endpoint = Some(endpoint.into());
        self
    }

    /// Set the trace sample rate in `0.0..=1.0`; values outside the range
    /// are clamped. A `NaN` falls back to the default `1.0` rather than
    /// silently dropping every span.
    #[must_use]
    pub fn sample_rate(mut self, rate: f32) -> Self {
        self.sample_rate = if rate.is_nan() {
            DEFAULT_SAMPLE_RATE
        } else {
            rate.clamp(0.0, 1.0)
        };
        self
    }
}

#[cfg(test)]
mod tests {
    // Exact binary fractions (0.0, 0.5, 1.0, …) are compared — equality
    // is exact for these values, so float_cmp is a false positive here.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
    use super::*;
    use crate::error::TelemetryError;

    #[test]
    fn defaults_match_estate_documentation() {
        let cfg = TelemetryConfig::new("payments-api");
        assert_eq!(cfg.service_name, "payments-api");
        assert_eq!(cfg.service_version, DEFAULT_SERVICE_VERSION);
        assert_eq!(cfg.log_level, DEFAULT_LOG_LEVEL);
        assert_eq!(cfg.log_format, LogFormat::Json);
        assert_eq!(cfg.sample_rate, 1.0);
        #[cfg(feature = "metrics")]
        assert_eq!(cfg.metrics_budget, DEFAULT_METRICS_BUDGET);
        #[cfg(feature = "otlp")]
        assert!(cfg.otlp_endpoint.is_none());
    }

    #[test]
    fn builder_chain_sets_every_field() {
        let cfg = TelemetryConfig::new("svc")
            .version("1.2.3")
            .log_level("debug,hyper=warn")
            .log_format(LogFormat::Pretty)
            .sample_rate(0.25);
        assert_eq!(cfg.service_version, "1.2.3");
        assert_eq!(cfg.log_level, "debug,hyper=warn");
        assert_eq!(cfg.log_format, LogFormat::Pretty);
        assert_eq!(cfg.sample_rate, 0.25);

        #[cfg(feature = "metrics")]
        let cfg = cfg.metrics_budget(1024);
        #[cfg(feature = "otlp")]
        let cfg = cfg.otlp_endpoint("http://localhost:4317");
        #[cfg(feature = "metrics")]
        assert_eq!(cfg.metrics_budget, 1024);
        #[cfg(feature = "otlp")]
        assert_eq!(cfg.otlp_endpoint.as_deref(), Some("http://localhost:4317"));
        let _ = cfg;
    }

    #[test]
    fn setters_overwrite_previous_values() {
        let cfg = TelemetryConfig::new("svc")
            .log_level("info")
            .log_level("warn")
            .version("a")
            .version("b")
            .log_format(LogFormat::Pretty)
            .log_format(LogFormat::Json);
        assert_eq!(cfg.log_level, "warn");
        assert_eq!(cfg.service_version, "b");
        assert_eq!(cfg.log_format, LogFormat::Json);
    }

    #[test]
    fn sample_rate_clamps_into_unit_range() {
        assert_eq!(TelemetryConfig::new("s").sample_rate(2.0).sample_rate, 1.0);
        assert_eq!(TelemetryConfig::new("s").sample_rate(-0.5).sample_rate, 0.0);
        assert_eq!(
            TelemetryConfig::new("s").sample_rate(0.125).sample_rate,
            0.125
        );
        assert_eq!(TelemetryConfig::new("s").sample_rate(0.0).sample_rate, 0.0);
        assert_eq!(TelemetryConfig::new("s").sample_rate(1.0).sample_rate, 1.0);
    }

    #[test]
    fn sample_rate_nan_falls_back_to_full_sampling() {
        assert_eq!(
            TelemetryConfig::new("s").sample_rate(f32::NAN).sample_rate,
            DEFAULT_SAMPLE_RATE
        );
    }

    #[test]
    fn log_format_traits() {
        assert_eq!(LogFormat::default(), LogFormat::Json);
        assert_ne!(LogFormat::Json, LogFormat::Pretty);
        assert_eq!(format!("{:?}", LogFormat::Pretty), "Pretty");
        let copied = LogFormat::Json;
        assert_eq!(copied, LogFormat::Json);
    }

    #[test]
    fn config_is_clone_and_debug() {
        let cfg = TelemetryConfig::new("svc").version("9.9.9");
        let cloned = cfg.clone();
        assert_eq!(cloned.service_name, "svc");
        let debug = format!("{cfg:?}");
        assert!(debug.contains("TelemetryConfig"));
        assert!(debug.contains("svc"));
    }

    #[test]
    fn empty_service_name_is_accepted() {
        // The service name flows into OTLP resource attributes; the
        // collector, not this crate, owns name validation.
        let cfg = TelemetryConfig::new("");
        assert_eq!(cfg.service_name, "");
    }

    #[test]
    fn unparseable_log_level_is_a_typed_error() {
        assert!(matches!(
            crate::telemetry::build_filter("a=b=c"),
            Err(TelemetryError::InitFailed(_))
        ));
    }

    #[test]
    fn composite_directives_parse() {
        crate::telemetry::build_filter("info,hyper=warn").unwrap();
    }
}
