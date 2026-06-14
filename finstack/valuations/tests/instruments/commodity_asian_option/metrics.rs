//! Metrics tests for commodity Asian options.
//!
//! Exercises the registered-but-previously-untested metric calculators:
//! `Delta`, `Vega`, `Dv01`, and `BucketedDv01`.

use crate::finstack_test_utils::{
    date, flat_discount_with_tenor, flat_price_curve, flat_vol_surface,
};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::commodity::commodity_asian_option::CommodityAsianOption;
use finstack_valuations::instruments::{
    Attributes, AveragingMethod, CommodityUnderlyingParams, Instrument, OptionType,
    PricingOverrides,
};
use finstack_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::sync::Arc;

/// Build an ATM (forward == strike) arithmetic-average commodity Asian option
/// with monthly fixings, plus its market. The flat forward equals the strike so
/// delta is a clean interior value.
fn atm_asian_option(option_type: OptionType) -> (CommodityAsianOption, MarketContext, Date) {
    let as_of = date(2025, 1, 1);
    let settlement = date(2025, 7, 2);
    let strike = 75.0;

    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.03, 2.0);
    // Asian's `forward_curve_id` resolves a PriceCurve (flat forward == strike).
    let price_curve = flat_price_curve("CL-FORWARD", as_of, strike, 2.0);
    let vol_surface = flat_vol_surface("CL-VOL", &[0.25, 0.5, 1.0, 2.0], &[60.0, 75.0, 90.0], 0.25);

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
        .insert_surface(vol_surface);

    let fixing_dates = vec![
        date(2025, 1, 31),
        date(2025, 2, 28),
        date(2025, 3, 31),
        date(2025, 4, 30),
        date(2025, 5, 31),
        date(2025, 6, 30),
    ];

    let option = CommodityAsianOption::builder()
        .id(InstrumentId::new("CL-ASIAN-METRICS"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(strike)
        .option_type(option_type)
        .averaging_method(AveragingMethod::Arithmetic)
        .fixing_dates(fixing_dates)
        .quantity(1000.0)
        .expiry(settlement)
        .forward_curve_id(CurveId::new("CL-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .day_count(finstack_core::dates::DayCount::Act365F)
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("should build");
    (option, market, as_of)
}

/// `Delta` and `Vega` are registered but were previously unexercised. The Asian
/// option is vanilla-like: a long call has positive delta, a long put negative
/// delta, and any long option has positive vega.
#[test]
fn test_commodity_asian_option_delta_and_vega_signs() -> finstack_core::Result<()> {
    let registry = standard_registry();

    // Call: positive delta, positive vega.
    let (call, market, as_of) = atm_asian_option(OptionType::Call);
    let pv = call.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(call),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let res = registry.compute(&[MetricId::Delta, MetricId::Vega], &mut ctx)?;
    let call_delta = *res.get(&MetricId::Delta).expect("delta");
    let vega = *res.get(&MetricId::Vega).expect("vega");
    assert!(
        call_delta.is_finite() && call_delta > 0.0,
        "long call delta should be positive, got {call_delta}"
    );
    assert!(
        vega.is_finite() && vega > 0.0,
        "long option vega should be positive, got {vega}"
    );

    // Put: negative delta.
    let (put, market_p, as_of_p) = atm_asian_option(OptionType::Put);
    let pv_p = put.value(&market_p, as_of_p)?;
    let mut ctx_p = MetricContext::new(
        Arc::new(put),
        Arc::new(market_p),
        as_of_p,
        pv_p,
        MetricContext::default_config(),
    );
    let put_delta = *registry
        .compute(&[MetricId::Delta], &mut ctx_p)?
        .get(&MetricId::Delta)
        .expect("delta");
    assert!(
        put_delta.is_finite() && put_delta < 0.0,
        "long put delta should be negative, got {put_delta}"
    );

    Ok(())
}

/// `Dv01` and `BucketedDv01` are registered but were previously unexercised.
/// Both must compute to finite values and the bucketed aggregate must reconcile
/// with the parallel DV01.
#[test]
fn test_commodity_asian_option_dv01_and_bucketed_dv01() -> finstack_core::Result<()> {
    let registry = standard_registry();
    let (call, market, as_of) = atm_asian_option(OptionType::Call);
    let pv = call.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(call),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let res = registry.compute(&[MetricId::Dv01, MetricId::BucketedDv01], &mut ctx)?;
    let dv01 = *res.get(&MetricId::Dv01).expect("dv01");
    let bucketed = *res.get(&MetricId::BucketedDv01).expect("bucketed_dv01");

    assert!(dv01.is_finite(), "DV01 should be finite, got {dv01}");
    assert!(
        bucketed.is_finite(),
        "BucketedDv01 aggregate should be finite, got {bucketed}"
    );
    assert!(
        (bucketed - dv01).abs() <= 1.0 + 0.05 * dv01.abs(),
        "BucketedDv01 ({bucketed}) should reconcile with parallel DV01 ({dv01})"
    );

    Ok(())
}
