//! WASM API modules mirroring the Rust umbrella crate structure.
//!
//! Each submodule corresponds to one Rust crate domain.

pub mod analytics;
pub mod attribution;
pub mod cashflows;
pub mod core;
pub mod covenants;
pub mod margin;
pub mod monte_carlo;
pub mod portfolio;
pub mod scenarios;
pub mod statements;
pub mod statements_analytics;
pub mod valuations;
