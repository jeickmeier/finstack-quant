//! CDS Tranche metrics module.
//!
//! Provides metric calculators specific to `CdsTranche`, split into focused
//! files. The calculators compose with the shared metrics framework and are
//! registered via `register_cds_tranche_metrics`.
//!
//! Exposed metrics:
//! - Spread DV01 (per 1bp change in running coupon)
//! - Expected loss (currency units)
//! - Jump-to-default (currency units)
//! - CS01 (parallel par-spread rebootstrap)
//! - Correlation01 (per 1% correlation change)
//! - Recovery01 (per 1% recovery change)

mod cs01;
mod recovery01;

use crate::cds_tranche::{CdsTranche, CdsTranchePricer};
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all CDS Tranche metrics with the registry
pub(crate) fn register_cds_tranche_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CdsTranche,
        metrics: [
            (SpreadDv01, SpreadDv01Calculator),
            (Correlation01, Correlation01Calculator),
            (Recovery01, recovery01::Recovery01Calculator),
            (Cs01, cs01::CdsTrancheCs01Calculator),
            (BucketedCs01, cs01::CdsTrancheBucketedCs01Calculator),
            (ParSpread, ParSpreadCalculator),
            (ExpectedLoss, ExpectedLossCalculator),
            (JumpToDefault, JumpToDefaultCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

/// Correlation01 calculator for CDS Tranche
///
/// Measures PV sensitivity to a shift in the base correlation curve, reported
/// **per 1% (0.01 absolute) correlation change** — the same per-1% unit as
/// `Recovery01`.
pub(crate) struct Correlation01Calculator;

impl MetricCalculator for Correlation01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CdsTranche = context.instrument_as()?;
        CdsTranchePricer::new().calculate_correlation_delta(tranche, &context.curves, context.as_of)
    }
}

/// Expected Loss calculator for CDS Tranche
///
/// Computes the total expected loss at maturity using the Gaussian Copula engine.
pub(crate) struct ExpectedLossCalculator;

impl MetricCalculator for ExpectedLossCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CdsTranche = context.instrument_as()?;
        // Propagate error when credit index data is missing rather than silently
        // returning zero, which would mask missing market data in risk reports.
        CdsTranchePricer::new().calculate_expected_loss(tranche, &context.curves)
    }
}

/// Jump-to-default calculator for CDS Tranche
///
/// Computes the instantaneous tranche loss if an average constituent defaults.
pub(crate) struct JumpToDefaultCalculator;

impl MetricCalculator for JumpToDefaultCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CdsTranche = context.instrument_as()?;
        // Propagate error when credit index data is missing rather than silently
        // returning zero, which would mask missing market data in risk reports.
        CdsTranchePricer::new().calculate_jump_to_default(tranche, &context.curves, context.as_of)
    }
}

/// Par spread calculator for CDS tranches.
///
/// Computes the running coupon in basis points that sets the tranche NPV to zero
/// using the Gaussian copula tranche pricer.
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CdsTranche = context.instrument_as()?;
        CdsTranchePricer::new().calculate_par_spread(tranche, &context.curves, context.as_of)
    }
}

/// Spread DV01 calculator for CDS Tranche
///
/// Measures the premium-leg PV change for a 1bp change in the running coupon.
pub(crate) struct SpreadDv01Calculator;

impl MetricCalculator for SpreadDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CdsTranche = context.instrument_as()?;
        // Propagate error when credit index data is missing rather than silently
        // returning zero, which would mask missing market data in risk reports.
        CdsTranchePricer::new().calculate_spread_dv01(tranche, &context.curves, context.as_of)
    }
}
