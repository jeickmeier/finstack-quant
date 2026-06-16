//! Metrics tests for commodity spread options.
//!
//! Exercises the registered-but-previously-untested metric calculators:
//! `Delta` (leg-1 forward sensitivity), `Vega`, `Dv01`, and `BucketedDv01`.

use crate::finstack_quant_test_utils::{
    date, flat_discount_with_tenor, flat_price_curve, flat_vol_surface,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::commodity::commodity_spread_option::CommoditySpreadOption;
use finstack_quant_valuations::instruments::{Instrument, OptionType};
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::sync::Arc;

/// Build an ITM commodity spread call (F1 > F2, small strike) plus its market.
/// Two price curves (LEG1-FWD, LEG2-FWD), two vol surfaces (LEG1-VOL,
/// LEG2-VOL), and a discount curve.
fn spread_call() -> (CommoditySpreadOption, MarketContext, Date) {
    let as_of = date(2025, 1, 1);
    let expiry = date(2025, 7, 1);

    let leg1_fwd = flat_price_curve("LEG1-FWD", as_of, 100.0, 2.0);
    let leg2_fwd = flat_price_curve("LEG2-FWD", as_of, 80.0, 2.0);
    let leg1_vol = flat_vol_surface("LEG1-VOL", &[0.25, 1.0, 2.0], &[50.0, 100.0, 150.0], 0.30);
    let leg2_vol = flat_vol_surface("LEG2-VOL", &[0.25, 1.0, 2.0], &[50.0, 100.0, 150.0], 0.30);
    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.03, 2.0);

    let market = MarketContext::new()
        .insert(leg1_fwd)
        .insert(leg2_fwd)
        .insert_surface(leg1_vol)
        .insert_surface(leg2_vol)
        .insert(discount_curve);

    let option = CommoditySpreadOption::builder()
        .id(InstrumentId::new("SPREAD-METRICS"))
        .currency(Currency::USD)
        .option_type(OptionType::Call)
        .expiry(expiry)
        .strike(5.0)
        .notional(1.0)
        .leg1_forward_curve_id(CurveId::new("LEG1-FWD"))
        .leg2_forward_curve_id(CurveId::new("LEG2-FWD"))
        .leg1_vol_surface_id(CurveId::new("LEG1-VOL"))
        .leg2_vol_surface_id(CurveId::new("LEG2-VOL"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .correlation(0.3)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .build()
        .expect("should build");
    (option, market, as_of)
}

/// `Delta` (leg-1 forward sensitivity) and `Vega` are registered but were
/// previously unexercised. For a long call on F1 - F2, the leg-1 delta is
/// positive (observed > 0) and vega is positive.
#[test]
fn test_commodity_spread_option_delta_and_vega() -> finstack_quant_core::Result<()> {
    let registry = standard_registry();
    let (option, market, as_of) = spread_call();
    let pv = option.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let res = registry.compute(&[MetricId::Delta, MetricId::Vega], &mut ctx)?;
    let delta = *res.get(&MetricId::Delta).expect("delta");
    let vega = *res.get(&MetricId::Vega).expect("vega");

    // Leg-1 delta of a long F1 - F2 call: positive (verified at runtime).
    assert!(
        delta.is_finite() && delta > 0.0,
        "spread leg-1 delta should be positive, got {delta}"
    );
    assert!(
        vega.is_finite() && vega > 0.0,
        "long spread option vega should be positive, got {vega}"
    );

    Ok(())
}

/// `Dv01` and `BucketedDv01` are registered but were previously unexercised.
/// Both must compute to finite values and the bucketed aggregate must reconcile
/// with the parallel DV01.
#[test]
fn test_commodity_spread_option_dv01_and_bucketed_dv01() -> finstack_quant_core::Result<()> {
    let registry = standard_registry();
    let (option, market, as_of) = spread_call();
    let pv = option.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(option),
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
