//! Integration tests for factor-native [`ForwardCurve`] functionality.

use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::bumps::{BumpSpec, Bumpable};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::Compounding;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use time::Month;

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 2).expect("valid date")
}

#[test]
fn flat_forward_curve_projects_exact_continuous_factors() {
    let curve =
        ForwardCurve::flat("USD-SOFR-3M", test_date(), 0.04, DayCount::Act360).expect("flat");
    assert!((curve.df(0.0).expect("df0") - 1.0).abs() < 1e-14);
    assert!((curve.df(1.0).expect("df1") - (-0.04_f64).exp()).abs() < 1e-12);
    assert!((curve.df(10.0).expect("df10") - (-0.40_f64).exp()).abs() < 1e-12);
    assert!(ForwardCurve::flat("BAD", test_date(), f64::NAN, DayCount::Act360).is_err());
    assert!(ForwardCurve::flat("BAD", test_date(), 0.04, DayCount::ActActIsma).is_err());
}

#[test]
fn clone_is_panic_free_and_equivalent() {
    let original = ForwardCurve::builder("USD-SOFR3M")
        .base_date(test_date())
        .day_count(DayCount::Act360)
        .index_tenor(Tenor::quarterly())
        .knots([
            (0.0, 1.0),
            (0.25, 0.992),
            (0.5, 0.983),
            (1.0, 0.964),
            (2.0, 0.925),
            (5.0, 0.815),
        ])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("curve builds");

    let cloned = original.clone();
    assert_eq!(original.id(), cloned.id());
    assert_eq!(original.base_date(), cloned.base_date());
    assert_eq!(original.day_count(), cloned.day_count());
    assert_eq!(original.index_tenor(), cloned.index_tenor());
    assert_eq!(original.knots(), cloned.knots());
    assert_eq!(original.factors(), cloned.factors());

    for t in [-0.25, 0.0, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0] {
        assert!((original.df(t).expect("df") - cloned.df(t).expect("cloned df")).abs() < 1e-14);
    }
}

#[test]
fn builder_and_serde_support_all_styles_and_reject_missing_or_invalid_inputs() {
    let styles = [
        InterpStyle::Linear,
        InterpStyle::LogLinear,
        InterpStyle::CubicHermite,
        InterpStyle::PiecewiseQuadraticForward,
        InterpStyle::MonotoneConvex,
    ];

    for style in styles {
        let original = ForwardCurve::builder("EUR-EURIBOR6M")
            .base_date(test_date())
            .day_count(DayCount::Act360)
            .index_tenor(Tenor::semi_annual())
            .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.935), (5.0, 0.83)])
            .interp(style)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .expect("style builds");

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: ForwardCurve = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original.index_tenor(), deserialized.index_tenor());
        for t in [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0] {
            assert!(
                (original.df(t).expect("orig") - deserialized.df(t).expect("deser")).abs() < 1e-12
            );
        }
    }

    assert!(
        ForwardCurve::builder("MISSING-BASE")
            .day_count(DayCount::Act360)
            .knots([(1.0, 0.96)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .is_err()
    );
    assert!(
        ForwardCurve::builder("NON-POS")
            .base_date(test_date())
            .day_count(DayCount::Act360)
            .knots([(0.0, 1.0), (1.0, 0.0)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .is_err()
    );
}

#[test]
fn rate_between_dates_uses_explicit_accrual_and_admits_endpoints() {
    let curve = ForwardCurve::from_zero_rates(
        "USD-SOFR3M",
        test_date(),
        &[(0.5, 0.045), (2.0, 0.045)],
        Compounding::Continuous,
        DayCount::Act365F,
        InterpStyle::LogLinear,
        ExtrapolationPolicy::Extend,
    )
    .expect("zero rates curve");

    let start = test_date() + time::Duration::days(90);
    let end = start + time::Duration::days(90);
    let alpha_act360 = 90.0 / 360.0;
    let r = curve
        .rate_between_dates(start, end, alpha_act360)
        .expect("explicit accrual rate");
    let growth = (curve.df_on_date(start).expect("p1") / curve.df_on_date(end).expect("p2")) - 1.0;
    assert!((r - growth / alpha_act360).abs() < 1e-13);

    // Pre-origin factor queries respect Error tails.
    let err_curve = ForwardCurve::builder("ERR")
        .base_date(test_date())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (2.0, 0.92)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Error)
        .build()
        .expect("err curve");
    let pre_start = test_date() - time::Duration::days(30);
    assert!(err_curve.rate_between_dates(pre_start, end, 0.25).is_err());
    assert!(err_curve.df_between_dates(pre_start, end).is_err());
}

#[test]
fn from_discount_curve_preserves_transform_bump_and_condition() {
    let base = test_date();
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.96), (3.0, 0.88), (5.0, 0.80)])
        .interp(InterpStyle::MonotoneConvex)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("disc")
        .apply_bump(BumpSpec::parallel_bp(5.0))
        .expect("bump");

    let fwd = ForwardCurve::from_discount_curve("USD-SOFR3M", &disc, Some(Tenor::quarterly()));
    let roll_date = Date::from_calendar_date(2025, Month::July, 2).expect("roll");
    let rolled_disc = disc.condition(roll_date).expect("rolled disc");
    let rolled_fwd = fwd.condition(roll_date).expect("rolled fwd");

    for target in [
        Date::from_calendar_date(2025, Month::December, 15).expect("d1"),
        Date::from_calendar_date(2027, Month::January, 2).expect("d2"),
    ] {
        assert!(
            (rolled_fwd.df_on_date(target).expect("fwd df")
                - rolled_disc.df_on_date(target).expect("disc df"))
            .abs()
                < 1e-13
        );
    }
}
