//! Dividend risk calculator for convertible bonds.
//!
//! Computes dividend risk (dividend yield sensitivity) using finite differences.
//! Dividend risk measures the change in PV for a 1bp (0.0001) change in dividend yield.
//!
//! # Note
//! For convertibles, dividend yield affects the equity option component.
//! Higher dividend yield reduces the forward price, making conversion less attractive.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::convertible::{
    market_inputs::resolve_dividend_yield_market_value_id, ConvertibleBond,
};
use crate::metrics::{
    replace_scalar_value, scalar_numeric_value, scaled_central_diff_by_width, MetricCalculator,
    MetricContext,
};
use finstack_core::Result;

/// Standard dividend yield bump: 1bp (0.0001)
const DIVIDEND_BUMP_BP: f64 = 0.0001;

/// Dividend risk calculator for convertible bonds.
pub(crate) struct DividendRiskCalculator;

impl MetricCalculator for DividendRiskCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let convertible: &ConvertibleBond = context.instrument_as()?;
        let as_of = context.as_of;

        let Some(div_yield_id) =
            resolve_dividend_yield_market_value_id(&context.curves, convertible)?
        else {
            return Ok(0.0); // No dividend yield available, risk is zero
        };

        // Get current dividend yield
        let current_scalar = context.curves.get_price(&div_yield_id)?;

        // Extract numeric baseline for robust bump-width handling (clamped at 0 on the downside).
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
        let pv_up = convertible.value(&curves_up, as_of)?.amount();

        let curves_down = replace_scalar_value(
            &context.curves,
            div_yield_id.as_str(),
            current_scalar,
            q_down_val,
        );
        let pv_down = convertible.value(&curves_down, as_of)?.amount();

        // MetricId contract: Dividend01 is $/bp (dPV for a 1bp absolute q move).
        // Use the *actual* bump width since the downside bump is clamped at 0;
        // the up-bump always lifts the width by `DIVIDEND_BUMP_BP`, so it is
        // non-degenerate. A degenerate width surfaces as an `Err`.
        scaled_central_diff_by_width(pv_up, pv_down, actual_width, DIVIDEND_BUMP_BP)
    }
}
