# Security Policy — telemetry-init

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✅        |

## Reporting a vulnerability

Report privately via [GitHub security advisories] for this repository, or
email **wyatt_au@protonmail.com**. Do **not** open a public issue for
security reports.

You will receive an acknowledgement within **72 hours**. Coordinated
disclosure: we ask for up to 90 days before public disclosure while a
patch ships.

## Scope notes

`telemetry-init` installs the logging/metrics/tracing stack for a process.
Security considerations for integrators:

- **Logs are a data-leak channel.** The JSON/pretty `fmt` layer writes
  event payloads to stdout verbatim; never record request bodies,
  credentials, tokens, or PII in `tracing` events, and treat log sinks as
  sensitive infrastructure.
- **The OTLP endpoint is trusted infrastructure.** The exporter sends
  spans (with resource attributes) unauthenticated; bind your collector on
  an internal interface and use network policy or a sidecar for auth.
- **Sampling is not redaction.** `sample_rate` limits volume, not
  content — sampled-in spans carry everything their spans contain.
- **Cardinality guard.** The metrics registry's series budget bounds the
  blast radius of label explosions; keep request-controlled values out of
  label sets regardless.
- **Single global install.** The global subscriber can be set once per
  process; a second `Telemetry::init` returns
  `TelemetryError::AlreadyInitialized` instead of panicking, so a
  misconfigured double bootstrap cannot be used to replace or shadow the
  logging stack.
- `#![forbid(unsafe_code)]` — no unsafe blocks exist in this crate.

[GitHub security advisories]:
    https://github.com/WyattAu/telemetry-init/security/advisories/new
