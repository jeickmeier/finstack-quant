use super::{
    calculate_accrued_interest, calculate_convertible_greeks, calculate_parity,
    compute_conversion_value, prepare_for_pricing, price_convertible_bond, settlement_date,
    ConvertibleBondValuator, ConvertibleTreeType,
};
use crate::cashflow::builder::specs::{CouponType, FixedCouponSpec};
use crate::instruments::fixed_income::convertible::ConvertibleBond;
use crate::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use time::Month;

fn create_test_bond() -> ConvertibleBond {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let conversion_spec = ConversionSpec {
        ratio: Some(10.0),
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let fixed_coupon = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),

            day_count: DayCount::Act365F,

            business_day_convention: BusinessDayConvention::Following,

            calendar_id: "weekends_only".to_string(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
            roll_rule: crate::cashflow::builder::specs::RollRule::None,
        },
    };

    ConvertibleBond {
        id: "TEST_CONVERTIBLE".to_string().into(),
        notional: Money::from((1000_i64, Currency::USD)),
        issue_date: issue,
        maturity,
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        settlement_days: None,
        recovery_rate: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: Some(fixed_coupon),
        floating_coupon: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn create_test_market_context() -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (10.0, 0.90)])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .build()
        .expect("should succeed");

    MarketContext::new()
        .insert(discount_curve)
        .insert_price("AAPL", MarketScalar::Unitless(150.0))
        .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02))
}

#[test]
fn coupon_on_valuation_date_is_not_added_to_tree_step_zero() {
    let as_of = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
    let market = create_test_market_context();
    let bond = create_test_bond();
    let inputs = prepare_for_pricing(&bond, &market, as_of).expect("pricing inputs");
    let valuator = ConvertibleBondValuator::new(
        &bond,
        &inputs.cashflow_schedule,
        inputs.time_to_maturity,
        50,
        as_of,
        &market,
        inputs.volatility,
    )
    .expect("valuator");

    assert!(
        !valuator.coupon_map.contains_key(&0),
        "coupon dated exactly on as_of must not be added at tree step 0"
    );
}

/// H1 pin test (drift-discount consistency on a non-flat curve).
///
/// A deep-in-the-money zero-coupon convertible (conversion value 50x the
/// bond floor), zero dividend yield, no calls/puts/credit, convertible
/// only at maturity, priced on a markedly upward-sloping curve (2% at 1y
/// rising to 4% at 5y) must price at `conversion_ratio * S0`: the
/// discounted risk-neutral expectation of `ratio * S_T` is a martingale,
/// so any deviation is a drift-discount mismatch. Conversion is windowed
/// to maturity so early exercise cannot mask the mismatch.
///
/// Before the per-step drift fix the tree used the single t=0 short rate
/// (~2%) for the drift while discounting at the full curve (~4% average),
/// undervaluing this bond by ~9.8% (measured: 45,100.59 vs 50,000).
#[test]
fn deep_itm_convertible_matches_parity_on_non_flat_curve() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let mut bond = create_test_bond();
    bond.fixed_coupon = None; // zero-coupon: isolates the equity claim
    bond.conversion.policy = ConversionPolicy::Window {
        start: bond.maturity,
        end: bond.maturity,
    };

    // Upward-sloping zero curve: 2% at 1y -> 4% at 5y.
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.02_f64).exp()),
            (5.0, (-0.04_f64 * 5.0).exp()),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .build()
        .expect("curve");

    // Spot 5000 with ratio 10 => conversion value 50,000 >> 1,000 face,
    // so the redemption floor contributes only a ~1e-12 tail probability.
    let market = MarketContext::new()
        .insert(curve)
        .insert_price("AAPL", MarketScalar::Unitless(5000.0))
        .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.0));

    let expected = 10.0 * 5000.0;
    for tree in [
        ConvertibleTreeType::Binomial(200),
        ConvertibleTreeType::Trinomial(200),
    ] {
        let price = price_convertible_bond(&bond, &market, tree, issue)
            .expect("should price")
            .amount();
        let rel_err = (price - expected).abs() / expected;
        assert!(
            rel_err < 1e-7,
            "deep-ITM convertible must equal parity {expected} on a non-flat curve \
             (martingale property); got {price} with {tree:?} (rel err {rel_err:.3e})"
        );
    }
}

/// Dividend-protection pin test (martingale identity).
///
/// Same construction as `deep_itm_convertible_matches_parity_on_non_flat_curve`
/// (deep-ITM zero-coupon convertible, conversion windowed to maturity, no
/// calls/puts/credit, upward-sloping curve), but with a nonzero dividend
/// yield. With FULL protection the conversion ratio accretes at the
/// dividend yield, so the discounted conversion claim is
/// `E[e^{-∫r} · ratio₀·e^{qT} · S_T] = ratio₀ · S₀` exactly (the stock
/// drifts at `r − q`): the model price must equal `ratio₀ · S₀`
/// INDEPENDENT of `q`, for both `AdjustRatio` and `AdjustPrice` (they are
/// the same mechanism). Without protection, `q = 6%` must drag the price
/// materially below parity by `e^{-qT}` (≈ 26% over ~5y).
#[test]
fn dividend_protection_restores_parity_independent_of_yield() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let mut bond = create_test_bond();
    bond.fixed_coupon = None; // zero-coupon: isolates the equity claim
    bond.conversion.policy = ConversionPolicy::Window {
        start: bond.maturity,
        end: bond.maturity,
    };

    // Upward-sloping zero curve: 2% at 1y -> 4% at 5y (non-flat is fine;
    // the identity only needs drift-discount consistency).
    let market_with_yield = |q: f64| {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.02_f64).exp()),
                (5.0, (-0.04_f64 * 5.0).exp()),
            ])
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .build()
            .expect("curve");
        MarketContext::new()
            .insert(curve)
            .insert_price("AAPL", MarketScalar::Unitless(5000.0))
            .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
            .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(q))
    };

    let expected = 10.0 * 5000.0;

    // Full protection: price pins to parity regardless of dividend yield.
    for adjustment in [
        DividendAdjustment::AdjustRatio,
        DividendAdjustment::AdjustPrice,
    ] {
        for q in [0.0, 0.06] {
            bond.conversion.dividend_adjustment = adjustment.clone();
            let market = market_with_yield(q);
            for tree in [
                ConvertibleTreeType::Binomial(200),
                ConvertibleTreeType::Trinomial(200),
            ] {
                let price = price_convertible_bond(&bond, &market, tree, issue)
                    .expect("should price")
                    .amount();
                let rel_err = (price - expected).abs() / expected;
                assert!(
                    rel_err < 1e-7,
                    "fully protected deep-ITM convertible must equal parity {expected} \
                     independent of q={q} ({adjustment:?}, {tree:?}); got {price} \
                     (rel err {rel_err:.3e})"
                );
            }
        }
    }

    // No protection: q = 6% leaks value from the conversion option; the
    // price must fall to ~parity * e^{-qT} (materially below parity).
    bond.conversion.dividend_adjustment = DividendAdjustment::None;
    let market = market_with_yield(0.06);
    let unprotected =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(200), issue)
            .expect("should price")
            .amount();
    let ttm = DayCount::Act365F
        .year_fraction(issue, bond.maturity, Default::default())
        .expect("year fraction");
    let dragged = expected * (-0.06 * ttm).exp();
    assert!(
        unprotected < 0.8 * expected,
        "unprotected convertible with q=6% must sit materially below parity \
         {expected}; got {unprotected}"
    );
    let rel_err = (unprotected - dragged).abs() / dragged;
    assert!(
        rel_err < 1e-3,
        "unprotected price should match the dividend-dragged parity {dragged}; \
         got {unprotected} (rel err {rel_err:.3e})"
    );
}

#[test]
fn test_convertible_bond_parity() {
    let bond = create_test_bond();
    let parity = calculate_parity(&bond, 150.0).expect("parity");
    assert!((parity - 1.5).abs() < 1e-9);
}

#[test]
fn test_convertible_bond_pricing() {
    let bond = create_test_bond();
    let market_context = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let price = price_convertible_bond(
        &bond,
        &market_context,
        ConvertibleTreeType::Binomial(50),
        as_of,
    );

    assert!(price.is_ok());
    let price = price.expect("should succeed");

    let conversion_value = 150.0 * 10.0;
    assert!(price.amount() >= conversion_value);
    assert!(price.amount() > 1000.0 && price.amount() < 2000.0);
}

#[test]
fn test_convertible_pricing_at_maturity_uses_payoff() {
    let bond = create_test_bond();
    let market_context = create_test_market_context();
    let as_of = bond.maturity;

    let price = price_convertible_bond(
        &bond,
        &market_context,
        ConvertibleTreeType::Binomial(10),
        as_of,
    )
    .expect("should price");

    let conversion_value = 150.0 * 10.0;
    assert!((price.amount() - conversion_value).abs() < 1e-6);
}

#[test]
fn test_convertible_greeks_calculation() {
    let bond = create_test_bond();
    let market_context = create_test_market_context();

    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let greeks = calculate_convertible_greeks(
        &bond,
        &market_context,
        ConvertibleTreeType::Binomial(50),
        Some(0.01),
        as_of,
    );

    assert!(greeks.is_ok());
    let greeks = greeks.expect("should succeed");

    assert!(greeks.delta > 0.0);
    assert!(greeks.gamma >= -1e-6);
    assert!(greeks.price > 1000.0);
}

/// Item 8 regression: theta must roll the discount curve to `t+1d`, not
/// reprice at `t+1d` against the curve still anchored at `t`.
///
/// The reported theta must equal an explicit reprice against the market
/// rolled to the next day. Comparing it with an unrolled reprice is not a
/// valid discriminator because date-relative discounting can make the two
/// values identical even on a steep curve.
#[test]
fn theta_rolls_the_discount_curve() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let as_of = issue;
    let bond = create_test_bond();

    // A steeply-sloped discount curve so curve roll-down is material.
    let steep_curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([
            (0.0, 1.0),
            (0.5, 0.95),
            (1.0, 0.88),
            (5.0, 0.55),
            (10.0, 0.30),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .build()
        .expect("steep curve");
    let market = MarketContext::new()
        .insert(steep_curve)
        .insert_price("AAPL", MarketScalar::Unitless(150.0))
        .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02));

    let tree = ConvertibleTreeType::Binomial(80);
    let greeks =
        calculate_convertible_greeks(&bond, &market, tree, Some(0.01), as_of).expect("greeks");
    assert!(greeks.theta.is_finite(), "theta must be finite");

    let next_day = as_of.next_day().expect("next day");
    let base_price = greeks.price;
    let rolled_market = market.roll_forward(1).expect("market roll");
    let rolled_price = price_convertible_bond(&bond, &rolled_market, tree, next_day)
        .expect("rolled price")
        .amount();
    let expected_theta = rolled_price - base_price;

    assert!(
        (greeks.theta - expected_theta).abs() < 1e-10,
        "theta {} should equal the explicit rolled-market theta {expected_theta}",
        greeks.theta
    );
}

#[test]
fn test_accrued_interest() {
    let bond = create_test_bond();
    // Mid-period: ~3 months into a 6-month coupon period
    let mid = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
    let accrued =
        calculate_accrued_interest(&bond, &MarketContext::new(), mid).expect("should compute");
    // The canonical schedule-driven linear engine gives N × r × elapsed year
    // fraction: 1000 × 5% × 90/365 from Jan 1 through Apr 1.
    let expected = 1_000.0 * 0.05 * 90.0 / 365.0;
    assert!(
        (accrued - expected).abs() < 1e-10,
        "accrued={accrued}, expected={expected}"
    );
}

#[test]
fn settlement_date_uses_coupon_holiday_calendar() {
    let mut bond = create_test_bond();
    bond.settlement_days = Some(1);
    bond.fixed_coupon
        .as_mut()
        .expect("test bond has fixed coupon")
        .schedule
        .calendar_id = "usny".to_string();

    // Thursday July 3, 2025 + one USNY business day skips Independence Day
    // and the weekend, settling Monday July 7.
    let trade = Date::from_calendar_date(2025, Month::July, 3).expect("valid date");
    let expected = Date::from_calendar_date(2025, Month::July, 7).expect("valid date");
    assert_eq!(
        settlement_date(&bond, trade).expect("USNY settlement should resolve"),
        expected
    );
}

#[test]
fn test_mandatory_conversion_forced_at_loss() {
    // DECS/PERCS: mandatory conversion even when conversion_value < redemption.
    // Spot=50, ratio=10, notional=1000 → conversion_value=500 < 1000.
    // Mandatory bond at maturity should price at conversion value, not redemption.
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let mut bond = create_test_bond();
    bond.conversion.policy = ConversionPolicy::MandatoryOn(maturity);

    // Market with OTM spot: conversion_value = 50 * 10 = 500 < 1000 face
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (10.0, 0.90)])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .build()
        .expect("should succeed");

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert_price("AAPL", MarketScalar::Unitless(50.0))
        .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02));

    // At maturity: forced conversion at loss
    let price_at_mat =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(10), maturity)
            .expect("should price");

    // conversion_value = 50 * 10 = 500 (must convert, can't choose 1000 redemption)
    assert!(
        (price_at_mat.amount() - 500.0).abs() < 1.0,
        "Mandatory at maturity should force conversion: got {}",
        price_at_mat.amount()
    );

    // Before maturity: should be below straight bond floor due to forced conversion risk
    let price_before =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(50), issue)
            .expect("should price");

    assert!(
        price_before.amount() < 1000.0,
        "Mandatory OTM bond should price below par: got {}",
        price_before.amount()
    );
}

/// Item 2 regression: the call branch must not force a conversion at a
/// node where conversion is *not* permitted.
///
/// Construct a callable bond whose conversion is gated on
/// `ChangeOfControl` — an event the tree cannot model, so
/// `conversion_allowed` is `false` at every node. With a very high equity
/// spot the conversion *value* (ratio·spot) is enormous, but the holder
/// can never actually convert. The bond is economically a callable
/// straight bond: its value is capped by the call price and must stay far
/// below the conversion value.
///
/// The pre-fix code used `conversion_val` in the call branch's cash/equity
/// split unconditionally, so at a call node it set `(conversion_val, 0)` —
/// forcing a conversion that is not allowed and inflating the price to the
/// conversion value. The fix gates the conversion response on
/// `can_convert`, so the called bond correctly redeems in cash.
#[test]
fn call_branch_does_not_force_disallowed_conversion() {
    use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
    use crate::instruments::fixed_income::convertible::ConversionEvent;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
    let as_of = issue;

    let mut bond = create_test_bond();
    // Conversion only on a change-of-control — never modellable in the
    // tree, so `conversion_allowed` is false at every node.
    bond.conversion.policy = ConversionPolicy::UponEvent(ConversionEvent::ChangeOfControl);
    // Callable for the whole life at 102% of par.
    bond.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.01);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: issue,
            end_date: maturity,
            price_pct_of_par: 102.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });

    // Very high spot: conversion value (ratio 10 × spot 5000 = 50,000) is
    // ~50× the 1,000 face. A callable non-convertible bond must ignore it.
    let base_date = issue;
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (10.0, 0.90)])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .build()
        .expect("curve");
    let market = MarketContext::new()
        .insert(discount_curve)
        .insert_price("AAPL", MarketScalar::Unitless(5000.0))
        .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02));

    let price = price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(60), as_of)
        .expect("should price");

    let conversion_value = 10.0 * 5000.0; // ratio × spot = 50,000
    let call_price = 1000.0 * 1.02; // 102% of par = 1,020

    // The bond must be valued as a callable straight bond: nowhere near
    // the conversion value, and capped around the call price (plus the
    // PV of coupons until the call).
    assert!(
        price.amount() < conversion_value / 10.0,
        "callable non-convertible bond priced at {} — far too close to the \
         conversion value {conversion_value}; the call branch is forcing a \
         disallowed conversion",
        price.amount()
    );
    // Sanity: it should be a sensible callable-bond value, near the call
    // price (the issuer calls the deep-premium bond), well under 2× par.
    assert!(
        price.amount() > 0.0 && price.amount() < 2.0 * call_price,
        "callable straight-bond value out of range: {}",
        price.amount()
    );
}

#[test]
fn test_thirty_360_day_count_corporate_convention() {
    // Verify that 30/360 day count (US corporate standard) works correctly.
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let conversion_spec = ConversionSpec {
        ratio: Some(10.0),
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution: super::super::AntiDilutionPolicy::None,
        dividend_adjustment: super::super::DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let fixed_coupon = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            // US corporate convention
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: crate::cashflow::builder::specs::RollRule::None,
        },
    };

    let bond = ConvertibleBond {
        id: "TEST_30360".to_string().into(),
        notional: Money::from((1000_i64, Currency::USD)),
        issue_date: issue,
        maturity,
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        settlement_days: None,
        recovery_rate: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: Some(fixed_coupon),
        floating_coupon: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let market = create_test_market_context();
    let as_of = issue;

    let price = price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(50), as_of)
        .expect("30/360 should price successfully");

    // Same economics as Act365F, should be in similar range
    let conversion_value = 150.0 * 10.0;
    assert!(price.amount() >= conversion_value);
    assert!(
        price.amount() > 1000.0 && price.amount() < 2000.0,
        "30/360 price out of range: {}",
        price.amount()
    );

    // Verify accrued interest works with 30/360
    let mid = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
    let accrued =
        calculate_accrued_interest(&bond, &MarketContext::new(), mid).expect("should compute");
    assert!(
        accrued > 5.0 && accrued < 20.0,
        "30/360 accrued should be reasonable: {}",
        accrued
    );
}

/// Item 11 regression: the trinomial node-spot grid must be built with the
/// proper middle factor, `S₀ · up^net · middle^(step − net)`.
///
/// For a recombining trinomial the recombination identity is
/// `up·down = middle²`. The previous `up^max(net,0)·down^max(-net,0)` form
/// dropped the middle factor and is only valid when `up·down = 1`. With
/// the corrected formula the trinomial spot grid is well-formed, so a
/// trinomial price must converge to the binomial price for the same bond
/// (both are consistent lattice discretizations of the same process).
#[test]
fn trinomial_spot_grid_well_formed_matches_binomial() {
    let bond = create_test_bond();
    let market = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let binomial =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(400), as_of)
            .expect("binomial price")
            .amount();
    let trinomial =
        price_convertible_bond(&bond, &market, ConvertibleTreeType::Trinomial(400), as_of)
            .expect("trinomial price")
            .amount();

    // Both lattices discretize the same process; with 400 steps they must
    // agree closely. A malformed trinomial grid would diverge sharply.
    let rel_diff = (binomial - trinomial).abs() / binomial.max(1.0);
    assert!(
        rel_diff < 0.01,
        "trinomial price {trinomial} should match binomial {binomial} \
         within 1% (rel diff {rel_diff:.4}); a malformed spot grid would diverge"
    );
}

#[test]
fn mandatory_variable_inverted_bounds_rejected_at_pricing() {
    // Data-entry inversion: lower > upper. Without the new guard, the
    // three-regime payoff in compute_conversion_value would silently fall
    // into the wrong branch and produce non-monotone PV. Pricing must
    // reject up front with a Validation error naming both bounds.
    let mut bond = create_test_bond();
    let conversion_date = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
    bond.conversion.policy = ConversionPolicy::MandatoryVariable {
        conversion_date,
        upper_conversion_price: 80.0,  // intentionally < lower
        lower_conversion_price: 120.0, // intentionally > upper
    };

    let market = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let err = price_convertible_bond(&bond, &market, ConvertibleTreeType::Binomial(50), as_of)
        .expect_err("inverted bounds must be rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("inverted") && msg.contains("120") && msg.contains("80"),
        "error must name the inverted bounds, got: {msg}"
    );
}

#[test]
fn mandatory_variable_inverted_bounds_rejected_in_compute_conversion_value() {
    // Direct call site (used at-maturity early-exit and reachable from
    // greeks recomputation).
    let mut bond = create_test_bond();
    let conversion_date = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
    bond.conversion.policy = ConversionPolicy::MandatoryVariable {
        conversion_date,
        upper_conversion_price: 50.0,
        lower_conversion_price: 200.0,
    };
    let err = compute_conversion_value(&bond, 100.0).expect_err("inverted bounds must be rejected");
    assert!(format!("{err}").contains("inverted"));
}

/// Regression test: recovery_rate must be validated explicitly, not
/// silently clamped to [0.0, 1.0]. Out-of-range or non-finite values
/// previously masked invalid inputs (e.g., a typo of 1.5 producing a
/// silently-changed 1.0 PV).
#[test]
fn convertible_recovery_rate_out_of_bounds_errors() {
    let mut bond = create_test_bond();
    let market = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::June, 1).expect("valid date");
    let tree_type = ConvertibleTreeType::Binomial(50);

    // Above 1.0 — previously clamped to 1.0, now rejected.
    bond.recovery_rate = Some(1.5);
    let err = price_convertible_bond(&bond, &market, tree_type, as_of)
        .expect_err("recovery_rate=1.5 must be rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("recovery_rate") && msg.contains("TEST_CONVERTIBLE"),
        "error must mention recovery_rate and bond id; got: {msg}"
    );

    // Negative — previously clamped to 0.0, now rejected.
    bond.recovery_rate = Some(-0.1);
    let _ = price_convertible_bond(&bond, &market, tree_type, as_of)
        .expect_err("negative recovery_rate must be rejected");

    // NaN — previously clamped to 0.0, now rejected.
    bond.recovery_rate = Some(f64::NAN);
    let _ = price_convertible_bond(&bond, &market, tree_type, as_of)
        .expect_err("NaN recovery_rate must be rejected");

    // None is valid when no credit adjustment is requested.
    bond.recovery_rate = None;
    let _ = price_convertible_bond(&bond, &market, tree_type, as_of)
        .expect("None recovery_rate is valid without a credit curve");

    bond.credit_curve_id = Some("USD-CREDIT".into());
    let err = price_convertible_bond(&bond, &market, tree_type, as_of)
        .expect_err("a credit curve requires explicit recovery");
    assert!(format!("{err}").contains("explicit recovery_rate"));
}

/// Regression: a put exercisable exactly at maturity must be honored by the
/// terminal step of the tree. Previously the terminal payoff considered only
/// conversion vs. face redemption, so a maturity-dated put struck above par
/// (e.g. an accreting put at 105) was silently dropped and the bond priced
/// identically to the unputtable bond.
#[test]
fn put_at_maturity_floors_terminal_payoff() {
    use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule};

    let as_of = Date::from_calendar_date(2025, Month::June, 1).expect("valid date");
    let market = create_test_market_context();
    let tree_type = ConvertibleTreeType::Binomial(200);

    // Deep OTM equity so conversion never binds: conversion value = 5 * 10 = 50
    // against a 1000 face. The bond is a pure debt instrument here.
    let market = market.insert_price("AAPL", MarketScalar::Unitless(5.0));

    let plain_bond = create_test_bond();
    let pv_plain = price_convertible_bond(&plain_bond, &market, tree_type, as_of)
        .expect("plain bond prices")
        .amount();

    let mut puttable_bond = create_test_bond();
    let mut schedule = CallPutSchedule::default();
    schedule.puts.push(CallPut {
        start_date: puttable_bond.maturity,
        end_date: puttable_bond.maturity,
        price_pct_of_par: 105.0, // accreting put above par at maturity
        make_whole: None,
    });
    puttable_bond
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.01);
    puttable_bond.call_put = Some(schedule);
    let pv_puttable = price_convertible_bond(&puttable_bond, &market, tree_type, as_of)
        .expect("puttable bond prices")
        .amount();

    // The put lifts the maturity payoff from 1000 to 1050; discounted at the
    // curve (~1% for ~4.6y) the uplift is close to 50 * DF ≈ 47-48.
    let uplift = pv_puttable - pv_plain;
    assert!(
        uplift > 40.0 && uplift < 50.0,
        "maturity put at 105 must lift PV by ~PV(50); got uplift {uplift} \
         (plain {pv_plain}, puttable {pv_puttable})"
    );
}
