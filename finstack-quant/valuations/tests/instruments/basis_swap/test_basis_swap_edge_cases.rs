//! Edge case and boundary condition tests for basis swaps.
//!
//! Tests validate robustness across extreme market conditions, unusual
//! instrument configurations, and boundary scenarios.

use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{currency::Currency::USD, math::interp::InterpStyle};
use finstack_quant_valuations::instruments::rates::basis_swap::{BasisSwap, BasisSwapLeg};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
}

const CALENDAR_ID: &str = "usny";

fn market() -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (0.5, 0.99), (1.0, 0.98), (5.0, 0.90)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 0.02), (0.5, 0.021), (1.0, 0.022), (5.0, 0.025)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 0.019), (0.5, 0.020), (1.0, 0.021), (5.0, 0.024)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    MarketContext::new().insert(disc).insert(f3m).insert(f1m)
}

fn make_leg(forward_curve: &str, start: Date, end: Date, spread_bp: Decimal) -> BasisSwapLeg {
    BasisSwapLeg {
        forward_curve_id: CurveId::new(forward_curve),
        discount_curve_id: CurveId::new("USD-OIS"),
        start,
        end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some(CALENDAR_ID.to_string()),
        stub: StubKind::ShortFront,
        spread_bp,
        payment_lag_days: 0,
        reset_lag_days: 0,
    }
}

#[test]
fn zero_notional() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "ZERO-NOTIONAL",
        Money::new(0.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01, MetricId::PvPrimary, MetricId::PvReference],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert_eq!(res.measures[MetricId::Dv01.as_str()], 0.0);
    assert_eq!(res.measures[MetricId::PvPrimary.as_str()], 0.0);
    assert_eq!(res.measures[MetricId::PvReference.as_str()], 0.0);

    let par_spread_result = swap.price_with_metrics(
        &ctx,
        as_of,
        &[MetricId::BasisParSpread],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );
    assert!(
        par_spread_result.is_err(),
        "BasisParSpread should error for zero notional"
    );
}

#[test]
fn very_small_notional() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "SMALL-NOTIONAL",
        Money::new(1.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, as_of).unwrap();
    assert!(npv.amount().is_finite());
    assert!(npv.amount().abs() < 10.0);
}

#[test]
fn very_large_notional() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "LARGE-NOTIONAL",
        Money::new(1_000_000_000_000.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = res
        .measures
        .get("bucketed_dv01::USD_x2dSOFR_x2d3M")
        .copied()
        .unwrap_or(0.0);

    assert!(dv01.is_finite());
    assert!(
        dv01 > 1_000_000.0,
        "Large notional should have substantial DV01: got {}",
        dv01
    );
}

#[test]
fn very_short_maturity() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "SHORT-MAT",
        Money::new(10_000_000.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2025, 2, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2025, 2, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, as_of);
    assert!(npv.is_ok(), "Short maturity swap should price");
}

#[test]
fn very_long_maturity() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "LONG-MAT",
        Money::new(10_000_000.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2055, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2055, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::AnnuityPrimary],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let annuity = res.measures[MetricId::AnnuityPrimary.as_str()];

    assert!(annuity.is_finite());
    assert!(annuity > 0.0);
}

#[test]
fn extreme_positive_spread() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "EXTREME-SPREAD",
        Money::new(10_000_000.0, USD),
        make_leg(
            "USD-SOFR-3M",
            d(2025, 1, 2),
            d(2026, 1, 2),
            Decimal::from(1000),
        ),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, as_of).unwrap();
    assert!(npv.amount().is_finite());
    assert!(npv.amount() > 500_000.0);
}

#[test]
fn extreme_negative_spread() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "EXTREME-NEG-SPREAD",
        Money::new(10_000_000.0, USD),
        make_leg(
            "USD-SOFR-3M",
            d(2025, 1, 2),
            d(2026, 1, 2),
            Decimal::from(-1000),
        ),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, as_of).unwrap();
    assert!(npv.amount().is_finite());
    assert!(npv.amount() < -500_000.0);
}

#[test]
fn flat_curves_zero_rates() {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (1.0, 1.0), (2.0, 1.0)])
        .interp(InterpStyle::LogLinear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1e-6), (1.0, 1e-6), (2.0, 1e-6)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1e-6), (1.0, 1e-6), (2.0, 1e-6)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(f3m).insert(f1m);

    let swap = BasisSwap::new(
        "ZERO-RATES",
        Money::new(10_000_000.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, d(2025, 1, 2)).unwrap();
    assert!(npv.amount().abs() < 1.0);
}

#[test]
fn negative_rates() {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (1.0, 1.005), (2.0, 1.01)])
        .interp(InterpStyle::LogLinear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1e-6), (1.0, 2e-6), (2.0, 3e-6)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1e-6), (1.0, 2e-6), (2.0, 3e-6)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(f3m).insert(f1m);

    let swap = BasisSwap::new(
        "NEG-RATES",
        Money::new(10_000_000.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            d(2025, 1, 2),
            &[MetricId::PvPrimary, MetricId::PvReference],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert!(res.measures[MetricId::PvPrimary.as_str()].is_finite());
    assert!(res.measures[MetricId::PvReference.as_str()].is_finite());
}

#[test]
fn valuation_at_maturity() {
    let ctx = market();
    let maturity = d(2026, 1, 2);

    let swap = BasisSwap::new(
        "AT-MATURITY",
        Money::new(10_000_000.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), maturity, Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), maturity, Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, maturity).unwrap();
    assert!(npv.amount().abs() < 100.0);
}

#[test]
fn valuation_after_maturity() {
    let ctx = market();
    let maturity = d(2026, 1, 2);
    let after_maturity = d(2026, 6, 1);

    let swap = BasisSwap::new(
        "AFTER-MATURITY",
        Money::new(10_000_000.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), maturity, Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), maturity, Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, after_maturity).unwrap();
    assert!(npv.amount().abs() < 1.0);
}

#[test]
fn identical_forward_curves() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new_allowing_same_curve(
        "IDENTICAL-CURVES",
        Money::new(10_000_000.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("swap construction with same curves allowed");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::PvPrimary,
                MetricId::PvReference,
                MetricId::BasisParSpread,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv_primary = res.measures[MetricId::PvPrimary.as_str()];
    let pv_reference = res.measures[MetricId::PvReference.as_str()];
    let par_spread = res.measures[MetricId::BasisParSpread.as_str()];

    assert!(
        (pv_primary - pv_reference).abs() < 1.0,
        "PVs should match for identical curves"
    );
    assert!(
        par_spread.abs() < 0.1,
        "Par spread should be near zero for identical curves"
    );
}

#[test]
fn steep_curve() {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (0.5, 0.95), (1.0, 0.85), (2.0, 0.70)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 0.01), (0.5, 0.05), (1.0, 0.10), (2.0, 0.15)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 0.01), (0.5, 0.04), (1.0, 0.09), (2.0, 0.14)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(f3m).insert(f1m);

    let swap = BasisSwap::new(
        "STEEP-CURVE",
        Money::new(10_000_000.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            d(2025, 1, 2),
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert!(res.measures[MetricId::Dv01.as_str()].is_finite());
}

#[test]
fn seasoned_swap_requires_fixings() {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 2, 15))
        .knots(vec![(0.0, 1.0), (0.5, 0.99), (1.0, 0.98), (5.0, 0.90)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(d(2025, 2, 15))
        .knots(vec![(0.0, 0.02), (0.5, 0.021), (1.0, 0.022), (5.0, 0.025)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(d(2025, 2, 15))
        .knots(vec![(0.0, 0.019), (0.5, 0.020), (1.0, 0.021), (5.0, 0.024)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(f3m).insert(f1m);

    let swap = BasisSwap::new(
        "SEASONED-NO-FIX",
        Money::new(10_000_000.0, USD),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 7, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 7, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let result = swap.value(&ctx, d(2025, 2, 15));
    assert!(
        result.is_err(),
        "Expected error for seasoned swap without fixings"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("fixing") || err_msg.contains("Seasoned"),
        "Error should mention fixings requirement: {}",
        err_msg
    );
}

#[test]
fn seasoned_swap_with_fixings_succeeds() {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (0.5, 0.99), (1.0, 0.98), (2.0, 0.96)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 0.02), (0.5, 0.021), (1.0, 0.022), (2.0, 0.023)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 0.019), (0.5, 0.020), (1.0, 0.021), (2.0, 0.022)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let fix_3m = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![(d(2024, 7, 2), 0.0195), (d(2024, 10, 2), 0.0198)],
        None,
    )
    .expect("fixings series");
    let fix_1m = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:USD-SOFR-1M",
        vec![(d(2024, 7, 2), 0.0185), (d(2024, 10, 2), 0.0188)],
        None,
    )
    .expect("fixings series");

    let ctx = MarketContext::new()
        .insert(disc)
        .insert(f3m)
        .insert(f1m)
        .insert_series(fix_3m)
        .insert_series(fix_1m);

    let swap = BasisSwap::new(
        "SEASONED-WITH-FIX",
        Money::new(10_000_000.0, USD),
        make_leg(
            "USD-SOFR-3M",
            d(2024, 7, 2),
            d(2026, 7, 2),
            Decimal::from(5),
        ),
        make_leg("USD-SOFR-1M", d(2024, 7, 2), d(2026, 7, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let result = swap.value(&ctx, d(2025, 1, 2));
    assert!(
        result.is_ok(),
        "Expected success with fixings: {:?}",
        result
    );

    let npv = result.unwrap();
    assert!(npv.amount().is_finite(), "NPV should be finite");
}
