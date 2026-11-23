//! Metric registration and calculators.

use super::{McOasConfig, calculate_mc_oas};
/// Prepayment rate-sensitivity coefficient β shared by every rate-dependent
/// prepayment adjustment in this module.
///
/// Applied as the refinancing-incentive multiplier `exp(-β·Δr)` on prepayment
/// speed. β is calibrated so a 100 bp rate move roughly doubles (rates down)
/// or halves (rates up) the prepayment speed — the standard rule-of-thumb
/// shape of the empirical refinancing S-curve (Hayre 2001; Fabozzi 2016):
/// `β = ln(2) / 0.01 ≈ 69.3`.
///
/// Both the effective duration/convexity engine ([`duration`]) and the
/// Monte-Carlo OAS engine ([`mc_oas`]) use this constant so the two engines
/// price the prepayment option consistently.
pub(crate) const PREPAY_RATE_SENSITIVITY: f64 = std::f64::consts::LN_2 / 0.01;

use crate::mbs_passthrough::AgencyMbsPassthrough;
use finstack_quant_valuations_core::metrics::{
    MetricCalculator, MetricContext, MetricId, MetricRegistry,
};

/// Compute effective duration and convexity in one bumped-repricing pass and
/// store both in `context.computed`, so a request for both metrics reprices
/// the pool three times, not six (the registry skips a metric that is already
/// computed).
fn duration_and_convexity(context: &mut MetricContext) -> finstack_quant_core::Result<(f64, f64)> {
    let mbs: &AgencyMbsPassthrough = context.instrument_as()?;
    let result =
        super::duration::duration_convexity(mbs, context.curves.as_ref(), context.as_of, None)?;
    context
        .computed
        .insert(MetricId::DurationMod, result.duration);
    context
        .computed
        .insert(MetricId::Convexity, result.convexity);
    Ok((result.duration, result.convexity))
}

/// Calculator for effective duration (mapped to DurationMod).
pub(crate) struct EffectiveDurationCalculator;

impl MetricCalculator for EffectiveDurationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        Ok(duration_and_convexity(context)?.0)
    }
}

/// Calculator for effective convexity (mapped to Convexity).
pub(crate) struct EffectiveConvexityCalculator;

impl MetricCalculator for EffectiveConvexityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        Ok(duration_and_convexity(context)?.1)
    }
}

/// Calculator for option-adjusted spread (OAS).
///
/// Reports the Monte Carlo OAS from [`calculate_mc_oas`], using stochastic
/// Hull-White rate paths and rate-dependent prepayment. The Hull-White mean
/// reversion κ and short-rate volatility σ come from
/// `instrument_pricing_overrides.model_config.hw1f_mean_reversion` and
/// `hw1f_sigma`, supplied together; without them the engine uses κ = 5% and
/// σ = 1%. These are user inputs: the metric does not calibrate them to
/// swaptions. `model_config.mc_paths` sets the path count (default 512) and
/// `model_config.mc_seed(&id)` the seed.
pub(crate) struct OasCalculator;

impl MetricCalculator for OasCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let mbs: &AgencyMbsPassthrough = context.instrument_as()?;
        let market_price = mbs
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "mbs.instrument_pricing_overrides.market_quotes.quoted_clean_price_pct"
                        .to_string(),
                })
            })?;
        // True option-adjusted spread from the Monte Carlo model.
        calculate_mc_oas(
            mbs,
            market_price,
            context.curves.as_ref(),
            context.as_of,
            &McOasConfig::resolve(mbs)?,
        )
    }
}

/// Register agency MBS passthrough metrics with the registry.
pub(crate) fn register_mbs_passthrough_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::AgencyMbsPassthrough,
        metrics: [
            (DurationMod, EffectiveDurationCalculator),
            (Convexity, EffectiveConvexityCalculator),
            (Oas, OasCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
