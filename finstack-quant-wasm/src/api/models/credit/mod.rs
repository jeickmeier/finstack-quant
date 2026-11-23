//! WASM bindings for the structural-credit models.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/` so the
//! Rust-canonical → PyO3 → wasm-bindgen triplet keeps file parity. Every
//! handle wraps the Rust type in `inner`; the JS facade nests the exports under
//! `models.credit`.

pub mod lgd;
pub mod liability_management;
mod merton;
pub mod migration;
mod paths;
pub mod pd;
mod rating;
mod recovery_hazard;
pub mod recovery_waterfall;
pub mod scoring;
mod toggle;

pub use merton::*;
pub use paths::*;
pub use rating::*;
pub use recovery_hazard::*;
pub use toggle::*;
