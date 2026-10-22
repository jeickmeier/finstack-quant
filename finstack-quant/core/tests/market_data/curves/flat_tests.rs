//! Tests for flat continuously-compounded discount curves (`DiscountCurve::flat`).
//!
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).expect("valid date")
}

#[test]
fn test_flat_curve_construction() {
    let base = base_date();
    let curve = DiscountCurve::flat("FLAT-5%", base, 0.05).expect("valid flat curve");

    assert_eq!(curve.id().as_str(), "FLAT-5%");
    assert_eq!(curve.base_date(), base);
    assert_eq!(curve.day_count(), DayCount::Act365F);
}

#[test]
fn test_flat_curve_discounting_zero_time() {
    let curve = DiscountCurve::flat("TEST", base_date(), 0.10).expect("valid flat curve");

    // At t=0, discount factor should be 1.0
    assert!((curve.df(0.0) - 1.0).abs() < 1e-12);
}

#[test]
fn test_flat_curve_discounting_various_tenors() {
    let curve = DiscountCurve::flat("TEST", base_date(), 0.10).expect("valid flat curve");

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
    let curve = DiscountCurve::flat("NEGATIVE", base_date(), -0.01).expect("valid flat curve");

    // Negative rates should produce discount factors > 1.0 for t > 0
    assert!(curve.df(1.0) > 1.0);
    assert!((curve.df(1.0) - (0.01_f64).exp()).abs() < 1e-12);
}

#[test]
fn test_flat_curve_zero_rate() {
    let curve = DiscountCurve::flat("ZERO", base_date(), 0.0).expect("valid flat curve");

    // Zero rate means df(t) = 1.0 for all t
    assert!((curve.df(0.0) - 1.0).abs() < 1e-12);
    assert!((curve.df(1.0) - 1.0).abs() < 1e-12);
    assert!((curve.df(10.0) - 1.0).abs() < 1e-12);
}

#[test]
fn test_flat_curve_high_rates() {
    let curve = DiscountCurve::flat("HIGH", base_date(), 0.50).expect("valid flat curve");

    // High rate (50%) should produce very small discount factors
    let df = curve.df(1.0);
    assert!(df < 0.65);
    assert!((df - (-0.50_f64).exp()).abs() < 1e-12);
}

#[test]
fn test_flat_curve_non_finite_rate_rejected() {
    let base = base_date();

    assert!(DiscountCurve::flat("NAN", base, f64::NAN).is_err());
    assert!(DiscountCurve::flat("INF", base, f64::INFINITY).is_err());
}

#[test]
fn test_flat_curve_very_small_times() {
    let curve = DiscountCurve::flat("TEST", base_date(), 0.05).expect("valid flat curve");

    // Very small t should give df close to 1.0
    let df = curve.df(0.001);
    assert!((df - 1.0).abs() < 0.001);
    assert!((df - (-0.05 * 0.001_f64).exp()).abs() < 1e-12);
}

#[test]
fn test_flat_curve_very_large_times() {
    let curve = DiscountCurve::flat("TEST", base_date(), 0.05).expect("valid flat curve");

    // Very large t should give very small df
    let df = curve.df(100.0);
    assert!(df < 0.01);
    assert!((df - (-5.0_f64).exp()).abs() < 1e-12);
}

#[test]
fn test_flat_curve_id_trait() {
    let curve = DiscountCurve::flat("MY-CURVE-ID", base_date(), 0.05).expect("valid flat curve");

    // Test TermStructure trait
    assert_eq!(curve.id().as_str(), "MY-CURVE-ID");
}

#[test]
fn test_flat_curve_discounting_trait() {
    let base = base_date();
    let curve = DiscountCurve::flat("TEST", base, 0.05).expect("valid flat curve");

    // Test Discounting trait methods
    assert_eq!(curve.base_date(), base);
    assert_eq!(curve.day_count(), DayCount::Act365F);

    let df = curve.df(1.0);
    assert!(df > 0.0 && df < 1.0);
}

#[test]
fn test_flat_curve_multiple_instances() {
    let base = base_date();

    let curve1 = DiscountCurve::flat("CURVE1", base, 0.03).expect("valid flat curve");
    let curve2 = DiscountCurve::flat("CURVE2", base, 0.07).expect("valid flat curve");

    // Different curves should have different discount factors
    let df1 = curve1.df(1.0);
    let df2 = curve2.df(1.0);

    assert!(df1 > df2); // Lower rate = higher DF
    assert!((df1 - (-0.03_f64).exp()).abs() < 1e-12);
    assert!((df2 - (-0.07_f64).exp()).abs() < 1e-12);
}

#[test]
fn test_flat_curve_clone() {
    let base = base_date();
    let curve = DiscountCurve::flat("ORIGINAL", base, 0.05).expect("valid flat curve");

    let cloned = curve.clone();

    assert_eq!(cloned.id().as_str(), curve.id().as_str());
    assert_eq!(cloned.base_date(), curve.base_date());
    assert_eq!(cloned.day_count(), curve.day_count());
    assert!((cloned.df(1.0) - curve.df(1.0)).abs() < 1e-12);
}

#[test]
fn test_flat_curve_rejects_percentage_typed_as_decimal() {
    let err = DiscountCurve::flat("PCT", base_date(), 4.0).expect_err("4.0 must be rejected");
    assert!(err.to_string().contains("decimal fractions"), "{err}");
    assert!(DiscountCurve::flat("NEG", base_date(), -0.5).is_ok());
}

#[test]
fn test_from_zero_rates_continuous_matches_flat() {
    use finstack_quant_core::math::Compounding;
    let curve = DiscountCurve::from_zero_rates(
        "ZR",
        base_date(),
        &[(1.0, 0.05), (2.0, 0.05), (5.0, 0.05)],
        Compounding::Continuous,
    )
    .expect("valid zero-rate curve");
    assert_eq!(curve.knots(), &[0.0, 1.0, 2.0, 5.0]);
    assert!((curve.df(2.0) - (-0.10_f64).exp()).abs() < 1e-12);

    let annual = DiscountCurve::from_zero_rates(
        "ZR-ANN",
        base_date(),
        &[(0.0, 0.0), (1.0, 0.05), (2.0, 0.05)],
        Compounding::Periodic(std::num::NonZeroU32::MIN),
    )
    .expect("valid annual zero-rate curve");
    assert!((annual.df(2.0) - 1.05_f64.powi(-2)).abs() < 1e-12);
    assert!(DiscountCurve::from_zero_rates(
        "EMPTY",
        base_date(),
        &[],
        Compounding::Periodic(std::num::NonZeroU32::MIN)
    )
    .is_err());
}

#[test]
fn test_from_dates_uses_day_count_for_pillar_times() {
    let one_year = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
    let curve = DiscountCurve::from_dates(
        "DATED",
        base_date(),
        &[(one_year, 0.95)],
        Some(DayCount::Act365F),
    )
    .expect("valid dated curve");
    assert_eq!(curve.day_count(), DayCount::Act365F);
    assert_eq!(curve.knots().len(), 2);
    assert!((curve.df(1.0) - 0.95).abs() < 1e-12);

    let before = Date::from_calendar_date(2024, Month::June, 1).expect("valid date");
    assert!(DiscountCurve::from_dates("BAD", base_date(), &[(before, 0.99)], None).is_err());
}
