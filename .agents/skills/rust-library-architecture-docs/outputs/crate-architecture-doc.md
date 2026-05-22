# Valuations Attribution Architecture

## Purpose

The attribution module decomposes valuation changes into factor-level contributions. It belongs in `finstack/valuations` because pricing and risk decomposition are canonical Rust behavior shared by Python and WASM bindings.

## Source Map

- Crates/modules: `finstack/valuations/src/attribution/`
- Public entry points: attribution result types and factor decomposition functions
- Tests/examples checked: attribution integration tests

## Architecture Overview

Callers provide valuation inputs and market-data changes. Rust computes factor attribution and returns structured results. Bindings convert the Rust result into host-language wrappers without recomputing attribution.

## Public API Boundaries

Stable surfaces include result type names, metric keys, serde fields, and binding-exported method names. Changing any of these requires parity-contract and binding updates.

## Evidence Log

- Module ownership: `finstack/valuations/src/attribution/mod.rs`
- Binding exposure: `finstack-py/src/bindings/valuations/attribution.rs`
- Tests: `finstack/valuations/tests/attribution/`

## Verification

- Run targeted attribution tests.
- Run binding parity checks if names or exported shapes change.

## Unverified Or Omitted

WASM examples and notebooks were not checked for this sample.
