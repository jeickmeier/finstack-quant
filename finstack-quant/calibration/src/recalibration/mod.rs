//! Quote-space market replay and batch-local recalibration caches.
//!
//! # Entry points by asset class
//!
//! | Asset class       | Functions                                                   |
//! |-------------------|-------------------------------------------------------------|
//! | Discount rates    | `bump_discount_curve_from_rate_calibration` |
//! | Credit hazard     | `bump_hazard_spreads`                                        |
//!
//! # Convention
//!
//! Rate/spread quote shocks use the valuations-owned
//! [`QuoteBump`](finstack_quant_valuations_core::recalibration::QuoteBump).
//! Direct curve and surface shocks use core `BumpSpec`/`Bumpable` instead.
//!
//! # Calibration policy
//!
//! Stored rate-calibration recipes retain the quote set, method, curve role,
//! day count, and OIS convention needed by the
//! `*_from_rate_calibration` entry points. Synthetic bump helpers operate
//! directly on curve knots and do not recalibrate.
//!
//! ## Induced-error bound
//!
//! For recalibrated bumps, both the base curve and the bumped curve reprice
//! every input quote to within their respective calibration tolerances, so residual leakage into a
//! sensitivity is bounded by roughly the **sum of the two repricing tolerances
//! divided by the bump size**. With the default `1e-8` fit tolerance and a 1bp
//! bump this is on the order of `2e-8 / 1e-4 ≈ 2e-4` of the PV unit.

mod cache;
pub(crate) mod hazard;
mod provider;
pub(crate) mod rates;
mod replay_fit;

pub use hazard::bump_hazard_spreads;
pub use provider::{CachedRecalibrationProvider, pricing_options};
pub use rates::discount::{
    CalibrationQuoteCoordinate, bump_discount_curve_from_rate_calibration, bump_discount_rate_quote,
};
pub use rates::forward_ois::{bump_forward_rate_quote, bump_linked_ois_rate_quote};

use replay_fit::ensure_replay_fit_accepted;
