//! WASM bindings for the calibration engine.
//!
//! Mirrors the Python `calibrate`, `validate_calibration`, and `dry_run` surface.
//!
//! # Number safety
//!
//! `calibrate`, `validateCalibration`, and `dryRun` return structured objects built by
//! `crate::utils::to_js_value`, so counts (`iterations`, `residual_evals`,
//! `lm_jacobian_evals`) arrive as JS numbers. That serializer refuses to emit
//! an integer above `Number.MAX_SAFE_INTEGER` (2^53 − 1) instead of rounding
//! it, so structured results never silently lose integer precision.
//!
//! On error, the host functions throw a JS `Error` with `name =
//! "CalibrationEnvelopeError"`. The error exposes `kind` (the execution
//! category), `stage`, `step_id`, `solver_diagnostics` (an object), the
//! strict-load `diagnostics` array (empty unless ingestion rejected the
//! document), JSON-string `details`, and a structured `cause` object. Absent
//! optional properties are `undefined`.
//!
//! # Submodules
//!
//! - `quotes`, `steps`: free-function twins of the Python quote, rate-bound
//!   and step constructors, returning the plain serde objects.
//! - `hull_white`: direct Hull-White one-factor calibrators on curve handles.
//! - `result`: free-function twins of the Python `CalibrationResultEnvelope` methods.

// `EnvelopeError` / `ExecuteError` are intentionally large structured errors
// (rich diagnostic payloads); boxing them would change their public API.
// The upstream `calibration::api::{engine, validate}` modules make the same
// allowance — keep the binding layer consistent.
#![allow(clippy::result_large_err)]

mod envelope;
mod execute;
mod hull_white;
mod quotes;
mod result;
mod steps;
#[cfg(test)]
mod tests;
mod validation;

pub use envelope::*;
pub use execute::*;
