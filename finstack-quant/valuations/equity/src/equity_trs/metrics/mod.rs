//! Risk metrics for Equity Total Return Swaps.
//!
//! This module provides equity TRS-specific risk metrics:
//! - **IndexDelta**: Lifecycle-aware spot sensitivity with reset fixings held constant
//! - **Dividend01**: Sensitivity to dividend yield changes
//! - **ParSpread**: Spread that makes NPV = 0
//! - **FinancingAnnuity**: Discounted notional-weighted financing accrual sum

mod annuity;
mod delta;
mod dividend_risk;
mod par_spread;

pub(crate) use annuity::FinancingAnnuityCalculator;
pub(crate) use delta::EquityDeltaCalculator;
pub(crate) use dividend_risk::Dividend01Calculator;
pub(crate) use par_spread::ParSpreadCalculator;

use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::pricer::InstrumentType;

/// Registers all equity TRS metrics with the metric registry.
///
/// # Arguments
/// * `registry` — Metric registry to add equity TRS metrics to
pub(crate) fn register_equity_trs_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::MetricId;
    use std::sync::Arc;

    let instruments = [InstrumentType::EquityTotalReturnSwap];

    // Equity TRS specific metrics
    registry.register_metric(
        MetricId::Dividend01,
        Arc::new(Dividend01Calculator),
        &instruments,
    )?;
    registry.register_metric(
        MetricId::IndexDelta,
        Arc::new(EquityDeltaCalculator),
        &instruments,
    )?;
    registry.register_metric(
        MetricId::ParSpread,
        Arc::new(ParSpreadCalculator),
        &instruments,
    )?;
    registry.register_metric(
        MetricId::FinancingAnnuity,
        Arc::new(FinancingAnnuityCalculator),
        &instruments,
    )?;

    // DV01 for financing leg sensitivity
    registry.register_metric(
        MetricId::Dv01,
        Arc::new(
            finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(
                finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined(),
            ),
        ),
        &instruments,
    )?;
    registry.register_metric(
        MetricId::BucketedDv01,
        Arc::new(
            finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(
                finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate(
                ),
            ),
        ),
        &instruments,
    )?;
    Ok(())
}
