//! Horizon metrics (theta, carry decomposition, iterative breakeven) hold
//! market-held fixings at their as-of values across the horizon.
//!
//! Each test compares the metric with an explicit repricing: the market with
//! the crossed observations injected by hand (the flat projection curve's
//! rate, or the as-of spot for a price series) and repriced at the horizon
//! under the metric's own curve convention (rolled for theta, unrolled for
//! carry and breakeven). The injected values are written independently of the
//! schedule's recorded projections.

use crate::instruments::loan_facility_wire_keys::{pricing_case_inputs, pricing_cases};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    calendar_by_id, BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor,
};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{MarketScalar, ScalarTimeSeries};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::variance_swap::VarianceSwap;
use finstack_quant_valuations::instruments::rates::irs::{
    FixedLegSpec, FloatLegSpec, FloatingLegCompounding, PayReceive,
};
use finstack_quant_valuations::instruments::{
    BreakevenConfig, BreakevenMode, BreakevenTarget, Instrument, InterestRateSwap, PricingOptions,
};
use finstack_quant_valuations::metrics::MetricId;
use std::collections::HashMap;
use time::macros::date;

const SOFR: f64 = 0.04;

fn usny_business_days(from: Date, through: Date) -> Vec<Date> {
    let calendar = calendar_by_id("usny").expect("USNY calendar");
    let mut days = Vec::new();
    let mut day = from;
    while day <= through {
        if calendar.is_business_day(day) {
            days.push(day);
        }
        day = day.next_day().expect("next day");
    }
    days
}

fn sofr_fixings(from: Date, through: Date) -> ScalarTimeSeries {
    let observations = usny_business_days(from, through)
        .into_iter()
        .map(|day| (day, SOFR))
        .collect();
    ScalarTimeSeries::new("FIXING:USD-SOFR", observations, None).expect("fixings")
}

/// Receive-fixed 1Y SOFR OIS swap valued inside its first accrual period.
fn seasoned_ois_swap(start: Date) -> InterestRateSwap {
    let end = start.add_months(12);
    InterestRateSwap::builder()
        .id("THETA-SOFR-OIS".into())
        .notional(Money::new(10_000_000.0, Currency::USD).expect("notional"))
        .side(PayReceive::Receive)
        .fixed_leg(FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.045).expect("rate"),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".into()),
            start,
            end,
            payment_lag_days: 0,
            end_of_month: false,
            stub: StubKind::ShortFront,
            par_method: None,
        })
        .float_leg(FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR".into(),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".into()),
            start,
            end,
            compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
            payment_lag_days: 0,
            end_of_month: false,
            spread_bp: rust_decimal::Decimal::ZERO,
            fixing_calendar_id: Some("usny".into()),
            stub: StubKind::ShortFront,
            reset_lag_days: 0,
        })
        .build()
        .expect("swap")
}

fn sofr_market(as_of: Date, fixings_from: Date) -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, (-SOFR * 5.0).exp())])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("discount");
    let forward = ForwardCurve::builder("USD-SOFR", 1.0 / 360.0)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, SOFR), (5.0, SOFR)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("forward");
    let history_end = as_of.previous_day().expect("previous day");
    MarketContext::new()
        .insert(discount)
        .insert(forward)
        .insert_series(sofr_fixings(fixings_from, history_end))
}

fn theta(instrument: &dyn Instrument, market: &MarketContext, as_of: Date) -> f64 {
    instrument
        .price_with_metrics(market, as_of, &[MetricId::Theta], PricingOptions::default())
        .expect("theta")
        .measures["theta"]
}

#[test]
fn ois_swap_theta_observes_crossed_sofr_fixings_at_the_as_of_forward() {
    let start = date!(2025 - 01 - 02);
    let as_of = date!(2025 - 02 - 03);
    let market = sofr_market(as_of, start);
    for (period, rolled) in [("1D", date!(2025 - 02 - 04)), ("1M", date!(2025 - 03 - 03))] {
        let mut swap = seasoned_ois_swap(start);
        swap.metric_pricing_overrides.theta_period = Some(Tenor::parse(period).expect("tenor"));
        let days = (rolled - as_of).whole_days();

        // Ordinary pricing still requires every past fixing.
        assert!(
            swap.value(&market.roll_forward(days).expect("roll"), rolled)
                .is_err(),
            "{period}: the roll must cross an unpublished SOFR fixing"
        );

        let injected = market
            .clone()
            .insert_series(sofr_fixings(start, rolled))
            .roll_forward(days)
            .expect("roll");
        let expected = swap.value(&injected, rolled).expect("rolled PV").amount()
            - swap.value(&market, as_of).expect("base PV").amount();
        let actual = theta(&swap, &market, as_of);
        assert!(actual.is_finite(), "{period}: theta {actual}");
        assert!(
            (actual - expected).abs() < 1e-6,
            "{period}: theta {actual} != explicit reprice {expected}"
        );
    }
}

fn metrics(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
    ids: &[MetricId],
) -> HashMap<MetricId, f64> {
    let result = instrument
        .price_with_metrics(market, as_of, ids, PricingOptions::default())
        .expect("metrics");
    ids.iter()
        .map(|id| {
            let value = *result
                .measures
                .get(id.as_str())
                .unwrap_or_else(|| panic!("{id} missing"));
            (id.clone(), value)
        })
        .collect()
}

#[test]
fn ois_swap_carry_decomposition_observes_crossed_sofr_fixings_at_the_as_of_forward() {
    let start = date!(2025 - 01 - 02);
    let as_of = date!(2025 - 02 - 03);
    let market = sofr_market(as_of, start);
    for (period, rolled) in [("1D", date!(2025 - 02 - 04)), ("1M", date!(2025 - 03 - 03))] {
        let mut swap = seasoned_ois_swap(start);
        swap.metric_pricing_overrides.theta_period = Some(Tenor::parse(period).expect("tenor"));

        // Carry reprices on the unrolled curves; without the crossed SOFR
        // fixings that reprice fails.
        assert!(
            swap.value(&market, rolled).is_err(),
            "{period}: the horizon must cross an unpublished SOFR fixing"
        );

        let injected = market.clone().insert_series(sofr_fixings(start, rolled));
        let expected_pv_change = swap.value(&injected, rolled).expect("horizon PV").amount()
            - swap.value(&market, as_of).expect("base PV").amount();

        let m = metrics(
            &swap,
            &market,
            as_of,
            &[
                MetricId::CarryTotal,
                MetricId::CouponIncome,
                MetricId::PullToPar,
                MetricId::RollDown,
                MetricId::FundingCost,
            ],
        );
        // roll_down = total_pv_change - pull_to_par by construction.
        let pv_change = m[&MetricId::PullToPar] + m[&MetricId::RollDown];
        assert!(pv_change.is_finite(), "{period}: pv change {pv_change}");
        assert!(
            (pv_change - expected_pv_change).abs() < 1e-6,
            "{period}: carry PV change {pv_change} != explicit reprice {expected_pv_change}"
        );
        assert_eq!(m[&MetricId::FundingCost], 0.0, "{period}: no repo curve");
        assert!(
            (m[&MetricId::CarryTotal] - (m[&MetricId::CouponIncome] + expected_pv_change)).abs()
                < 1e-6,
            "{period}: carry_total {} != coupon income + explicit PV change",
            m[&MetricId::CarryTotal]
        );
    }
}

#[test]
fn ois_swap_iterative_breakeven_observes_crossed_sofr_fixings_at_the_as_of_forward() {
    let start = date!(2025 - 01 - 02);
    let as_of = date!(2025 - 02 - 03);
    let rolled = date!(2025 - 03 - 03);
    let market = sofr_market(as_of, start);
    let mut swap = seasoned_ois_swap(start);
    swap.metric_pricing_overrides = swap
        .metric_pricing_overrides
        .clone()
        .with_theta_period(Tenor::monthly())
        .with_breakeven_config(BreakevenConfig {
            target: BreakevenTarget::Ytm,
            mode: BreakevenMode::Iterative,
        });
    assert!(
        swap.value(&market, rolled).is_err(),
        "the horizon must cross an unpublished SOFR fixing"
    );

    let m = metrics(
        &swap,
        &market,
        as_of,
        &[MetricId::Dv01, MetricId::CarryTotal, MetricId::Breakeven],
    );
    let breakeven = m[&MetricId::Breakeven];
    let carry = m[&MetricId::CarryTotal];
    assert!(
        breakeven.is_finite() && breakeven.abs() > 1e-9,
        "expected a meaningful breakeven, got {breakeven}"
    );

    // The solved discount-curve shift zeroes the horizon P&L with the crossed
    // fixings injected by hand at their unbumped as-of forward, in both the
    // base and the bumped market.
    let injected = market.insert_series(sofr_fixings(start, rolled));
    let bumped = injected
        .bump([MarketBump::Curve {
            id: "USD-OIS".into(),
            spec: BumpSpec::parallel_bp(breakeven),
        }])
        .expect("bump");
    let residual = carry + swap.value(&bumped, rolled).expect("bumped PV").amount()
        - swap.value(&injected, rolled).expect("base PV").amount();
    assert!(
        residual.abs() < 1e-6,
        "breakeven {breakeven}bp should zero the horizon P&L, residual = {residual}"
    );
}

#[test]
fn variance_swap_theta_observes_the_as_of_spot_over_the_horizon() {
    let cases = pricing_cases();
    let case = cases["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["type"] == "variance_swap")
        .expect("variance swap case");
    let (_, market) = pricing_case_inputs(case);
    let as_of = date!(2023 - 12 - 28);
    let rolled = date!(2024 - 01 - 28);
    let mut swap = VarianceSwap::example().expect("example");
    swap.metric_pricing_overrides.theta_period = Some(Tenor::parse("1M").expect("tenor"));

    let spot = match market.get_price("SPX").expect("spot") {
        MarketScalar::Unitless(value) => *value,
        MarketScalar::Price(money) => money.amount(),
    };
    let closes: Vec<(Date, f64)> = swap
        .observation_dates()
        .expect("observation dates")
        .into_iter()
        .filter(|day| (as_of..=rolled).contains(day))
        .map(|day| (day, spot))
        .collect();
    assert!(closes.len() >= 2, "the roll must cross observations");
    let days = (rolled - as_of).whole_days();
    assert!(swap
        .value(&market.roll_forward(days).expect("roll"), rolled)
        .is_err());

    let injected = market
        .clone()
        .insert_series(ScalarTimeSeries::new("SPX", closes, None).expect("closes"))
        .roll_forward(days)
        .expect("roll");
    let expected = swap.value(&injected, rolled).expect("rolled PV").amount()
        - swap.value(&market, as_of).expect("base PV").amount();
    let actual = theta(&swap, &market, as_of);
    assert!(
        (actual - expected).abs() < 1e-6,
        "theta {actual} != explicit reprice {expected}"
    );
}
