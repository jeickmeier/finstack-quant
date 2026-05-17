//! Dividend risk calculator for equity options.
//!
//! Computes dividend risk (dividend yield sensitivity) using finite differences.
//! Dividend risk measures the change in PV for a 1bp (0.0001) change in dividend yield.
//!
//! # Formula
//! ```text
//! Dividend01 = (PV(q + dq) - PV(q - dq)) / (q_up - q_down) * dq
//! ```
//! Where `dq` is the bump size (e.g., 0.0001 for 1bp).
//!
//! # Note
//! For options, dividend yield affects the forward price: F = S * exp((r - q) * T).
//! Higher dividend yield reduces the forward, making calls less valuable and puts more valuable.
//!
//! # One-sided degradation near zero dividend yield
//! The downside bump `q - dq` is clamped at `0` to avoid a negative dividend
//! yield. When the baseline yield `q0 < DIVIDEND_BUMP_BP` (1bp) the clamp
//! engages: `actual_width < 2 * dq`, so the finite difference is no longer
//! centered on `q0`. `scaled_central_diff_by_width` still rescales by the
//! true `actual_width`, so the reported `$/bp` magnitude is correct, but the
//! Greek silently degrades from a symmetric central difference to a one-sided
//! (forward) difference. This is an accepted approximation — a near-zero
//! dividend yield is rare and the bias is small — but callers interpreting
//! the sensitivity as exactly centered should be aware of it.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::equity_option::EquityOption;
use crate::metrics::{
    replace_scalar_value, scalar_numeric_value, scaled_central_diff_by_width, MetricCalculator,
    MetricContext,
};
use finstack_core::Result;

/// Standard dividend yield bump: 1bp (0.0001)
const DIVIDEND_BUMP_BP: f64 = 0.0001;

/// Dividend risk calculator for equity options.
pub(crate) struct DividendRiskCalculator;

impl MetricCalculator for DividendRiskCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &EquityOption = context.instrument_as()?;
        let as_of = context.as_of;

        // Check if expired
        let t = option.day_count.year_fraction(
            as_of,
            option.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        // If no dividend yield ID, risk is zero
        let div_yield_id = match &option.div_yield_id {
            Some(id) => id.clone(),
            None => return Ok(0.0),
        };

        // Get current scalar to clone its structure
        let current_scalar = context.curves.get_price(&div_yield_id)?;

        // Extract numeric baseline for robust bump-width handling (clamped at 0 on the downside).
        //
        // When `q0 < DIVIDEND_BUMP_BP`, the downside bump clamps at 0 and
        // `actual_width < 2 * DIVIDEND_BUMP_BP`: the central difference
        // degrades to a one-sided (forward) difference. The result is still
        // rescaled by `actual_width` below so the `$/bp` magnitude is correct,
        // but it is no longer centered on `q0`. See the module-level docs.
        let q0 = scalar_numeric_value(current_scalar);
        let q_up_val = q0 + DIVIDEND_BUMP_BP;
        let q_down_val = (q0 - DIVIDEND_BUMP_BP).max(0.0);
        let actual_width = q_up_val - q_down_val;

        let curves_up = replace_scalar_value(
            &context.curves,
            div_yield_id.as_str(),
            current_scalar,
            q_up_val,
        );
        let pv_up = option.value(&curves_up, as_of)?.amount();

        let curves_down = replace_scalar_value(
            &context.curves,
            div_yield_id.as_str(),
            current_scalar,
            q_down_val,
        );
        let pv_down = option.value(&curves_down, as_of)?.amount();

        // MetricId contract: Dividend01 is $/bp (dPV for a 1bp absolute q move).
        // Use actual bump width since the downside bump is clamped at 0; the
        // up-bump always lifts the width by `DIVIDEND_BUMP_BP`, so it is
        // non-degenerate. A degenerate width surfaces as an `Err`.
        scaled_central_diff_by_width(pv_up, pv_down, actual_width, DIVIDEND_BUMP_BP)
    }
}

#[cfg(test)]
mod tests {
    use super::DIVIDEND_BUMP_BP;

    /// Mirror the bump-width logic from `calculate` for a given baseline yield.
    fn bump_geometry(q0: f64) -> (f64, f64, f64) {
        let q_up = q0 + DIVIDEND_BUMP_BP;
        let q_down = (q0 - DIVIDEND_BUMP_BP).max(0.0);
        (q_up, q_down, q_up - q_down)
    }

    /// W-35: with `q0 >= 1bp` the bump is a symmetric central difference of
    /// full width `2 * DIVIDEND_BUMP_BP` centered on `q0`.
    #[test]
    fn symmetric_when_yield_above_one_bp() {
        let q0 = 0.02; // 2% — well above 1bp
        let (q_up, q_down, width) = bump_geometry(q0);
        // Full symmetric width (to floating-point rounding).
        assert!((width - 2.0 * DIVIDEND_BUMP_BP).abs() < 1e-12);
        // Centered: q0 is the midpoint of [q_down, q_up].
        assert!(((q_up + q_down) / 2.0 - q0).abs() < 1e-15);
    }

    /// W-35: with `q0 < 1bp` the downside bump clamps at 0, the width shrinks
    /// below `2 * DIVIDEND_BUMP_BP`, and the difference is no longer centered
    /// on `q0` — it has silently degraded to a one-sided derivative.
    #[test]
    fn degrades_to_one_sided_when_yield_below_one_bp() {
        let q0 = 0.3 * DIVIDEND_BUMP_BP; // 0.3bp — below the bump size
        let (q_up, q_down, width) = bump_geometry(q0);
        // Downside clamped to exactly zero.
        assert!(q_down.abs() < 1e-18);
        // Width is strictly less than the symmetric 2*dq width.
        assert!(width < 2.0 * DIVIDEND_BUMP_BP);
        // And the difference is no longer centered on q0.
        let midpoint = (q_up + q_down) / 2.0;
        assert!((midpoint - q0).abs() > 1e-9);
    }
}
