//! Agency CMO risk metrics.
//!
//! CMO-specific metrics including tranche-level static Z-spread, duration,
//! and scenario analysis. A true Monte Carlo OAS (stochastic rates +
//! rate-dependent prepayment, as in `mbs_passthrough::metrics::mc_oas`) is
//! deferred; the spread metric exposed here is an honest static Z-spread.

pub(crate) mod zspread;

pub(crate) use zspread::calculate_tranche_zspread;

use crate::instruments::fixed_income::cmo::AgencyCmo;
use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};

/// Calculator for the tranche static Z-spread.
pub(crate) struct ZSpreadCalculator;

impl MetricCalculator for ZSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let cmo: &AgencyCmo = context.instrument_as()?;
        let market_price = cmo
            .pricing_overrides
            .market_quotes
            .quoted_clean_price
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "cmo.pricing_overrides.quoted_clean_price".to_string(),
                })
            })?;
        // Non-convergence propagates as an error (no silent zero spread).
        let result =
            calculate_tranche_zspread(cmo, market_price, context.curves.as_ref(), context.as_of)?;
        Ok(result.zspread)
    }
}

/// Register agency CMO metrics with the registry.
pub(crate) fn register_cmo_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::AgencyCmo,
        metrics: [
            (ZSpread, ZSpreadCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::AgencyCmo,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::AgencyCmo,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
