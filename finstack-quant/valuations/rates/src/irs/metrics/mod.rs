//! Interest rate swap metrics and risk calculations.
//!
//! Provides specialized metric calculators for IRS instruments including:
//! - **Par Rate**: The fixed rate that makes the swap's NPV zero
//! - **Annuity**: Sum of discounted accrual factors (fixed leg)
//! - **DV01**: Dollar value of 1bp parallel curve shift
//! - **Bucketed DV01**: Key rate sensitivities at curve vertices
//! - **IR Convexity**: Second-order parallel rate sensitivity (gamma)
//! - **IR Cross-Gamma**: Mixed second derivative (discount vs forward curve)
//! - **PV Fixed/Float**: Individual leg present values
//!
//! # Module Organization
//!
//! Each metric is implemented in its own file for clarity:
//! - [`par_rate`]: Par swap rate calculation
//! - [`annuity`]: Fixed leg annuity
//! - [`pv_fixed`]: Fixed leg PV
//! - [`pv_float`]: Floating leg PV
//!
//! DV01 metrics use the unified calculator framework from `finstack_quant_valuations_core::metrics`.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_valuations_rates::irs::InterestRateSwap;
//! use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
//! use finstack_quant_valuations_core::metrics::MetricId;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::dates::Date;
//! # use time::Month;
//!
//! # fn example() -> finstack_quant_core::Result<()> {
//! let irs = InterestRateSwap::example()?;
//! // Build market context with required curves
//! let mut context = MarketContext::new();
//! // ... add USD-OIS and USD-SOFR-3M curves ...
//!
//! let as_of = Date::from_calendar_date(2024, Month::January, 1)
//!     .map_err(|e| finstack_quant_core::Error::Validation(format!("{}", e)))?;
//!
//! // Compute par rate and DV01
//! let metrics = vec![MetricId::ParRate, MetricId::Dv01];
//! let result = irs.price_with_metrics(&context, as_of, &metrics, PricingOptions::default())?;
//!
//! let par_rate = result.measures.get(MetricId::ParRate.as_str()).copied();
//! let dv01 = result.measures.get(MetricId::Dv01.as_str()).copied();
//! # let _ = (par_rate, dv01);
//! # Ok(())
//! # }
//! ```

use crate::InterestRateSwap;
use finstack_quant_valuations_core::instruments::pricing::swap_legs::ANNUITY_EPSILON;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricId;

pub(crate) mod annuity;
pub(crate) mod dv01;
pub(crate) mod ir_convexity;

/// Registers all IRS metrics into a provided registry.
pub(crate) fn register_irs_metrics(
    registry: &mut finstack_quant_valuations_core::metrics::MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Irs,
        metrics: [
            (Annuity, annuity::AnnuityCalculator),
            (ParRate, ParRateCalculator),

            // Theta is now registered universally in crate::RATES.metrics()

            (Dv01, dv01::IrsDv01Calculator),

            // PV01 per-curve: bump each rate curve individually, store as pv01::{curve}
            (Pv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_per_curve()
                .with_series_id(finstack_quant_valuations_core::metrics::MetricId::Pv01))),

            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),

            (PvFixed, FixedLegPvCalculator),
            (PvFloat, FloatLegPvCalculator),
            (IrConvexity, ir_convexity::IrConvexityCalculator),
            (IrCrossGamma, ir_convexity::CrossGammaCalculator),
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
        par_rate_pv_based(context.instrument_as()?, context)
    }
}

/// Par rate calculation based on PV of the floating leg.
///
/// Refactored to reuse the pricer's own floating leg PV logic for perfect consistency.
fn par_rate_pv_based(
    irs: &InterestRateSwap,
    ctx: &MetricContext,
) -> finstack_quant_core::Result<f64> {
    let as_of = ctx.as_of;
    let annuity = ctx.computed.get(&MetricId::Annuity).copied().unwrap_or(0.0);

    if annuity.abs() < ANNUITY_EPSILON {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Par rate calculation failed: annuity ({:.2e}) is below numerical stability \
             threshold ({:.2e}).",
            annuity, ANNUITY_EPSILON
        )));
    }

    let pv_float = irs.pv_float_leg(&ctx.curves, as_of)?;

    // Par rate = float_pv / (notional * annuity)
    Ok(pv_float / (irs.notional.amount() * annuity))
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
/// Projects forward rates and discounts floating coupon payments. Automatically
/// detects OIS swaps (overnight compounding) and uses the appropriate pricing method.
/// For seasoned swaps with past resets, uses historical fixings if available.
///
/// Computes the present value of all future floating rate payments by
/// projecting forward rates from the forward curve and discounting.
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
/// For swaps with compounded-in-arrears floating legs, uses the
/// discount-only identity:
/// ```text
/// PV_float = Notional × (DF(start) - DF(end)) + Spread_Annuity
/// ```
///
/// This is exact when the forward curve matches the discount curve.
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
