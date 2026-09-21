# Changelog

All notable changes to this project are documented here. Format: [Keep a
Changelog](https://keepachangelog.com/) — versions follow [semver](https://semver.org).

## [0.1.0] - 2026-09-19

### Added

- `Telemetry::init(TelemetryConfig)` — one-call bootstrap installing the
  `EnvFilter` + `fmt` global subscriber (JSON default, `RUST_LOG`
  override), the metrics-kit registry under a cardinality budget, and —
  with the `otlp` feature + endpoint — an `opentelemetry-otlp`
  (HTTP/protobuf, batch) tracer wired as a `tracing-opentelemetry` layer
  with `service.name`/`service.version` resource attributes and
  parent-based ratio sampling.
- `Telemetry::metrics()` — the registry `Arc` for hot-path handle
  registration (metrics-kit pattern; no hidden globals).
- `Telemetry::shutdown()` — explicit, idempotent OTLP flush/teardown;
  `Drop` as a documented best-effort safety net.
- `TelemetryError` (`InitFailed`, `AlreadyInitialized`) — typed failure
  for invalid directives, feature/format mismatches, exporter build
  failures, and double initialization.
- `build_subscriber()` — the uninstalled stack for benches and advanced
  composition.
- Features: `metrics` (default on), `json` (default on), `otlp`.
