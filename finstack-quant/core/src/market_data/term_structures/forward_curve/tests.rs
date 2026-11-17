//! Unit tests for factor-native [`ForwardCurve`] construction, evaluation, bumps, and conditioning.

use super::ForwardCurve;
use crate::dates::{Date, DayCount, Tenor};
use crate::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable};
use crate::market_data::term_structures::DiscountCurve;
use crate::math::Compounding;
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use time::Month;

fn d(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid date")
}

#[test]
fn flat_and_zero_rate_and_dated_builders_construct_positive_factors() {
    let base = d(2025, Month::January, 2);
    let flat = ForwardCurve::flat("USD-SOFR-3M", base, 0.04, DayCount::Act360).expect("flat");
    assert_eq!(flat.index_tenor(), None);
    let f1 = flat.df(1.0).expect("df(1)");
    assert!((f1 - (-0.04_f64).exp()).abs() < 1e-12);
    let r = flat.rate_between(0.0, 0.25, 0.25).expect("3m rate");
    let expected = ((0.04_f64 * 0.25).exp() - 1.0) / 0.25;
    assert!((r - expected).abs() < 1e-12);

    let zr = ForwardCurve::from_zero_rates(
        "EUR-EURIBOR-3M",
        base,
        &[(0.5, -0.005), (2.0, 0.01)],
        Compounding::Continuous,
        DayCount::Act360,
        InterpStyle::LogLinear,
        ExtrapolationPolicy::Extend,
    )
    .expect("negative zero rate allowed");
    assert!(zr.df(0.5).expect("neg rate factor > 1") > 1.0);
    assert!(zr.rate_between(0.0, 0.5, 0.5).expect("neg simple rate") < 0.0);

    let dated = ForwardCurve::from_dates(
        "DATED-FWD",
        base,
        &[
            (d(2025, Month::July, 2), 0.98),
            (d(2026, Month::January, 2), 0.96),
        ],
        DayCount::Act365F,
        InterpStyle::Linear,
        ExtrapolationPolicy::Flat,
    )
    .expect("from_dates");
    assert!((dated.df_on_date(d(2025, Month::July, 2)).expect("df") - 0.98).abs() < 1e-12);
}

#[test]
fn supports_all_five_interp_styles_including_monotone_convex_and_bus252() {
    let base = d(2025, Month::January, 2);
    for style in [
        InterpStyle::Linear,
        InterpStyle::LogLinear,
        InterpStyle::CubicHermite,
        InterpStyle::PiecewiseQuadraticForward,
        InterpStyle::MonotoneConvex,
    ] {
        let curve = ForwardCurve::builder("USD-SOFR-3M")
            .base_date(base)
            .day_count(DayCount::Bus252)
            .calendar_id("usny")
            .index_tenor(Tenor::quarterly())
            .knots([(1.0, 0.96), (2.0, 0.92), (5.0, 0.82)])
            .interp(style)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .expect("all styles including MonotoneConvex supported");
        assert_eq!(curve.calendar_id(), Some("usny"));
        assert_eq!(curve.index_tenor(), Some(Tenor::quarterly()));
        let inst = curve.instantaneous_forward(1.5).expect("inst fwd");
        assert!(inst.is_finite());
    }

    assert!(
        ForwardCurve::builder("NO-CAL")
            .base_date(base)
            .day_count(DayCount::Bus252)
            .knots([(1.0, 0.96), (2.0, 0.92)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .is_err()
    );
}

#[test]
fn rate_between_dates_handles_pre_spot_and_thirty360_mar30_to_mar31() {
    let base = d(2025, Month::April, 1);
    let pre_spot = d(2025, Month::March, 28);
    let end = d(2025, Month::June, 28);

    let extend = ForwardCurve::flat("EXT", base, 0.04, DayCount::Act360).expect("flat");
    let pre_factor = extend
        .df_on_date(pre_spot)
        .expect("pre-origin factor admitted");
    assert!(pre_factor > 1.0);
    let pre_rate = extend
        .rate_between_dates(pre_spot, end, 92.0 / 360.0)
        .expect("pre-spot reset admitted under Extend tail");
    assert!(pre_rate.is_finite() && pre_rate > 0.0);

    let err_tail = ForwardCurve::builder("ERR")
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.96)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Error)
        .build()
        .expect("err tail");
    assert!(err_tail.df_on_date(pre_spot).is_err());
    assert!(err_tail.rate_between_dates(pre_spot, end, 0.25).is_err());

    // ThirtyE360 clamps day 31 -> 30 unconditionally, so 2025-03-30 and 2025-03-31 map to
    // equal source coordinates and return 0.0 once both endpoint factors admit.
    let c30 = ForwardCurve::builder("C30")
        .base_date(d(2025, Month::January, 1))
        .day_count(DayCount::ThirtyE360)
        .knots([(0.0, 1.0), (1.0, 0.96)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("thirty360 curve");
    let mar30 = d(2025, Month::March, 30);
    let mar31 = d(2025, Month::March, 31);
    assert_eq!(
        c30.rate_between_dates(mar30, mar31, 1.0 / 360.0)
            .expect("equal coordinate admits 0.0"),
        0.0
    );
}

#[test]
fn from_discount_curve_bumps_and_conditions_match_discount_curve() {
    let base = d(2025, Month::January, 2);
    let roll = d(2025, Month::July, 2);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.5, 0.985), (1.0, 0.968), (2.0, 0.935), (5.0, 0.84)])
        .interp(InterpStyle::MonotoneConvex)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("disc")
        .apply_bump(BumpSpec::parallel_bp(15.0))
        .expect("bumped disc")
        .condition(roll)
        .expect("conditioned disc");

    let fwd = ForwardCurve::from_discount_curve("USD-SOFR-3M", &disc, Some(Tenor::quarterly()));
    assert_eq!(fwd.base_date(), disc.base_date());
    assert_eq!(fwd.source_base_date(), disc.source_base_date());
    assert_eq!(fwd.index_tenor(), Some(Tenor::quarterly()));
    assert!(fwd.rate_calibration().is_none());

    for t in [-0.25, 0.0, 0.5, 1.25, 4.0] {
        assert!((fwd.df(t).expect("fwd df") - disc.df(t).expect("disc df")).abs() < 1e-13);
        assert!(
            (fwd.instantaneous_forward(t).expect("fwd inst")
                - disc.instantaneous_forward(t).expect("disc inst"))
            .abs()
                < 1e-13
        );
    }

    // Rejects multiplicative shocks.
    assert!(fwd.apply_bump(BumpSpec::multiplier(1.1)).is_err());
    assert!(
        fwd.apply_bump(BumpSpec {
            mode: BumpMode::Multiplicative,
            units: BumpUnits::Factor,
            value: 1.05,
            bump_type: BumpType::Parallel,
        })
        .is_err()
    );

    // JSON round-trip preserves conditioned transform state.
    let json = serde_json::to_string(&fwd).expect("serialize");
    let roundtrip: ForwardCurve = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(roundtrip.source_base_date(), fwd.source_base_date());
    assert!((roundtrip.df(1.25).expect("rt df") - fwd.df(1.25).expect("df")).abs() < 1e-13);
}
