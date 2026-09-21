//! One-call observability bootstrap for Rust services.
//!
//! `telemetry-kit` replaces the estate's hand-wired
//! `tracing_subscriber` + `otelkit` + `metrics-kit` init blocks (~40–80
//! lines each, six dialects) with a single fallible call that wires, in
//! estate-default shape:
//!
//! - **Logs** — `tracing_subscriber` with an `EnvFilter` (the configured
//!   directive, overridden by `RUST_LOG`) and a `fmt` layer: single-line
//!   JSON by default, human format for local dev.
//! - **Metrics** — a lock-free
//!   [`metrics_kit::Registry`] with a cardinality budget, handed to you
//!   as an [`Arc`] so hot-path handles register through it (feature
//!   `metrics`, default on).
//! - **Traces** — an `opentelemetry-otlp` exporter wired as a
//!   `tracing-opentelemetry` layer with `service.name`/`service.version`
//!   resource attributes and parent-based ratio sampling (feature
//!   `otlp`, default off).
//!
//! # Design
//!
//! - **One init, typed failure.** [`Telemetry::init`] returns
//!   [`TelemetryError`]; double-initialization — the global subscriber is
//!   a once-per-process resource — is [`TelemetryError::AlreadyInitialized`],
//!   never a panic.
//! - **No hidden globals for metrics.** The registry is returned to the
//!   caller as an [`Arc`] (the metrics-kit pattern). Only the tracing
//!   subscriber is global, because that is tracing's own design.
//! - **Explicit shutdown, best-effort drop.** [`Telemetry::shutdown`] is
//!   the guaranteed flush path and idempotent; [`Drop`] does a
//!   best-effort flush in case teardown is forgotten.
//! - **No silent fallbacks.** Requesting [`LogFormat::Json`] without the
//!   `json` feature is a configuration error, not quiet plain text.
//!
//! # Example
//!
//! ```
//! use telemetry_kit::{LogFormat, Telemetry, TelemetryConfig};
//!
//! let telemetry = Telemetry::init(
//!     TelemetryConfig::new("payments-api")
//!         .version(env!("CARGO_PKG_VERSION"))
//!         .log_format(LogFormat::Pretty)
//!         .log_level("info"),
//! )?;
//!
//! tracing::info!("service started");
//!
//! telemetry.shutdown()?;
//! # Ok::<(), telemetry_kit::TelemetryError>(())
//! ```
//!
//! # Metrics
//!
//! With the default `metrics` feature, [`Telemetry::metrics`] hands back
//! the registry for hot-path registration:
//!
//! ```
//! # #[cfg(feature = "metrics")]
//! # {
//! use telemetry_kit::{Telemetry, TelemetryConfig};
//!
//! let telemetry = Telemetry::init(
//!     TelemetryConfig::new("payments-api").metrics_budget(4096),
//! )?;
//!
//! let requests = telemetry
//!     .metrics()
//!     .counter("http_requests_total", "Total HTTP requests.", &[])
//!     .expect("unique series name");
//! requests.inc();
//!
//! assert!(telemetry.metrics().render().contains("http_requests_total 1"));
//! telemetry.shutdown()?;
//! # }
//! # Ok::<(), telemetry_kit::TelemetryError>(())
//! ```
//!
//! # Traces (feature `otlp`)
//!
//! ```rust,ignore
//! let telemetry = Telemetry::init(
//!     TelemetryConfig::new("payments-api")
//!         .otlp_endpoint("http://localhost:4317")
//!         .sample_rate(0.1),
//! )?;
//! ```
//!
//! # Why not `otelkit::init`?
//!
//! `otelkit` v2's public API is a whole-subscriber init: `otelkit::init`
//! installs its own global subscriber and returns a flush guard. It
//! cannot compose as a layer inside this crate's single subscriber, and
//! its OTLP path ignores the log format and `RUST_LOG`. telemetry-kit
//! therefore wires `opentelemetry-otlp` + `tracing-opentelemetry`
//! directly, keeping one code path for every feature combination.
//!
//! # Feature flags
//!
//! | Feature  | Default | Description |
//! |----------|---------|-------------|
//! | `metrics` | yes | metrics-kit registry via [`Telemetry::metrics`] |
//! | `json`    | yes | JSON log format (`LogFormat::Json`) |
//! | `otlp`    | no  | OTLP trace export via `opentelemetry-otlp` |
//!
//! [`Arc`]: std::sync::Arc

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod config;
mod error;
mod telemetry;

#[cfg(feature = "metrics")]
pub use config::DEFAULT_METRICS_BUDGET;
pub use config::{
    LogFormat, TelemetryConfig, DEFAULT_LOG_LEVEL, DEFAULT_SAMPLE_RATE, DEFAULT_SERVICE_VERSION,
};
pub use error::TelemetryError;
pub use telemetry::{build_subscriber, Telemetry};
