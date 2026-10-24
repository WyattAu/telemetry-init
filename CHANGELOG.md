# Changelog

All notable changes to this project are documented here. Format: [Keep a
Changelog](https://keepachangelog.com/): versions follow [semver](https://semver.org).

## [1.0.0] - 2026-10-24

### Declared stable

First 1.0. `telemetry-init` bootstraps production observability; this release commits the one-call init, the estate defaults, and the feature surface (metrics, json, otlp) as API-stable.

### Guarantees from here on

- Breaking changes to public items require a minor bump plus a `#[deprecated]`
  window of at least one minor release.
- MSRV bumps are minor releases, not patch releases.
- `#![deny(missing_docs)]` and `#![forbid(unsafe_code)]` are enforced.

### Explicitly not guaranteed

- Default sampling and batching parameters may be tuned in minors when the estate's own dashboards say they should be.

## [0.1.1] - 2026-09-28

### Added
- `Subscriber` newtype (real `Debug`, forwards `max_level_hint`) so hosts
  get `unwrap`/`unwrap_err` ergonomics on `build_subscriber` results.
- `effective_max_level(config)`: the max verbosity init would install,
  honoring `RUST_LOG`.

### Fixed
- `Subscriber` forwards `max_level_hint` (the trait default under-reports
  the installed filter).

## [0.1.0] - 2026-09-19

### Added

- `Telemetry::init(TelemetryConfig)`: one-call bootstrap installing the
  `EnvFilter` + `fmt` global subscriber (JSON default, `RUST_LOG`
  override), the metrics-kit registry under a cardinality budget, and,
  with the `otlp` feature + endpoint: an `opentelemetry-otlp`
  (HTTP/protobuf, batch) tracer wired as a `tracing-opentelemetry` layer
  with `service.name`/`service.version` resource attributes and
  parent-based ratio sampling.
- `Telemetry::metrics()`: the registry `Arc` for hot-path handle
  registration (metrics-kit pattern; no hidden globals).
- `Telemetry::shutdown()`: explicit, idempotent OTLP flush/teardown;
  `Drop` as a documented best-effort safety net.
- `TelemetryError` (`InitFailed`, `AlreadyInitialized`): typed failure
  for invalid directives, feature/format mismatches, exporter build
  failures, and double initialization.
- `build_subscriber()`: the uninstalled stack for benches and advanced
  composition.
- Features: `metrics` (default on), `json` (default on), `otlp`.
