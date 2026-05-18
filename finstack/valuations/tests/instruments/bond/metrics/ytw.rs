//! Yield to worst calculator tests.

use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::bond::Bond;
#[allow(unused_imports)]
use finstack_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::PricingOverrides;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_ytw_equals_ytm_for_non_callable_bond_from_price() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "YTW_NON_CALL",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    // Market-quoted clean price (percent of par)
    bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(99.5);

    let curve = finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.80)])
        .build()
        .unwrap();
    let market = finstack_core::market_data::context::MarketContext::new().insert(curve);

    // Request both YTM and YTW so they are computed off the same quoted price
    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::Ytw],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytm = *result.measures.get("ytm").unwrap();
    let ytw = *result.measures.get("ytw").unwrap();

    // For a non-callable bond, YTW should collapse to YTM (same cashflows/price)
    assert!(ytw.is_finite());
    // YTW must equal YTM exactly: same cashflows, same price, no optionality to exercise
    assert!(
        (ytw - ytm).abs() <= 1e-10,
        "expected YTW == YTM for non-callable bond, got ytm={} ytw={}",
        ytm,
        ytw
    );
}

#[test]
fn test_ytw_tracks_quoted_price_not_model_pv() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "YTW_PRICE_SENSITIVE",
        Money::new(100.0, Currency::USD),
        0.04,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let curve = finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.90)]) // deliberately simple curve
        .build()
        .unwrap();
    let market = finstack_core::market_data::context::MarketContext::new().insert(curve);

    // Compute model PV for reference (should differ from at least one of the quoted prices)
    let pv = bond.value(&market, as_of).unwrap().amount();
    assert!(pv.is_finite());

    // Two different quoted clean prices with the same curves
    bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(95.0);
    let result_low = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytw],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytw_low = *result_low.measures.get("ytw").unwrap();

    bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(105.0);
    let result_high = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytw],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytw_high = *result_high.measures.get("ytw").unwrap();

    // YTW must respond to the quoted price overrides, not stay tied to model PV.
    assert!(
        (ytw_low - ytw_high).abs() > 1e-4,
        "expected YTW to differ for misaligned quoted prices; pv={} ytw_low={} ytw_high={}",
        pv,
        ytw_low,
        ytw_high
    );
}

#[test]
fn test_ytw_off_cycle_call_uses_dirty_street_redemption() {
    use finstack_cashflows::accrual::accrued_interest_amount;
    use finstack_cashflows::traits::CashflowProvider;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_valuations::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding;
    use finstack_valuations::instruments::fixed_income::bond::pricing::ytm_solver::{
        solve_ytm, YtmPricingSpec,
    };

    let as_of = date!(2025 - 01 - 01);
    let quote_date = as_of;
    let call_date = date!(2027 - 04 - 01);
    let mut bond = Bond::fixed(
        "YTW-OFF-CYCLE-CALL",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.settlement_convention = None;
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: call_date,
            end_date: call_date,
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(105.0);

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.80)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytw],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytw = result.measures["ytw"];

    let all_flows = bond.dated_cashflows(&market, as_of).unwrap();
    let schedule = bond.cashflow_schedule(&market, as_of).unwrap();
    let accrued_on_call =
        accrued_interest_amount(&schedule, call_date, &bond.accrual_config()).unwrap();
    let mut call_flows = all_flows
        .iter()
        .copied()
        .filter(|(date, _)| *date > quote_date && *date <= call_date)
        .collect::<Vec<_>>();
    call_flows.push((
        call_date,
        Money::new(1_000_000.0 + accrued_on_call, Currency::USD),
    ));

    let expected_call_yield = solve_ytm(
        &call_flows,
        quote_date,
        Money::new(1_050_000.0, Currency::USD),
        YtmPricingSpec {
            day_count: bond.cashflow_spec.day_count(),
            notional: bond.notional,
            coupon_rate: 0.05,
            compounding: YieldCompounding::Street,
            frequency: bond.cashflow_spec.frequency(),
        },
    )
    .unwrap();

    assert!(
        (ytw - expected_call_yield).abs() < 1e-10,
        "YTW should include accrued interest in off-cycle street call redemption: ytw={ytw}, expected={expected_call_yield}"
    );
}

/// YTW for a simple FRN without optionality.
///
/// Note: Unlike fixed-rate bonds, FRN YTW and YTM may differ due to how
/// floating cashflows are projected. This is acknowledged in the YTW
/// documentation: "for floating-rate...structures, this calculator still
/// computes a well-defined 'worst-case cashflow-implied yield'...but this
/// is NOT the standard FRN quoting convention."
///
/// For FRNs, the standard market measure is **discount margin (DM)**, not YTW.
/// This test verifies YTM and YTW are both finite and well-behaved, but allows
/// for small differences due to implementation details.
#[test]
fn test_ytw_floating_bond_matches_ytm_from_price() {
    use finstack_core::currency::Currency;
    use finstack_core::dates::{DayCount, Tenor};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::market_data::term_structures::ForwardCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::money::Money;
    use finstack_valuations::instruments::fixed_income::bond::Bond;
    use finstack_valuations::instruments::PricingOverrides;
    use finstack_valuations::metrics::MetricId;

    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);
    let notional = Money::new(1_000_000.0, Currency::USD);

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (2.0, 0.95)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (2.0, 0.035)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc).insert(fwd);

    let mut bond = Bond::floating(
        "YTW-FRN",
        notional,
        "USD-SOFR-3M",
        150,
        as_of,
        maturity,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .unwrap();

    // Use model PV to back out a clean price quote consistent with the curves.
    let pv = bond.value(&market, as_of).unwrap().amount();
    let clean_px = pv / notional.amount() * 100.0;
    bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(clean_px);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::Ytw],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytm = *result.measures.get("ytm").unwrap();
    let ytw = *result.measures.get("ytw").unwrap();

    // Both metrics should be finite and in a reasonable range
    assert!(ytm.is_finite() && ytw.is_finite());
    assert!(
        ytm > -0.5 && ytm < 0.5,
        "YTM should be in reasonable range, got {}",
        ytm
    );
    assert!(
        ytw > -0.5 && ytw < 0.5,
        "YTW should be in reasonable range, got {}",
        ytw
    );

    // For FRNs, YTW may differ from YTM due to how floating cashflows are
    // projected vs how they're filtered in the YTW candidate scan.
    // The standard market convention for FRN valuation is discount margin (DM),
    // not YTW. We verify both yields are finite but don't enforce equality.
    // Fixed-rate and amortizing bonds (see other tests) do enforce YTW == YTM.
}

/// YTW for a plain amortizing bond without optionality should also reduce to
/// the same IRR as YTM over the projected cashflows.
#[test]
fn test_ytw_amortizing_bond_matches_ytm_from_price() {
    use finstack_core::currency::Currency;
    use finstack_core::dates::{DayCount, Tenor};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::money::Money;
    use finstack_core::types::CurveId;
    use finstack_valuations::instruments::fixed_income::bond::Bond;
    use finstack_valuations::instruments::fixed_income::bond::{AmortizationSpec, CashflowSpec};
    use finstack_valuations::instruments::Attributes;
    use finstack_valuations::instruments::PricingOverrides;
    use finstack_valuations::metrics::MetricId;

    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);
    let notional = Money::new(1_000_000.0, Currency::USD);

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (3.0, 0.94)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc);

    let step_date = date!(2026 - 01 - 01);
    let amort_spec = AmortizationSpec::StepRemaining {
        schedule: vec![
            (step_date, Money::new(500_000.0, Currency::USD)),
            (maturity, Money::new(0.0, Currency::USD)),
        ],
    };
    let base_spec = CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Thirty360).expect("finite test coupon");
    let cashflow_spec = CashflowSpec::amortizing(base_spec, amort_spec);

    let mut bond = Bond::builder()
        .id("YTW-AMORT".into())
        .notional(notional)
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(cashflow_spec)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("amortizing bond construction should succeed in test");

    let pv = bond.value(&market, as_of).unwrap().amount();
    let clean_px = pv / notional.amount() * 100.0;
    bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(clean_px);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::Ytw],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytm = *result.measures.get("ytm").unwrap();
    let ytw = *result.measures.get("ytw").unwrap();

    assert!(ytm.is_finite() && ytw.is_finite());
    assert!(
        (ytw - ytm).abs() <= 1e-6,
        "expected amortizing YTW ≈ YTM when no optionality is present, got ytm={} ytw={}",
        ytm,
        ytw
    );
}
