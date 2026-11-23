//! Shared rates curve bumping logic (plan-driven calibration).

pub(crate) mod discount;
pub(crate) mod forward_ois;
#[cfg(test)]
#[path = "../tests/rates_review.rs"]
mod review_regressions;
#[cfg(test)]
mod tests;

pub(crate) use discount::{
    RateRecalibrationCache, bump_discount_curve_from_rate_calibration_cached,
};
pub(crate) use forward_ois::{
    bump_market_via_rate_quote_shock_cached, bump_single_ois_market_via_rate_quote_shock_cached,
};
