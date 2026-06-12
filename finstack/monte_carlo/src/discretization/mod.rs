//! Time-stepping schemes for stochastic differential equations.
//!
//! Start with [`exact`] whenever an analytical transition is available because
//! it avoids discretization bias. This module includes general-purpose Euler /
//! Milstein schemes and model-specific QE / jump schemes for Heston, CIR, and
//! jump-diffusion dynamics.
//!
//! Each discretization module documents the assumptions it makes about the
//! process state, convergence behavior, and positivity / stability guarantees.

pub mod cheyette_rough;
pub mod euler;
pub mod exact;
pub mod exact_gbm_dividends;
pub mod exact_hw1f;
pub mod jump_euler;
pub mod lmm_predictor_corrector;
pub mod milstein;
pub mod qe_bates;
pub mod qe_cir;
pub(crate) mod qe_common;
pub mod qe_heston;
pub mod rough_bergomi;
pub mod rough_heston;
pub mod schwartz_smith;

pub use cheyette_rough::CheyetteRoughEuler;
pub use euler::{EulerMaruyama, LogEuler};
pub use exact::{ExactGbm, ExactMultiGbm, ExactMultiGbmCorrelated};
pub use exact_gbm_dividends::ExactGbmWithDividends;
pub use exact_hw1f::ExactHullWhite1F;
pub use jump_euler::JumpEuler;
pub use milstein::{LogMilstein, Milstein};
pub use qe_bates::QeBates;
pub use qe_cir::QeCir;
pub use qe_heston::QeHeston;
pub use rough_bergomi::RoughBergomiEuler;
pub use rough_heston::RoughHestonHybrid;
pub use schwartz_smith::ExactSchwartzSmith;
