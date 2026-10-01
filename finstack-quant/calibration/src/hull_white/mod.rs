//! Hull-White one-factor model calibration to European swaptions.
//!
//! Plan-driven engine steps `hull_white` and `cap_floor_hull_white` are the
//! canonical public entry. The functions in this module are the solvers those
//! steps invoke after converting envelope volatility quotes.
//!
//! Calibrates the two Hull-White parameters (mean reversion κ and short rate
//! volatility σ) by minimising squared swaption price errors using the
//! Levenberg-Marquardt algorithm.
//!
//! # Mathematical Foundation
//!
//! The Hull-White one-factor model specifies the short rate dynamics:
//!
//! ```text
//! dr(t) = [θ(t) − κ r(t)] dt + σ dW(t)
//!
//! where:
//!   κ = mean reversion speed
//!   σ = short rate volatility
//!   θ(t) = time-dependent drift chosen to match the initial term structure
//! ```
//!
//! # Swaption Pricing
//!
//! European swaptions are priced analytically using the Jamshidian (1989)
//! decomposition, which expresses a coupon bond option as a portfolio of
//! zero-coupon bond options under the HW1F model.
//!
//! The zero-coupon bond option volatility is:
//!
//! ```text
//! σ_P(t, T, S) = B(T,S) × σ × √((1 − e^{−2κt}) / (2κ))
//!
//! where B(T,S) = (1/κ)(1 − e^{−κ(S−T)})
//! ```
//!
//! # References
//!
//! - Hull, J. & White, A. (1990). "Pricing Interest-Rate-Derivative Securities."
//!   *Review of Financial Studies*, 3(4), 573-592. `docs/REFERENCES.md#hull-white-1990-pricing-ird`
//! - Jamshidian, F. (1989). "An Exact Bond Option Formula."
//!   *Journal of Finance*, 44(1), 205-209. `docs/REFERENCES.md#jamshidian-1989-bond-option`
//! - Brigo, D. & Mercurio, F. (2006). *Interest Rate Models — Theory and Practice*.
//!   Springer Finance (2nd ed.), Chapter 3. `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`

use finstack_quant_core::math::piecewise::PiecewiseConstantCurve;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::math::special_functions::{norm_cdf, norm_pdf};
use std::collections::BTreeMap;

use crate::config::CalibrationConfig;
use crate::solver::global::GlobalFitOptimizer;
use crate::solver::multi_start::MultiStartConfig;
use crate::solver::traits::GlobalSolveTarget;
use crate::CalibrationReport;

mod cap_floor;
mod curves;
mod pricing;
mod quotes;
mod swaption;
mod targets;

pub use cap_floor::{
    bootstrap_hull_white_sigma_schedule_to_cap_floors, calibrate_hull_white_to_cap_floors,
    PiecewiseSigmaCalibrationConfig,
};
pub use curves::{
    bootstrap_hull_white_sigma_schedule_to_cap_floors_from_curves,
    calibrate_hull_white_to_cap_floors_from_curves, calibrate_hull_white_to_swaptions_from_curve,
};
pub use finstack_quant_models::rates::hull_white::{
    capfloor_hw1f_scalar_keys, capfloor_hw1f_sigma_schedule_key, hw1f_scalar_keys,
    HullWhiteCalibrationParams, HullWhiteParams,
};
pub use quotes::{
    CapFloorCalibrationConfig, CapFloorQuote, SwapFrequency, SwaptionQuote, SwaptionSchedule,
};
pub use swaption::calibrate_hull_white_to_swaptions;

#[cfg(test)]
pub(crate) use pricing::hw1f_cap_floor_implied_normal_vol;
pub(crate) use pricing::{
    bachelier_cap_floor_price, hw1f_cap_floor_price, hw1f_cap_floor_price_with_model,
    CapFloorPriceSpec,
};
#[cfg(test)]
pub(crate) use swaption::{compute_swap_annuity_and_rate, hw1f_swaption_price};

#[cfg(test)]
mod tests;
