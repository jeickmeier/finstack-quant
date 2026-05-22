# Refactor Summary

## Target

`finstack/valuations/src/attribution/helpers.rs`

## Preserved Invariants

- Public attribution result names unchanged.
- Metric-key format unchanged.
- Rust/Python/WASM exposed behavior unchanged.

## Structural Operations

- Extracted repeated factor-key construction into one private helper.
- Removed a wrapper that only reordered arguments.
- Kept public exports stable.

## Surfaces Checked

- Rust call sites
- PyO3 wrapper registration
- Parity tests for exported names

## Verification

- `mise run rust-test` for targeted attribution tests.
- Binding parity checks if public names change.
