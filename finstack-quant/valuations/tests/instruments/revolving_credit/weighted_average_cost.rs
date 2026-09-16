//! Weighted average cost: floating facilities are projected at the index
//! forward averaged over the remaining reset grid, so the curve's slope moves
//! the metric with the facility's horizon instead of a single curve point.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

use crate::common::test_helpers::{flat_discount_curve, flat_forward_curve};
use crate::revolving_credit::draw_option_cost::floating_spec;

const AS_OF: Date = date!(2024 - 01 - 15);

/// Deterministic SOFR + 250bp facility, USD 50M committed and USD 10M drawn.
fn facility(maturity: Date) -> RevolvingCredit {
    RevolvingCredit::builder()
        .id("RCF-WAC".into())
        .commitment_amount(Money::new(50_000_000.0, Currency::USD).expect("money"))
        .drawn_amount(Money::new(10_000_000.0, Currency::USD).expect("money"))
        .commitment_date(AS_OF)
        .maturity(maturity)
        .base_rate_spec(BaseRateSpec::Floating(floating_spec()))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0).expect("fees"))
        .draw_repay_spec(DrawRepaySpec::Deterministic(Vec::new()))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .build()
        .expect("facility")
}

fn market_with(forward: ForwardCurve) -> MarketContext {
    let fixings: Vec<(Date, f64)> = (0..25)
        .map(|days| (AS_OF - time::Duration::days(days), 0.02))
        .collect();
    MarketContext::new()
        .insert(flat_discount_curve(0.03, AS_OF, "USD-OIS"))
        .insert(forward)
        .insert_series(ScalarTimeSeries::new("FIXING:USD-SOFR-3M", fixings, None).expect("fixings"))
}

fn weighted_average_cost(facility: &RevolvingCredit, market: &MarketContext) -> f64 {
    facility
        .price_with_metrics(
            market,
            AS_OF,
            &[MetricId::custom("weighted_average_cost")],
            PricingOptions::default(),
        )
        .expect("metrics")
        .measures
        .get("weighted_average_cost")
        .copied()
        .expect("weighted average cost")
}

/// On a curve rising 100bp a year the one-year facility averages ≈2.4% and
/// the five-year one ≈4.4%; the drawn 20% of the commitment carries that
/// ≈200bp gap into a ≈40bp difference in the weighted average cost.
#[test]
fn rising_forward_curve_raises_the_cost_of_a_longer_facility() {
    let rising = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(AS_OF)
        .knots(vec![(0.0, 0.02), (5.0, 0.07)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("forward curve");
    let market = market_with(rising);
    let one_year = weighted_average_cost(&facility(date!(2025 - 01 - 15)), &market);
    let five_years = weighted_average_cost(&facility(date!(2029 - 01 - 15)), &market);
    let gap = five_years - one_year;
    assert!(
        (0.003..0.005).contains(&gap),
        "five-year {five_years} vs one-year {one_year}: gap {gap}"
    );
}

/// A flat index leaves the horizon irrelevant: both facilities cost the same.
#[test]
fn flat_forward_curve_makes_the_cost_independent_of_the_horizon() {
    let market = market_with(flat_forward_curve(0.04, AS_OF, "USD-SOFR-3M"));
    let one_year = weighted_average_cost(&facility(date!(2025 - 01 - 15)), &market);
    let five_years = weighted_average_cost(&facility(date!(2029 - 01 - 15)), &market);
    assert!(
        (five_years - one_year).abs() < 1e-12,
        "five-year {five_years} vs one-year {one_year}"
    );
}
