//! CDS Option metrics module.
//!
//! Provides metric calculators specific to `CDSOption`, split into focused
//! files. The calculators compose with the shared metrics framework and are
//! registered via `register_cds_option_metrics`.
//!
//! Exposed metrics:
//! - Delta, Gamma, Vega, Theta
//! - CS01 (quoted CDS spread sensitivity)
//! - DV01 (swap-curve quote sensitivity; canonical IR sensitivity)
//! - ParSpread (Bloomberg CDSO ATM forward spread in bp)
//! - Implied Volatility (placeholder)

pub(crate) mod delta;
mod dv01;
pub(crate) mod gamma;
mod implied_vol;
mod par_spread;
mod recovery01;
mod spread_dv01;
mod theta;
// risk_bucketed_dv01 - now using generic implementation
pub(crate) mod vega;

use crate::metrics::MetricRegistry;

/// Per-deal CS01 conventions for [`CDSOption`].
///
/// Drives the generic credit CS01 calculator
/// ([`crate::metrics::sensitivities::cs01::CreditParallelCs01`]). The CDS
/// option does not carry its own doc clause / valuation convention; CS01 is a
/// quote-spread risk measured against the *synthetic underlying CDS*, so the
/// conventions are read off `synthetic_underlying_cds`.
///
/// `cs01_precheck` reproduces the two legacy guards: a `0.0` short-circuit
/// once the option has expired, and a hard calibration error when the hazard
/// curve carries no CDS quote / par-spread points.
///
/// CDS options price through their `value` path (Bloomberg CDSO), so
/// `cs01_use_pricer_registry` returns `false` to keep scenario overrides and
/// avoid the registry's raw path skipping them.
impl crate::metrics::sensitivities::cs01::CdsCs01Conventions
    for crate::instruments::credit_derivatives::cds_option::CDSOption
{
    fn cs01_bootstrap_convention(
        &self,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<(
        crate::market::conventions::ids::CdsDocClause,
        crate::instruments::credit_derivatives::cds::CdsValuationConvention,
    )> {
        // CS01 is a quote-spread risk measured against the synthetic
        // underlying CDS; its doc clause and valuation convention drive the
        // hazard re-bootstrap.
        let synthetic =
            crate::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds(
                self, as_of,
            )?;
        Ok((
            crate::instruments::credit_derivatives::cds::metrics::market_doc_clause(&synthetic),
            synthetic.valuation_convention,
        ))
    }

    fn cs01_precheck(
        &self,
        context: &crate::metrics::MetricContext,
        hazard_id: &finstack_core::types::CurveId,
    ) -> finstack_core::Result<Option<f64>> {
        let as_of = context.as_of;
        if as_of >= self.expiry {
            tracing::debug!(
                instrument_id = %self.id,
                as_of = %as_of,
                expiry = %self.expiry,
                "CDS Option CS01: Instrument already expired, returning 0.0"
            );
            return Ok(Some(0.0));
        }

        let hazard = context.curves.get_hazard(hazard_id.as_str())?;
        if hazard.par_spread_points().next().is_none() {
            return Err(finstack_core::Error::Calibration {
                message: format!(
                    "CDS option '{}' CS01 requires CDS quote/par-spread points on hazard curve '{}'",
                    self.id,
                    hazard_id.as_str()
                ),
                category: "cs01_quote_bump".to_string(),
            });
        }
        Ok(None)
    }

    fn cs01_use_pricer_registry(&self) -> bool {
        false
    }
}

/// Register all CDS Option metrics with the registry
pub(crate) fn register_cds_option_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // Recovery01 (custom metric - recovery rate sensitivity)
    registry.register_metric(
        MetricId::Recovery01,
        Arc::new(recovery01::Recovery01Calculator),
        &[InstrumentType::CDSOption],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CDSOption,
        metrics: [
            (Delta, delta::DeltaCalculator),
            (Gamma, gamma::GammaCalculator),
            (Vega, vega::VegaCalculator),
            (Cs01, crate::metrics::sensitivities::cs01::CreditParallelCs01::<
                crate::instruments::credit_derivatives::cds_option::CDSOption,
            >::default()),
            (BucketedCs01, crate::metrics::sensitivities::cs01::CreditBucketedCs01::<
                crate::instruments::credit_derivatives::cds_option::CDSOption,
            >::default()),
            (SpreadDv01, spread_dv01::UnderlyingSpreadDv01Calculator),
            (Dv01, dv01::CdsOptionDv01Calculator),
            (Theta, theta::ThetaCalculator),
            (ParSpread, par_spread::ParSpreadCalculator),
            (ImpliedVol, implied_vol::ImpliedVolCalculator),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::CDSOption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
