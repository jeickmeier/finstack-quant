//! WebAssembly bindings for the Finstack Quant financial computation library.
//!
//! The public API is consumed through a hand-written JS/TS facade (`index.js`)
//! that groups raw `wasm-bindgen` exports into crate-level namespaces mirroring
//! the Rust umbrella crate structure.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]

use wasm_bindgen::prelude::*;

pub mod api;
pub mod utils;

/// Module initializer: installs the panic hook.
///
/// wasm-bindgen runs this once when the module is instantiated, so a Rust
/// panic is reported through `console.error` with its message and location
/// instead of surfacing only as `RuntimeError: unreachable`.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_does_not_panic() {
        start();
    }
}
