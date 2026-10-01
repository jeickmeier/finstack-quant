//! Market DV01 for interest rate swaps.
//!
//! DV01 is a full revaluation: the rate market moves one basis point and the
//! swap is repriced, so coupons that have already fixed carry no projection
//! risk. The closed form `N·[(K − S)·dA − A·1bp]` is only valid for an
//! unseasoned swap, because it moves the swap's own par rate by a full basis
//! point even when part of the floating leg is realized.
//!
//! When the discount and projection curves carry rate-calibration metadata,
//! the shock is applied to the calibration quotes and the curves are
//! re-bootstrapped. That is the par-curve DV01 Bloomberg SWPM reports
//! (`docs/REFERENCES.md#bloomberg-swpm`). Otherwise the fitted curves are
//! bumped in parallel through [`UnifiedDv01Calculator`].

use crate::instruments::rates::irs::InterestRateSwap;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{
    Dv01CalculatorConfig, MetricCalculator, MetricContext, UnifiedDv01Calculator,
};
use finstack_quant_core::Result;

/// IRS DV01 calculator: quote-shock re-bootstrap when the curves carry
/// calibration metadata, fitted-curve parallel bump otherwise.
pub(crate) struct IrsDv01Calculator;

impl MetricCalculator for IrsDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let irs: &InterestRateSwap = context.instrument_as()?;
        let market = context.curves.as_ref();
        let discount_id = irs.fixed_leg.discount_curve_id.clone();
        let forward_id = irs.float_leg.forward_curve_id.clone();
        let discount_has_replay = market
            .get_discount(discount_id.as_str())?
            .rate_calibration()
            .is_some();

        // One OIS curve both discounts and projects the overnight leg.
        let single_curve_ois =
            discount_id == forward_id && market.get_forward(forward_id.as_str()).is_err();
        let has_replay = if single_curve_ois {
            discount_has_replay
        } else {
            discount_has_replay
                && market
                    .get_forward(forward_id.as_str())?
                    .rate_calibration()
                    .is_some()
        };
        if !has_replay {
            return UnifiedDv01Calculator::<InterestRateSwap>::new(
                Dv01CalculatorConfig::parallel_combined(),
            )
            .calculate(context);
        }

        let bump_bp = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_pricing_overrides(),
        )?
        .rate_bump_bp;
        let bumped_market = |bp: f64| {
            if single_curve_ois {
                context.bump_single_ois_rate_market_cached(&discount_id, bp)
            } else {
                context.bump_rate_market_cached(&discount_id, &forward_id, bp)
            }
        };
        let pv_up = context.reprice_raw(bumped_market(bump_bp)?.as_ref(), context.as_of)?;
        let pv_down = context.reprice_raw(bumped_market(-bump_bp)?.as_ref(), context.as_of)?;
        Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
    }
}
