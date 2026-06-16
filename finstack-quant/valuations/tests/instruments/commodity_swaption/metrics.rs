//! Metrics tests for commodity swaptions.
//!
//! Exercises the registered-but-previously-untested rate-risk calculators:
//! `Dv01` and `BucketedDv01`.

use crate::finstack_quant_test_utils::{
    date, flat_discount_with_tenor, flat_price_curve, flat_vol_surface,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor, TenorUnit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::commodity::commodity_swaption::CommoditySwaption;
use finstack_quant_valuations::instruments::{CommodityUnderlyingParams, Instrument, OptionType};
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::sync::Arc;

/// Build a NG commodity swaption (1y pay-fixed receiver schedule) plus its
/// market: a discount curve, a price (forward) curve, and a flat vol surface.
fn ng_swaption() -> (CommoditySwaption, MarketContext, Date) {
    let as_of = date(2025, 1, 1);

    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.03, 2.0);
    let price_curve = flat_price_curve("NG-FORWARD", as_of, 3.50, 2.0);
    let vol_surface = flat_vol_surface("NG-VOL", &[0.25, 0.5, 1.0, 2.0], &[2.0, 3.5, 5.0], 0.30);

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
        .insert_surface(vol_surface);

    let swaption = CommoditySwaption::builder()
        .id(InstrumentId::new("NG-SWAPTION-METRICS"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "NG",
            "MMBTU",
            Currency::USD,
        ))
        .option_type(OptionType::Call)
        .expiry(date(2025, 6, 15))
        .swap_start(date(2025, 7, 1))
        .swap_end(date(2026, 6, 30))
        .swap_frequency(Tenor::new(1, TenorUnit::Months))
        .fixed_price(3.50)
        .notional(10_000.0)
        .forward_curve_id(CurveId::new("NG-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("NG-VOL"))
        .build()
        .expect("should build");
    (swaption, market, as_of)
}

/// `Dv01` and `BucketedDv01` are registered but were previously unexercised.
/// Both must compute to finite values and the bucketed aggregate must reconcile
/// with the parallel DV01.
#[test]
fn test_commodity_swaption_dv01_and_bucketed_dv01() -> finstack_quant_core::Result<()> {
    let registry = standard_registry();
    let (swaption, market, as_of) = ng_swaption();
    let pv = swaption.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(swaption),
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
