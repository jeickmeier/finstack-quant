//! Metric registration and calculators.

use crate::irs::InterestRateSwap;
use finstack_quant_valuations_core::constants::ANNUITY_EPSILON;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricId;

/// Registers all IRS metrics into a provided registry.
pub(crate) fn register_irs_metrics(
    registry: &mut finstack_quant_valuations_core::metrics::MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Irs,
        metrics: [
            (Annuity, super::annuity::AnnuityCalculator),
            (ParRate, ParRateCalculator),

            (Dv01, super::dv01::IrsDv01Calculator),

            // PV01 per-curve: bump each rate curve individually, store as pv01::{curve}
            (Pv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_per_curve()
                .with_series_id(finstack_quant_valuations_core::metrics::MetricId::Pv01))),

            (BucketedDv01, crate::rate_bucketed_dv01::RateDv01(true)),

            (PvFixed, FixedLegPvCalculator, NativeMoney),
            (PvFloat, FloatLegPvCalculator, NativeMoney),
            (IrConvexity, super::ir_convexity::IrConvexityCalculator),
            (IrCrossGamma, super::ir_convexity::CrossGammaCalculator),
        ]
    }
    Ok(())
}

/// Par fixed rate from actual floating-leg PV divided by fixed-leg unit annuity.
/// The same contractual coupons, historical fixings, dates and payment lags
/// drive par quotation and pricing, including mismatched leg maturities.
pub(crate) struct ParRateCalculator;

impl MetricCalculator for ParRateCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Annuity]
    }
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        // Reuses the pricer's own floating-leg PV for consistency with pricing.
        let irs: &InterestRateSwap = context.instrument_as()?;
        let as_of = context.as_of;
        let annuity = context
            .computed
            .get(&MetricId::Annuity)
            .copied()
            .unwrap_or(0.0);

        if annuity.abs() < ANNUITY_EPSILON {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Par rate calculation failed: annuity ({:.2e}) is below numerical stability \
             threshold ({:.2e}).",
                annuity, ANNUITY_EPSILON
            )));
        }

        let pv_float = irs.pv_float_leg(&context.curves, as_of)?;

        // Par rate = float_pv / (notional * annuity)
        Ok(pv_float / (irs.notional.amount() * annuity))
    }
}

/// Present value calculator for the fixed leg of an interest rate swap.
///
/// Discounts all future fixed coupon payments using the discount curve
/// specified in the swap's fixed leg specification.
///
/// Computes the present value of all future fixed coupon payments by
/// discounting them using the discount curve specified in the fixed leg.
///
/// # Calculation
///
/// ```text
/// PV_fixed = Σ Notional × Fixed_Rate × α_i × DF(T_i)
/// ```
///
/// where:
/// - `α_i` = accrual factor for period i (from day count convention)
/// - `DF(T_i)` = discount factor to payment date i
/// - Sum includes only future cashflows (T_i > as_of)
///
/// # Use Cases
///
/// - Computing individual leg PVs for risk analysis
/// - Decomposing swap value into fixed vs floating components
/// - Calibration and curve fitting workflows
pub(crate) struct FixedLegPvCalculator;

impl MetricCalculator for FixedLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let irs: &InterestRateSwap = context.instrument_as()?;
        let as_of = context.as_of;
        let disc = context
            .curves
            .get_discount(&irs.fixed_leg.discount_curve_id)?;

        let pv = irs.pv_fixed_leg(&disc, as_of)?;
        Ok(pv)
    }
}

/// Present value calculator for the floating leg of an interest rate swap.
///
/// Projects forward rates and discounts floating coupon payments through the
/// same signed schedule the cashflow surface emits (see
/// `pricer::compute_float_leg_pv`). Seasoned swaps read past resets from
/// historical fixings.
///
/// # Calculation
///
/// ## Term-Rate Swaps (LIBOR-style)
///
/// ```text
/// PV_float = Σ Notional × (Forward_i + Spread) × α_i × DF(T_i)
/// ```
///
/// where:
/// - `Forward_i` = forward rate for period i from the forward curve
/// - `Spread` = quoted spread in basis points
/// - `α_i` = accrual factor for period i
/// - `DF(T_i)` = discount factor to payment date i
///
/// ## OIS/Overnight Swaps (RFR-style)
///
/// Compounded-in-arrears legs project each coupon with the shared overnight
/// compounding engine, splicing realized fixings into started periods and
/// discounting with the fixed-leg discount curve.
///
/// # References
///
/// - **ISDA 2021 Definitions**: Current term-rate and compounded-RFR framework `docs/REFERENCES.md#isda-2021-definitions`
/// - **ISDA 2006 Definitions**: Legacy term-rate transactions `docs/REFERENCES.md#isda-2006-definitions`
pub(crate) struct FloatLegPvCalculator;

impl MetricCalculator for FloatLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let irs: &InterestRateSwap = context.instrument_as()?;
        crate::irs::pricer::compute_float_leg_pv(irs, &context.curves, context.as_of)
    }
}
