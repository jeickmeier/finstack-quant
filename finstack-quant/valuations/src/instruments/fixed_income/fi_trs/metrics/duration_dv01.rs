//! Duration-based DV01 calculator for fixed income index TRS.

use crate::instruments::common_impl::parameters::trs_common::TrsSide;
use crate::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::Result;

/// Calculates duration-based DV01 for fixed income index TRS.
///
/// Measures the dollar value change for a 1 basis point yield shift:
///
/// ```text
/// DurationDv01 = Notional × Duration × 0.0001
/// ```
///
/// This is a yield sensitivity metric (not an index-level delta). For equity TRS,
/// use `IndexDelta` which measures `dV/dS` per unit of index level change.
///
/// # Sign Convention
///
/// Returns **negative** for `ReceiveTotalReturn` and **positive** for
/// `PayTotalReturn`. A total-return receiver is economically long the reference
/// bond index, so its signed rate sensitivity is negative: the position loses
/// value when yields rise. This matches the ISDA SIMM IR-delta convention (see
/// the `Marginable::simm_sensitivities` implementation for this instrument) and
/// the workspace CS01 convention (long credit → negative to spread widening).
///
/// Note: the underlying FI-TRS pricer is carry/income-only — it has no price
/// mark-to-market term, so `dV/dy` of the carry model itself would not produce
/// this sign. The convention here is chosen for economic consistency with the
/// rest of the workspace, not derived from the carry model.
///
/// # Errors
///
/// Returns an error if `duration_id` is configured but missing from market data.
/// When `duration_id` is `None`, defaults to 5.0 years (broad market assumption).
pub(crate) struct DurationDv01Calculator;

impl MetricCalculator for DurationDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let trs: &FIIndexTotalReturnSwap = context.instrument_as()?;

        // Get duration from market data.
        // If duration_id is configured, the data MUST be present (error on missing).
        // If duration_id is None, default to 5.0 years (broad index assumption).
        let duration = match &trs.underlying.duration_id {
            Some(id) => {
                let scalar = context.curves.get_price(id.as_str()).map_err(|_| {
                    finstack_quant_core::Error::Validation(format!(
                        "Index duration data '{}' is configured but not found in market context. \
                         Provide the duration scalar or remove duration_id to use 5.0Y default.",
                        id
                    ))
                })?;
                match scalar {
                    MarketScalar::Unitless(v) => *v,
                    MarketScalar::Price(_) => {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "Market scalar '{}' for index duration has type Price, but duration \
                             is a unitless quantity. Use MarketScalar::Unitless instead.",
                            id
                        )));
                    }
                }
            }
            // Default 5.0Y duration when not provided — may be inappropriate
            // for short-duration indices (money market, T-bill indices).
            // Consider supplying an explicit duration_id for non-broad-market indices.
            None => 5.0,
        };

        // DV01 = Notional × Duration × 1bp
        let dv01 = trs.notional.amount() * duration * 0.0001;

        // Long the reference bond (receive TR) loses when yields rise → negative.
        Ok(match trs.side {
            TrsSide::ReceiveTotalReturn => -dv01,
            TrsSide::PayTotalReturn => dv01,
        })
    }
}
