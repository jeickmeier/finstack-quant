//! Simultaneous weighted least-squares calibration using Levenberg–Marquardt.
//!
//! Notes:
//! - Optional multi-start support to escape local minima (see [`MultiStartConfig`]).
//! - Residual weighting is supported via per-quote weights (weighted least squares).

mod diagnostics;
mod optimize;
#[cfg(test)]
mod tests;

pub(crate) use optimize::{optimize, optimize_with_multi_start};
