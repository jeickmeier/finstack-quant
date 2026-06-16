//! Equity BucketedDv01 smoke tests
//!
//! Note: While equity spot positions don't have direct interest rate cashflows,
//! they do have rate sensitivity through:
//! 1. Discounting of the position value
//! 2. Forward pricing adjustments
//! 3. Portfolio-level aggregation where equities mix with fixed income
//!
//! These tests validate that the generic DV01 calculator properly handles
//! equity instruments in multi-asset portfolios.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::Equity;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::Month;

fn build_flat_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.5, (-rate * 0.5).exp()),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .expect("Failed to build discount curve")
}

fn sum_bucketed_dv01(result: &finstack_quant_valuations::results::ValuationResult) -> f64 {
    result
        .measures
        .iter()
        .filter(|(id, _)| id.as_str().starts_with("bucketed_dv01::"))
        .map(|(_, v)| *v)
        .sum()
}

#[test]
fn test_equity_bucketed_dv01_computed() {
    let as_of = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    let equity = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_shares(100.0)
        .with_price(150.0);

    let usd_curve = build_flat_curve(0.05, as_of, "USD");
    let market = MarketContext::new().insert(usd_curve).insert_price(
        "AAPL",
        MarketScalar::Price(Money::new(150.0, Currency::USD)),
    );

    let result = equity
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::BucketedDv01, MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // BucketedDv01 should be present
    assert!(
        result.measures.contains_key("bucketed_dv01"),
        "BucketedDv01 should be computed"
    );

    let bucketed_dv01 = *result.measures.get("bucketed_dv01").unwrap();
    assert!(bucketed_dv01.is_finite(), "BucketedDv01 should be finite");

    let dv01 = *result.measures.get("dv01").unwrap();
    let bucket_sum = sum_bucketed_dv01(&result);
    let diff = (bucket_sum - dv01).abs();
    let tol = 1e-6_f64.max(1e-3 * dv01.abs());
    assert!(
        diff < tol,
        "Sum of bucketed DV01 should match parallel DV01: bucket_sum={}, dv01={}, diff={}",
        bucket_sum,
        dv01,
        diff
    );
}

#[test]
fn test_equity_bucketed_dv01_with_market_price() {
    let as_of = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    let equity = Equity::new("MSFT", "MSFT", Currency::USD).with_shares(200.0);

    let usd_curve = build_flat_curve(0.03, as_of, "USD");
    let market = MarketContext::new().insert(usd_curve).insert_price(
        "MSFT",
        MarketScalar::Price(Money::new(350.0, Currency::USD)),
    );

    let result = equity
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::BucketedDv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let bucketed_dv01 = *result.measures.get("bucketed_dv01").unwrap();
    assert!(bucketed_dv01.is_finite());
}
