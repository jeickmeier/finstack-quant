//! Shared rates pricing utilities.

/// Pathwise money-market account (bank-account numeraire) helpers.
pub mod bank_account;
pub use bank_account::{accumulate_bank_factors, bank_step_factor};
/// Bermudan call provision shared across callable exotic rate products.
pub mod bermudan_call;
/// Deterministic coupon / payoff helpers for exotic rate products.
pub mod coupon_profiles;
/// Cumulative coupon tracker for path-dependent products (TARN, Snowball).
pub mod cumulative_coupon;
pub use cumulative_coupon::CouponEvent;
/// Forward swap rate and annuity helpers shared by CMS instruments.
pub mod forward_swap_rate;
/// Monte Carlo configuration shared across rate exotic pricers.
pub mod mc_config;
pub use mc_config::RateExoticMcConfig;

/// HW1F parameter resolution with overrides/market-scalar/default precedence.
pub mod hw1f_calibration;
pub use hw1f_calibration::{
    resolve_hw1f_params, Hw1fCalibrationFlavor, Hw1fCapletSurfacePoint, Hw1fResolveRequest,
    Hw1fSurfaceCalibration,
};

/// HW1F θ(t) curve calibration and term-forward bond reconstruction.
pub mod hw1f_curve;
pub use hw1f_curve::{
    calibrate_hw1f_params, initial_short_rate_from_curve, Hw1fTermForward, PeriodForwardCoeffs,
};

/// Historical CMS (par swap rate) fixing lookups for seasoned CMS trades.
pub(crate) mod fixings;

/// Exercise-boundary protocol and basis helpers for LSMC-priced rate exotics.
pub mod exercise;
pub use exercise::{extended_basis, standard_basis, ExerciseBoundaryPayoff};

/// Generic HW1F Monte Carlo orchestrator for path-dependent rate exotics.
pub mod hw1f_mc;
pub use hw1f_mc::RateExoticHw1fMcPricer;

/// HW1F Longstaff-Schwartz MC pricer for callable rate exotics.
pub mod hw1f_lsmc;
pub use hw1f_lsmc::RateExoticHw1fLsmcPricer;
