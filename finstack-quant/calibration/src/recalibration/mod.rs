//! Quote-space market replay and batch-local recalibration caches.
//!
//! # Entry points by asset class
//!
//! | Asset class       | Functions                                                   |
//! |-------------------|-------------------------------------------------------------|
//! | Discount rates    | `bump_discount_curve_from_rate_calibration` |
//! | Forward rates     | `bump_forward_curve_from_rate_calibration`                    |
//! | Credit hazard     | `bump_hazard_spreads`                                        |
//!
//! # Convention
//!
//! Rate/spread quote shocks use the valuations-owned
//! [`QuoteBump`](finstack_quant_valuations::recalibration::QuoteBump).
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

pub use hazard::bump_hazard_spreads;
pub use provider::CachedRecalibrationProvider;
pub use rates::{
    bump_discount_curve_from_rate_calibration, bump_forward_curve_from_rate_calibration,
};

use crate::CalibrationConfig;

pub(super) fn ensure_replay_fit_accepted(
    curve_id: &str,
    config: &CalibrationConfig,
    report: &crate::CalibrationReport,
) -> finstack_quant_core::Result<()> {
    if !config.fail_on_bad_fit || report.success {
        return Ok(());
    }

    let tolerance = report
        .success_tolerance
        .map(|value| format!("{value:.3e}"))
        .unwrap_or_else(|| "unspecified".to_string());
    let worst = match (&report.worst_quote_id, report.worst_quote_residual) {
        (Some(id), Some(residual)) => {
            format!(", worst quote '{id}' residual {residual:.3e}")
        }
        _ => String::new(),
    };
    Err(finstack_quant_core::Error::Calibration {
        message: format!(
            "calibration replay for '{curve_id}' failed fit acceptance: max residual {:.3e} \
             with tolerance {tolerance}{worst}; {}",
            report.max_residual,
            report
                .validation_error
                .as_deref()
                .unwrap_or(&report.convergence_reason)
        ),
        category: "replay_bad_fit".to_string(),
    })
}
