//! Comprehensive tests for all repo metrics calculators.

use super::fixtures::*;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_valuations::instruments::rates::repo::{CollateralSpec, Repo};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::*;
use rust_decimal::Decimal;
use std::sync::Arc;

fn create_test_repo() -> Repo {
    let collateral = treasury_collateral();
    Repo::term(
        "METRICS_TEST",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed")
}

fn create_metric_context(repo: Repo, context: MarketContext, as_of: Date) -> MetricContext {
    let pv = repo.value(&context, as_of).unwrap();
    MetricContext::new(
        Arc::new(repo),
        Arc::new(context),
        as_of,
        pv,
        MetricContext::default_config(),
    )
}

#[test]
fn test_collateral_value_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo, context, date(2025, 1, 10));

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::CollateralValue], &mut mctx)
        .unwrap();

    let collateral_value = results.get(&MetricId::CollateralValue).unwrap();

    // 1M * 1.02 = 1,020,000
    assert_approx_eq(*collateral_value, 1_020_000.0, 1.0);
}

#[test]
fn test_required_collateral_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo, context, date(2025, 1, 10));

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::RequiredCollateral], &mut mctx)
        .unwrap();

    let required = results.get(&MetricId::RequiredCollateral).unwrap();

    // 1M / (1 - 0.02) = 1,020,408.16
    assert_approx_eq(*required, 1_020_408.16, 1.0);
}

#[test]
fn test_collateral_coverage_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo, context, date(2025, 1, 10));

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::CollateralCoverage], &mut mctx)
        .unwrap();

    let coverage = results.get(&MetricId::CollateralCoverage).unwrap();

    // 1,020,000 / 1,020,000 = 1.0
    assert_approx_eq(*coverage, 1.0, 0.01);
}

#[test]
fn test_collateral_coverage_overcollateralized() {
    // Use enough collateral to be over-collateralized
    // Collateral: 2M units at 105% = 2,100,000
    // Required: 1M * 1.02 = 1,020,000
    // Coverage: 2,100,000 / 1,020,000 = 2.06
    let collateral = CollateralSpec::new("SPECIAL_BOND", 2_000_000.0, "SPECIAL_BOND_PRICE");
    let repo = Repo::term(
        "OVERCOLLATERALIZED",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo, context, date(2025, 1, 10));

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::CollateralCoverage], &mut mctx)
        .unwrap();

    let coverage = results.get(&MetricId::CollateralCoverage).unwrap();

    // Should be > 1.0 (expecting ~2.06)
    assert!(
        *coverage > 1.5,
        "Coverage should exceed 1.5 for overcollateralized repo, got {}",
        coverage
    );
}

#[test]
fn test_repo_interest_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo.clone(), context, date(2025, 1, 10));

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::RepoInterest], &mut mctx)
        .unwrap();

    let metric_interest = results.get(&MetricId::RepoInterest).unwrap();
    let direct_interest = repo.interest_amount().unwrap().amount();

    assert_approx_eq(*metric_interest, direct_interest, 1e-9);
}

#[test]
fn test_effective_rate_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo.clone(), context, date(2025, 1, 10));

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::EffectiveRate], &mut mctx)
        .unwrap();

    let effective_rate = results.get(&MetricId::EffectiveRate).unwrap();

    assert_approx_eq(
        *effective_rate,
        repo.effective_rate().expect("effective rate"),
        1e-9,
    );
}

#[test]
fn test_time_to_maturity_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let as_of = date(2025, 1, 10);
    let mut mctx = create_metric_context(repo, context, as_of);

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::TimeToMaturity], &mut mctx)
        .unwrap();

    let ttm = results.get(&MetricId::TimeToMaturity).unwrap();

    // Should be positive and reasonable (roughly 0.25 years for 3 months)
    assert!(*ttm > 0.0);
    assert!(*ttm < 1.0);
}

#[test]
fn test_time_to_maturity_at_maturity() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let maturity = repo.maturity;
    let mut mctx = create_metric_context(repo, context, maturity);

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::TimeToMaturity], &mut mctx)
        .unwrap();

    let ttm = results.get(&MetricId::TimeToMaturity).unwrap();

    assert_approx_eq(*ttm, 0.0, 1e-6);
}

#[test]
fn test_implied_collateral_return_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo, context, date(2025, 1, 10));

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::ImpliedCollateralReturn], &mut mctx)
        .unwrap();

    let implied_return = results.get(&MetricId::ImpliedCollateralReturn).unwrap();

    // For adequately collateralized repo, should be near zero
    assert!(implied_return.abs() < 0.1);
}

#[test]
fn test_dv01_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo, context, date(2025, 1, 10));

    let registry = standard_registry();
    let results = registry.compute(&[MetricId::Dv01], &mut mctx).unwrap();

    let dv01 = results.get(&MetricId::Dv01).unwrap();

    // DV01 = PV(bumped) - PV(base); when rates rise, PV falls, so DV01 is negative
    assert!(*dv01 <= 0.0);

    // For $1M 3M repo, DV01 ≈ notional × maturity × 1bp ≈ $1M × 0.25 × 0.0001 = $25
    // Upper bound of $100 is generous for this instrument
    assert!(
        dv01.abs() < 100.0,
        "Repo DV01 should be small, got: {}",
        dv01.abs()
    );
}

#[test]
fn test_funding_risk_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo, context, date(2025, 1, 10));

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::FundingRisk], &mut mctx)
        .unwrap();

    let funding_risk = results.get(&MetricId::FundingRisk).unwrap();

    // Increasing repo rate typically increases PV (more interest earned).
    // FundingRisk follows the dPV/dy convention (bumped − base), so it must
    // be positive for the cash lender.
    assert!(*funding_risk >= 0.0);
}

#[test]
fn test_theta_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo, context, date(2025, 1, 10));

    let registry = standard_registry();
    let results = registry.compute(&[MetricId::Theta], &mut mctx).unwrap();

    let theta = results.get(&MetricId::Theta).unwrap();

    // Theta measures time decay; should be non-zero for mid-life repo
    assert!(theta.abs() > 0.0);
}

#[test]
fn test_accrued_interest_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();

    // Value mid-term to have accrued interest
    let mid_date = date(2025, 2, 15);
    let mut mctx = create_metric_context(repo.clone(), context, mid_date);

    let registry = standard_registry();
    let results = registry.compute(&[MetricId::Accrued], &mut mctx).unwrap();

    let accrued = results.get(&MetricId::Accrued).unwrap();

    // Should have accrued some interest by mid-term
    assert!(*accrued > 0.0);

    // Should be less than total interest
    let total_interest = repo.interest_amount().unwrap().amount();
    assert!(*accrued < total_interest);
}

#[test]
fn test_accrued_interest_before_start() {
    let repo = create_test_repo();
    let context = create_standard_market_context();

    // Value before start
    let before_start = date(2025, 1, 10);
    let mut mctx = create_metric_context(repo, context, before_start);

    let registry = standard_registry();
    let results = registry.compute(&[MetricId::Accrued], &mut mctx).unwrap();

    let accrued = results.get(&MetricId::Accrued).unwrap();

    // No accrual before start
    assert_approx_eq(*accrued, 0.0, 1e-6);
}

#[test]
fn test_accrued_interest_at_maturity() {
    let repo = create_test_repo();
    let context = create_standard_market_context();

    let maturity = repo.maturity;
    let mut mctx = create_metric_context(repo.clone(), context, maturity);

    let registry = standard_registry();
    let results = registry.compute(&[MetricId::Accrued], &mut mctx).unwrap();

    let accrued = results.get(&MetricId::Accrued).unwrap();
    let total_interest = repo.interest_amount().unwrap().amount();

    // At maturity, accrued should equal total interest
    assert_approx_eq(*accrued, total_interest, 1.0);
}

#[test]
fn test_bucketed_dv01_metric() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo, context, date(2025, 1, 10));

    let registry = standard_registry();
    let results = registry
        .compute(&[MetricId::BucketedDv01], &mut mctx)
        .unwrap();

    // Should successfully compute
    assert!(results.contains_key(&MetricId::BucketedDv01));
}

#[test]
fn test_multiple_metrics_computed_together() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo, context, date(2025, 1, 10));

    let metrics = vec![
        MetricId::CollateralValue,
        MetricId::RequiredCollateral,
        MetricId::CollateralCoverage,
        MetricId::RepoInterest,
        MetricId::EffectiveRate,
        MetricId::TimeToMaturity,
    ];

    let registry = standard_registry();
    let results = registry.compute(&metrics, &mut mctx).unwrap();

    // All metrics should be present
    for metric in &metrics {
        assert!(results.contains_key(metric), "Missing metric: {:?}", metric);
    }
}

#[test]
fn test_metric_dependencies_resolved() {
    let repo = create_test_repo();
    let context = create_standard_market_context();
    let mut mctx = create_metric_context(repo, context, date(2025, 1, 10));

    let registry = standard_registry();

    // CollateralCoverage depends on CollateralValue and RequiredCollateral
    let results = registry
        .compute(&[MetricId::CollateralCoverage], &mut mctx)
        .unwrap();

    // The metric should be computed
    assert!(
        results.contains_key(&MetricId::CollateralCoverage),
        "CollateralCoverage should be computed"
    );

    // Note: The current metrics framework may not return dependencies in results
    // Just verify the main metric works
    let coverage = results.get(&MetricId::CollateralCoverage).unwrap();
    assert!(*coverage > 0.0, "Coverage should be positive");
}

#[test]
fn test_metrics_with_price_with_metrics() {
    let repo = create_test_repo();
    let context = create_standard_market_context();

    let metrics = vec![
        MetricId::CollateralValue,
        MetricId::RequiredCollateral,
        MetricId::RepoInterest,
        MetricId::Dv01,
    ];

    let result = repo
        .price_with_metrics(
            &context,
            date(2025, 1, 10),
            &metrics,
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Check base valuation
    assert_eq!(result.value.currency(), Currency::USD);

    // Check metrics are present in results
    assert!(result.measures.contains_key("collateral_value"));
    assert!(result.measures.contains_key("required_collateral"));
    assert!(result.measures.contains_key("repo_interest"));
    assert!(result.measures.contains_key("dv01"));
}

#[test]
fn test_metrics_registry_has_repo_metrics() {
    let registry = standard_registry();

    // Verify all repo metrics are registered
    assert!(registry.has_metric(MetricId::CollateralValue));
    assert!(registry.has_metric(MetricId::RequiredCollateral));
    assert!(registry.has_metric(MetricId::CollateralCoverage));
    assert!(registry.has_metric(MetricId::RepoInterest));
    assert!(registry.has_metric(MetricId::EffectiveRate));
    assert!(registry.has_metric(MetricId::Dv01));
    assert!(registry.has_metric(MetricId::FundingRisk));
    assert!(registry.has_metric(MetricId::TimeToMaturity));
    assert!(registry.has_metric(MetricId::ImpliedCollateralReturn));
    assert!(registry.has_metric(MetricId::Theta));
    assert!(registry.has_metric(MetricId::Accrued));
    assert!(registry.has_metric(MetricId::BucketedDv01));
}

#[test]
fn test_metrics_applicable_to_repo() {
    let registry = standard_registry();

    // Check applicability to Repo instrument type
    assert!(registry.is_applicable(
        &MetricId::CollateralValue,
        finstack_valuations::pricer::InstrumentType::Repo
    ));
    assert!(registry.is_applicable(
        &MetricId::RequiredCollateral,
        finstack_valuations::pricer::InstrumentType::Repo
    ));
    assert!(registry.is_applicable(
        &MetricId::Dv01,
        finstack_valuations::pricer::InstrumentType::Repo
    ));
    assert!(registry.is_applicable(
        &MetricId::Theta,
        finstack_valuations::pricer::InstrumentType::Repo
    ));
}

/// Invariant test: Metrics use business-day adjusted dates consistently with PV.
///
/// This test verifies that accrued interest, TTM, and implied collateral return
/// all use the same adjusted dates as the PV calculation, ensuring no date
/// inconsistencies when the stored dates fall on weekends/holidays.
#[test]
fn test_metric_date_handling_uses_adjusted_dates() {
    use finstack_core::dates::{BusinessDayConvention, DayCount};
    use finstack_core::types::CurveId;
    use finstack_valuations::instruments::rates::repo::RepoType;
    use finstack_valuations::instruments::Attributes;

    // Create a repo with WEEKEND dates (Saturday -> Monday adjustment expected)
    // Saturday Jan 4, 2025 -> Monday Jan 6, 2025 (Following BDC with TARGET2 calendar)
    // Saturday Jan 11, 2025 -> Monday Jan 13, 2025
    let start_saturday = date(2025, 1, 4);
    let maturity_saturday = date(2025, 1, 11);

    let collateral = treasury_collateral();

    let repo_weekend = finstack_valuations::instruments::rates::repo::RepoBuilder::new()
        .id("WEEKEND-DATES".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral.clone())
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(start_saturday)
        .maturity(maturity_saturday)
        .haircut(0.02)
        .repo_type(RepoType::Term)
        .triparty(false)
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(Some("target2".into()))
        .discount_curve_id(CurveId::from("USD-OIS"))
        .margin_spec_opt(None)
        .attributes(Attributes::default())
        .build()
        .expect("Weekend repo should build");

    // Create a repo with pre-adjusted dates (Monday dates directly)
    let start_monday = date(2025, 1, 6);
    let maturity_monday = date(2025, 1, 13);

    let repo_adjusted = finstack_valuations::instruments::rates::repo::RepoBuilder::new()
        .id("ADJUSTED-DATES".into())
        .cash_amount(Money::new(1_000_000.0, Currency::USD))
        .collateral(collateral)
        .repo_rate(Decimal::try_from(0.05).expect("valid decimal"))
        .start_date(start_monday)
        .maturity(maturity_monday)
        .haircut(0.02)
        .repo_type(RepoType::Term)
        .triparty(false)
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(Some("target2".into()))
        .discount_curve_id(CurveId::from("USD-OIS"))
        .margin_spec_opt(None)
        .attributes(Attributes::default())
        .build()
        .expect("Adjusted repo should build");

    let context = create_standard_market_context();

    // Value on Thursday Jan 9, 2025 (mid-term for both repos after adjustment)
    let as_of = date(2025, 1, 9);

    let mut mctx_weekend = create_metric_context(repo_weekend.clone(), context.clone(), as_of);
    let mut mctx_adjusted = create_metric_context(repo_adjusted.clone(), context.clone(), as_of);

    let registry = standard_registry();

    // Compute metrics that depend on dates
    let metrics = vec![
        MetricId::TimeToMaturity,
        MetricId::Accrued,
        MetricId::ImpliedCollateralReturn,
        MetricId::RepoInterest,
    ];

    let results_weekend = registry.compute(&metrics, &mut mctx_weekend).unwrap();
    let results_adjusted = registry.compute(&metrics, &mut mctx_adjusted).unwrap();

    // INVARIANT: All date-dependent metrics should match between weekend and pre-adjusted repos
    // because all metrics should use adjusted_dates() internally.

    let ttm_weekend = results_weekend.get(&MetricId::TimeToMaturity).unwrap();
    let ttm_adjusted = results_adjusted.get(&MetricId::TimeToMaturity).unwrap();
    assert!(
        (ttm_weekend - ttm_adjusted).abs() < 1e-9,
        "TimeToMaturity should be identical: weekend={}, adjusted={}",
        ttm_weekend,
        ttm_adjusted
    );

    let accrued_weekend = results_weekend.get(&MetricId::Accrued).unwrap();
    let accrued_adjusted = results_adjusted.get(&MetricId::Accrued).unwrap();
    assert!(
        (accrued_weekend - accrued_adjusted).abs() < 1e-6,
        "Accrued interest should be identical: weekend={}, adjusted={}",
        accrued_weekend,
        accrued_adjusted
    );

    let implied_weekend = results_weekend
        .get(&MetricId::ImpliedCollateralReturn)
        .unwrap();
    let implied_adjusted = results_adjusted
        .get(&MetricId::ImpliedCollateralReturn)
        .unwrap();
    assert!(
        (implied_weekend - implied_adjusted).abs() < 1e-9,
        "ImpliedCollateralReturn should be identical: weekend={}, adjusted={}",
        implied_weekend,
        implied_adjusted
    );

    let interest_weekend = results_weekend.get(&MetricId::RepoInterest).unwrap();
    let interest_adjusted = results_adjusted.get(&MetricId::RepoInterest).unwrap();
    assert!(
        (interest_weekend - interest_adjusted).abs() < 1e-6,
        "RepoInterest should be identical: weekend={}, adjusted={}",
        interest_weekend,
        interest_adjusted
    );

    // Also verify PV consistency
    let pv_weekend = repo_weekend.value(&context, as_of).unwrap();
    let pv_adjusted = repo_adjusted.value(&context, as_of).unwrap();
    assert!(
        (pv_weekend.amount() - pv_adjusted.amount()).abs() < 1e-6,
        "PV should be identical: weekend={}, adjusted={}",
        pv_weekend.amount(),
        pv_adjusted.amount()
    );
}
