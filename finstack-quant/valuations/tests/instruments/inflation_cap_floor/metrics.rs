//! Metric tests for inflation caps/floors.

use crate::finstack_quant_test_utils::flat_vol_surface;
use crate::inflation_swap::fixtures::{flat_discount, flat_inflation_curve};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, StubKind, Tenor, TenorUnit,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::rates::inflation_cap_floor::{
    InflationCapFloor, InflationCapFloorType,
};
use finstack_quant_valuations::instruments::{
    Attributes, Instrument, PricingOptions, PricingOverrides,
};
use finstack_quant_valuations::metrics::MetricId;
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

/// Build a 1Y inflation cap/floor of the given type and strike (otherwise
/// identical to `build_caplet`).
fn build_option(option_type: InflationCapFloorType, strike: f64) -> InflationCapFloor {
    let start = Date::from_calendar_date(2026, Month::January, 2).unwrap();
    let end = Date::from_calendar_date(2027, Month::January, 2).unwrap();
    InflationCapFloor::builder()
        .id("INF-CF".into())
        .option_type(option_type)
        .notional(Money::new(5_000_000.0, Currency::USD))
        .strike(Decimal::try_from(strike).expect("valid decimal"))
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

/// `Gamma` is registered but was previously unexercised. A long inflation cap
/// is convex in the underlying inflation rate, so its gamma is positive.
#[test]
fn test_gamma_positive_for_long_cap() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let market = build_market(as_of, 0.02);
    let caplet = build_caplet();

    let gamma = *caplet
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .unwrap();

    assert!(gamma.is_finite(), "gamma should be finite, got {gamma}");
    assert!(
        gamma > 0.0,
        "long cap gamma should be positive, got {gamma}"
    );
}

/// `Dv01` and `BucketedDv01` are registered but were previously unexercised.
/// Both must be finite and the bucketed key-rate DV01s must reconcile with the
/// parallel DV01.
#[test]
fn test_dv01_and_bucketed_dv01_reconcile() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let market = build_market(as_of, 0.02);
    let caplet = build_caplet();

    let result = caplet
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01, MetricId::BucketedDv01],
            PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();
    let bucketed = *result.measures.get("bucketed_dv01").unwrap();

    assert!(dv01.is_finite(), "DV01 should be finite, got {dv01}");
    assert!(
        bucketed.is_finite(),
        "BucketedDv01 aggregate should be finite, got {bucketed}"
    );
    assert!(
        (bucketed - dv01).abs() <= 1.0 + 0.05 * dv01.abs(),
        "BucketedDv01 ({bucketed}) should reconcile with parallel DV01 ({dv01})"
    );
}

/// Strike monotonicity: a cap (call on inflation) loses value as its strike
/// rises, while a floor (put on inflation) gains value. A fundamental
/// no-arbitrage property that must hold regardless of the pricing details.
#[test]
fn test_cap_value_falls_and_floor_rises_with_strike() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let market = build_market(as_of, 0.02);

    let cap_low = build_option(InflationCapFloorType::Caplet, 0.02)
        .value(&market, as_of)
        .unwrap()
        .amount();
    let cap_high = build_option(InflationCapFloorType::Caplet, 0.03)
        .value(&market, as_of)
        .unwrap()
        .amount();
    let floor_low = build_option(InflationCapFloorType::Floorlet, 0.02)
        .value(&market, as_of)
        .unwrap()
        .amount();
    let floor_high = build_option(InflationCapFloorType::Floorlet, 0.03)
        .value(&market, as_of)
        .unwrap()
        .amount();

    assert!(
        cap_low > cap_high,
        "cap value should fall as strike rises: K=2% {cap_low}, K=3% {cap_high}"
    );
    assert!(
        floor_high > floor_low,
        "floor value should rise as strike rises: K=2% {floor_low}, K=3% {floor_high}"
    );
}

/// Cap−Floor put-call parity holds within the model: `Cap(K) − Floor(K) =
/// DF·N·τ·(F − K)`, where `F` is the YoY *convexity-adjusted* forward
/// (Jarrow-Yildirim / Mercurio 2005). Because that forward legitimately depends
/// on the inflation vol, `Cap(K) − Floor(K)` is itself vol-dependent — this is
/// the YoY convexity adjustment, NOT a parity violation.
///
/// The clean, model-agnostic way to confirm parity (i.e. that the optionality
/// time value cancels and only the linear forward leg survives) is to difference
/// across strikes: the shared `F` cancels, leaving
/// `[Cap(K1) − Floor(K1)] − [Cap(K2) − Floor(K2)] = DF·N·τ·(K2 − K1)`, which is
/// independent of volatility. We assert that this strike-difference matches
/// across two very different vols.
#[test]
fn test_cap_floor_parity_strike_difference_is_vol_independent() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let (k1, k2) = (0.02, 0.03);

    let strike_diff = |vol: f64| -> f64 {
        let market = build_market(as_of, vol);
        let v = |ty: InflationCapFloorType, k: f64| {
            build_option(ty, k).value(&market, as_of).unwrap().amount()
        };
        let parity_k1 =
            v(InflationCapFloorType::Caplet, k1) - v(InflationCapFloorType::Floorlet, k1);
        let parity_k2 =
            v(InflationCapFloorType::Caplet, k2) - v(InflationCapFloorType::Floorlet, k2);
        parity_k1 - parity_k2
    };

    let diff_low_vol = strike_diff(0.02);
    let diff_high_vol = strike_diff(0.05);

    // = DF·N·τ·(K2 − K1) > 0, and the convexity-adjusted forward cancels, so the
    // result must not move with vol even though Cap−Floor at a single strike does.
    assert!(
        diff_low_vol.abs() > 1.0,
        "strike-difference should be a material forward-leg value, got {diff_low_vol}"
    );
    assert!(
        (diff_low_vol - diff_high_vol).abs() <= 1e-6 * diff_low_vol.abs().max(1.0),
        "Cap−Floor put-call parity: the strike-difference must be vol-independent \
         (only the convexity-adjusted forward carries vol dependence): \
         2% vol {diff_low_vol}, 5% vol {diff_high_vol}"
    );
}
