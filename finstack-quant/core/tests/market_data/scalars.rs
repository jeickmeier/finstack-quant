//! Tests for market data scalar types.
//!
//! This module covers:
//! - DividendSchedule construction and filtering
//! - ScalarTimeSeries (tested in serde.rs)

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::dividends::{DividendKind, DividendSchedule};
use finstack_quant_core::market_data::scalars::{InflationIndex, ScalarTimeSeries};
use finstack_quant_core::money::Money;
use time::Month;

fn jan(day: u8) -> Date {
    Date::from_calendar_date(2025, Month::January, day).unwrap()
}

// =============================================================================
// DividendSchedule Tests
// =============================================================================

#[test]
fn dividend_schedule_builds_and_filters() {
    let schedule = DividendSchedule::new("AAPL-DIVS")
        .with_underlying("AAPL")
        .with_currency(Currency::USD)
        .add_cash(jan(5), Money::new(1.25, Currency::USD))
        .add_yield(jan(12), 0.03)
        .add_stock(jan(20), 0.05);

    // Sorting should order events ascending
    let mut sortable = schedule.clone();
    sortable.sort_by_date();
    assert!(sortable.events.first().unwrap().date <= sortable.events.last().unwrap().date);

    let between = sortable.events_between(jan(6), jan(15));
    assert_eq!(between.len(), 1);
    assert!(matches!(between[0].kind, DividendKind::Yield(_)));

    let cash_events: Vec<_> = schedule.cash_events().collect();
    assert_eq!(cash_events.len(), 1);
    assert_eq!(cash_events[0].0, jan(5));
}

#[test]
fn dividend_schedule_validate_rejects_negative_cash() {
    let schedule = DividendSchedule::new("NEG").add_cash(jan(5), Money::new(-1.0, Currency::USD));
    assert!(schedule.validate().is_err());
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
