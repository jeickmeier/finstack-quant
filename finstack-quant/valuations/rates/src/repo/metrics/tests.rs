//! Pricing and metric helpers for interest-rate instruments.
//!
use super::*;
use crate::repo::{CollateralSpec, Repo};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use time::Month;

fn test_date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("valid date"), day)
        .expect("valid date")
}

fn create_test_repo() -> Repo {
    let collateral = CollateralSpec::new("BOND_ABC", 1000.0, "BOND_ABC_PRICE");
    Repo::term(
        "REPO_001",
        Money::from((1000000_i64, Currency::USD)),
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("should succeed");
    MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .insert_price(
            "BOND_ABC_PRICE",
            MarketScalar::price(Money::new(1.02, Currency::USD).expect("valid test amount")),
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
        Money::from((0_i64, Currency::USD)),
        MetricContext::default_config(),
    );

    let calculator = CollateralValueCalculator;
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
        Money::from((0_i64, Currency::USD)),
        MetricContext::default_config(),
    );

    let calculator = RequiredCollateralCalculator;
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
        Money::from((0_i64, Currency::USD)),
        MetricContext::default_config(),
    );

    let calculator = EffectiveRateCalculator;
    let rate = calculator.calculate(&mut context).expect("should succeed");
    assert!((rate - 0.05).abs() < 1e-9);
}

#[test]
fn test_dv01_negative_when_rates_rise_price_falls() {
    use finstack_quant_valuations_core::metrics::MetricId;

    let as_of = test_date(2025, 1, 10);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (5.0, 0.80)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("should succeed");
    let repo = create_test_repo();
    let ctx = create_test_context()
        .try_insert(disc)
        .expect("insert market curve");
    let pv = repo.value(&ctx, as_of).expect("should succeed");
    let reg = crate::RATES.metrics();
    let mut mctx = finstack_quant_valuations_core::metrics::MetricContext::new(
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
    use finstack_quant_valuations_core::metrics::MetricId;

    let as_of = test_date(2025, 1, 10);
    let repo = create_test_repo();
    let ctx = create_test_context();
    let pv = repo.value(&ctx, as_of).expect("should succeed");
    let mut mctx = finstack_quant_valuations_core::metrics::MetricContext::new(
        std::sync::Arc::new(repo.clone()),
        std::sync::Arc::new(ctx),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let reg = crate::RATES.metrics();
    let res = reg
        .compute(&[MetricId::RepoInterest], &mut mctx)
        .expect("should succeed");
    let m_interest = *res.get(&MetricId::RepoInterest).expect("should succeed");
    let direct = repo.interest_amount().expect("should succeed").amount();
    assert!((m_interest - direct).abs() < 1e-9);
}

#[test]
fn test_collateral_coverage_ratio_reasonable() {
    use finstack_quant_valuations_core::metrics::MetricId;

    let as_of = test_date(2025, 1, 10);
    let repo = create_test_repo();
    let ctx = create_test_context();
    let pv = repo.value(&ctx, as_of).expect("should succeed");
    let mut mctx = finstack_quant_valuations_core::metrics::MetricContext::new(
        std::sync::Arc::new(repo),
        std::sync::Arc::new(ctx),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let reg = crate::RATES.metrics();
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
    use finstack_quant_valuations_core::metrics::MetricId;

    let as_of = test_date(2025, 1, 10);
    let repo = create_test_repo();
    let ctx = create_test_context();
    let pv = repo.value(&ctx, as_of).expect("should succeed");
    let mut mctx = finstack_quant_valuations_core::metrics::MetricContext::new(
        std::sync::Arc::new(repo),
        std::sync::Arc::new(ctx),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let reg = crate::RATES.metrics();
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
    use finstack_quant_valuations_core::metrics::MetricId;

    let as_of = test_date(2025, 1, 10);
    let repo = create_test_repo();
    let ctx = create_test_context();
    let pv = repo.value(&ctx, as_of).expect("should succeed");
    let mut mctx = finstack_quant_valuations_core::metrics::MetricContext::new(
        std::sync::Arc::new(repo),
        std::sync::Arc::new(ctx),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let reg = crate::RATES.metrics();
    let res = reg
        .compute(&[MetricId::FundingRisk], &mut mctx)
        .expect("should succeed");
    let fr = *res.get(&MetricId::FundingRisk).expect("should succeed");
    // dPV/dy convention: a +1bp repo-rate bump raises the lender's repurchase
    // payment, so ΔPV must be positive for a cash lender.
    assert!(fr >= 0.0);
}

#[test]
fn test_cross_currency_collateral_coverage_and_expired_repo_zero() {
    use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
    use finstack_quant_valuations_core::metrics::MetricId;
    use std::sync::Arc;

    struct FixedFx(f64);
    impl FxProvider for FixedFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            match (from, to) {
                (Currency::EUR, Currency::USD) => Ok(self.0),
                _ => Err(finstack_quant_core::Error::Validation(
                    "unsupported pair".into(),
                )),
            }
        }
    }

    let mut repo = create_test_repo();
    repo.maturity = test_date(2025, 1, 18); // Saturday -> adjusts to Monday 2025-01-20
    let (_, adj_maturity) = repo.adjusted_dates().expect("adjusted dates");
    assert_eq!(repo.expiry(), Some(adj_maturity));

    // Collateral priced in EUR (10_000 units * EUR 100 = EUR 1_000_000) vs USD cash.
    let base_ctx = create_test_context().insert_price(
        "BOND_ABC_PRICE",
        finstack_quant_core::market_data::scalars::MarketScalar::price(
            Money::new(100.0, Currency::EUR).expect("price"),
        ),
    );
    let as_of = test_date(2025, 1, 10);
    let pv = repo.value(&base_ctx, as_of).expect("pv");
    let reg = crate::RATES.metrics();

    // Missing FX fails loudly instead of dividing EUR by USD.
    let mut no_fx_ctx = finstack_quant_valuations_core::metrics::MetricContext::new(
        Arc::new(repo.clone()),
        Arc::new(base_ctx.clone()),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    assert!(
        reg.compute(&[MetricId::CollateralCoverage], &mut no_fx_ctx)
            .is_err()
    );

    // With EURUSD = 1.10 FX, coverage converts EUR 1M -> USD 1.1M cleanly.
    let ctx_fx = base_ctx.insert_fx(FxMatrix::new(Arc::new(FixedFx(1.10)), None));
    let mut fx_mctx = finstack_quant_valuations_core::metrics::MetricContext::new(
        Arc::new(repo.clone()),
        Arc::new(ctx_fx.clone()),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let res = reg
        .compute(
            &[
                MetricId::CollateralCoverage,
                MetricId::ImpliedCollateralReturn,
            ],
            &mut fx_mctx,
        )
        .expect("fx coverage");
    let req = repo.required_collateral_value().expect("req").amount();
    // Collateral: 1,000 units * EUR 100 * 1.10 = USD 110,000.
    let expected_cov = (100_000.0 * 1.10) / req;
    assert!((res[&MetricId::CollateralCoverage] - expected_cov).abs() < 1e-9);
    assert!(res[&MetricId::ImpliedCollateralReturn].is_finite());

    // Expired repo returns 0.0 for TimeToMaturity & ImpliedCollateralReturn instead of DateRange error,
    // and simm_sensitivities honors adjusted maturity (still live between raw 18th and adj 20th).
    let between = test_date(2025, 1, 19);
    let simm_live = finstack_quant_margin::Marginable::simm_sensitivities(&repo, &ctx_fx, between)
        .expect("simm live");
    assert!(!simm_live.ir_delta.is_empty());

    let after = test_date(2025, 1, 25);
    let pv_after = repo.value(&ctx_fx, after).expect("expired pv");
    let mut after_ctx = finstack_quant_valuations_core::metrics::MetricContext::new(
        Arc::new(repo),
        Arc::new(ctx_fx),
        after,
        pv_after,
        MetricContext::default_config(),
    );
    let expired_res = reg
        .compute(
            &[MetricId::TimeToMaturity, MetricId::ImpliedCollateralReturn],
            &mut after_ctx,
        )
        .expect("expired metrics");
    assert_eq!(expired_res[&MetricId::TimeToMaturity], 0.0);
    assert_eq!(expired_res[&MetricId::ImpliedCollateralReturn], 0.0);
}
