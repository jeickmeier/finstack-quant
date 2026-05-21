//! Recovery01 calculator for CDS Option.
//!
//! Computes Recovery01 (recovery rate sensitivity) using finite differences.
//! Recovery01 measures the change in PV for a 1% (100bp) change in recovery rate.
//!
//! When the hazard curve carries its bootstrap par-spread quotes, Recovery01
//! reboots the hazard curve under the bumped recovery so quoted spreads remain
//! invariant. Curves without par quotes fall back to a frozen-curve local bump.

use crate::calibration::bumps::hazard::recalibrate_hazard_with_recovery_and_doc_clause_and_valuation_convention;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds::metrics::market_doc_clause;
use crate::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::Result;

/// Standard recovery rate bump: 1% (0.01)
const RECOVERY_BUMP: f64 = 0.01;

const MIN_EFFECTIVE_BUMP: f64 = 1e-6;

/// Recovery01 calculator for CDS Option.
pub(crate) struct Recovery01Calculator;

fn price_at_bumped_recovery(
    option: &CDSOption,
    base_market: &MarketContext,
    new_recovery: f64,
    as_of: finstack_core::dates::Date,
) -> Result<f64> {
    let mut bumped_option = option.clone();
    bumped_option.recovery_rate = new_recovery;

    let hazard = base_market.get_hazard(&option.credit_curve_id)?;
    let has_par_quotes = hazard.par_spread_points().next().is_some();
    let market_for_pricing = if has_par_quotes {
        let synthetic = synthetic_underlying_cds(option, as_of)?;
        let recalibrated =
            recalibrate_hazard_with_recovery_and_doc_clause_and_valuation_convention(
                hazard.as_ref(),
                new_recovery,
                base_market,
                Some(&option.discount_curve_id),
                Some(market_doc_clause(&synthetic)),
                Some(synthetic.valuation_convention),
            )?;
        base_market.clone().insert(recalibrated)
    } else {
        // Frozen-curve fallback: keep the hazard λ knots unchanged but realign
        // the curve's `recovery_rate` metadata with the bumped trade recovery
        // so the recovery-consistency guard in the underlying CDS pricer
        // accepts the (trade, curve) pair. The priced PV is the documented
        // LGD-only sensitivity since λ is untouched.
        let frozen = hazard.with_recovery_rate(new_recovery)?;
        base_market.clone().insert(frozen)
    };

    Ok(bumped_option.value(&market_for_pricing, as_of)?.amount())
}

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CDSOption = context.instrument_as()?;
        let as_of = context.as_of;
        let base_recovery = option.recovery_rate;

        let bumped_up = (base_recovery + RECOVERY_BUMP).clamp(0.001, 0.999);
        let bumped_down = (base_recovery - RECOVERY_BUMP).clamp(0.001, 0.999);
        let up_delta = bumped_up - base_recovery;
        let down_delta = base_recovery - bumped_down;

        let can_bump_up = up_delta > MIN_EFFECTIVE_BUMP;
        let can_bump_down = down_delta > MIN_EFFECTIVE_BUMP;

        let slope = match (can_bump_up, can_bump_down) {
            (true, true) => {
                let pv_up = price_at_bumped_recovery(option, &context.curves, bumped_up, as_of)?;
                let pv_down =
                    price_at_bumped_recovery(option, &context.curves, bumped_down, as_of)?;
                (pv_up - pv_down) / (up_delta + down_delta)
            }
            (true, false) => {
                let pv_up = price_at_bumped_recovery(option, &context.curves, bumped_up, as_of)?;
                (pv_up - context.base_value.amount()) / up_delta
            }
            (false, true) => {
                let pv_down =
                    price_at_bumped_recovery(option, &context.curves, bumped_down, as_of)?;
                (context.base_value.amount() - pv_down) / down_delta
            }
            (false, false) => 0.0,
        };

        Ok(slope * RECOVERY_BUMP)
    }
}
