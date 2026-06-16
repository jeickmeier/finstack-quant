//! Repo metrics module.
//!
//! Provides metric calculators specific to `Repo`, split into focused files.
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
mod collateral_coverage;
mod collateral_price01;
mod collateral_value;
mod effective_rate;
mod funding_risk;
mod haircut01;
mod implied_collateral_return;
mod repo_interest;
mod required_collateral;
// risk_bucketed_dv01, dv01, and theta now using generic implementations
mod time_to_maturity;

use crate::metrics::MetricRegistry;

/// Register all Repo metrics with the registry.
pub(crate) fn register_repo_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::{MetricCalculator, MetricId};
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    let accrued_calc: Arc<dyn MetricCalculator> =
        Arc::new(accrued_interest::AccruedInterestCalculator);
    registry.register_metric(MetricId::Accrued, accrued_calc, &[InstrumentType::Repo]);

    // Repo-specific risk metrics (custom metrics)
    registry.register_metric(
        MetricId::CollateralHaircut01,
        Arc::new(haircut01::Haircut01Calculator),
        &[InstrumentType::Repo],
    );
    registry.register_metric(
        MetricId::CollateralPrice01,
        Arc::new(collateral_price01::CollateralPrice01Calculator),
        &[InstrumentType::Repo],
    );

    // Standard metrics using macro
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Repo,
        metrics: [
            (CollateralValue, collateral_value::CollateralValueCalculator),
            (RequiredCollateral, required_collateral::RequiredCollateralCalculator),
            (CollateralCoverage, collateral_coverage::CollateralCoverageCalculator),
            (RepoInterest, repo_interest::RepoInterestCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Repo,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (FundingRisk, funding_risk::FundingRiskCalculator),
            (EffectiveRate, effective_rate::EffectiveRateCalculator),
            (TimeToMaturity, time_to_maturity::TimeToMaturityCalculator),
            (ImpliedCollateralReturn, implied_collateral_return::ImpliedCollateralReturnCalculator),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Repo,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    };
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests;
