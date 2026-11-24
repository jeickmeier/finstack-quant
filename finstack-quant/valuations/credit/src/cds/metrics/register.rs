//! Metric registration and calculators.

use crate::cds::CreditDefaultSwap;
use crate::cds::pricing::{
    par_spread, pv_premium_leg, pv_protection_leg, risky_annuity, risky_pv01,
};
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Regional documentation family a hazard replay is asserted against: the
/// family of an explicit meta clause (`isda_eu`, `isda_au`, ...), else the
/// CDS's own `convention`. Exact restructuring clauses (`cr14`, `xr14`, ...)
/// carry no family of their own, so they defer to the convention.
pub(crate) fn market_doc_clause(
    cds: &crate::cds::CreditDefaultSwap,
) -> finstack_quant_valuations_core::market::conventions::ids::CdsDocClause {
    cds.doc_clause
        .and_then(finstack_quant_valuations_core::market::conventions::CdsConvention::family_from_doc_clause)
        .unwrap_or(cds.convention)
        .as_doc_clause()
}

pub(crate) fn deal_quote_override(
    cds: &crate::cds::CreditDefaultSwap,
) -> Option<finstack_quant_valuations_core::recalibration::DealCdsQuoteOverride> {
    let quote_bp = cds
        .instrument_pricing_overrides
        .market_quotes
        .cds_quote_bp?;
    if !cds.uses_clean_price() {
        return None;
    }
    Some(
        finstack_quant_valuations_core::recalibration::DealCdsQuoteOverride {
            contract_end: cds.premium_leg.end,
            spread_bp: quote_bp,
        },
    )
}

/// Per-deal CS01 conventions for `CreditDefaultSwap`.
///
/// Drives the generic credit CS01 calculators
/// ([`crate::cs01::CreditParallelCs01`] /
/// [`CreditBucketedCs01`](crate::cs01::CreditBucketedCs01))
/// so the hazard curve is re-bootstrapped under this CDS's doc clause and
/// valuation convention. The optional curve override reproduces the legacy
/// deal-quote hazard substitution.
impl crate::cs01::CdsCs01Conventions for crate::cds::CreditDefaultSwap {
    fn cs01_bootstrap_convention(
        &self,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(
        finstack_quant_valuations_core::market::conventions::ids::CdsDocClause,
        finstack_quant_valuations_core::market::conventions::CdsValuationConvention,
    )> {
        Ok((market_doc_clause(self), self.valuation_convention))
    }

    fn cs01_deal_quote_override(
        &self,
    ) -> Option<finstack_quant_valuations_core::recalibration::DealCdsQuoteOverride> {
        deal_quote_override(self)
    }
}

/// Register all CDS metrics with the registry
pub(crate) fn register_cds_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    // Standard metrics using macro
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Cds,
        metrics: [
            (ParSpread, ParSpreadCalculator),
            (RiskyAnnuity, RiskyAnnuityCalculator),
            (Cs01, crate::cs01::CreditParallelCs01::<
                crate::cds::CreditDefaultSwap,
            >::default(), NativeMoney),
            (BucketedCs01, crate::cs01::CreditBucketedCs01::<
                crate::cds::CreditDefaultSwap,
            >::default(), NativeMoney),
            (CsGamma, super::cs_gamma::CsGammaCalculator),
            (ProtectionLegPv, ProtectionLegPvCalculator),
            (PremiumLegPv, PremiumLegPvCalculator),
            (ExpectedLoss, ExpectedLossCalculator),
            (JumpToDefault, super::jump_to_default::JumpToDefaultCalculator),
            (DefaultExposure, super::jump_to_default::DefaultExposureCalculator),
            (Dv01, super::dv01::CdsDv01Calculator(false)),
            (BucketedDv01, super::dv01::CdsDv01Calculator(true)),
            (RiskyPv01, RiskyPv01Calculator),
            // Recovery01 (custom metric - recovery rate sensitivity)
            (Recovery01, super::recovery01::Recovery01Calculator),
        ],
        custom: [
            // JumpToDefaultLgdOnly (custom metric - LGD only, excludes accrued)
            ("jump_to_default_lgd_only", super::jump_to_default::JumpToDefaultLgdOnlyCalculator),
        ]
    }
    Ok(())
}

/// Expected Loss calculator for single-name CDS.
///
/// Calculates the expected loss at maturity using the market-standard formula:
/// EL = Notional × PD × LGD
///
/// Where:
/// - PD (Probability of Default) is conditional on survival to the valuation date
/// - LGD (Loss Given Default) = 1 - Recovery Rate
pub(crate) struct ExpectedLossCalculator;

impl MetricCalculator for ExpectedLossCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let as_of = context.as_of;

        let hazard = context
            .curves
            .get_hazard(cds.protection_leg.credit_curve_id.as_str())?;

        // If already at/after maturity, no forward expected loss (under deterministic recovery).
        if as_of >= cds.premium_leg.end {
            return Ok(0.0);
        }

        let default_prob = hazard.conditional_default_prob(as_of, cds.premium_leg.end)?;

        // Loss given default
        let lgd = 1.0 - cds.protection_leg.recovery_rate;

        // Expected loss in currency units
        let expected_loss = cds.notional.amount() * default_prob * lgd;

        Ok(expected_loss)
    }
}

/// Par spread calculator for CDS
///
/// Computes the fixed spread in basis points that sets the CDS NPV to zero,
/// using the pricing engine's par-spread calculation. This is independent
/// of the instrument's current quoted spread.
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium_leg.discount_curve_id)?;
        let surv = context
            .curves
            .get_hazard(&cds.protection_leg.credit_curve_id)?;
        par_spread(cds, disc.as_ref(), surv.as_ref(), context.as_of)
    }
}

/// Premium leg PV calculator
///
/// Computes present value of the premium leg using discount and hazard curves
/// via the engine. The value is returned in currency units.
pub(crate) struct PremiumLegPvCalculator;

impl MetricCalculator for PremiumLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium_leg.discount_curve_id)?;
        let surv = context
            .curves
            .get_hazard(&cds.protection_leg.credit_curve_id)?;
        let pv = pv_premium_leg(cds, disc.as_ref(), surv.as_ref(), context.as_of)?;
        Ok(pv.amount())
    }
}

/// Protection leg PV calculator
///
/// Computes present value of the protection leg using the configured curves
/// and the engine's implementation. The measure is returned in currency units.
pub(crate) struct ProtectionLegPvCalculator;

impl MetricCalculator for ProtectionLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium_leg.discount_curve_id)?;
        let surv = context
            .curves
            .get_hazard(&cds.protection_leg.credit_curve_id)?;
        let pv = pv_protection_leg(cds, disc.as_ref(), surv.as_ref(), context.as_of)?;
        Ok(pv.amount())
    }
}

/// Risky annuity calculator for CDS
///
/// Returns the canonical risky annuity `Σ DF(t) × SP(t) × YearFrac` (sum over
/// coupon periods), independent of the par-spread denominator policy. The par
/// spread metric handles its own denominator choice via the CDS valuation
/// convention.
pub(crate) struct RiskyAnnuityCalculator;

impl MetricCalculator for RiskyAnnuityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium_leg.discount_curve_id)?;
        let surv = context
            .curves
            .get_hazard(&cds.protection_leg.credit_curve_id)?;
        risky_annuity(cds, disc.as_ref(), surv.as_ref(), context.as_of)
    }
}

/// Risky PV01 calculator for CDS
///
/// Returns Risky PV01 = `annuity × Notional / 10000` on the instrument's
/// valuation convention: the clean annuity (with accrual-on-default, less
/// accrued premium) for the Bloomberg CDSW clean conventions, the coupon-only
/// risky annuity otherwise.
pub(crate) struct RiskyPv01Calculator;

impl MetricCalculator for RiskyPv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium_leg.discount_curve_id)?;
        let surv = context
            .curves
            .get_hazard(&cds.protection_leg.credit_curve_id)?;
        risky_pv01(cds, disc.as_ref(), surv.as_ref(), context.as_of)
    }
}
