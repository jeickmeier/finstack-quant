use super::*;
use crate::dates::calendar_by_id_strict;
use crate::dates::{Date, DayCount};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};

#[test]
fn flat_curve_uses_continuous_rate_at_all_maturities() {
    let base = Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");
    let curve = DiscountCurve::flat("USD-OIS", base, 0.04, DayCount::Act365F).expect("flat curve");
    for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
        let expected = crate::math::Compounding::Continuous
            .df_from_rate(0.04, t)
            .expect("valid df");
        assert!((curve.df(t).expect("df") - expected).abs() < 1e-12);
    }
}

#[test]
fn flat_curve_supports_zero_and_negative_continuous_rates() {
    let base = Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");

    for rate in [0.0, -0.01] {
        let curve = DiscountCurve::flat("EUR-OIS", base, rate, DayCount::Act365F)
            .expect("flat discount curve");
        for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
            let expected = crate::math::Compounding::Continuous
                .df_from_rate(rate, t)
                .expect("valid df");
            assert!((curve.df(t).expect("df") - expected).abs() < 1e-12);
        }
        assert!((curve.forward(2.0, 9.0).expect("flat forward") - rate).abs() < 1e-12);
    }
}

#[test]
fn flat_curve_matches_manually_built_continuous_curve() {
    let base = Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");
    let rate = 0.04;
    let one_year_df = crate::math::Compounding::Continuous
        .df_from_rate(rate, 1.0)
        .expect("valid df");
    let flat =
        DiscountCurve::flat("USD-OIS", base, rate, DayCount::Act365F).expect("flat discount curve");
    let manual = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, one_year_df)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("manual continuous curve");

    for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
        assert!((flat.df(t).expect("df") - manual.df(t).expect("df")).abs() < 1e-12);
    }
}

#[test]
fn optional_rate_calibration_round_trips_as_none() {
    let canonical = serde_json::json!({
        "id": "USD-OIS",
        "base": "2025-01-02",
        "day_count": "act_365f",
        "knot_points": [[0.0, 1.0], [5.0, 0.8]],
        "interp_style": "linear",
        "extrapolation": "extend",
        "rate_calibration": null,
        "fx_policy": null
    });

    let curve: DiscountCurve =
        serde_json::from_value(canonical).expect("canonical serialized curve");
    let serialized = serde_json::to_value(curve).expect("serialize curve");
    let restored: DiscountCurve =
        serde_json::from_value(serialized.clone()).expect("round-trip curve");

    assert!(serialized["rate_calibration"].is_null());
    assert!(restored.rate_calibration().is_none());
    assert!(serialized.get("calendar_id").is_none());
}

#[test]
fn bus252_curve_calendar_round_trips_and_survives_roll_and_bump() {
    use crate::dates::DayCountContext;
    use crate::market_data::bumps::{BumpSpec, Bumpable};
    use crate::market_data::traits::Discounting;

    let base = Date::from_calendar_date(2025, time::Month::January, 6).expect("base");
    let pay = Date::from_calendar_date(2025, time::Month::February, 3).expect("pay");
    let curve = DiscountCurve::builder("BRL-CDI")
        .base_date(base)
        .day_count(DayCount::Bus252)
        .calendar_id("brbd")
        .knots([(0.0, 1.0), (1.0, 0.9)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("Bus/252 curve with calendar");

    let brbd = calendar_by_id_strict("brbd").expect("brbd");
    let business_days = brbd
        .count_business_days(base, pay)
        .expect("calendar lookup");
    let t = DayCount::Bus252
        .year_fraction(
            base,
            pay,
            DayCountContext {
                calendar: Some(brbd),
                ..DayCountContext::default()
            },
        )
        .expect("bus/252 time");
    assert_eq!(t, f64::from(business_days) / 252.0);
    let df = curve
        .df_between_dates(base, pay)
        .expect("curve-owned calendar");
    assert!((df - curve.df(t).expect("df")).abs() < 1e-15);

    let json = serde_json::to_value(&curve).expect("serialize");
    assert_eq!(json["calendar_id"], "brbd");
    let restored: DiscountCurve = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored.calendar_id(), Some("brbd"));
    assert_eq!(restored.df_between_dates(base, pay).expect("restored"), df);

    let rolled = curve
        .condition(
            Date::from_calendar_date(2025, time::Month::January, 13).expect("reference date"),
        )
        .expect("condition");
    assert_eq!(rolled.calendar_id(), Some("brbd"));
    let bumped = curve
        .apply_bump(BumpSpec::parallel_bp(1.0))
        .expect("bump keeps calendar");
    assert_eq!(bumped.calendar_id(), Some("brbd"));
    assert!(Discounting::day_count_context(&bumped).calendar.is_some());
}
