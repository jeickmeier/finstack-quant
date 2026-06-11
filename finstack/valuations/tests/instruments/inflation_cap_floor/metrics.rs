//! Metric tests for inflation caps/floors.

use crate::finstack_test_utils::flat_vol_surface;
use crate::inflation_swap::fixtures::{flat_discount, flat_inflation_curve};
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor, TenorUnit};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::rates::inflation_cap_floor::{
    InflationCapFloor, InflationCapFloorType,
};
use finstack_valuations::instruments::{Attributes, Instrument, PricingOptions, PricingOverrides};
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

fn build_caplet() -> InflationCapFloor {
    let start = Date::from_calendar_date(2026, Month::January, 2).unwrap();
    let end = Date::from_calendar_date(2027, Month::January, 2).unwrap();
    InflationCapFloor::builder()
        .id("INF-CAP-VEGA".into())
        .option_type(InflationCapFloorType::Caplet)
        .notional(Money::new(5_000_000.0, Currency::USD))
        .strike(Decimal::try_from(0.025).expect("valid decimal"))
        .start_date(start)
        .maturity(end)
        .frequency(Tenor::new(1, TenorUnit::Years))
        .day_count(DayCount::Act365F)
        .stub(StubKind::None)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(None)
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("US-CPI-VOL"))
        .pricing_overrides(PricingOverrides::default())
        .lag_override_opt(None)
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

fn build_market(as_of: Date, vol: f64) -> MarketContext {
    MarketContext::new()
        .insert(flat_discount("USD-OIS", as_of, 0.02).unwrap())
        .insert(flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.025).unwrap())
        .insert_surface(flat_vol_surface("US-CPI-VOL", &[1.0, 5.0], &[0.025], vol))
}

/// Vega must be expressed per vol point (per 1% = 0.01 absolute vol change),
/// matching the workspace-wide convention (swaption `VOL_PCT_SCALE`, nominal
/// cap/floor, CMS, fd_greeks). For a flat surface the reference value is the
/// central difference of PV under ±1 vol point, divided by 2 vol points:
/// `(PV(σ+0.01) − PV(σ−0.01)) / 2`.
#[test]
fn vega_is_per_vol_point() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let vol = 0.02;
    let market = build_market(as_of, vol);
    let caplet = build_caplet();

    let result = caplet
        .price_with_metrics(&market, as_of, &[MetricId::Vega], PricingOptions::default())
        .unwrap();
    let vega = *result.measures.get("vega").unwrap();

    let bump = 0.01;
    let pv_up = caplet
        .value(&build_market(as_of, vol + bump), as_of)
        .unwrap()
        .amount();
    let pv_down = caplet
        .value(&build_market(as_of, vol - bump), as_of)
        .unwrap()
        .amount();
    let expected = (pv_up - pv_down) / 2.0;

    assert!(
        expected.abs() > 1.0,
        "test setup must have non-trivial vega, got reference {expected}"
    );
    assert!(
        (vega - expected).abs() <= 1e-6 * expected.abs().max(1.0),
        "vega must be per vol point: metric={vega}, reference={expected} \
         (a ~100x mismatch means the per-unit-vol scaling regressed)"
    );
}
