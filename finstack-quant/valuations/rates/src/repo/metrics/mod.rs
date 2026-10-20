//! Repo metrics module.
//!
//! Provides metric calculators specific to `Repo`.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_repo_metrics`.
//!
//! Exposed metrics:
//! - Collateral value and required collateral
//! - Collateral coverage ratio
//! - Repo interest amount
//! - DV01 (discount curve parallel bp)
//! - Funding risk (sensitivity to repo rate)
//! - Effective rate (with special collateral adj.)
//! - Time to maturity (years)
//! - Implied collateral return
//! - Accrued interest (currency amount)

mod accrued_interest;
mod implied_collateral_return;

use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use rust_decimal::Decimal;

/// Register all Repo metrics with the registry.
pub(crate) fn register_repo_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    // Standard metrics using macro
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Repo,
        metrics: [
            (Accrued, accrued_interest::AccruedInterestCalculator),
            (CollateralValue, CollateralValueCalculator),
            (RequiredCollateral, RequiredCollateralCalculator),
            (CollateralCoverage, CollateralCoverageCalculator),
            (RepoInterest, RepoInterestCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (FundingRisk, FundingRiskCalculator),
            (EffectiveRate, EffectiveRateCalculator),
            (TimeToMaturity, TimeToMaturityCalculator),
            (ImpliedCollateralReturn, implied_collateral_return::ImpliedCollateralReturnCalculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            // Repo-specific risk metrics (custom metrics)
        ]
    };
    Ok(())
}

#[cfg(test)]
mod tests;

/// Calculate collateral coverage ratio (market value / required value).
///
/// Computes `market_value / required_value` using pre-computed metrics.
pub(crate) struct CollateralCoverageCalculator;

impl MetricCalculator for CollateralCoverageCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::CollateralValue, MetricId::RequiredCollateral]
    }

    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Dependencies are declared above, so missing entries indicate a
        // registry/dependency-resolution bug — propagate instead of silently
        // substituting defaults that would fabricate a coverage ratio.
        let collateral_value = context
            .computed
            .get(&MetricId::CollateralValue)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "CollateralCoverage requires CollateralValue to be computed first".to_string(),
                )
            })?;
        let required_value = context
            .computed
            .get(&MetricId::RequiredCollateral)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "CollateralCoverage requires RequiredCollateral to be computed first"
                        .to_string(),
                )
            })?;

        if required_value == 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "CollateralCoverage is undefined: required collateral is zero".to_string(),
            ));
        }

        Ok(collateral_value / required_value)
    }
}

/// Calculate the market value of collateral.
///
/// Computes the current market value of the collateral backing the repo
/// using prices from `MarketContext`.
pub(crate) struct CollateralValueCalculator;

impl MetricCalculator for CollateralValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo = context.instrument_as::<crate::repo::Repo>()?;
        let collateral_value = repo.collateral.market_value(&context.curves)?;
        Ok(collateral_value.amount())
    }
}

/// Calculate effective repo rate considering special collateral.
///
/// Returns the effective rate accounting for any special collateral
/// adjustments configured on the instrument.
pub(crate) struct EffectiveRateCalculator;

impl MetricCalculator for EffectiveRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo = context.instrument_as::<crate::repo::Repo>()?;
        repo.effective_rate()
    }
}

/// Calculate funding risk (repo rate sensitivity).
///
/// Sensitivity of the repo PV to a +1bp change in the instrument's repo rate
/// parameter, reported as `ΔPV = PV(rate + 1bp) − PV(rate)` — the workspace
/// dPV/dy convention shared with Dv01. From the cash lender's perspective a
/// higher repo rate raises the repurchase payment, so FundingRisk is
/// typically **positive** for a lender (and negative for a reverse repo).
pub(crate) struct FundingRiskCalculator;

impl MetricCalculator for FundingRiskCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo = context.instrument_as::<crate::repo::Repo>()?;
        let base_pv = repo.value(&context.curves, context.as_of)?.amount();
        let mut repo_bumped = repo.clone();
        repo_bumped.repo_rate +=
            Decimal::try_from(finstack_quant_core::math::units::ONE_BASIS_POINT)
                .map_err(|_| finstack_quant_core::InputError::ConversionOverflow)?;
        let bumped_pv = repo_bumped.value(&context.curves, context.as_of)?.amount();
        // dPV/dy convention: ΔPV for a +1bp bump (NOT the negated change).
        Ok(bumped_pv - base_pv)
    }
}

/// Calculate repo interest amount.
///
/// Computes the accrued interest between start and maturity using the
/// instrument's day count and effective rate.
pub(crate) struct RepoInterestCalculator;

impl MetricCalculator for RepoInterestCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo = context.instrument_as::<crate::repo::Repo>()?;
        let interest = repo.interest_amount()?;
        Ok(interest.amount())
    }
}

/// Calculate required collateral value including haircut.
///
/// Computes the required collateral value including haircut for a repo.
pub(crate) struct RequiredCollateralCalculator;

impl MetricCalculator for RequiredCollateralCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo = context.instrument_as::<crate::repo::Repo>()?;
        let required_value = repo.required_collateral_value()?;
        Ok(required_value.amount())
    }
}

/// Calculate time to maturity in years.
///
/// Uses business-day adjusted maturity for consistency with PV calculations.
///
/// Computes the year fraction from `as_of` to repo maturity using the
/// instrument's day-count convention.
///
/// # Market Standard
///
/// Uses business-day adjusted maturity date for consistency with PV and
/// other metric calculations.
pub(crate) struct TimeToMaturityCalculator;

impl MetricCalculator for TimeToMaturityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo = context.instrument_as::<crate::repo::Repo>()?;
        // Use adjusted maturity for consistency with PV and interest calculations
        let (_, adj_maturity) = repo.adjusted_dates()?;
        let ttm = repo.day_count.year_fraction(
            context.as_of,
            adj_maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        Ok(ttm)
    }
}
