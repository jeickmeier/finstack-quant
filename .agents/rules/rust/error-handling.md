---
trigger: model_decision
description: Information about the error handling framework for the finstack-quant workspace.
globs:
---
# Rust Error Handling Framework

## Overview

To ensure consistency and maintainability across the `finstack-quant` workspace ([ADR 0006](../../../docs/decisions/standards/0006-structured-errors.md), [ADR 0036](../../../docs/decisions/standards/0036-safe-rust-and-recoverable-failures.md), [ADR 0049](../../../docs/decisions/standards/0049-explicit-public-error-matching.md)):

1. **Structured domain errors (`thiserror`)** for **library crates** (`core`, `statements`, `valuations`, etc.).
2. **Centralized, kind-aware adapters** for **language bindings** (`finstack-quant-py` via `crate::errors::core_to_py` / `kind_to_py`, and `finstack-quant-wasm` via `crate::utils::to_js_err`).
3. **`anyhow`** only for standalone CLI binaries or examples where structured caller handling is not required.

This approach preserves structured error categories (`NotFound`, `Validation`, `Computation`) and diagnostic context across Rust, Python, and WASM without parsing message strings.

## 1. Library Crates: `thiserror`

A library crate **SHOULD** reuse `finstack_quant_core::Error` unless it has a
genuinely distinct failure domain.

Today, five crates define their own root `Error`: `core` (the shared base itself),
`valuations-core` (re-exported by the `valuations` umbrella), `portfolio`,
`scenarios` and `statements`. Every other library crate reuses core's at the
root, including `analytics`, `attribution`, `calibration`, `cashflows`,
`covenants`, `features`, `margin`, `models`, `factor`, `arrow-interchange` and
the `valuations/*` family crates (`statements-analytics` mostly propagates
`finstack_quant_statements::Result`, which wraps it). That reuse is the default,
not drift. Treat this list as a snapshot to re-check against each crate's
`error` module rather than as a fixed count.

Module-scoped `thiserror` enums for a narrow failure domain are a separate,
allowed thing and do not make a crate a root-`Error` crate — for example
`core::math::linalg::CorrelationError`, `models::correlation::Error`,
`finstack_quant_factor::FactorMatchError`, and
`finstack_quant_factor::credit::decomposition::DecompositionError`.
(`test-utils` is a dev-only supporting crate rather than a domain crate; it has
a small `Error` of its own.)

Introduce a new crate-level `Error` only when the crate's failure modes are not
expressible as core variants, and say why in the PR description. `thiserror` is
used where a crate-specific enum is justified.

### Guidelines

- Crates with a distinct error domain should expose a comprehensive public error type.
- Crates reusing `finstack_quant_core::Error` should propagate it directly with `?`.
- Do not add wrapper variants solely to hide another workspace crate's error type.

### Example: a crate-level `thiserror` root error

Illustrative shape only; the real shared type is in
`finstack-quant/core/src/error/mod.rs` (`#[non_exhaustive]`, with `ErrorKind`
classification and `InputError` for input failures).

```rust
use thiserror::Error;
use time::Date;

#[derive(Error, Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    #[error("Invalid currency code: {0}")]
    InvalidCurrency(String),

    #[error("Date calculation error: {0}")]
    DateCalculation(String),

    #[error("FX rate not found for pair {base}/{quote} on {date}")]
    FxRateNotFound {
        base: String,
        quote: String,
        date: Date,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
```

### Usage

Functions within a crate with its own root error return `crate::Result<T>`;
crates reusing core's error return `finstack_quant_core::Result<T>`.

```rust
use crate::{Error, Result};

pub fn do_something(currency_code: &str) -> Result<()> {
    if currency_code.len() != 3 {
        return Err(Error::InvalidCurrency(currency_code.to_string()));
    }
    // ...
    Ok(())
}
```

## 2. Language Bindings: Centralized Kind-Aware Adapters

Language bindings (`finstack-quant-py` and `finstack-quant-wasm`) **MUST NOT** erase error structure with `anyhow` or bare string errors (`JsValue::from_str`). Instead, Rust determines the error category and diagnostic fields, and bindings adapt them through centralized helpers ([ADR 0006](../../../docs/decisions/standards/0006-structured-errors.md), [ADR 0049](../../../docs/decisions/standards/0049-explicit-public-error-matching.md)):

- **Python (`finstack-quant-py`)**: Map Rust errors through `crate::errors::core_to_py` (or the centralized domain adapters in `finstack-quant-py/src/errors.rs`), preserving `KeyError` (`NotFound`), `ValueError` (`Validation`), and `RuntimeError` / domain subclasses (`Computation`).
- **WASM (`finstack-quant-wasm`)**: Map Rust errors through `crate::utils::to_js_err`, which constructs a structured `FinstackError` carrying the canonical `kind` and diagnostic metadata.
- **Explicit variant matching ([ADR 0049](../../../docs/decisions/standards/0049-explicit-public-error-matching.md))**: Keep exhaustive semantic classification (`kind()`, `code()`, conversions) in the owning Rust crate so new variants require explicit review at their semantic boundary.

## 3. Standalone Binaries & Examples: `anyhow`

Standalone binaries (`main.rs`) and examples may use `anyhow::Result<T>` for convenient top-level reporting.

### Example: `finstack-quant/examples/some_example.rs`

```rust
use anyhow::{Context, Result};
use finstack_quant_core::some_module;

fn main() -> Result<()> {
    run_example().context("Failed to run example")?;
    Ok(())
}

fn run_example() -> Result<()> {
    some_module::do_something("USD")
        .context("Initial operation failed")?;
    Ok(())
}
```

## Rationale

- **Libraries as Contracts**: Library error types are part of their public API. `thiserror` and explicit semantic classifiers allow callers to handle failures reliably without parsing message text.
- **Cross-Language Parity**: Centralized binding adapters (`core_to_py` and `to_js_err`) guarantee that Python and WASM callers observe the same failure categories and context as Rust callers.
