//! Recovery01 calculator for CDS Option.
//!
//! Computes Recovery01 (recovery rate sensitivity) using finite differences.
//! Recovery01 measures the change in PV for a 1% (100bp) change in recovery rate.
//!
//! When the hazard curve carries its bootstrap par-spread quotes, Recovery01
//! reboots the hazard curve under the bumped recovery so quoted spreads remain
//! invariant. Curves without par quotes fall back to a frozen-curve local bump.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds::metrics::market_doc_clause;
use crate::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
use crate::instruments::credit_derivatives::cds_option::CdsOption;
use crate::metrics::{MetricCalculator, MetricContext};
use crate::recalibration::{HazardRecalibrationAction, HazardRecalibrationRequest};
use finstack_quant_core::Result;

/// Standard recovery rate bump: 1% (0.01)
const RECOVERY_BUMP: f64 = 0.01;

const MIN_EFFECTIVE_BUMP: f64 = 1e-6;

/// Recovery01 calculator for CDS Option.
pub(crate) struct Recovery01Calculator;

fn price_at_bumped_recovery(
    option: &CdsOption,
    context: &MetricContext,
    new_recovery: f64,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    let base_market = context.curves.as_ref();
    let mut bumped_option = option.clone();
    bumped_option.recovery_rate = new_recovery;

    let hazard = base_market.get_hazard(&option.credit_curve_id)?;
    let has_par_quotes = hazard.hazard_calibration().is_some();
    let market_for_pricing = if has_par_quotes {
        let synthetic = synthetic_underlying_cds(option, as_of)?;
        let recalibrated = context.rebuild_hazard_curve(
            HazardRecalibrationRequest {
                hazard,
                source_market: std::sync::Arc::clone(&context.curves),
                target_market: std::sync::Arc::clone(&context.curves),
                discount_curve_id: option.discount_curve_id.clone(),
                doc_clause: Some(market_doc_clause(&synthetic)),
                cds_valuation_convention: Some(synthetic.valuation_convention),
                deal_quote_override: None,
                action: HazardRecalibrationAction::RecoveryRateReplay {
                    recovery_rate: new_recovery,
                },
            },
            "recovery01",
        )?;
        base_market.clone().insert(recalibrated.as_ref().clone())
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
        let option: &CdsOption = context.instrument_as()?;
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
                let pv_up = price_at_bumped_recovery(option, context, bumped_up, as_of)?;
                let pv_down = price_at_bumped_recovery(option, context, bumped_down, as_of)?;
                (pv_up - pv_down) / (up_delta + down_delta)
            }
            (true, false) => {
                let pv_up = price_at_bumped_recovery(option, context, bumped_up, as_of)?;
                (pv_up - context.base_value.amount()) / up_delta
            }
            (false, true) => {
                let pv_down = price_at_bumped_recovery(option, context, bumped_down, as_of)?;
                (context.base_value.amount() - pv_down) / down_delta
            }
            (false, false) => 0.0,
        };

        Ok(slope * RECOVERY_BUMP)
    }
}
