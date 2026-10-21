//! Agency CMO risk metrics.
//!
//! CMO-specific metrics including tranche-level static Z-spread, duration,
//! and scenario analysis. A true Monte Carlo OAS (stochastic rates +
//! rate-dependent prepayment, as in `mbs_passthrough::metrics::mc_oas`) is
//! deferred; the spread metric exposed here is an honest static Z-spread.

pub(crate) mod zspread;

pub(crate) use zspread::calculate_tranche_zspread;

use crate::cmo::AgencyCmo;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricRegistry};

/// Calculator for the tranche static Z-spread.
pub(crate) struct ZSpreadCalculator;

impl MetricCalculator for ZSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let cmo: &AgencyCmo = context.instrument_as()?;
        let market_price = cmo
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "cmo.instrument_pricing_overrides.market_quotes.quoted_clean_price_pct"
                        .to_string(),
                })
            })?;
        // Non-convergence propagates as an error (no silent zero spread).
        calculate_tranche_zspread(cmo, market_price, context.curves.as_ref(), context.as_of)
    }
}

/// Register agency CMO metrics with the registry.
pub(crate) fn register_cmo_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::AgencyCmo,
        metrics: [
            (ZSpread, ZSpreadCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
