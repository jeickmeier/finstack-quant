//! Draw option cost metric for revolving credit facilities.

use crate::instruments::fixed_income::revolving_credit::pricing::RevolvingCreditPricer;
use crate::instruments::fixed_income::revolving_credit::types::DrawRepaySpec;
use crate::instruments::RevolvingCredit;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculator for the Monte Carlo mean draw option cost: the value to the
/// lender of the facility's simulated draws having been made at the
/// contractual margin instead of each path's fair spread, anchored to the
/// margin at the valuation date so only spread changes count (negative when
/// spreads widen). Deterministic facilities have no simulated draws and
/// report `0.0`.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct DrawOptionCostCalculator;

impl MetricCalculator for DrawOptionCostCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        match &facility.draw_repay_spec {
            DrawRepaySpec::Deterministic(_) => Ok(0.0),
            DrawRepaySpec::Stochastic(_) => {
                let result = RevolvingCreditPricer::price_with_paths(
                    facility,
                    &context.curves,
                    context.as_of,
                )?;
                Ok(result.draw_option_cost.mean.amount())
            }
        }
    }
}
