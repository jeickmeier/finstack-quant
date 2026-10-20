//! FRA-specific metrics module.
//!
//! Provides metric calculators for FRAs. Metrics include:
//! - DV01 (parallel rate sensitivity via generic calculator)
//! - Par rate
//! - Bucketed DV01 (key-rate sensitivity)
//!
//! Note: PV is available in `ValuationResult.value`, not as a metric in measures.
//!
//! See unit tests and `examples/` for usage.

use crate::fra::ForwardRateAgreement;
use finstack_quant_core::market_data::term_structures::{
    RateCalibrationQuote, RateCalibrationRecipe,
};
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::sensitivities::cs01::sensitivity_central_diff;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Registers all FRA metrics to a registry.
///
/// Each metric is registered with the "FRA" instrument type to ensure
/// proper applicability filtering.
pub(crate) fn register_fra_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    // Standard metrics using macro
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Fra,
        metrics: [
            (Dv01, FraRateCurveDv01Calculator),
            (Pv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_forward_only())),
            (ForwardPv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_forward_only())),
            (ParRate, FraParRateCalculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

/// Par rate for FRAs (fixed rate that zeroes PV).
///
/// Computes the fixed rate that makes the FRA's PV zero under current curves.
/// For standard FRA settlement-at-start conventions and consistent curves,
/// this equals the forward rate over the period:
///
/// par_rate = (projection_df(start) / projection_df(end) - 1) / index_accrual
///
/// Discount-factor time mapping uses the forward curve's own day count and base
/// date. Annualization uses the FRA's contractual index day count.
pub(crate) struct FraParRateCalculator;

impl MetricCalculator for FraParRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fra: &ForwardRateAgreement = context.instrument_as()?;

        // Forward rate over [t_start, t_end]
        let fwd = context.curves.get_forward(fra.forward_curve_id.as_str())?;

        let period_length = (fra.maturity - fra.start_date).whole_days();
        if period_length == 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FRA '{}': period length is zero ({period_length}); cannot compute par rate",
                fra.id
            )));
        }

        fwd.as_ref().rate_between_dates(
            fra.start_date,
            fra.maturity,
            fra.day_count.year_fraction(
                fra.start_date,
                fra.maturity,
                finstack_quant_core::dates::DayCountContext::default(),
            )?,
        )
    }
}

// FRA quote-shock DV01.
//
// When the discount and forward curves carry rate-calibration metadata, this
// reports quote-shock/rebootstrap DV01 through the shared
// `bump_market_via_rate_quote_shock` helper. When metadata is unavailable, or
// the forward curve was calibrated from tenor-basis quotes (which the shared
// replay does not support), it falls back to the generic fitted-curve bump
// path.

/// FRA DV01 calculator. Prefers quote-shock/rebootstrap when replayable
/// calibration metadata is available, falling back to the generic fitted-curve
/// bump.
pub(crate) struct FraRateCurveDv01Calculator;

impl MetricCalculator for FraRateCurveDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fra: &ForwardRateAgreement = context.instrument_as()?;
        let market = context.curves.as_ref();
        let discount = market.get_discount(fra.discount_curve_id.as_str())?;
        let forward = market.get_forward(fra.forward_curve_id.as_str())?;

        let replayable = discount.rate_calibration().is_some()
            && forward
                .rate_calibration()
                .is_some_and(|recipe| !uses_basis_quotes(recipe));
        if !replayable {
            return generic_fallback(context);
        }

        let bump_bp = sens_config::resolve(context)?.rate_bump_bp;

        let discount_id = &fra.discount_curve_id;
        let forward_id = &fra.forward_curve_id;

        let bumped_up = context.bump_rate_market_cached(discount_id, forward_id, bump_bp)?;
        let pv_up = context.reprice_raw(bumped_up.as_ref(), context.as_of)?;
        let bumped_down = context.bump_rate_market_cached(discount_id, forward_id, -bump_bp)?;
        let pv_down = context.reprice_raw(bumped_down.as_ref(), context.as_of)?;
        Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
    }
}

fn generic_fallback(context: &mut MetricContext) -> Result<f64> {
    finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(
        finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined(),
    )
    .calculate(context)
}

fn uses_basis_quotes(calibration: &RateCalibrationRecipe) -> bool {
    calibration
        .quotes
        .iter()
        .any(|quote| matches!(quote, RateCalibrationQuote::Basis { .. }))
}
