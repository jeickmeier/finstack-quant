//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{currency::Currency::USD, math::interp::InterpStyle};
use finstack_quant_valuations_core::instruments::FloatLegSpec;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_rates::basis_swap::BasisSwap;
use rust_decimal::Decimal;
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
}

const CALENDAR_ID: &str = "usny";

fn market() -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (0.5, 0.99), (1.0, 0.98), (2.0, 0.96)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.997487536532938),
            (0.25, 0.994950292856076),
            (0.375, 0.992388490374784),
            (0.5, 0.98980235222721),
            (0.625, 0.98719210325196),
            (0.75, 0.984557969955606),
            (0.875, 0.981900180480004),
            (1.0, 0.97921896456946),
            (1.125, 0.976522182587467),
            (1.25, 0.973817611559672),
            (1.375, 0.971105357458722),
            (1.5, 0.96838552632258),
            (1.625, 0.965658224247723),
            (1.75, 0.962923557382358),
            (1.875, 0.96018163191965),
            (2.0, 0.957432554090967),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.9976122302682),
            (0.25, 0.995199061524078),
            (0.375, 0.992760705844714),
            (0.5, 0.990297377149241),
            (0.625, 0.987809291167626),
            (0.75, 0.985296665409241),
            (0.875, 0.982759719131244),
            (1.0, 0.980198673306755),
            (1.125, 0.97762138823012),
            (1.25, 0.975035644686228),
            (1.375, 0.972441545744155),
            (1.5, 0.969839194590702),
            (1.625, 0.96722869452364),
            (1.75, 0.964610148944963),
            (1.875, 0.961983661354156),
            (2.0, 0.959349335341472),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    // Provide fixings for theta calculations where the rolled as_of date
    // puts the first reset in the past.
    let fix_3m = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![(d(2025, 1, 2), 0.02)],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("fixings series");
    let fix_1m = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:USD-SOFR-1M",
        vec![(d(2025, 1, 2), 0.019)],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("fixings series");
    MarketContext::new()
        .insert(disc)
        .insert(f3m)
        .insert(f1m)
        .insert_series(fix_3m)
        .insert_series(fix_1m)
}

fn swap() -> BasisSwap {
    BasisSwap::new(
        "BASIS-TEST",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap")
}

#[test]
fn dv01_metrics() {
    let s = swap();
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let res = s
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::Dv01,
                MetricId::BasisParSpread,
                MetricId::PvPrimary,
                MetricId::PvReference,
            ],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = res.measures[MetricId::Dv01.as_str()];

    // Dv01 is now configured in PerCurve mode, so it returns the sum of individual curve sensitivities
    // and stores the breakdown in measures with reversible composite curve keys.

    // Extract per-curve DV01s from measures using composite keys
    let dv01_discount = res
        .measures
        .get("bucketed_dv01::USD-OIS")
        .copied()
        .expect("discount-curve bucketed DV01 must be present");
    let dv01_primary_fwd = res
        .measures
        .get("bucketed_dv01::USD-SOFR-3M")
        .copied()
        .expect("primary-forward bucketed DV01 must be present");
    let dv01_reference_fwd = res
        .measures
        .get("bucketed_dv01::USD-SOFR-1M")
        .copied()
        .expect("reference-forward bucketed DV01 must be present");

    // Total DV01 should equal sum of individual curve sensitivities
    assert!((dv01 - (dv01_discount + dv01_primary_fwd + dv01_reference_fwd)).abs() < 1e-6);

    // All DV01 components should be finite
    assert!(dv01.is_finite());
    assert!(dv01_discount.is_finite());
    assert!(dv01_primary_fwd.is_finite());
    assert!(dv01_reference_fwd.is_finite());

    // Par spread should move opposite to PV mismatch
    let pv_p = res.measures[MetricId::PvPrimary.as_str()];
    let pv_r = res.measures[MetricId::PvReference.as_str()];
    let spread = res.measures[MetricId::BasisParSpread.as_str()];
    // If legs are balanced (equal PV), par spread close to 0
    assert!(pv_p.is_finite() && pv_r.is_finite());
    assert!(spread.is_finite());
}
