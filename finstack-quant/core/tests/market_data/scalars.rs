//! Tests for market data scalar types.
//!
//! This module covers:
//! - DividendSchedule construction and filtering
//! - ScalarTimeSeries (tested in serde.rs)

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::dividends::{DividendKind, DividendSchedule};
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationLag, ScalarTimeSeries, SeriesInterpolation,
};
use finstack_quant_core::money::Money;
use time::Month;

#[test]
fn inflation_lag_display_round_trips_with_from_str() {
    for lag in [
        InflationLag::None,
        InflationLag::Months(3),
        InflationLag::Days(90),
    ] {
        let parsed: InflationLag = lag.to_string().parse().expect("display form parses");
        assert_eq!(parsed, lag);
    }
    assert_eq!(InflationLag::Months(3).to_string(), "3M");
    assert_eq!(InflationLag::None.to_string(), "none");
}

#[test]
fn series_interpolation_parse_error_names_input() {
    let err = "cubic"
        .parse::<SeriesInterpolation>()
        .expect_err("cubic is not supported");
    assert!(err.to_string().contains("cubic"), "{err}");
}

fn jan(day: u8) -> Date {
    Date::from_calendar_date(2025, Month::January, day).unwrap()
}

// DividendSchedule Tests

#[test]
fn dividend_schedule_builds_and_filters() {
    let schedule = DividendSchedule::builder("AAPL-DIVS")
        .underlying("AAPL")
        .currency(Currency::USD)
        .cash(
            jan(5),
            Money::new(1.25, Currency::USD).expect("valid money fixture"),
        )
        .yield_div(jan(12), 0.03)
        .stock(jan(20), 0.05)
        .build()
        .expect("valid dividend schedule");

    assert!(schedule
        .get_events()
        .windows(2)
        .all(|pair| pair[0].date <= pair[1].date));
    let between = schedule.events_between(jan(6), jan(15));
    assert_eq!(between.len(), 1);
    assert!(matches!(between[0].kind, DividendKind::Yield(_)));

    let cash_events: Vec<_> = schedule.cash_events().collect();
    assert_eq!(cash_events.len(), 1);
    assert_eq!(cash_events[0].0, jan(5));
}

#[test]
fn dividend_schedule_validate_rejects_negative_cash() {
    let schedule = DividendSchedule::builder("NEG")
        .cash(
            jan(5),
            Money::new(-1.0, Currency::USD).expect("valid money fixture"),
        )
        .build();
    assert!(schedule.is_err());
}

#[test]
fn scalar_time_series_rejects_pre_history_lookups() {
    let series =
        ScalarTimeSeries::new("TEST-SERIES", vec![(jan(10), 10.0), (jan(20), 20.0)], None).unwrap();

    let result = series.value_on(jan(5));
    assert!(
        result.is_err(),
        "queries before the first observation should error instead of leaking the first value"
    );
}

#[test]
fn scalar_time_series_rejects_non_finite_observations() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let error = ScalarTimeSeries::new("INVALID", vec![(jan(10), value)], None)
            .expect_err("non-finite observations must be rejected");
        assert!(error.to_string().contains("finite"));
    }
}

#[test]
fn inflation_index_rejects_pre_history_lookups() {
    let index = InflationIndex::new(
        "US-CPI",
        vec![(jan(10), 100.0), (jan(20), 101.0)],
        Currency::USD,
    )
    .unwrap();

    let result = index.value_on(jan(5));
    assert!(
        result.is_err(),
        "inflation index lookups before history starts should error"
    );
}
