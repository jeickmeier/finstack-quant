//! Exchange settlement tests for arithmetic and compounded overnight futures.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::ir_future::{
    FutureContractSpecs, InterestRateFuture, RateAveragingMethod,
};
use finstack_quant_valuations::instruments::Position;
use time::macros::date;

fn flat_forward(id: &str, as_of: Date, rate: f64, day_count: DayCount) -> ForwardCurve {
    ForwardCurve::builder(id, 1.0 / 365.0)
        .base_date(as_of)
        .day_count(day_count)
        .knots([(0.0, rate), (1.0, rate)])
        .build()
        .expect("forward curve")
}

fn overnight_future(
    id: &str,
    currency: Currency,
    curve_id: &str,
    start: Date,
    end: Date,
    day_count: DayCount,
    rate_averaging: RateAveragingMethod,
) -> InterestRateFuture {
    InterestRateFuture::builder()
        .id(id.into())
        .notional(Money::new(1_000_000.0, currency).expect("valid money fixture"))
        .expiry(end)
        .fixing_date(end)
        .period_start(start)
        .period_end(end)
        .quoted_price(95.0)
        .day_count(day_count)
        .position(Position::Long)
        .contract_specs(FutureContractSpecs {
            convexity_adjustment: Some(0.0),
            ..FutureContractSpecs::default()
        })
        .discount_curve_id("UNUSED-DISCOUNT".into())
        .forward_curve_id(curve_id.into())
        .rate_averaging(rate_averaging)
        .fixing_index_id(curve_id.into())
        .attributes(Default::default())
        .build()
        .expect("overnight future")
}

#[test]
fn one_month_sofr_uses_calendar_day_weighted_arithmetic_average() {
    let start = date!(2025 - 01 - 02);
    let end = date!(2025 - 01 - 07);
    let as_of = end;
    let curve_id = "USD-SOFR";
    let future = overnight_future(
        "CME-SR1-JAN25",
        Currency::USD,
        curve_id,
        start,
        end,
        DayCount::Act360,
        RateAveragingMethod::ArithmeticAverage,
    );
    let fixings = ScalarTimeSeries::new(
        "FIXING:USD-SOFR",
        vec![
            (date!(2025 - 01 - 02), 0.04),
            (date!(2025 - 01 - 03), 0.05),
            (date!(2025 - 01 - 06), 0.06),
        ],
        None,
    )
    .expect("fixings");
    let market = MarketContext::new()
        .insert(flat_forward(curve_id, as_of, 0.10, DayCount::Act360))
        .insert_series(fixings);

    let rate = future
        .model_settlement_rate(&market, as_of)
        .expect("settlement rate");
    let expected = (0.04 + 3.0 * 0.05 + 0.06) / 5.0;
    assert!((rate - expected).abs() < 1.0e-14);
}

#[test]
fn one_month_sofr_carries_the_prior_fixing_when_month_starts_on_weekend() {
    let start = date!(2025 - 02 - 01);
    let end = date!(2025 - 02 - 04);
    let as_of = end;
    let curve_id = "USD-SOFR";
    let future = overnight_future(
        "CME-SR1-FEB25",
        Currency::USD,
        curve_id,
        start,
        end,
        DayCount::Act360,
        RateAveragingMethod::ArithmeticAverage,
    );
    let fixings = ScalarTimeSeries::new(
        "FIXING:USD-SOFR",
        vec![(date!(2025 - 01 - 31), 0.04), (date!(2025 - 02 - 03), 0.06)],
        None,
    )
    .expect("fixings");
    let market = MarketContext::new()
        .insert(flat_forward(curve_id, as_of, 0.10, DayCount::Act360))
        .insert_series(fixings);

    let rate = future
        .model_settlement_rate(&market, as_of)
        .expect("settlement rate");
    let expected = (2.0 * 0.04 + 0.06) / 3.0;
    assert!((rate - expected).abs() < 1.0e-14);
}

#[test]
fn corra_compounds_on_actual_365_with_weekend_weights() {
    let start = date!(2025 - 01 - 02);
    let end = date!(2025 - 01 - 07);
    let as_of = end;
    let curve_id = "CAD-CORRA";
    let future = overnight_future(
        "MX-COA-JAN25",
        Currency::CAD,
        curve_id,
        start,
        end,
        DayCount::Act365F,
        RateAveragingMethod::CompoundedOvernight,
    );
    let fixings = ScalarTimeSeries::new(
        "FIXING:CAD-CORRA",
        vec![
            (date!(2025 - 01 - 02), 0.04),
            (date!(2025 - 01 - 03), 0.05),
            (date!(2025 - 01 - 06), 0.06),
        ],
        None,
    )
    .expect("fixings");
    let market = MarketContext::new()
        .insert(flat_forward(curve_id, as_of, 0.10, DayCount::Act365F))
        .insert_series(fixings);

    let rate = future
        .model_settlement_rate(&market, as_of)
        .expect("settlement rate");
    let factor = (1.0 + 0.04 / 365.0) * (1.0 + 0.05 * 3.0 / 365.0) * (1.0 + 0.06 / 365.0);
    let expected = (factor - 1.0) * 365.0 / 5.0;
    assert!((rate - expected).abs() < 1.0e-14);
}

#[test]
fn seasoned_overnight_future_requires_every_published_fixing() {
    let start = date!(2025 - 01 - 02);
    let end = date!(2025 - 01 - 07);
    let as_of = date!(2025 - 01 - 06);
    let curve_id = "USD-SOFR";
    let future = overnight_future(
        "CME-SR1-MISSING",
        Currency::USD,
        curve_id,
        start,
        end,
        DayCount::Act360,
        RateAveragingMethod::ArithmeticAverage,
    );
    let incomplete =
        ScalarTimeSeries::new("FIXING:USD-SOFR", vec![(date!(2025 - 01 - 02), 0.04)], None)
            .expect("fixings");
    let market = MarketContext::new()
        .insert(flat_forward(curve_id, as_of, 0.05, DayCount::Act360))
        .insert_series(incomplete);

    let error = future
        .model_settlement_rate(&market, as_of)
        .expect_err("missing Friday fixing must fail");
    assert!(error.to_string().contains("2025-01-03"));
}

#[test]
fn realized_arithmetic_observations_drop_out_of_forward_risk() {
    let start = date!(2025 - 01 - 02);
    let end = date!(2025 - 01 - 07);
    let as_of = date!(2025 - 01 - 03);
    let curve_id = "USD-SOFR";
    let future = overnight_future(
        "CME-SR1-PARTIAL",
        Currency::USD,
        curve_id,
        start,
        end,
        DayCount::Act360,
        RateAveragingMethod::ArithmeticAverage,
    );
    let fixings =
        ScalarTimeSeries::new("FIXING:USD-SOFR", vec![(date!(2025 - 01 - 02), 0.04)], None)
            .expect("fixings");
    let low = MarketContext::new()
        .insert(flat_forward(curve_id, as_of, 0.03, DayCount::Act360))
        .insert_series(fixings.clone());
    let high = MarketContext::new()
        .insert(flat_forward(curve_id, as_of, 0.05, DayCount::Act360))
        .insert_series(fixings);

    let low_rate = future
        .model_settlement_rate(&low, as_of)
        .expect("low projection");
    let high_rate = future
        .model_settlement_rate(&high, as_of)
        .expect("high projection");
    assert!((high_rate - low_rate - 0.02 * 4.0 / 5.0).abs() < 1.0e-12);
}
