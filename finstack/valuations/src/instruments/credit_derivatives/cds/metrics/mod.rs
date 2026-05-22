//! CDS metrics module.
//!
//! Provides metric calculators specific to `CreditDefaultSwap`, split into
//! focused files. The calculators compose with the shared metrics framework
//! and are registered via `register_cds_metrics`.
//!
//! Exposed metrics:
//! - Par spread (bps)
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
// risk_bucketed_dv01 and theta now using generic implementations
mod risky_annuity;
mod risky_pv01;

use crate::metrics::MetricRegistry;
use finstack_core::dates::DayCountContext;
use finstack_core::market_data::term_structures::{HazardCurve, ParInterp};

pub(crate) fn market_doc_clause(
    cds: &crate::instruments::credit_derivatives::cds::CreditDefaultSwap,
) -> crate::market::conventions::ids::CdsDocClause {
    use crate::instruments::credit_derivatives::cds::CdsDocClause as InstrumentClause;
    use crate::market::conventions::ids::CdsDocClause as MarketClause;

    match cds.doc_clause_effective() {
        InstrumentClause::Cr14 | InstrumentClause::Mr14 | InstrumentClause::Xr14 => {
            MarketClause::IsdaNa
        }
        InstrumentClause::Mm14 => MarketClause::IsdaEu,
        InstrumentClause::IsdaNa => MarketClause::IsdaNa,
        InstrumentClause::IsdaEu => MarketClause::IsdaEu,
        InstrumentClause::IsdaAs => MarketClause::IsdaAs,
        InstrumentClause::IsdaAu | InstrumentClause::IsdaNz => MarketClause::IsdaAs,
        InstrumentClause::Custom => MarketClause::Custom,
    }
}

pub(crate) fn hazard_with_deal_quote(
    cds: &crate::instruments::credit_derivatives::cds::CreditDefaultSwap,
    hazard: &HazardCurve,
) -> finstack_core::Result<Option<HazardCurve>> {
    let Some(quote_bp) = cds.pricing_overrides.market_quotes.cds_quote_bp else {
        return Ok(None);
    };
    if !cds.uses_clean_price() {
        return Ok(None);
    }

    let raw_quote_tenor = hazard.day_count().year_fraction(
        hazard.base_date(),
        cds.premium.end,
        DayCountContext::default(),
    )?;
    let quote_tenor = raw_quote_tenor;

    Ok(Some(
        HazardCurve::builder(hazard.id().clone())
            .base_date(hazard.base_date())
            .recovery_rate(hazard.recovery_rate())
            .day_count(hazard.day_count())
            .knots(hazard.knot_points())
            .par_spreads([(quote_tenor, quote_bp)])
            .par_interp(ParInterp::Linear)
            .issuer_opt(hazard.issuer().map(str::to_owned))
            .seniority_opt(hazard.seniority)
            .currency_opt(hazard.currency())
            .build()?,
    ))
}

/// Per-deal CS01 conventions for [`CreditDefaultSwap`].
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
        _as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<(
        crate::market::conventions::ids::CdsDocClause,
        crate::instruments::credit_derivatives::cds::CdsValuationConvention,
    )> {
        Ok((market_doc_clause(self), self.valuation_convention))
    }

    fn cs01_curve_override(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        hazard_id: &finstack_core::types::CurveId,
        _as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<finstack_core::market_data::context::MarketContext>> {
        let hazard = curves.get_hazard(hazard_id.as_str())?;
        Ok(hazard_with_deal_quote(self, hazard.as_ref())?
            .map(|quote_hazard| curves.clone().insert(quote_hazard)))
    }
}

/// Register all CDS metrics with the registry
pub(crate) fn register_cds_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::RiskyPv01,
        Arc::new(risky_pv01::RiskyPv01Calculator),
        &[InstrumentType::CDS],
    );

    // Recovery01 (custom metric - recovery rate sensitivity)
    registry.register_metric(
        MetricId::Recovery01,
        Arc::new(recovery01::Recovery01Calculator),
        &[InstrumentType::CDS],
    );

    // JumpToDefaultLgdOnly (custom metric - LGD only, excludes accrued)
    registry.register_metric(
        MetricId::custom("jump_to_default_lgd_only"),
        Arc::new(jump_to_default::JumpToDefaultLgdOnlyCalculator),
        &[InstrumentType::CDS],
    );

    // Standard metrics using macro
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CDS,
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
}
