//! CDS metrics module.
//!
//! Provides metric calculators specific to `CreditDefaultSwap`, split into
//! focused files. The calculators compose with the shared metrics framework
//! and are registered via `register_cds_metrics`.
//!
//! Exposed metrics:
//! - Par spread (bp)
//! - Risky PV01
//! - Risky annuity
//! - CS01
//! - Protection leg PV
//! - Premium leg PV
//! - Expected loss
//! - Jump to default (includes accrued premium)
//! - Jump to default LGD only (excludes accrued premium)

mod cs_gamma;
mod dv01;
mod expected_loss;
mod jump_to_default;
mod par_spread;
mod pv_premium;
mod pv_protection;
mod recovery01;
mod risky_annuity;
mod risky_pv01;

use crate::metrics::MetricRegistry;

pub(crate) fn market_doc_clause(
    cds: &crate::instruments::credit_derivatives::cds::CreditDefaultSwap,
) -> crate::market::conventions::ids::CdsDocClause {
    use crate::instruments::credit_derivatives::cds::CdsDocClause as InstrumentClause;
    use crate::market::conventions::ids::CdsDocClause as MarketClause;

    match cds.doc_clause_effective() {
        InstrumentClause::Cr14 | InstrumentClause::Mr14 | InstrumentClause::Xr14 => {
            MarketClause::IsdaNa
        }
        InstrumentClause::Mm14 | InstrumentClause::IsdaEu => MarketClause::IsdaEu,
        InstrumentClause::IsdaNa => MarketClause::IsdaNa,
        InstrumentClause::IsdaAs | InstrumentClause::IsdaAu | InstrumentClause::IsdaNz => {
            MarketClause::IsdaAs
        }
        InstrumentClause::Custom => MarketClause::Custom,
    }
}

pub(crate) fn deal_quote_override(
    cds: &crate::instruments::credit_derivatives::cds::CreditDefaultSwap,
) -> Option<crate::recalibration::DealCdsQuoteOverride> {
    let quote_bp = cds
        .instrument_pricing_overrides
        .market_quotes
        .cds_quote_bp?;
    if !cds.uses_clean_price() {
        return None;
    }
    Some(crate::recalibration::DealCdsQuoteOverride {
        contract_end: cds.premium.end,
        spread_bp: quote_bp,
    })
}

/// Per-deal CS01 conventions for `CreditDefaultSwap`.
///
/// Drives the generic credit CS01 calculators
/// ([`crate::metrics::sensitivities::cs01::CreditParallelCs01`] /
/// [`CreditBucketedCs01`](crate::metrics::sensitivities::cs01::CreditBucketedCs01))
/// so the hazard curve is re-bootstrapped under this CDS's doc clause and
/// valuation convention. The optional curve override reproduces the legacy
/// deal-quote hazard substitution.
impl crate::metrics::sensitivities::cs01::CdsCs01Conventions
    for crate::instruments::credit_derivatives::cds::CreditDefaultSwap
{
    fn cs01_bootstrap_convention(
        &self,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(
        crate::market::conventions::ids::CdsDocClause,
        crate::instruments::credit_derivatives::cds::CdsValuationConvention,
    )> {
        Ok((market_doc_clause(self), self.valuation_convention))
    }

    fn cs01_deal_quote_override(&self) -> Option<crate::recalibration::DealCdsQuoteOverride> {
        deal_quote_override(self)
    }
}

/// Register all CDS metrics with the registry
pub(crate) fn register_cds_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::RiskyPv01,
        Arc::new(risky_pv01::RiskyPv01Calculator),
        &[InstrumentType::Cds],
    )?;

    // Recovery01 (custom metric - recovery rate sensitivity)
    registry.register_metric(
        MetricId::Recovery01,
        Arc::new(recovery01::Recovery01Calculator),
        &[InstrumentType::Cds],
    )?;

    // JumpToDefaultLgdOnly (custom metric - LGD only, excludes accrued)
    registry.register_metric(
        MetricId::custom("jump_to_default_lgd_only"),
        Arc::new(jump_to_default::JumpToDefaultLgdOnlyCalculator),
        &[InstrumentType::Cds],
    )?;

    // Standard metrics using macro
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Cds,
        metrics: [
            (ParSpread, par_spread::ParSpreadCalculator),
            (RiskyAnnuity, risky_annuity::RiskyAnnuityCalculator),
            (Cs01, crate::metrics::sensitivities::cs01::CreditParallelCs01::<
                crate::instruments::credit_derivatives::cds::CreditDefaultSwap,
            >::default()),
            (BucketedCs01, crate::metrics::sensitivities::cs01::CreditBucketedCs01::<
                crate::instruments::credit_derivatives::cds::CreditDefaultSwap,
            >::default()),
            (CsGamma, cs_gamma::CsGammaCalculator),
            (ProtectionLegPv, pv_protection::ProtectionLegPvCalculator),
            (PremiumLegPv, pv_premium::PremiumLegPvCalculator),
            (ExpectedLoss, expected_loss::ExpectedLossCalculator),
            (JumpToDefault, jump_to_default::JumpToDefaultCalculator),
            (DefaultExposure, jump_to_default::DefaultExposureCalculator),
            (Dv01, dv01::CdsDv01Calculator),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::CreditDefaultSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;

    #[test]
    fn cds_quote_override_returns_provider_contract() {
        let mut cds = CreditDefaultSwap::example();
        cds.instrument_pricing_overrides.market_quotes.cds_quote_bp = Some(321.0);
        let request = deal_quote_override(&cds).expect("deal quote override");
        assert_eq!(request.contract_end, cds.premium.end);
        assert_eq!(request.spread_bp, 321.0);
    }
}
