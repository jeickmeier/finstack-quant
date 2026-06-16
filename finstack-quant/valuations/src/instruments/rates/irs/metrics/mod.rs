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
//! DV01 metrics use the unified calculator framework from `crate::metrics`.
//!
//! # Examples
//!
//! ```ignore
//! use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
//! use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
//! use finstack_quant_valuations::metrics::MetricId;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::dates::Date;
//! # use time::Month;
//!
//! # fn example() -> finstack_quant_core::Result<()> {
//! let irs = InterestRateSwap::example_standard()?;
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

pub(crate) mod annuity;
pub(crate) mod dv01;
pub(crate) mod ir_convexity;
pub(crate) mod par_rate;
pub(crate) mod pv_fixed;
pub(crate) mod pv_float;
pub(crate) mod schedule_diagnostics;
// risk_bucketed_dv01, dv01, and theta now using generic implementations

/// Registers all IRS metrics into a provided registry.
pub(crate) fn register_irs_metrics(registry: &mut crate::metrics::MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::IRS,
        metrics: [
            (Annuity, annuity::AnnuityCalculator),
            (ParRate, par_rate::ParRateCalculator),

            // Theta is now registered universally in metrics::standard_registry()

            (Dv01, dv01::IrsDv01Calculator),

            // PV01 per-curve: bump each rate curve individually, store as pv01::{curve}
            (Pv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::InterestRateSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_per_curve()
                .with_series_id(crate::metrics::MetricId::Pv01))),

            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::InterestRateSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),

            (PvFixed, pv_fixed::FixedLegPvCalculator),
            (PvFloat, pv_float::FloatLegPvCalculator),
            (IrConvexity, ir_convexity::IrConvexityCalculator),
            (IrCrossGamma, ir_convexity::CrossGammaCalculator),
            (FixedLegPaymentCount, schedule_diagnostics::FixedLegPaymentCountCalculator),
            (FloatingLegPaymentCount, schedule_diagnostics::FloatingLegPaymentCountCalculator),
            (FixedFirstPaymentDate, schedule_diagnostics::FixedFirstPaymentDateCalculator),
            (FixedLastPaymentDate, schedule_diagnostics::FixedLastPaymentDateCalculator),
            (FloatingFirstPaymentDate, schedule_diagnostics::FloatingFirstPaymentDateCalculator),
            (FloatingLastPaymentDate, schedule_diagnostics::FloatingLastPaymentDateCalculator),
            (FixedFirstAccrualFactor, schedule_diagnostics::FixedFirstAccrualFactorCalculator),
            (FloatingFirstAccrualFactor, schedule_diagnostics::FloatingFirstAccrualFactorCalculator),
        ]
    }
}
