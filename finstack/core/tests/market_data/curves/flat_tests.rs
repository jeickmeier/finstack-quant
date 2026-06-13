use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::term_structures::FlatCurve;
use finstack_core::market_data::traits::{Discounting, TermStructure};
use time::Month;

#[test]
fn test_flat_curve_construction() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(0.05, base, DayCount::Act365F, "FLAT-5%");

    assert_eq!(curve.id().as_str(), "FLAT-5%");
    assert_eq!(curve.rate(), 0.05);
    assert_eq!(curve.base_date(), base);
    assert_eq!(curve.day_count(), DayCount::Act365F);
}

#[test]
fn test_flat_curve_discounting_zero_time() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(0.10, base, DayCount::Act365F, "TEST");

    // At t=0, discount factor should be 1.0
    assert!((curve.df(0.0) - 1.0).abs() < 1e-12);
}

#[test]
fn test_flat_curve_discounting_various_tenors() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(0.10, base, DayCount::Act365F, "TEST");

    // t=1 -> df=e^-0.1
    assert!((curve.df(1.0) - (-0.1_f64).exp()).abs() < 1e-12);

    // t=2 -> df=e^-0.2
    assert!((curve.df(2.0) - (-0.2_f64).exp()).abs() < 1e-12);

    // t=0.5 -> df=e^-0.05
    assert!((curve.df(0.5) - (-0.05_f64).exp()).abs() < 1e-12);

    // t=10 -> df=e^-1.0
    assert!((curve.df(10.0) - (-1.0_f64).exp()).abs() < 1e-12);
}

#[test]
fn test_flat_curve_negative_rates() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(-0.01, base, DayCount::Act365F, "NEGATIVE");

    // Negative rates should produce discount factors > 1.0 for t > 0
    assert!(curve.df(1.0) > 1.0);
    assert!((curve.df(1.0) - (0.01_f64).exp()).abs() < 1e-12);
}

#[test]
fn test_flat_curve_zero_rate() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(0.0, base, DayCount::Act365F, "ZERO");

    // Zero rate means df(t) = 1.0 for all t
    assert!((curve.df(0.0) - 1.0).abs() < 1e-12);
    assert!((curve.df(1.0) - 1.0).abs() < 1e-12);
    assert!((curve.df(10.0) - 1.0).abs() < 1e-12);
}

#[test]
fn test_flat_curve_high_rates() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(0.50, base, DayCount::Act365F, "HIGH");

    // High rate (50%) should produce very small discount factors
    let df = curve.df(1.0);
    assert!(df < 0.65);
    assert!((df - (-0.50_f64).exp()).abs() < 1e-12);
}

#[test]
fn test_flat_curve_set_rate() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let mut curve = FlatCurve::new(0.05, base, DayCount::Act365F, "MUTABLE");

    assert_eq!(curve.rate(), 0.05);
    let df_before = curve.df(1.0);

    curve.set_rate(0.10);
    assert_eq!(curve.rate(), 0.10);

    let df_after = curve.df(1.0);
    assert!((df_after - (-0.10_f64).exp()).abs() < 1e-12);
    assert!(df_after != df_before);
}

#[test]
fn test_flat_curve_different_day_counts() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let curve_act365 = FlatCurve::new(0.05, base, DayCount::Act365F, "ACT365");
    let curve_act360 = FlatCurve::new(0.05, base, DayCount::Act360, "ACT360");
    let curve_30_360 = FlatCurve::new(0.05, base, DayCount::Thirty360, "30/360");

    assert_eq!(curve_act365.day_count(), DayCount::Act365F);
    assert_eq!(curve_act360.day_count(), DayCount::Act360);
    assert_eq!(curve_30_360.day_count(), DayCount::Thirty360);

    // All should produce same df for same year fraction
    // (day count only affects date-to-year-fraction conversion)
    assert!((curve_act365.df(1.0) - curve_act360.df(1.0)).abs() < 1e-12);
}

#[test]
fn test_flat_curve_very_small_times() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(0.05, base, DayCount::Act365F, "TEST");

    // Very small t should give df close to 1.0
    let df = curve.df(0.001);
    assert!((df - 1.0).abs() < 0.001);
    assert!((df - (-0.05 * 0.001_f64).exp()).abs() < 1e-12);
}

#[test]
fn test_flat_curve_very_large_times() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(0.05, base, DayCount::Act365F, "TEST");

    // Very large t should give very small df
    let df = curve.df(100.0);
    assert!(df < 0.01);
    assert!((df - (-5.0_f64).exp()).abs() < 1e-12);
}

#[test]
fn test_flat_curve_extrapolation_behavior() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(0.05, base, DayCount::Act365F, "TEST");

    // Flat curve should extrapolate naturally (no special behavior)
    // Just verify it doesn't panic or produce NaN
    let df_neg = curve.df(-1.0); // Extrapolate backwards (unusual but should work)
    assert!(df_neg.is_finite());
    assert!((df_neg - (0.05_f64).exp()).abs() < 1e-12); // e^(+0.05) for negative time

    let df_far = curve.df(1000.0); // Extrapolate far forward
    assert!(df_far.is_finite());
    assert!(df_far > 0.0);
}

#[test]
fn test_flat_curve_id_trait() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(0.05, base, DayCount::Act365F, "MY-CURVE-ID");

    // Test TermStructure trait
    assert_eq!(curve.id().as_str(), "MY-CURVE-ID");
}

#[test]
fn test_flat_curve_discounting_trait() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(0.05, base, DayCount::Act365F, "TEST");

    // Test Discounting trait methods
    assert_eq!(curve.base_date(), base);
    assert_eq!(curve.day_count(), DayCount::Act365F);

    let df = curve.df(1.0);
    assert!(df > 0.0 && df < 1.0);
}

#[test]
fn test_flat_curve_multiple_instances() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

    let curve1 = FlatCurve::new(0.03, base, DayCount::Act365F, "CURVE1");
    let curve2 = FlatCurve::new(0.07, base, DayCount::Act365F, "CURVE2");

    // Different curves should have different discount factors
    let df1 = curve1.df(1.0);
    let df2 = curve2.df(1.0);

    assert!(df1 > df2); // Lower rate = higher DF
    assert!((df1 - (-0.03_f64).exp()).abs() < 1e-12);
    assert!((df2 - (-0.07_f64).exp()).abs() < 1e-12);
}

#[test]
fn test_flat_curve_clone() {
    let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let curve = FlatCurve::new(0.05, base, DayCount::Act365F, "ORIGINAL");

    let cloned = curve.clone();

    assert_eq!(cloned.id().as_str(), curve.id().as_str());
    assert_eq!(cloned.rate(), curve.rate());
    assert_eq!(cloned.base_date(), curve.base_date());
    assert_eq!(cloned.day_count(), curve.day_count());
    assert!((cloned.df(1.0) - curve.df(1.0)).abs() < 1e-12);
}
