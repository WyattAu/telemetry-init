# telemetry-init

One-call observability bootstrap for Rust — the shared init pattern of the
WyattAu estate, wiring **logs** (`tracing_subscriber`), **metrics**
(`metrics-kit`), and **traces** (`opentelemetry-otlp`) in a single fallible
call instead of six hand-rolled dialects of the same 40–80 lines.

- **One init, typed failure**: `Telemetry::init` returns `TelemetryError`;
  double initialization is `AlreadyInitialized`, never a panic.
- **Logs**: `EnvFilter` from the configured directive with `RUST_LOG`
  override, JSON by default (`json` feature), human format for local dev.
- **Metrics**: a lock-free `metrics-kit::Registry` with a cardinality
  budget, handed to you as an `Arc` — no hidden globals.
- **Traces** (`otlp` feature): OTLP/HTTP exporter wired as a
  `tracing-opentelemetry` layer with `service.name` / `service.version`
  resource attributes and parent-based ratio sampling.
- **Explicit shutdown, best-effort drop**: `shutdown()` is the guaranteed,
  idempotent flush path; `Drop` is a documented safety net.
- **No silent fallbacks**: `LogFormat::Json` without the `json` feature is a
  configuration error.
- **`#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`**, clippy
  `unwrap_used`/`expect_used`/`panic`/`indexing_slicing` denied.

## Install

```toml
[dependencies]
telemetry-init = "0.1"
```

## Example

```rust
use telemetry_init::{Telemetry, TelemetryConfig};

let telemetry = Telemetry::init(
    TelemetryConfig::new("payments-api")
        .version(env!("CARGO_PKG_VERSION"))
        .log_level("info"),          // RUST_LOG wins when set
)?;

// Hot-path handles register through the handed-out Arc.
let requests = telemetry
    .metrics()
    .counter("http_requests_total", "Total HTTP requests.", &[])?;
requests.inc();

assert!(telemetry.metrics().render().contains("http_requests_total 1"));
telemetry.shutdown()?;
# Ok::<(), telemetry_init::TelemetryError>(())
```

With traces enabled:

```toml
[dependencies]
telemetry-init = { version = "0.1", features = ["otlp"] }
```

```rust,ignore
let telemetry = Telemetry::init(
    TelemetryConfig::new("payments-api")
        .otlp_endpoint("http://localhost:4317")
        .sample_rate(0.1),
)?;
```

Spans export over OTLP/HTTP (protobuf, batched); the endpoint is trusted
infrastructure — point it at your collector, not the public internet.

## The config surface

| Builder call | Default | Notes |
|---|---|---|
| `new("service-name")` | — | required; becomes `service.name` |
| `.version("1.2.3")` | `"0.0.0"` | becomes `service.version` |
| `.log_level("info")` | `"info"` | any `env-filter` directive; `RUST_LOG` wins |
| `.log_format(LogFormat::Json)` | `Json` | `Json` needs the `json` feature |
| `.metrics_budget(8192)` | `8192` | series cardinality guard |
| `.otlp_endpoint("…")` | none | `otlp` feature; none = no traces |
| `.sample_rate(1.0)` | `1.0` | clamped to `0.0..=1.0` |

## Feature flags

| Feature | Default | Description |
|---|---|---|
| `metrics` | yes | metrics-kit registry via `Telemetry::metrics()` |
| `json` | yes | JSON log format via `tracing-subscriber/json` |
| `otlp` | no | OTLP trace export (`opentelemetry-otlp` + `tracing-opentelemetry`) |

`--no-default-features` builds a logs-only bootstrap (choose
`LogFormat::Pretty` explicitly; `Json` errors without the feature).

## Why not `otelkit::init`?

`otelkit` v2 exposes a whole-subscriber init: it installs its own global
subscriber and returns a flush guard, so it cannot compose as a layer inside
this crate's single subscriber — and its OTLP path ignores the log format and
`RUST_LOG`. telemetry-init wires `opentelemetry-otlp` +
`tracing-opentelemetry` directly to keep one code path for every feature
combination.

## Testing note

The global subscriber is a once-per-process resource. All tests that call
`Telemetry::init` live in a single deterministic function
(`tests/global_init.rs`), so the default multi-threaded harness cannot race
double-init assertions.

## Performance

Init is a startup cost, measured as a smoke benchmark
(`benches/init_bench.rs`, `cargo bench`): subscriber construction is
filter-parse + layer assembly, microseconds-class; the metrics hot path is
metrics-kit's lock-free recording (see its README for the measured numbers).

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT)
at your option.
