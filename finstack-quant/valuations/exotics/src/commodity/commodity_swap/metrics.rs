//! Commodity swap metrics module.
//!
//! Provides metric calculators specific to `CommoditySwap`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_commodity_swap_metrics`.
//!
//! Exposed metrics:
//! - commodity delta (sensitivity to floating index)
//! - theta (time decay)
//! - DV01 (interest rate sensitivity)

use crate::commodity::commodity_swap::CommoditySwap;
use finstack_quant_core::Result;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_valuations_core::instruments::PayReceive;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all CommoditySwap metrics with the registry.
pub(crate) fn register_commodity_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CommoditySwap,
        metrics: [
            (Delta, DeltaCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_discount_only())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_discount_only())),
        ]
    }
    Ok(())
}

/// Delta calculator for commodity swaps (per 1.0 unit of floating price).
///
/// Uses `df_between_dates(as_of, payment_date)` for base-date-safe discounting,
/// consistent with the NPV calculation in `CommoditySwap::floating_leg_pv()`.
///
/// Delta measures the sensitivity of the swap's NPV to changes in the floating price.
/// For each payment period:
///
/// Delta contribution = sign × Q × DF(as_of → payment_date)
///
/// Uses `df_between_dates(as_of, payment_date)` for base-date-safe discounting.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap: &CommoditySwap = context.instrument_as()?;
        swap.validate()?;
        let _ = context
            .curves
            .get_price_curve(swap.forward_curve_id.as_str())?;
        let disc = context
            .curves
            .get_discount(swap.discount_curve_id.as_str())?;
        let is_business_day =
            crate::commodity::averaging::business_day_filter(swap.calendar_id.as_deref())?;
        let float_sign = match swap.side {
            PayReceive::Pay => 1.0,
            PayReceive::Receive => -1.0,
        };
        let mut delta = 0.0;
        for period in swap
            .periods()?
            .into_iter()
            .filter(|p| p.payment_date >= context.as_of)
        {
            let (obs_start, obs_end) = swap.shifted_observation_window(&period)?;
            let (projected, total) = crate::commodity::averaging::business_day_observation_weights(
                &is_business_day,
                obs_start,
                obs_end,
                period.accrual_end == swap.maturity,
                context.as_of,
            )?;
            let df_pay = disc.df_between_dates(context.as_of, period.payment_date)?;
            delta += float_sign * swap.quantity * df_pay * (projected as f64 / total as f64);
        }
        Ok(delta)
    }
}
