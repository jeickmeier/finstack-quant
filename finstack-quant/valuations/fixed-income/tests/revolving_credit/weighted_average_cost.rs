//! Weighted average cost: floating facilities are projected at the index
//! forward averaged over the remaining reset grid, so the curve's slope moves
//! the metric with the facility's horizon instead of a single curve point.

use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_fixed_income::revolving_credit::{
    DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use time::macros::date;

use crate::support::revolver_draws::floating_spec;
use finstack_quant_valuations_test_support::test_helpers::{
    flat_discount_curve, flat_forward_curve,
};

const AS_OF: Date = date!(2024 - 01 - 15);

/// Deterministic SOFR + 250bp facility, USD 50M committed and USD 10M drawn.
fn facility(maturity: Date) -> RevolvingCredit {
    RevolvingCredit::builder()
        .id("RCF-WAC".into())
        .commitment(Money::new(50_000_000.0, Currency::USD).expect("money"))
        .drawn(Money::new(10_000_000.0, Currency::USD).expect("money"))
        .issue_date(AS_OF)
        .maturity(maturity)
        .rate(RateSpec::Floating(floating_spec()))
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0).expect("fees"))
        .draw_repay_spec(DrawRepaySpec::Deterministic(Vec::new()))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .build()
        .expect("facility")
}

fn market_with(forward: ForwardCurve) -> MarketContext {
    let fixings: Vec<(Date, f64)> = (0..25)
        .rev()
        .map(|days| (AS_OF - time::Duration::days(days), 0.02))
        .collect();
    MarketContext::new()
        .try_insert(flat_discount_curve(0.03, AS_OF, "USD-OIS"))
        .expect("insert market curve")
        .try_insert(forward)
        .expect("insert market curve")
        .insert_series(
            ScalarTimeSeries::new(
                "FIXING:USD-SOFR-3M",
                fixings,
                None,
                finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
            )
            .expect("fixings"),
        )
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
    let rising = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(AS_OF)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.3125, 0.993284370061577),
            (0.625, 0.98565082003172),
            (0.9375, 0.977121249011583),
            (1.25, 0.967719990077532),
            (1.5625, 0.957473694654899),
            (1.875, 0.946411206689668),
            (2.1875, 0.93456342744914),
            (2.5, 0.921963171837898),
            (2.8125, 0.908645017163215),
            (3.125, 0.894645145324014),
            (3.4375, 0.880001179429489),
            (3.75, 0.864752015877161),
            (4.0625, 0.848937652935618),
            (4.375, 0.832599016884203),
            (4.6875, 0.815777786760778),
            (5.0, 0.798516218759377),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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

/// A flat index leaves the horizon irrelevant: both facilities cost the same
/// (up to sub-bp quarter-length convexity between 90/91/92-day quarters on a continuous curve).
#[test]
fn flat_forward_curve_makes_the_cost_independent_of_the_horizon() {
    let market = market_with(flat_forward_curve(0.04, AS_OF, "USD-SOFR-3M"));
    let one_year = weighted_average_cost(&facility(date!(2025 - 01 - 15)), &market);
    let five_years = weighted_average_cost(&facility(date!(2029 - 01 - 15)), &market);
    assert!(
        (five_years - one_year).abs() < 1e-6,
        "five-year {five_years} vs one-year {one_year}"
    );
}
