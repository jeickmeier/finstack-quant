//! Forward swap rate and annuity calculation tests

use crate::swaption::common::*;
use finstack_core::dates::{BusinessDayConvention, DayCount, DayCountContext, StubKind, Tenor};
use finstack_valuations::instruments::rates::swaption::Swaption;
use finstack_valuations::instruments::rates::swaption::{CashSettlementMethod, SwaptionSettlement};
use finstack_valuations::instruments::FixedLegSpec;
use rust_decimal::Decimal;

fn expected_forward_rate(
    swaption: &Swaption,
    market: &finstack_core::market_data::context::MarketContext,
    as_of: finstack_core::dates::Date,
) -> f64 {
    equivalent_vanilla_irs_par_rate(swaption, market, as_of)
}

struct SingleCurveForwardSpec {
    start: finstack_core::dates::Date,
    end: finstack_core::dates::Date,
    frequency: Tenor,
    day_count: DayCount,
    stub: StubKind,
    payment_lag_days: i32,
}

fn expected_single_curve_payment_date_forward(
    spec: SingleCurveForwardSpec,
    market: &finstack_core::market_data::context::MarketContext,
    as_of: finstack_core::dates::Date,
) -> f64 {
    let disc = market.get_discount("USD_OIS").expect("discount curve");
    let schedule = finstack_cashflows::builder::periods::build_periods(
        finstack_cashflows::builder::periods::BuildPeriodsParams {
            start: spec.start,
            end: spec.end,
            frequency: spec.frequency,
            stub: spec.stub,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: finstack_cashflows::builder::calendar::WEEKENDS_ONLY_ID,
            end_of_month: false,
            day_count: spec.day_count,
            payment_lag_days: spec.payment_lag_days,
            reset_lag_days: None,
            adjust_accrual_dates: false,
        },
    )
    .expect("fixed schedule");
    let mut forward_leg = 0.0;
    let mut annuity = 0.0;
    for period in schedule {
        let df_start = disc
            .df_between_dates(as_of, period.accrual_start)
            .expect("df start");
        let df_end = disc
            .df_between_dates(as_of, period.accrual_end)
            .expect("df end");
        let df_pay = disc
            .df_between_dates(as_of, period.payment_date)
            .expect("df pay");
        let forward = (df_start / df_end - 1.0) / period.accrual_year_fraction;
        forward_leg += period.accrual_year_fraction * forward * df_pay;
        annuity += period.accrual_year_fraction * df_pay;
    }
    forward_leg / annuity
}

#[test]
fn test_forward_swap_rate_calculation() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let strike = 0.05;
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);

    let market = create_flat_market(as_of, 0.05, 0.30);

    let forward = swaption.forward_swap_rate(&market, as_of).unwrap();

    let expected = expected_forward_rate(&swaption, &market, as_of);
    assert_approx_eq(forward, expected, 1e-8, "Forward swap rate");
}

#[test]
fn test_swap_annuity_positive() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let strike = 0.05;
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);

    let market = create_flat_market(as_of, 0.05, 0.30);
    let disc = market.get_discount("USD_OIS").unwrap();

    let annuity = swaption.swap_annuity(disc.as_ref(), as_of).unwrap();

    // 5Y semi-annual swap should have annuity around 4.0-5.0
    assert_reasonable(annuity, 3.0, 6.0, "Swap annuity");
}

#[test]
fn test_annuity_increases_with_tenor() {
    let (as_of, expiry, swap_start, _) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);
    let disc = market.get_discount("USD_OIS").unwrap();

    // 2Y swap
    let swap_end_2y = time::macros::date!(2027 - 01 - 01);
    let swaption_2y = create_standard_payer_swaption(expiry, swap_start, swap_end_2y, 0.05);
    let annuity_2y = swaption_2y.swap_annuity(disc.as_ref(), as_of).unwrap();

    // 5Y swap
    let swap_end_5y = time::macros::date!(2030 - 01 - 01);
    let swaption_5y = create_standard_payer_swaption(expiry, swap_start, swap_end_5y, 0.05);
    let annuity_5y = swaption_5y.swap_annuity(disc.as_ref(), as_of).unwrap();

    // 10Y swap
    let swap_end_10y = time::macros::date!(2035 - 01 - 01);
    let swaption_10y = create_standard_payer_swaption(expiry, swap_start, swap_end_10y, 0.05);
    let annuity_10y = swaption_10y.swap_annuity(disc.as_ref(), as_of).unwrap();

    assert!(annuity_2y < annuity_5y, "5Y annuity should exceed 2Y");
    assert!(annuity_5y < annuity_10y, "10Y annuity should exceed 5Y");
}

#[test]
fn test_year_fraction_act360() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);

    let yf = swaption
        .day_count
        .year_fraction(as_of, expiry, DayCountContext::default())
        .unwrap();

    // 1 year ≈ 1.0 under Act/360
    assert_approx_eq(yf, 1.0, 0.02, "Year fraction for 1Y");
}

#[test]
fn test_forward_rate_consistency() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);

    // Test with different curve levels
    for rate in [0.02, 0.05, 0.08] {
        let market = create_flat_market(as_of, rate, 0.30);

        let forward = swaption.forward_swap_rate(&market, as_of).unwrap();

        let expected = expected_forward_rate(&swaption, &market, as_of);
        assert_approx_eq(
            forward,
            expected,
            1e-8,
            &format!("Forward rate at {}%", rate * 100.0),
        );
    }
}

#[test]
fn test_annuity_dispatch_matches_selected_settlement_method() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);
    let disc = market.get_discount("USD_OIS").unwrap();
    let base = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let forward = base.forward_swap_rate(&market, as_of).unwrap();

    let physical = base.annuity(disc.as_ref(), as_of, forward).unwrap();
    let expected_physical = base.swap_annuity(disc.as_ref(), as_of).unwrap();
    assert_approx_eq(
        physical,
        expected_physical,
        1e-12,
        "physical settlement annuity",
    );

    let par_yield = base
        .clone()
        .with_settlement(SwaptionSettlement::Cash)
        .with_cash_settlement_method(CashSettlementMethod::ParYield);
    let par_yield_annuity = par_yield.annuity(disc.as_ref(), as_of, forward).unwrap();
    // ParYield is the at-expiry cash annuity discounted from expiry to as_of.
    let df_expiry = disc.df_between_dates(as_of, par_yield.expiry).unwrap();
    let expected_par_yield = par_yield.cash_annuity_par_yield(forward).unwrap() * df_expiry;
    assert_approx_eq(
        par_yield_annuity,
        expected_par_yield,
        1e-12,
        "par-yield cash annuity",
    );

    let isda = base
        .clone()
        .with_settlement(SwaptionSettlement::Cash)
        .with_cash_settlement_method(CashSettlementMethod::IsdaParPar);
    let isda_annuity = isda.annuity(disc.as_ref(), as_of, forward).unwrap();
    assert_approx_eq(isda_annuity, expected_physical, 1e-12, "isda cash annuity");

    let zero_coupon = base
        .with_settlement(SwaptionSettlement::Cash)
        .with_cash_settlement_method(CashSettlementMethod::ZeroCoupon);
    let zero_coupon_annuity = zero_coupon.annuity(disc.as_ref(), as_of, forward).unwrap();
    let expected_zero_coupon = zero_coupon
        .cash_annuity_zero_coupon(disc.as_ref(), as_of)
        .unwrap();
    assert_approx_eq(
        zero_coupon_annuity,
        expected_zero_coupon,
        1e-12,
        "zero-coupon cash annuity",
    );
}

#[test]
fn test_par_yield_annuity_discounted_from_expiry() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);
    let disc = market.get_discount("USD_OIS").unwrap();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05)
        .with_settlement(SwaptionSettlement::Cash)
        .with_cash_settlement_method(CashSettlementMethod::ParYield);
    let forward = swaption.forward_swap_rate(&market, as_of).unwrap();

    let annuity = swaption.annuity(disc.as_ref(), as_of, forward).unwrap();
    let undiscounted = swaption.cash_annuity_par_yield(forward).unwrap();

    // Positive rates => DF(as_of -> expiry) < 1, so the settled annuity must be
    // strictly below the undiscounted at-expiry cash annuity.
    assert!(
        annuity < undiscounted,
        "ParYield annuity ({annuity}) should be discounted below the at-expiry cash annuity ({undiscounted})"
    );

    // On a flat curve the par-yield approximation is close to the ISDA
    // par/par (true curve) annuity once both are discounted to as_of.
    let isda = swaption.swap_annuity(disc.as_ref(), as_of).unwrap();
    let rel_err = (annuity - isda).abs() / isda;
    assert!(
        rel_err < 0.02,
        "ParYield annuity ({annuity}) should approximate ISDA annuity ({isda}) on a flat curve; rel err {rel_err}"
    );
}

#[test]
fn test_zero_coupon_cash_annuity_matches_tenor_times_maturity_df() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);
    let disc = market.get_discount("USD_OIS").unwrap();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05)
        .with_settlement(SwaptionSettlement::Cash)
        .with_cash_settlement_method(CashSettlementMethod::ZeroCoupon);

    let tenor = swaption
        .day_count
        .year_fraction(swap_start, swap_end, DayCountContext::default())
        .unwrap();
    let expected = tenor * disc.df_between_dates(as_of, swap_end).unwrap();
    let actual = swaption
        .cash_annuity_zero_coupon(disc.as_ref(), as_of)
        .unwrap();
    assert_approx_eq(actual, expected, 1e-12, "zero coupon annuity formula");
}

#[test]
fn test_forward_swap_rate_single_curve_matches_payment_date_cashflows() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);
    let mut swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    swaption.forward_curve_id = swaption.discount_curve_id.clone();

    let forward = swaption.forward_swap_rate(&market, as_of).unwrap();
    let expected = expected_single_curve_payment_date_forward(
        SingleCurveForwardSpec {
            start: swap_start,
            end: swap_end,
            frequency: swaption.fixed_freq,
            day_count: swaption.day_count,
            stub: StubKind::None,
            payment_lag_days: 0,
        },
        &market,
        as_of,
    );
    assert_approx_eq(forward, expected, 1e-12, "single-curve forward swap rate");
}

#[test]
fn test_single_curve_forward_honors_explicit_fixed_leg_payment_cashflows() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);
    let mut swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    swaption.forward_curve_id = swaption.discount_curve_id.clone();
    swaption.underlying_fixed_leg = Some(FixedLegSpec {
        discount_curve_id: swaption.discount_curve_id.clone(),
        rate: Decimal::ZERO,
        frequency: Tenor::annual(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::ShortFront,
        start: swap_start,
        end: swap_end,
        par_method: None,
        compounding_simple: true,
        payment_lag_days: 2,
        end_of_month: false,
    });

    let expected = expected_single_curve_payment_date_forward(
        SingleCurveForwardSpec {
            start: swap_start,
            end: swap_end,
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            stub: StubKind::ShortFront,
            payment_lag_days: 2,
        },
        &market,
        as_of,
    );

    let forward = swaption
        .forward_swap_rate(&market, as_of)
        .expect("forward swap rate");

    assert_approx_eq(
        forward,
        expected,
        1e-10,
        "single-curve forward should match explicit underlier par rate",
    );
}
