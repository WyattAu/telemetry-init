# Pattern adoption: telemetry-init

Telemetry bootstrap crate. Declared per
`pattern-standards/references/adoption-format.md`.

| Pattern | Status | Evidence |
|---|---|---|
| T13 | adopted | src/config.rs:38 (TelemetryConfig builder is pure data) and src/telemetry.rs:1 (global subscriber install is the imperative shell) |
| P118 | adopted | src/config.rs:98 (metrics_budget caps exporter cardinality at config time) |
