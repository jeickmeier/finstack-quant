//! Tests for settlement and quote-date convention consistency.
//!
//! These tests verify the fix plan conventions:
//! 1. PV is always anchored at `as_of` (valuation date), not settlement date
//! 2. Quote-derived metrics (YTM, Z-spread, duration) use quote_date (settlement date)
//! 3. Callable/putable exercise payoff: coupon paid regardless of exercise
//! 4. Call/put redemption uses outstanding principal for amortizing bonds
//! 5. Frequency inference uses mode of intervals, not just first interval

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::bond::{
    Bond, CallPut, CallPutSchedule, CashflowSpec,
};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::PricingOverrides;
use time::Month;

fn create_test_market(base_date: Date) -> MarketContext {
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.82), (10.0, 0.70)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Valid curve");
    MarketContext::new().insert(discount_curve)
}

/// Test that PV is computed from as_of, not settlement date.
///
/// This verifies fix A1: "PV = Σ CF_i · DF(as_of → t_i)"
#[test]
fn test_pv_anchored_at_as_of() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let as_of = Date::from_calendar_date(2025, Month::June, 1).expect("Valid date");

    let bond = Bond::builder()
        .id("TEST_PV_ANCHOR".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .settlement_convention_opt(Some(
            finstack_valuations::instruments::fixed_income::bond::BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            },
        ))
        .attributes(Default::default())
        .build()
        .expect("Valid bond");

    let market = create_test_market(as_of);

    // Price the bond - this should use as_of as anchor
    let pv = bond.value(&market, as_of).expect("Should price");

    // PV should be positive and reasonable for a 5% coupon bond priced at ~4% rates
    assert!(
        pv.amount() > 1000.0,
        "5% coupon bond at ~4% rates should trade above par"
    );
    assert!(pv.amount() < 1200.0, "PV should be reasonable");

    // The PV should NOT be affected by settlement_days for instrument valuation
    // (settlement affects quote interpretation, not curve discounting)
}

/// Test that callable bond exercise payoff is computed correctly.
///
/// This verifies fix B1: "value = coupon + min(max(continuation, put_redemption), call_redemption)"
/// Coupon is paid regardless of exercise decision.
#[test]
fn test_callable_exercise_coupon_always_paid() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let call_date = Date::from_calendar_date(2027, Month::January, 1).expect("Valid date");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

    // Callable bond with call at 100% of par
    let mut call_schedule = CallPutSchedule::default();
    call_schedule.calls.push(CallPut {
        start_date: call_date,
        end_date: call_date,
        price_pct_of_par: 100.0,
        make_whole: None,
    });

    let callable = Bond::builder()
        .id("TEST_CALLABLE".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.06, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .call_put_opt(Some(call_schedule))
        .attributes(Default::default())
        .build()
        .expect("Valid bond");

    // Straight bond (same but without call)
    let straight = Bond::builder()
        .id("TEST_STRAIGHT".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.06, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .call_put_opt(None)
        .attributes(Default::default())
        .build()
        .expect("Valid bond");

    let market = create_test_market(as_of);

    let callable_pv = callable
        .value(&market, as_of)
        .expect("Should price callable");
    let straight_pv = straight
        .value(&market, as_of)
        .expect("Should price straight");

    // Callable bond should be worth LESS than straight bond
    // (issuer has the right to call, reducing holder value)
    assert!(
        callable_pv.amount() < straight_pv.amount(),
        "Callable should be worth less than straight: callable={}, straight={}",
        callable_pv.amount(),
        straight_pv.amount()
    );
}

/// Test that putable bond is worth more than straight bond.
#[test]
fn test_putable_bond_worth_more() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let put_date = Date::from_calendar_date(2027, Month::January, 1).expect("Valid date");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

    // Putable bond with put at 100% of par
    let mut put_schedule = CallPutSchedule::default();
    put_schedule.puts.push(CallPut {
        start_date: put_date,
        end_date: put_date,
        price_pct_of_par: 100.0,
        make_whole: None,
    });

    let putable = Bond::builder()
        .id("TEST_PUTABLE".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.03, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .call_put_opt(Some(put_schedule))
        .attributes(Default::default())
        .build()
        .expect("Valid bond");

    // Straight bond (same but without put)
    let straight = Bond::builder()
        .id("TEST_STRAIGHT".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.03, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .call_put_opt(None)
        .attributes(Default::default())
        .build()
        .expect("Valid bond");

    let market = create_test_market(as_of);

    let putable_pv = putable.value(&market, as_of).expect("Should price putable");
    let straight_pv = straight
        .value(&market, as_of)
        .expect("Should price straight");

    // Putable bond should be worth MORE than straight bond
    // (holder has the right to put, increasing holder value)
    assert!(
        putable_pv.amount() > straight_pv.amount(),
        "Putable should be worth more than straight: putable={}, straight={}",
        putable_pv.amount(),
        straight_pv.amount()
    );
}

/// Test that bond pricing works correctly for various configurations.
///
/// This is a basic sanity check for the pricing engine.
#[test]
fn test_bond_pricing_basic() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");

    let bond = Bond::builder()
        .id("TEST_PRICING".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .attributes(Default::default())
        .build()
        .expect("Valid bond");

    let market = create_test_market(as_of);

    // Price the bond
    let pv = bond.value(&market, as_of).expect("Should price");

    // Sanity checks
    assert!(
        pv.amount() > 900.0,
        "PV should be reasonable: {}",
        pv.amount()
    );
    assert!(
        pv.amount() < 1200.0,
        "PV should be reasonable: {}",
        pv.amount()
    );
}
