//! Time-stepping schemes for stochastic differential equations.
//!
//! Start with [`exact`] whenever an analytical transition is available because
//! it avoids discretization bias. This module includes general-purpose Euler /
//! Milstein schemes and model-specific QE schemes for Heston and CIR dynamics.
//!
//! Each discretization module documents the assumptions it makes about the
//! process state, convergence behavior, and positivity / stability guarantees.

pub mod cheyette_rough;
pub mod credit_line;
pub mod euler;
pub mod exact;
pub mod exact_gbm_dividends;
pub mod exact_hw1f;
pub mod lmm_predictor_corrector;
pub mod milstein;
pub mod qe_cir;
pub(crate) mod qe_common;
pub mod qe_heston;
mod rough;
pub mod rough_bergomi;
pub mod rough_heston;
pub mod schwartz_smith;
#[cfg(test)]
mod tests;

use rough::rough_level;
