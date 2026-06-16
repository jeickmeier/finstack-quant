use super::*;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::repo::{CollateralSpec, Repo};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use time::Month;

fn test_date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("valid date"), day)
        .expect("valid date")
}

fn create_test_repo() -> Repo {
    let collateral = CollateralSpec::new("BOND_ABC", 1000.0, "BOND_ABC_PRICE");
    Repo::term(
        "REPO_001",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        test_date(2025, 1, 15),
        test_date(2025, 4, 15),
        finstack_quant_core::types::CurveId::from("USD-OIS"),
    )
    .expect("test repo construction")
}

fn create_test_context() -> MarketContext {
    let as_of = test_date(2025, 1, 10);
    // Use a near-zero rate curve (0.01% = 1bp) for testing
    // DF(10) = exp(-0.0001 * 10) ≈ 0.999
    let disc = finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (10.0, 0.999)])
        .build()
        .expect("should succeed");
    MarketContext::new().insert(disc).insert_price(
        "BOND_ABC_PRICE",
        MarketScalar::Price(Money::new(1.02, Currency::USD)),
    )
}

#[test]
fn test_collateral_value_calculator() {
    let repo = create_test_repo();
    let market_context = create_test_context();
    let mut context = MetricContext::new(
        std::sync::Arc::new(repo),
        std::sync::Arc::new(market_context),
        test_date(2025, 1, 10),
        Money::new(0.0, Currency::USD),
        MetricContext::default_config(),
    );

    let calculator = collateral_value::CollateralValueCalculator;
    let value = calculator.calculate(&mut context).expect("should succeed");
    assert!((value - 1020.0).abs() < 1e-6);
}

#[test]
fn test_required_collateral_calculator() {
    let repo = create_test_repo();
    let market_context = create_test_context();
    let mut context = MetricContext::new(
        std::sync::Arc::new(repo),
        std::sync::Arc::new(market_context),
        test_date(2025, 1, 10),
        Money::new(0.0, Currency::USD),
        MetricContext::default_config(),
    );

    let calculator = required_collateral::RequiredCollateralCalculator;
    let value = calculator.calculate(&mut context).expect("should succeed");
    assert!((value - 1_020_408.16).abs() < 1.0);
}

#[test]
fn test_effective_rate_calculator() {
    let repo = create_test_repo();
    let market_context = create_test_context();
    let mut context = MetricContext::new(
        std::sync::Arc::new(repo),
        std::sync::Arc::new(market_context),
        test_date(2025, 1, 10),
        Money::new(0.0, Currency::USD),
        MetricContext::default_config(),
    );

    let calculator = effective_rate::EffectiveRateCalculator;
    let rate = calculator.calculate(&mut context).expect("should succeed");
    assert!((rate - 0.05).abs() < 1e-9);
}

#[test]
fn test_dv01_negative_when_rates_rise_price_falls() {
    use crate::metrics::{standard_registry, MetricId};

    let as_of = test_date(2025, 1, 10);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (5.0, 0.80)])
        .build()
        .expect("should succeed");
    let repo = create_test_repo();
    let ctx = create_test_context().insert(disc);
    let pv = repo.value(&ctx, as_of).expect("should succeed");
    let reg = standard_registry();
    let mut mctx = crate::metrics::MetricContext::new(
        std::sync::Arc::new(repo),
        std::sync::Arc::new(ctx),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let res = reg
        .compute(&[MetricId::Dv01], &mut mctx)
        .expect("should succeed");
    let dv01 = *res.get(&MetricId::Dv01).expect("should succeed");
    assert!(dv01 <= 0.0);
}

#[test]
fn test_repo_interest_metric_matches_direct_interest() {
    use crate::metrics::{standard_registry, MetricId};

    let as_of = test_date(2025, 1, 10);
    let repo = create_test_repo();
    let ctx = create_test_context();
    let pv = repo.value(&ctx, as_of).expect("should succeed");
    let mut mctx = crate::metrics::MetricContext::new(
        std::sync::Arc::new(repo.clone()),
        std::sync::Arc::new(ctx),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let reg = standard_registry();
    let res = reg
        .compute(&[MetricId::RepoInterest], &mut mctx)
        .expect("should succeed");
    let m_interest = *res.get(&MetricId::RepoInterest).expect("should succeed");
    let direct = repo.interest_amount().expect("should succeed").amount();
    assert!((m_interest - direct).abs() < 1e-9);
}

#[test]
fn test_collateral_coverage_ratio_reasonable() {
    use crate::metrics::{standard_registry, MetricId};

    let as_of = test_date(2025, 1, 10);
    let repo = create_test_repo();
    let ctx = create_test_context();
    let pv = repo.value(&ctx, as_of).expect("should succeed");
    let mut mctx = crate::metrics::MetricContext::new(
        std::sync::Arc::new(repo),
        std::sync::Arc::new(ctx),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let reg = standard_registry();
    let res = reg
        .compute(&[MetricId::CollateralCoverage], &mut mctx)
        .expect("should succeed");
    let cov = *res
        .get(&MetricId::CollateralCoverage)
        .expect("should succeed");
    let expected = 1020.0 / 1_020_408.16;
    assert!((cov - expected).abs() < 1e-9);
}

#[test]
fn test_time_to_maturity_and_implied_collateral_return() {
    use crate::metrics::{standard_registry, MetricId};

    let as_of = test_date(2025, 1, 10);
    let repo = create_test_repo();
    let ctx = create_test_context();
    let pv = repo.value(&ctx, as_of).expect("should succeed");
    let mut mctx = crate::metrics::MetricContext::new(
        std::sync::Arc::new(repo),
        std::sync::Arc::new(ctx),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let reg = standard_registry();
    let res = reg
        .compute(
            &[
                MetricId::TimeToMaturity,
                MetricId::CollateralValue,
                MetricId::RequiredCollateral,
                MetricId::ImpliedCollateralReturn,
            ],
            &mut mctx,
        )
        .expect("should succeed");
    let ttm = *res.get(&MetricId::TimeToMaturity).expect("should succeed");
    assert!(ttm > 0.0);
    let cv = *res.get(&MetricId::CollateralValue).expect("should succeed");
    let req = *res
        .get(&MetricId::RequiredCollateral)
        .expect("should succeed");
    let implied = *res
        .get(&MetricId::ImpliedCollateralReturn)
        .expect("should succeed");
    let expected = if req == 0.0 || ttm <= 0.0 {
        0.0
    } else {
        (cv / req - 1.0) / ttm
    };
    assert!((implied - expected).abs() < 1e-9);
}

#[test]
fn test_funding_risk_sign() {
    use crate::metrics::{standard_registry, MetricId};

    let as_of = test_date(2025, 1, 10);
    let repo = create_test_repo();
    let ctx = create_test_context();
    let pv = repo.value(&ctx, as_of).expect("should succeed");
    let mut mctx = crate::metrics::MetricContext::new(
        std::sync::Arc::new(repo),
        std::sync::Arc::new(ctx),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let reg = standard_registry();
    let res = reg
        .compute(&[MetricId::FundingRisk], &mut mctx)
        .expect("should succeed");
    let fr = *res.get(&MetricId::FundingRisk).expect("should succeed");
    // dPV/dy convention: a +1bp repo-rate bump raises the lender's repurchase
    // payment, so ΔPV must be positive for a cash lender.
    assert!(fr >= 0.0);
}
