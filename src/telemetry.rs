//! Subscriber construction, global installation, and the [`Telemetry`]
//! handle.

use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "metrics")]
use std::sync::Arc;

use tracing::Subscriber;
use tracing_subscriber::layer::{Layer, SubscriberExt};
use tracing_subscriber::{fmt, EnvFilter};

use crate::config::{LogFormat, TelemetryConfig};
use crate::error::TelemetryError;

/// The `OTel` tracer provider retained for flush/shutdown. Compiled out when
/// the `otlp` feature is off; the alias keeps one signature for both
/// builds.
#[cfg(feature = "otlp")]
type OtelProvider = opentelemetry_sdk::trace::SdkTracerProvider;
#[cfg(not(feature = "otlp"))]
type OtelProvider = ();

/// The installed stack handed to callers: an installed global subscriber
/// is a process-wide singleton, so the handle carries only what callers
/// need afterwards.
///
/// `Telemetry` is [`Send`] + [`Sync`]; clone-free sharing is by design —
/// hand the [`Telemetry::metrics`] registry `Arc` to workers instead.
pub struct Telemetry {
    #[cfg(feature = "metrics")]
    metrics: Arc<metrics_kit::Registry>,
    #[cfg(feature = "otlp")]
    tracer_provider: Option<OtelProvider>,
    /// Set once shutdown has run (explicitly or via [`Drop`]) so the
    /// operation is idempotent.
    shutdown: AtomicBool,
}

impl std::fmt::Debug for Telemetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Internal handles are noise for the Debug output; the shape is
        // what matters (and `unwrap_err` on `init` needs any Debug).
        f.debug_struct("Telemetry").finish_non_exhaustive()
    }
}

impl Telemetry {
    /// Bootstrap the estate telemetry stack and install it as the global
    /// tracing subscriber.
    ///
    /// Installs, in one call:
    ///
    /// 1. a `tracing_subscriber` `EnvFilter` — built from the configured
    ///    `log_level`, with `RUST_LOG` winning when set and non-empty —
    ///    and a `fmt` layer ([`LogFormat::Json`] via the `json` feature,
    ///    [`LogFormat::Pretty`] otherwise);
    /// 2. a metrics-kit [`Registry`](metrics_kit::Registry) under the
    ///    `metrics` feature, sized by
    ///    [`metrics_budget`](TelemetryConfig::metrics_budget) and handed
    ///    to the caller through [`Telemetry::metrics`];
    /// 3. with the `otlp` feature and an endpoint configured: an
    ///    `opentelemetry-otlp` span exporter (HTTP/protobuf, batch) wired
    ///    as a `tracing-opentelemetry` layer, with `service.name` and
    ///    `service.version` resource attributes and the configured sample
    ///    rate.
    ///
    /// # Errors
    ///
    /// - [`TelemetryError::AlreadyInitialized`] — the global subscriber is
    ///   already installed (this crate init'ed twice, or another crate
    ///   owns the global). The process-wide subscriber can only be set
    ///   once; this is a typed error, never a panic.
    /// - [`TelemetryError::InitFailed`] — the `env-filter` directive does
    ///   not parse, [`LogFormat::Json`] was requested without the `json`
    ///   feature, or the OTLP exporter could not be built.
    // By value per the estate API contract: configs flow out of a builder
    // chain and are logically consumed by init even though the pipeline
    // construction only borrows them.
    #[allow(clippy::needless_pass_by_value)]
    pub fn init(config: TelemetryConfig) -> Result<Self, TelemetryError> {
        #[cfg(feature = "metrics")]
        let metrics = Arc::new(metrics_kit::Registry::with_max_series(
            config.metrics_budget,
        ));

        #[cfg(feature = "otlp")]
        let (subscriber, provider) = build_pipeline(&config)?;
        #[cfg(not(feature = "otlp"))]
        let (subscriber, _) = build_pipeline(&config)?;

        if tracing::subscriber::set_global_default(subscriber).is_err() {
            // The provider was built before the install attempt; tear it
            // down so the failed init leaks no exporter runtime.
            #[cfg(feature = "otlp")]
            if let Some(provider) = provider {
                let _ = provider.shutdown();
            }
            return Err(TelemetryError::AlreadyInitialized);
        }

        Ok(Self {
            #[cfg(feature = "metrics")]
            metrics,
            #[cfg(feature = "otlp")]
            tracer_provider: provider,
            shutdown: AtomicBool::new(false),
        })
    }

    /// The metrics registry for hot-path handle registration.
    ///
    /// Handles register through this `Arc`; every clone of the returned
    /// `Arc` renders identically. Under `no-default-features` (no
    /// `metrics` feature) this method does not exist.
    #[cfg(feature = "metrics")]
    pub fn metrics(&self) -> Arc<metrics_kit::Registry> {
        Arc::clone(&self.metrics)
    }

    /// Explicitly flush and tear down the export pipeline.
    ///
    /// Idempotent: the first call flushes and shuts down the OTLP tracer
    /// provider (when one was built); every later call returns `Ok(())`
    /// without touching anything. After `shutdown`, spans are no longer
    /// exported; the log subscriber keeps working for the process
    /// lifetime.
    ///
    /// # Errors
    ///
    /// [`TelemetryError::InitFailed`] if the first call's provider
    /// shutdown fails; later calls never fail.
    pub fn shutdown(&self) -> Result<(), TelemetryError> {
        if self.shutdown.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        #[cfg(feature = "otlp")]
        if let Some(provider) = &self.tracer_provider {
            provider
                .shutdown()
                .map_err(|e| TelemetryError::InitFailed(e.to_string()))?;
        }
        Ok(())
    }
}

impl Drop for Telemetry {
    /// Best-effort flush of in-flight spans. This is a safety net, not a
    /// contract: batch export races process teardown, so call
    /// [`Telemetry::shutdown`] explicitly on the way down.
    fn drop(&mut self) {
        if self.shutdown.swap(true, Ordering::AcqRel) {
            return; // already shut down explicitly
        }
        #[cfg(feature = "otlp")]
        if let Some(provider) = self.tracer_provider.take() {
            // Best-effort: flush errors at teardown are not actionable.
            let _ = provider.force_flush();
        }
    }
}

/// Construct the estate subscriber described by `config` *without*
/// installing it.
///
/// This is the exact stack [`Telemetry::init`] installs — the seam used
/// by the init-cost bench and by applications that must compose extra
/// layers before installing. The OTLP tracer provider handle is **not**
/// retained, so shutdown flushing is unavailable on this path; use
/// [`Telemetry::init`] for the managed pipeline.
///
/// # Errors
///
/// Same conditions as [`Telemetry::init`] minus
/// [`TelemetryError::AlreadyInitialized`] (nothing global is touched).
pub fn build_subscriber(
    config: &TelemetryConfig,
) -> Result<Box<dyn Subscriber + Send + Sync>, TelemetryError> {
    build_pipeline(config).map(|(subscriber, _)| subscriber)
}

/// Build the effective `env-filter` from config: `RUST_LOG` wins when set
/// and non-empty; otherwise the configured directive applies.
pub(crate) fn resolve_filter(config: &TelemetryConfig) -> Result<EnvFilter, TelemetryError> {
    match std::env::var("RUST_LOG") {
        Ok(directive) if !directive.trim().is_empty() => build_filter(&directive),
        _ => build_filter(&config.log_level),
    }
}

/// Parse an `env-filter` directive string without consulting the
/// environment.
pub(crate) fn build_filter(directive: &str) -> Result<EnvFilter, TelemetryError> {
    EnvFilter::try_new(directive).map_err(|e| {
        TelemetryError::InitFailed(format!("invalid env-filter directive {directive:?}: {e}"))
    })
}

/// Compose filter + fmt layer (+ `OTel` tracer layer under `otlp`) into the
/// boxed subscriber, returning the tracer provider alongside for
/// flush/shutdown ownership.
fn build_pipeline(
    config: &TelemetryConfig,
) -> Result<(Box<dyn Subscriber + Send + Sync>, Option<OtelProvider>), TelemetryError> {
    let filter = resolve_filter(config)?;

    #[cfg(not(feature = "otlp"))]
    {
        let subscriber = tracing_subscriber::registry()
            .with(filter)
            .with(build_fmt_layer(config.log_format)?);
        return Ok((Box::new(subscriber), None));
    }

    #[cfg(feature = "otlp")]
    {
        let provider = build_tracer_provider(config)?;
        let otel_layer = provider.as_ref().map(|p| {
            use opentelemetry::trace::TracerProvider as _;
            let tracer = p.tracer(config.service_name.clone());
            tracing_opentelemetry::layer().with_tracer(tracer)
        });
        let subscriber = tracing_subscriber::registry()
            .with(filter)
            .with(build_fmt_layer(config.log_format)?)
            .with(otel_layer);
        Ok((Box::new(subscriber), provider))
    }
}

/// Build the `fmt` layer for the configured format, boxed over the
/// subscriber `S` it will be applied to.
// The Result carries the `json`-feature-disabled error path; with the
// feature compiled in this fn is infallible, but the signature must not
// change shape between feature combinations.
#[allow(clippy::unnecessary_wraps)]
fn build_fmt_layer<S>(format: LogFormat) -> Result<Box<dyn Layer<S> + Send + Sync>, TelemetryError>
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    match format {
        LogFormat::Json => {
            #[cfg(feature = "json")]
            {
                // JSON to stdout, ANSI escapes off: machine parsing must
                // not meet terminal color codes.
                Ok(fmt::layer().json().with_ansi(false).boxed())
            }
            #[cfg(not(feature = "json"))]
            {
                Err(TelemetryError::InitFailed(
                    "LogFormat::Json requested but the `json` feature is not compiled in; \
                     enable the default `json` feature or select LogFormat::Pretty"
                        .to_owned(),
                ))
            }
        }
        LogFormat::Pretty => Ok(fmt::layer().boxed()),
    }
}

/// Build the OTLP tracer provider when an endpoint is configured;
/// `Ok(None)` (no traces) otherwise.
///
/// Exporter construction is offline — the HTTP client connects lazily at
/// export time — so this path needs no network to test.
#[cfg(feature = "otlp")]
fn build_tracer_provider(config: &TelemetryConfig) -> Result<Option<OtelProvider>, TelemetryError> {
    use opentelemetry_otlp::WithExportConfig;

    let Some(endpoint) = config.otlp_endpoint.as_deref() else {
        return Ok(None);
    };

    let resource = opentelemetry_sdk::Resource::builder()
        .with_attributes([
            opentelemetry::KeyValue::new("service.name", config.service_name.clone()),
            opentelemetry::KeyValue::new("service.version", config.service_version.clone()),
        ])
        .build();

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| TelemetryError::InitFailed(format!("OTLP exporter build failed: {e}")))?;

    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_resource(resource)
        .with_sampler(opentelemetry_sdk::trace::Sampler::ParentBased(Box::new(
            opentelemetry_sdk::trace::Sampler::TraceIdRatioBased(f64::from(config.sample_rate)),
        )))
        .with_batch_exporter(exporter)
        .build();

    Ok(Some(provider))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use tracing::level_filters::LevelFilter;

    /// Serializes every test that mutates `RUST_LOG`; env vars are
    /// process-global and lib tests run multi-threaded.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Build a `Telemetry` without going through the global install, for
    /// shutdown/drop-path tests. Feature-gated fields are handled here so
    /// call sites stay readable.
    #[cfg(feature = "otlp")]
    fn raw_telemetry(provider: Option<OtelProvider>) -> Telemetry {
        Telemetry {
            #[cfg(feature = "metrics")]
            metrics: Arc::new(metrics_kit::Registry::new()),
            tracer_provider: provider,
            shutdown: AtomicBool::new(false),
        }
    }

    #[cfg(not(feature = "otlp"))]
    fn raw_telemetry() -> Telemetry {
        Telemetry {
            shutdown: AtomicBool::new(false),
        }
    }

    /// Sets `RUST_LOG` for the test body and restores the previous state
    /// afterwards.
    fn with_rust_log<T>(value: Option<&str>, f: impl FnOnce() -> T) -> T {
        let guard = ENV_LOCK.lock().unwrap();
        let saved = std::env::var("RUST_LOG").ok();
        match value {
            Some(v) => std::env::set_var("RUST_LOG", v),
            None => std::env::remove_var("RUST_LOG"),
        }
        let out = f();
        match saved {
            Some(v) => std::env::set_var("RUST_LOG", v),
            None => std::env::remove_var("RUST_LOG"),
        }
        drop(guard);
        out
    }

    #[test]
    fn build_filter_parses_directives_and_levels() {
        assert_eq!(
            build_filter("info").unwrap().max_level_hint(),
            Some(LevelFilter::INFO)
        );
        assert_eq!(
            build_filter("warn,hyper=debug").unwrap().max_level_hint(),
            Some(LevelFilter::DEBUG)
        );
        assert!(matches!(
            build_filter("hyper=notalevel"),
            Err(TelemetryError::InitFailed(_))
        ));
        assert!(matches!(
            build_filter("a=b=c"),
            Err(TelemetryError::InitFailed(_))
        ));
    }

    #[test]
    fn resolve_filter_uses_config_when_rust_log_unset() {
        with_rust_log(None, || {
            let cfg = TelemetryConfig::new("svc").log_level("warn");
            assert_eq!(
                resolve_filter(&cfg).unwrap().max_level_hint(),
                Some(LevelFilter::WARN)
            );
        });
    }

    #[test]
    fn resolve_filter_rust_log_wins_over_config() {
        with_rust_log(Some("debug"), || {
            let cfg = TelemetryConfig::new("svc").log_level("info");
            assert_eq!(
                resolve_filter(&cfg).unwrap().max_level_hint(),
                Some(LevelFilter::DEBUG)
            );
        });
    }

    #[test]
    fn resolve_filter_treats_empty_rust_log_as_unset() {
        with_rust_log(Some("   "), || {
            let cfg = TelemetryConfig::new("svc").log_level("error");
            assert_eq!(
                resolve_filter(&cfg).unwrap().max_level_hint(),
                Some(LevelFilter::ERROR)
            );
        });
    }

    #[test]
    fn resolve_filter_propagates_invalid_rust_log() {
        with_rust_log(Some("a=b=c"), || {
            let cfg = TelemetryConfig::new("svc").log_level("info");
            assert!(matches!(
                resolve_filter(&cfg),
                Err(TelemetryError::InitFailed(_))
            ));
        });
    }

    #[test]
    fn build_subscriber_smoke_pretty() {
        let cfg = TelemetryConfig::new("svc").log_format(LogFormat::Pretty);
        let subscriber = build_subscriber(&cfg).unwrap();
        assert_eq!(subscriber.max_level_hint(), Some(LevelFilter::INFO));
    }

    #[cfg(feature = "json")]
    #[test]
    fn build_subscriber_smoke_json() {
        let cfg = TelemetryConfig::new("svc");
        let subscriber = build_subscriber(&cfg).unwrap();
        assert_eq!(subscriber.max_level_hint(), Some(LevelFilter::INFO));
    }

    #[test]
    fn init_reports_already_initialized_when_global_is_taken() {
        // Idempotent regardless of test order: first caller wins the
        // global; everyone else's install attempt is ignored.
        let _ = tracing::subscriber::set_global_default(tracing::subscriber::NoSubscriber::new());
        let err =
            Telemetry::init(TelemetryConfig::new("svc").log_format(LogFormat::Pretty)).unwrap_err();
        assert!(matches!(err, TelemetryError::AlreadyInitialized));
    }

    #[cfg(feature = "otlp")]
    #[test]
    fn init_with_otlp_reports_already_initialized_without_leaking_provider() {
        // Same global-first pattern; the otlp endpoint forces the
        // provider-build + cleanup branch (exporter build is offline).
        let _ = tracing::subscriber::set_global_default(tracing::subscriber::NoSubscriber::new());
        let cfg = TelemetryConfig::new("svc").otlp_endpoint("http://127.0.0.1:4317");
        let err = Telemetry::init(cfg).unwrap_err();
        assert!(matches!(err, TelemetryError::AlreadyInitialized));
    }

    #[cfg(feature = "otlp")]
    #[test]
    fn shutdown_is_idempotent() {
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
        let t = raw_telemetry(Some(provider));
        t.shutdown().unwrap();
        t.shutdown().unwrap();
        // Idempotent across the Drop path too.
        drop(t);
    }

    #[cfg(feature = "otlp")]
    #[test]
    fn shutdown_maps_provider_error_to_init_failed_then_stays_ok() {
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
        provider.shutdown().unwrap(); // externally torn down already
        let t = raw_telemetry(Some(provider));
        assert!(matches!(
            t.shutdown().unwrap_err(),
            TelemetryError::InitFailed(_)
        ));
        // Second call hits the idempotency flag: Ok, never a repeat error.
        t.shutdown().unwrap();
    }

    #[cfg(feature = "otlp")]
    #[test]
    fn drop_is_a_best_effort_flush_short_circuit() {
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
        let t = raw_telemetry(Some(provider));
        t.shutdown().unwrap(); // flag set; Drop must take the early return
        drop(t);
        // Fresh handle dropped without explicit shutdown: best-effort
        // flush path, no panic.
        drop(raw_telemetry(Some(
            opentelemetry_sdk::trace::SdkTracerProvider::builder().build(),
        )));
    }

    #[cfg(not(feature = "otlp"))]
    #[test]
    fn shutdown_without_otlp_is_trivially_idempotent() {
        let t = raw_telemetry();
        t.shutdown().unwrap();
        t.shutdown().unwrap();
        drop(t);
    }
}
