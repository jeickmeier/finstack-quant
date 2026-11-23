//! WASM bindings for vectorized panel feature transforms.
//!
//! The binding accepts JavaScript arrays/objects, converts them into the Rust
//! crate's canonical inputs, and delegates all transform behavior to
//! `finstack-quant-features`.

pub mod panel;
mod transforms;

pub use transforms::*;
