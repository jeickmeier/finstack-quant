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
