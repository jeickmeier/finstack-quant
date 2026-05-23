---
trigger: model_decision
description: Information about the error handling framework for the finstack workspace.
globs:
---
# Rust Error Handling Framework

## Overview

To ensure consistency and maintainability across the `finstack` workspace, we are adopting a dual-crate approach for error handling:

1. **`thiserror`** for **library crates** (`core`, `statements`, `valuations`, etc.).
2. **`anyhow`** for **application-level crates** (binaries, examples, and language bindings like `finstack-py` and `finstack-wasm`).

This approach provides the best of both worlds: structured, specific error types for libraries, and convenient, easy-to-manage error propagation for applications. It avoids over-engineering by using simple, popular, and well-understood crates.

## 1. Library Crates: `thiserror`

All library crates **MUST** define a crate-specific, public `Error` enum. This allows consumers of the library to match on specific error variants and handle them accordingly. `thiserror` is used to reduce the boilerplate required to implement `std::error::Error`.

### Guidelines

- Each library crate (e.g., `finstack/core`, `finstack/valuations`) must have a `src/errors.rs` or similar module containing its public `Error` type.
- The `Error` enum should be comprehensive, covering all possible failure modes for that crate.
- When a library function calls a function from another library crate within our workspace, it should wrap the error in its own `Error` type.

### Example: `finstack/core/src/errors.rs`

```rust
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum Error {
    #[error("Invalid currency code: {0}")]
    InvalidCurrency(String),

    #[error("Date calculation error: {0}")]
    DateCalculation(String),

    #[error("FX rate not found for pair {base}/{quote} on {date}")]
    FxRateNotFound {
        base: String,
        quote: String,
        date: chrono::NaiveDate,
    },

    // Example of wrapping an error from another crate (e.g., an I/O error)
    #[error("I/O error")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
```

### Usage

Functions within the library will return `crate::errors::Result<T>`.

```rust
use crate::errors::{Error, Result};

pub fn do_something(currency_code: &str) -> Result<()> {
    if currency_code.len() != 3 {
        return Err(Error::InvalidCurrency(currency_code.to_string()));
    }
    // ...
    Ok(())
}
```

## 2. Application & Binding Crates: `anyhow`

Application-level crates (anything with a `main.rs`, examples, and the `finstack-py`/`finstack-wasm` binding crates) **SHOULD** use `anyhow` for error handling. `anyhow::Error` is a dynamic error type that can wrap any error that implements `std::error::Error`, which includes all of our library errors derived with `thiserror`.

### Guidelines

- Functions should return `anyhow::Result<T>`.
- Use the `?` operator to propagate errors. It will automatically convert `thiserror`-based errors into `anyhow::Error`.
- Use `anyhow::Context` to add explanatory context to errors as they propagate up the call stack.
- Use `anyhow::bail!` or `anyhow::ensure!` for new errors within application-level code.

### Example: `finstack/examples/some_example.rs`

```rust
use anyhow::{Context, Result};
use finstack_core::some_module; // Assuming this has functions returning `finstack_core::errors::Result`

fn main() -> Result<()> {
    run_example().context("Failed to run example")?;
    Ok(())
}

fn run_example() -> Result<()> {
    // The `?` operator converts `finstack_core::errors::Error` into `anyhow::Error`
    some_module::do_something("USD")
        .context("Initial operation failed")?;

    // ... more code

    Ok(())
}
```

## Rationale

- **Libraries as Contracts**: Library error types are part of their public API. `thiserror` allows us to define a stable, well-documented contract for library consumers.
- **Application Simplicity**: Applications often don't need to handle every specific error type. Their main concern is to report the error to the user (e.g., log it, print it to the console) and terminate gracefully. `anyhow` excels at this.
- **Idiomatic Rust**: This pattern is widely adopted and considered a best practice in the Rust community.

## Implementation Plan

1. **Add Dependencies**:
    - Add `thiserror = "1.0"` to the `[dependencies]` section of each library crate's `Cargo.toml`.
    - Add `anyhow = "1.0"` to the `[dependencies]` section of application/binding crates' `Cargo.toml`.
2. **Create Error Types**: Create `errors.rs` modules and define `thiserror`-based `Error` enums for each library crate.
3. **Refactor Functions**: Update functions in library crates to return the new crate-specific `Result` type.
4. **Update Application-Level Crates**: Refactor `main` functions, examples, and binding entry points to use `anyhow::Result` for their return types.
