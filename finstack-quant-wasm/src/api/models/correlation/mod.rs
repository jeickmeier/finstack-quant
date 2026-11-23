//! WASM bindings for the credit-correlation module.
//!
//! Exposes copula models, recovery models, and joint probability utilities
//! to JavaScript/TypeScript via `wasm-bindgen`, mirroring the Rust module
//! [`finstack_quant_models::correlation`]. The JS facade nests these exports
//! under `models.correlation`.

mod copula;
mod latent;
mod matrix;
mod recovery;

pub use copula::*;
pub use latent::*;
pub use matrix::*;
pub use recovery::*;
