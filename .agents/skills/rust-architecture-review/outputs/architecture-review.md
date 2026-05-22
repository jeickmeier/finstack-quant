# Verdict

PASS WITH CHANGES: the crate boundaries are mostly sound, but one public registry path exposes internal construction details that should stay private.

## Architecture Map

- Crates/modules reviewed: `finstack/valuations/src/attribution/`
- Public boundaries: attribution result types, factor decomposition entry points, Python/WASM wrappers
- Key dependency flows: valuations core -> bindings -> parity tests
- Tests/examples checked: targeted attribution tests and binding registration

## Findings

### Major
- `finstack/valuations/src/attribution/parallel.rs`: parallel attribution has a separate public entry point from the serial path. This increases the chance that behavior diverges. Keep one canonical attribution API and make execution strategy an internal detail or explicit parameter.

### Minor
- `finstack-py/src/bindings/valuations/attribution.rs`: wrapper registration mirrors internal module names. Check whether the Python package should expose a simpler user-facing namespace.

## What Works

- Rust remains the canonical implementation for attribution behavior.
- The binding layer primarily wraps and maps results.

## Remediation Order

1. Collapse public serial/parallel entry points or document the intentional split.
2. Add parity tests if Python/WASM expose both paths.

## Verification

- `mise run rust-test` for attribution tests.
- Binding parity tests if public names change.
