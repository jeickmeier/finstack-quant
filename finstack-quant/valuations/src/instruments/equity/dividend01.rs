//! Shared Dividend01 finite difference for equity instruments.
//!
//! Dividend01 is the change in PV for a 1bp (0.0001) absolute move in the
//! dividend-yield scalar, computed as a central difference with the downside
//! bump clamped at zero.
//!
//! # One-sided degradation near zero dividend yield
//!
//! When the baseline yield `q0 < DIVIDEND_BUMP_BP` the clamp engages and
//! `actual_width < 2 * dq`, so the difference is no longer centred on `q0`.
//! `scaled_central_diff_by_width` rescales by the true width, so the `$/bp`
//! magnitude is correct, but the Greek silently degrades to a one-sided
//! (forward) difference. This is an accepted approximation.
//!
//! The dividend-yield scalar may be stored as either `MarketScalar::Unitless`
//! or `MarketScalar::Price`; both are read through `scalar_numeric_value`.

use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::{
    replace_scalar_value, scalar_numeric_value, scaled_central_diff_by_width, MetricContext,
};
use finstack_quant_core::types::PriceId;
use finstack_quant_core::Result;

/// Standard dividend yield bump: 1bp (0.0001)
pub(crate) const DIVIDEND_BUMP_BP: f64 = 0.0001;

/// Central-difference Dividend01 of `instrument` with respect to the
/// `div_yield_id` scalar in `context.curves`.
///
/// # Arguments
///
/// * `instrument` - Instrument revalued via `Instrument::value` at each bump.
/// * `div_yield_id` - Dividend-yield scalar to bump; `None` means no dividend
///   exposure and yields `0.0`.
/// * `context` - Metric context supplying the base market and valuation date.
pub(crate) fn dividend01_central_diff(
    instrument: &dyn Instrument,
    div_yield_id: Option<&PriceId>,
    context: &MetricContext,
) -> Result<f64> {
    let Some(div_yield_id) = div_yield_id else {
        return Ok(0.0);
    };
    let as_of = context.as_of;
    let current_scalar = context.curves.get_price(div_yield_id)?;
    let q0 = scalar_numeric_value(current_scalar);
    let q_up_val = q0 + DIVIDEND_BUMP_BP;
    let q_down_val = (q0 - DIVIDEND_BUMP_BP).max(0.0);
    let actual_width = q_up_val - q_down_val;

    let curves_up = replace_scalar_value(
        &context.curves,
        div_yield_id.as_str(),
        current_scalar,
        q_up_val,
    )?;
    let pv_up = instrument.value(&curves_up, as_of)?.amount();
    let curves_down = replace_scalar_value(
        &context.curves,
        div_yield_id.as_str(),
        current_scalar,
        q_down_val,
    )?;
    let pv_down = instrument.value(&curves_down, as_of)?.amount();

    // MetricId contract: Dividend01 is $/bp (dPV for a 1bp absolute q move).
    // The up-bump always lifts the width by `DIVIDEND_BUMP_BP`, so it is
    // non-degenerate; a degenerate width surfaces as an `Err`.
    scaled_central_diff_by_width(pv_up, pv_down, actual_width, DIVIDEND_BUMP_BP)
}

#[cfg(test)]
mod tests {
    use super::DIVIDEND_BUMP_BP;

    /// Mirror the bump-width logic from `dividend01_central_diff`.
    fn bump_geometry(q0: f64) -> (f64, f64, f64) {
        let q_up = q0 + DIVIDEND_BUMP_BP;
        let q_down = (q0 - DIVIDEND_BUMP_BP).max(0.0);
        (q_up, q_down, q_up - q_down)
    }

    #[test]
    fn symmetric_when_yield_above_one_bp() {
        let q0 = 0.02;
        let (q_up, q_down, width) = bump_geometry(q0);
        assert!((width - 2.0 * DIVIDEND_BUMP_BP).abs() < 1e-12);
        assert!(((q_up + q_down) / 2.0 - q0).abs() < 1e-15);
    }

    #[test]
    fn degrades_to_one_sided_when_yield_below_one_bp() {
        let q0 = 0.3 * DIVIDEND_BUMP_BP;
        let (q_up, q_down, width) = bump_geometry(q0);
        assert!(q_down.abs() < 1e-18);
        assert!(width < 2.0 * DIVIDEND_BUMP_BP);
        assert!(((q_up + q_down) / 2.0 - q0).abs() > 1e-9);
    }
}
