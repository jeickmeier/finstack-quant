//! Tests for asset swap (ASW) metrics.

use finstack_core::currency::Currency;
use finstack_core::dates::{DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::market_data::term_structures::ForwardCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::bond::AssetSwapConfig;
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::fixed_income::bond::CashflowSpec;
use finstack_valuations::instruments::fixed_income::bond::{
    AssetSwapMarketCalculator, AssetSwapParCalculator,
};
use finstack_valuations::instruments::PricingOverrides;
use finstack_valuations::metrics::{standard_registry, MetricCalculator, MetricContext, MetricId};
use std::sync::Arc;
use time::macros::date;

fn simple_discount_curve(id: &str, as_of: time::Date) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap()
}

fn simple_fixed_bond(as_of: time::Date) -> Bond {
    Bond::fixed(
        "ASW-TEST",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .expect("Test bond creation should succeed")
}

fn simple_forward_curve(id: &str, as_of: time::Date) -> ForwardCurve {
    ForwardCurve::builder(id, 0.25)
        .base_date(as_of)
        .knots(vec![(0.5, 0.04), (5.0, 0.045), (10.0, 0.05)])
        .build()
        .unwrap()
}

#[test]
fn test_asw_market_requires_accrued_when_clean_price_present() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = simple_fixed_bond(as_of);
    // Attach a clean price so the market ASW calculator will require Accrued.
    bond.pricing_overrides = finstack_valuations::instruments::PricingOverrides::default()
        .with_quoted_clean_price(101.0);

    // Market with a simple discount curve
    let disc = simple_discount_curve("USD-OIS", as_of);
    let market = MarketContext::new().insert(disc);

    // Metric context with quoted clean price but without Accrued metric
    let mut ctx = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        Money::new(100.0, Currency::USD),
        MetricContext::default_config(),
    );

    let calc = AssetSwapMarketCalculator::default();
    let result = calc.calculate(&mut ctx);

    match result {
        Err(e) => {
            let msg = format!("{}", e);
            assert!(
                msg.contains("metric:Accrued"),
                "expected missing Accrued error, got {}",
                msg
            );
        }
        Ok(v) => panic!(
            "expected ASW market calculation to fail without Accrued, got {}",
            v
        ),
    }
}

#[test]
fn test_asw_par_with_config_uses_fixed_leg_conventions() {
    let as_of = date!(2025 - 01 - 01);
    let bond = simple_fixed_bond(as_of);

    let disc = simple_discount_curve("USD-OIS", as_of);
    let market = MarketContext::new().insert(disc);

    let mut ctx_default = MetricContext::new(
        Arc::new(bond.clone()),
        Arc::new(market.clone()),
        as_of,
        Money::new(100.0, Currency::USD),
        MetricContext::default_config(),
    );
    let asw_default = AssetSwapParCalculator::default()
        .calculate(&mut ctx_default)
        .expect("ASW par with default config should succeed");

    // Override fixed-leg conventions to annual 30E/360 and verify we still get
    // a finite result and that the value changes relative to the default.
    let config = AssetSwapConfig {
        fixed_leg_day_count: Some(DayCount::ThirtyE360),
        fixed_leg_frequency: Some(Tenor::annual()),
        fixed_leg_bdc: None,
        fixed_leg_calendar_id: None,
        fixed_leg_stub: None,
    };
    let mut ctx_custom = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        Money::new(100.0, Currency::USD),
        MetricContext::default_config(),
    );
    let asw_custom = AssetSwapParCalculator::with_config(config)
        .calculate(&mut ctx_custom)
        .expect("ASW par with custom config should succeed");

    assert!(
        asw_custom.is_finite(),
        "ASW par with custom config should be finite"
    );
    assert!(
        (asw_custom - asw_default).abs() > 1e-12,
        "ASW par with custom conventions should differ from default"
    );
}

#[test]
fn test_asw_par_tracks_coupon_minus_par_rate() {
    let as_of = date!(2025 - 01 - 01);
    let bond = simple_fixed_bond(as_of);

    let disc = simple_discount_curve("USD-OIS", as_of);
    let market = MarketContext::new().insert(disc.clone());

    let registry = standard_registry();
    let mut ctx = MetricContext::new(
        Arc::new(bond.clone()),
        Arc::new(market),
        as_of,
        Money::new(100.0, Currency::USD),
        MetricContext::default_config(),
    );

    let asw_par = *registry
        .compute(&[MetricId::ASWPar], &mut ctx)
        .expect("ASW par")
        .get(&MetricId::ASWPar)
        .expect("ASW par result");

    // Market-standard approximation: ASW_par ≈ coupon - par swap rate (same conventions).
    let CashflowSpec::Fixed(spec) = &bond.cashflow_spec else {
        panic!("expected fixed bond")
    };
    let periods = finstack_cashflows::builder::periods::build_periods(
        finstack_cashflows::builder::periods::BuildPeriodsParams {
            start: bond.issue_date,
            end: bond.maturity,
            frequency: spec.freq,
            stub: spec.stub,
            bdc: spec.bdc,
            calendar_id: &spec.calendar_id,
            end_of_month: spec.end_of_month,
            day_count: spec.dc,
            payment_lag_days: spec.payment_lag_days,
            reset_lag_days: None,
            adjust_accrual_dates: false,
        },
    )
    .expect("schedule build should succeed");
    let annuity: f64 = periods
        .iter()
        .map(|p| {
            finstack_valuations::instruments::pricing::time::relative_df_discount_curve(
                &disc,
                as_of,
                p.payment_date,
            )
            .expect("df")
                * p.accrual_year_fraction
        })
        .sum();
    let p0 = finstack_valuations::instruments::pricing::time::relative_df_discount_curve(
        &disc,
        as_of,
        bond.issue_date,
    )
    .expect("df");
    let pn = finstack_valuations::instruments::pricing::time::relative_df_discount_curve(
        &disc,
        as_of,
        periods.last().expect("periods").payment_date,
    )
    .expect("df");
    let par_rate = (p0 - pn) / annuity;

    let rate_f64 = rust_decimal::prelude::ToPrimitive::to_f64(&spec.rate).unwrap_or(0.0);
    assert!(
        (asw_par - (rate_f64 - par_rate)).abs() < 1e-6,
        "ASW par should align with coupon - par rate; got asw_par={asw_par}, coupon={}, par_rate={par_rate}",
        spec.rate
    );
}

#[test]
fn test_asw_market_tightens_when_price_rises() {
    let as_of = date!(2025 - 01 - 01);
    let mut rich_bond = simple_fixed_bond(as_of);
    rich_bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(101.0);

    let mut cheap_bond = simple_fixed_bond(as_of);
    cheap_bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(99.0);

    let disc = simple_discount_curve("USD-OIS", as_of);
    let market = MarketContext::new().insert(disc);

    let registry = standard_registry();

    let mut ctx_rich = MetricContext::new(
        Arc::new(rich_bond),
        Arc::new(market.clone()),
        as_of,
        Money::new(100.0, Currency::USD),
        MetricContext::default_config(),
    );
    let asw_rich = *registry
        .compute(&[MetricId::ASWMarket], &mut ctx_rich)
        .expect("ASW rich")
        .get(&MetricId::ASWMarket)
        .expect("ASW rich result");

    let mut ctx_cheap = MetricContext::new(
        Arc::new(cheap_bond),
        Arc::new(market),
        as_of,
        Money::new(100.0, Currency::USD),
        MetricContext::default_config(),
    );
    let asw_cheap = *registry
        .compute(&[MetricId::ASWMarket], &mut ctx_cheap)
        .expect("ASW cheap")
        .get(&MetricId::ASWMarket)
        .expect("ASW cheap result");

    // ASW should respond to price changes (non-degenerate sensitivity).
    assert_ne!(asw_rich, asw_cheap, "ASW should move with price changes");
    assert!(
        (asw_rich - asw_cheap).abs() > 1e-4,
        "ASW difference should be economically meaningful: rich={asw_rich}, cheap={asw_cheap}"
    );
}

#[test]
fn test_asw_par_metric_rejects_matured_schedule() {
    let issue = date!(2020 - 01 - 01);
    let maturity = date!(2025 - 01 - 01);
    let as_of = maturity;

    let bond = Bond::fixed(
        "ASW-PAR-METRIC-MATURED",
        Money::new(100.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .expect("bond");

    let disc = simple_discount_curve("USD-OIS", issue);
    let market = MarketContext::new().insert(disc);
    let registry = standard_registry();

    let mut ctx = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        Money::new(0.0, Currency::USD),
        MetricContext::default_config(),
    );
    let err = registry
        .compute(&[MetricId::ASWPar], &mut ctx)
        .expect_err("matured schedule should fail");
    let msg = format!("{err}");
    assert!(
        msg.contains("ASW par calculation requires at least two fixed-leg schedule dates"),
        "unexpected error: {msg}"
    );
}

#[test]
fn test_asw_market_metric_rejects_matured_schedule() {
    let issue = date!(2020 - 01 - 01);
    let maturity = date!(2025 - 01 - 01);
    let as_of = maturity;

    let mut bond = Bond::fixed(
        "ASW-MKT-METRIC-MATURED",
        Money::new(100.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .expect("bond");
    bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(100.0);

    let disc = simple_discount_curve("USD-OIS", issue);
    let market = MarketContext::new().insert(disc);
    let registry = standard_registry();

    let mut ctx = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        Money::new(0.0, Currency::USD),
        MetricContext::default_config(),
    );
    let err = registry
        .compute(&[MetricId::ASWMarket], &mut ctx)
        .expect_err("matured schedule should fail");
    let msg = format!("{err}");
    assert!(
        msg.contains("ASW market calculation requires at least two fixed-leg schedule dates"),
        "unexpected error: {msg}"
    );
}

#[test]
fn test_asw_par_with_forward_day_count_override_changes_result() {
    let as_of = date!(2025 - 01 - 01);
    let bond = simple_fixed_bond(as_of);
    let disc = simple_discount_curve("USD-OIS", as_of);
    let fwd = simple_forward_curve("USD-SOFR-3M", as_of);
    let market = MarketContext::new().insert(disc).insert(fwd);

    let par_default = finstack_valuations::instruments::fixed_income::bond::asw_par_with_forward(
        &bond,
        &market,
        as_of,
        "USD-SOFR-3M",
        0.0,
    )
    .expect("par asw default");

    let par_act365f =
        finstack_valuations::instruments::fixed_income::bond::asw_par_with_forward_config(
            &bond,
            &market,
            as_of,
            "USD-SOFR-3M",
            0.0,
            Some(DayCount::Act365F),
        )
        .expect("par asw override");

    assert!(
        par_default.is_finite() && par_act365f.is_finite(),
        "ASW par with forward should be finite"
    );
    assert!(
        (par_default - par_act365f).abs() > 1e-10,
        "Changing fixed-leg day-count should change par ASW"
    );
}

#[test]
fn test_asw_market_with_forward_requires_dirty_price() {
    let as_of = date!(2025 - 01 - 01);
    let bond = simple_fixed_bond(as_of);
    let disc = simple_discount_curve("USD-OIS", as_of);
    let fwd = simple_forward_curve("USD-SOFR-3M", as_of);
    let market = MarketContext::new().insert(disc).insert(fwd);

    let err = finstack_valuations::instruments::fixed_income::bond::asw_market_with_forward_config(
        &bond,
        &market,
        as_of,
        "USD-SOFR-3M",
        0.0,
        None,
        None,
    )
    .expect_err("should require dirty price");

    let msg = format!("{err}");
    assert!(
        msg.contains("dirty_price_ccy"),
        "expected missing dirty price error, got {msg}"
    );
}

#[test]
fn test_asw_market_with_forward_moves_with_dirty_price() {
    let as_of = date!(2025 - 01 - 01);
    let bond = simple_fixed_bond(as_of);
    let disc = simple_discount_curve("USD-OIS", as_of);
    let fwd = simple_forward_curve("USD-SOFR-3M", as_of);
    let market = MarketContext::new().insert(disc).insert(fwd);

    let par_asw = finstack_valuations::instruments::fixed_income::bond::asw_par_with_forward(
        &bond,
        &market,
        as_of,
        "USD-SOFR-3M",
        0.0,
    )
    .expect("par asw");

    let ann_asw_par_px =
        finstack_valuations::instruments::fixed_income::bond::asw_market_with_forward_config(
            &bond,
            &market,
            as_of,
            "USD-SOFR-3M",
            0.0,
            Some(bond.notional.amount()),
            None,
        )
        .expect("asw mkt at par");

    let rich_asw =
        finstack_valuations::instruments::fixed_income::bond::asw_market_with_forward_config(
            &bond,
            &market,
            as_of,
            "USD-SOFR-3M",
            0.0,
            Some(1.01 * bond.notional.amount()),
            None,
        )
        .expect("asw rich");

    assert!(
        (ann_asw_par_px - par_asw).abs() < 1e-10,
        "Market ASW at par price should match par ASW"
    );
    assert!(
        rich_asw < ann_asw_par_px,
        "Higher dirty price should tighten ASW per forward formula"
    );
}

#[test]
fn test_asw_par_forward_returns_zero_for_zero_notional() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = simple_fixed_bond(as_of);
    bond.notional = Money::new(0.0, Currency::USD);

    let disc = simple_discount_curve("USD-OIS", as_of);
    let fwd = simple_forward_curve("USD-SOFR-3M", as_of);
    let market = MarketContext::new().insert(disc).insert(fwd);

    let asw = finstack_valuations::instruments::fixed_income::bond::asw_par_with_forward(
        &bond,
        &market,
        as_of,
        "USD-SOFR-3M",
        0.0,
    )
    .expect("asw par");

    assert_eq!(asw, 0.0, "Zero notional should return zero ASW");
}

#[test]
fn test_asw_par_forward_rejects_matured_schedule() {
    let issue = date!(2020 - 01 - 01);
    let maturity = date!(2025 - 01 - 01);
    let as_of = maturity;

    let bond = Bond::fixed(
        "ASW-PAR-MATURED",
        Money::new(100.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .expect("bond");

    let disc = simple_discount_curve("USD-OIS", issue);
    let fwd = simple_forward_curve("USD-SOFR-3M", issue);
    let market = MarketContext::new().insert(disc).insert(fwd);

    let err = finstack_valuations::instruments::fixed_income::bond::asw_par_with_forward(
        &bond,
        &market,
        as_of,
        "USD-SOFR-3M",
        0.0,
    )
    .expect_err("matured schedule should fail");
    let msg = format!("{err}");
    assert!(
        msg.contains("ASW forward par calculation requires at least two fixed-leg schedule dates"),
        "unexpected error: {msg}"
    );
}

#[test]
fn test_asw_market_forward_rejects_matured_schedule() {
    let issue = date!(2020 - 01 - 01);
    let maturity = date!(2025 - 01 - 01);
    let as_of = maturity;

    let bond = Bond::fixed(
        "ASW-MKT-MATURED",
        Money::new(100.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .expect("bond");

    let disc = simple_discount_curve("USD-OIS", issue);
    let fwd = simple_forward_curve("USD-SOFR-3M", issue);
    let market = MarketContext::new().insert(disc).insert(fwd);

    let err = finstack_valuations::instruments::fixed_income::bond::asw_market_with_forward(
        &bond,
        &market,
        as_of,
        "USD-SOFR-3M",
        0.0,
        Some(bond.notional.amount()),
    )
    .expect_err("matured schedule should fail");
    let msg = format!("{err}");
    assert!(
        msg.contains(
            "ASW forward market calculation requires at least two fixed-leg schedule dates"
        ),
        "unexpected error: {msg}"
    );
}
