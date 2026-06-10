//! Duration calculator tests (Macaulay and Modified).

use finstack_cashflows::CashflowProvider;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_duration_zero_coupon() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let mut bond = Bond::fixed(
        "DUR1",
        Money::new(100.0, Currency::USD),
        0.0,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();
    bond.pricing_overrides =
        finstack_valuations::instruments::PricingOverrides::default().with_quoted_clean_price(70.0);

    let curve = finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.70)])
        .build()
        .unwrap();
    let market = finstack_core::market_data::context::MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DurationMac],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let mac_dur = *result.measures.get("duration_mac").unwrap();
    assert!((mac_dur - 5.0).abs() < 0.01); // Zero coupon duration ≈ maturity (day count convention may give ~5.003)
}

#[test]
fn test_yield_duration_convexity_act_act_isma() {
    // ACT/ACT (ICMA) bonds (UK gilts, TIPS convention) must support the full
    // yield/duration/convexity path. Regression: these calculators used a
    // default DayCountContext and errored with MissingFrequencyForActActIsma.
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_valuations::instruments::fixed_income::bond::CashflowSpec;

    let as_of = date!(2025 - 01 - 15);
    let maturity = date!(2030 - 01 - 15);
    let mut bond = Bond::fixed(
        "DUR-ISMA",
        Money::new(100.0, Currency::GBP),
        0.04,
        as_of,
        maturity,
        "GBP-OIS",
    )
    .unwrap();
    if let CashflowSpec::Fixed(spec) = &mut bond.cashflow_spec {
        spec.dc = DayCount::ActActIsma;
    }
    bond.pricing_overrides =
        finstack_valuations::instruments::PricingOverrides::default().with_quoted_clean_price(98.0);

    let curve = DiscountCurve::builder("GBP-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, (-(0.045_f64 * 10.0_f64)).exp())])
        .build()
        .unwrap();
    let market = finstack_core::market_data::context::MarketContext::new().insert(curve);

    let res = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::DurationMac, MetricId::Convexity],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("ActActIsma yield/duration/convexity metrics must succeed");

    let ytm = *res.measures.get("ytm").unwrap();
    let d_mac = *res.measures.get("duration_mac").unwrap();
    let convexity = *res.measures.get("convexity").unwrap();
    assert!(ytm > 0.0 && ytm < 0.15, "unreasonable YTM {ytm}");
    assert!(d_mac > 1.0 && d_mac < 5.5, "unreasonable duration {d_mac}");
    assert!(convexity > 0.0, "convexity should be positive: {convexity}");
}

#[test]
fn test_modified_duration_matches_macaulay_over_yield() {
    // For a simple fixed bond, Duration_mod ≈ Duration_mac / (1 + y/m)
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let bond = Bond::fixed(
        "DUR2",
        Money::new(100.0, Currency::USD),
        0.06,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Flat 5% curve
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (10.0, (-(0.05_f64 * 10.0_f64)).exp())])
        .build()
        .unwrap();
    let market = finstack_core::market_data::context::MarketContext::new().insert(curve);

    let res = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::DurationMac, MetricId::DurationMod],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytm = *res.measures.get("ytm").unwrap();
    let d_mac = *res.measures.get("duration_mac").unwrap();
    let d_mod = *res.measures.get("duration_mod").unwrap();

    // Semiannual frequency → m = 2 by default in helper
    let expected = d_mac / (1.0 + ytm / 2.0);
    assert!((d_mod - expected).abs() < 1e-4); // d_mod = d_mac / (1 + ytm/2) is an analytical identity
}

#[test]
fn test_convexity_matches_numerical_second_derivative() {
    // Closed-form convexity should align with a numerical second derivative.
    use finstack_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let bond = Bond::fixed(
        "CONV1",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.6)])
        .build()
        .unwrap();
    let market = finstack_core::market_data::context::MarketContext::new().insert(curve);

    let res = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::Convexity],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytm = *res.measures.get("ytm").unwrap();
    let conv_closed = *res.measures.get("convexity").unwrap();

    let flows = bond.dated_cashflows(&market, as_of).unwrap();
    let quote_date = as_of + time::Duration::days(bond.settlement_days().unwrap_or(0) as i64);
    let dy = 1e-4;
    let p0 = finstack_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_ytm(
        &bond, &flows, quote_date, ytm,
    )
    .unwrap();
    let p_up = finstack_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_ytm(
        &bond,
        &flows,
        quote_date,
        ytm + dy,
    )
    .unwrap();
    let p_dn = finstack_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_ytm(
        &bond,
        &flows,
        quote_date,
        ytm - dy,
    )
    .unwrap();
    let conv_numeric = (p_up + p_dn - 2.0 * p0) / (p0 * dy * dy) / 100.0;

    let rel = (conv_closed - conv_numeric).abs() / conv_numeric.abs().max(1e-9);
    assert!(rel < 1e-3, "convexity rel diff {}", rel);
}

fn callable_risk_bond(as_of: finstack_core::dates::Date) -> Bond {
    use finstack_valuations::instruments::PricingOverrides;
    let mut bond = Bond::fixed(
        "CALLABLE-RISK",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2032 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 01),
            end_date: date!(2028 - 01 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.pricing_overrides = PricingOverrides::default()
        .with_quoted_clean_price(99.0)
        .with_implied_vol(0.01);
    bond
}

fn callable_risk_market(
    as_of: finstack_core::dates::Date,
) -> finstack_core::market_data::context::MarketContext {
    use finstack_core::market_data::term_structures::DiscountCurve;
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (7.0, 0.78)])
        .build()
        .unwrap();
    finstack_core::market_data::context::MarketContext::new().insert(curve)
}

#[test]
fn test_callable_quoted_bond_defaults_to_bullet_discountable_risk_basis() {
    let as_of = date!(2025 - 01 - 01);
    let callable = callable_risk_bond(as_of);
    let mut bullet = callable.clone();
    bullet.call_put = None;
    let market = callable_risk_market(as_of);

    let callable_result = callable
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::DurationMod,
                MetricId::Convexity,
                MetricId::Dv01,
                MetricId::YieldDv01,
            ],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("callable quoted bond risk metrics should compute");
    let bullet_result = bullet
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::DurationMod,
                MetricId::Convexity,
                MetricId::Dv01,
                MetricId::YieldDv01,
            ],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("bullet equivalent risk metrics should compute");

    for key in ["duration_mod", "convexity", "dv01", "yield_dv01"] {
        let callable_value = callable_result.measures[key];
        let bullet_value = bullet_result.measures[key];
        assert!(
            (callable_value - bullet_value).abs() < 1e-8,
            "{key} should default to bullet/workout basis: callable={callable_value}, bullet={bullet_value}"
        );
    }
}

#[test]
fn test_callable_quoted_bond_can_request_callable_oas_risk_basis() {
    use finstack_valuations::instruments::BondRiskBasis;

    let as_of = date!(2025 - 01 - 01);
    let bullet_basis = callable_risk_bond(as_of);
    let mut callable_basis = callable_risk_bond(as_of);
    callable_basis.pricing_overrides = callable_basis
        .pricing_overrides
        .with_bond_risk_basis(BondRiskBasis::CallableOas);
    let market = callable_risk_market(as_of);

    let bullet_result = bullet_basis
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::DurationMod,
                MetricId::Convexity,
                MetricId::Dv01,
                MetricId::YieldDv01,
            ],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("default risk metrics should compute");
    let callable_result = callable_basis
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::DurationMod,
                MetricId::Convexity,
                MetricId::Dv01,
                MetricId::YieldDv01,
            ],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("callable-oas risk metrics should compute");

    assert!(
        (callable_result.measures["duration_mod"] - bullet_result.measures["duration_mod"]).abs()
            > 1e-4,
        "callable_oas basis should use option-aware duration"
    );
    assert!(
        (callable_result.measures["dv01"] - bullet_result.measures["dv01"]).abs() > 1e-2,
        "callable_oas basis should use option-aware DV01"
    );
}

/// Item 3a regression: a callable bond on the default (`BulletDiscountable` /
/// Workout) risk basis with **no market price quote** must not report a
/// straight-bullet curve-bump DV01 that silently discards the embedded option.
///
/// On the Workout basis `DurationMod`, `Convexity` and `YieldDv01` are all
/// yield-basis measures (and pick up the call via the workout path when a
/// price is quoted). `Dv01` must be consistent with that family — the dollar
/// analogue of modified duration — rather than a curve-bump of a structurally
/// mutated `call_put = None` clone. So on this basis `Dv01 == YieldDv01`.
#[test]
fn test_callable_no_quote_default_basis_dv01_is_yield_basis() {
    let as_of = date!(2025 - 01 - 01);

    let mut callable = Bond::fixed(
        "CALLABLE-NOQUOTE",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2032 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    callable.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 01),
            end_date: date!(2028 - 01 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    // Deliberately NO price quote: exercise the no-price-driver branch.

    let market = callable_risk_market(as_of);

    let result = callable
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01, MetricId::YieldDv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("callable no-quote default-basis risk metrics should compute");

    let dv01 = result.measures["dv01"];
    let yield_dv01 = result.measures["yield_dv01"];

    assert!(
        (dv01 - yield_dv01).abs() < 1e-8,
        "default-basis callable DV01 must equal the yield-basis DV01 (no \
         straight-bullet curve-bump): dv01={dv01}, yield_dv01={yield_dv01}"
    );
    // Sanity: both are negative for a long bond.
    assert!(dv01 < 0.0, "long-bond DV01 should be negative, got {dv01}");
}
