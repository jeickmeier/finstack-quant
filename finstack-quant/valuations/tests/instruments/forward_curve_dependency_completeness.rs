//! Forward curve dependency completeness tests.
//!
//! Ensures instruments declare complete forward curves via `MarketDependencies`
//! can be priced with only those curves in the market context.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::rates::fra::ForwardRateAgreement;
use finstack_quant_valuations::instruments::rates::irs::PayReceive;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

fn build_discount_curve(id: &str, rate: f64) -> DiscountCurve {
    let as_of = date!(2025 - 01 - 01);
    DiscountCurve::builder(id)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-rate).exp()),
            (5.0f64, (-rate * 5.0).exp()),
            (10.0f64, (-rate * 10.0).exp()),
        ])
        .build()
        .expect("Discount curve construction should succeed")
}

fn build_forward_curve(id: &str, tenor_years: f64, rate: f64) -> ForwardCurve {
    let as_of = date!(2025 - 01 - 01);
    ForwardCurve::builder(id, tenor_years)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .reset_lag(2)
        .knots([(0.0, rate), (1.0, rate + 0.0025), (5.0, rate + 0.0050)])
        .build()
        .expect("Forward curve construction should succeed")
}

fn build_minimal_market(discount_ids: &[&str], forward_ids: &[&str]) -> MarketContext {
    let mut market = MarketContext::new();
    for &id in discount_ids {
        market = market.insert(build_discount_curve(id, 0.03));
    }
    for &id in forward_ids {
        market = market.insert(build_forward_curve(id, 0.25, 0.035));
    }
    market
}

#[test]
fn test_fra_forward_dependencies_complete() {
    let as_of = date!(2025 - 01 - 01);
    let start = as_of.add_months(3);
    let end = as_of.add_months(6);

    let fra = ForwardRateAgreement::builder()
        .id(InstrumentId::new("FRA-FWD-DEPS"))
        .notional(Money::new(5_000_000.0, Currency::USD))
        .fixing_date(start)
        .start_date(start)
        .maturity(end)
        .fixed_rate(rust_decimal::Decimal::try_from(0.0325).expect("valid decimal"))
        .day_count(DayCount::Act360)
        .reset_lag(2)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .forward_curve_id(CurveId::new("USD-SOFR-3M"))
        .side(PayReceive::Receive)
        .build()
        .expect("FRA construction should succeed");

    let deps = fra
        .market_dependencies()
        .expect("market_dependencies")
        .curves;
    let discount_ids: Vec<&str> = deps.discount_curves.iter().map(|id| id.as_str()).collect();
    let forward_ids: Vec<&str> = deps.forward_curves.iter().map(|id| id.as_str()).collect();

    let market = build_minimal_market(&discount_ids, &forward_ids);
    let result = fra.value(&market, as_of);
    assert!(
        result.is_ok(),
        "FRA pricing with minimal market should succeed, got: {:?}",
        result.err()
    );
}

#[test]
fn test_missing_forward_curve_fails() {
    let as_of = date!(2025 - 01 - 01);
    let start = as_of.add_months(3);
    let end = as_of.add_months(6);

    let fra = ForwardRateAgreement::builder()
        .id(InstrumentId::new("FRA-FWD-MISSING"))
        .notional(Money::new(5_000_000.0, Currency::USD))
        .fixing_date(start)
        .start_date(start)
        .maturity(end)
        .fixed_rate(rust_decimal::Decimal::try_from(0.0325).expect("valid decimal"))
        .day_count(DayCount::Act360)
        .reset_lag(2)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .forward_curve_id(CurveId::new("USD-SOFR-3M"))
        .side(PayReceive::Receive)
        .build()
        .expect("FRA construction should succeed");

    let market = build_minimal_market(&["USD-OIS"], &[]);
    let result = fra.value(&market, as_of);
    assert!(
        result.is_err(),
        "FRA pricing should fail when the forward curve is missing"
    );
}
