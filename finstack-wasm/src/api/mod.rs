//! WASM API modules mirroring the Rust umbrella crate structure.
//!
//! Each submodule corresponds to one Rust crate domain.
//!
//! Native tests cannot inspect `JsValue` errors because `js_sys::Error` and
//! other JS constructors only work under `wasm32`. When a binding has
//! meaningful validation or diagnostics, keep the Rust work in a private
//! `*_inner` helper that returns the domain error, and make the
//! `#[wasm_bindgen]` function a thin wrapper that converts that error at the
//! boundary. This keeps native tests precise while preserving JS-facing errors.

pub mod analytics;
pub mod attribution;
pub mod cashflows;
pub mod core;
pub mod covenants;
pub mod factor_model;
pub mod margin;
pub mod monte_carlo;
pub mod portfolio;
pub mod scenarios;
pub mod statements;
pub mod statements_analytics;
pub mod valuations;
