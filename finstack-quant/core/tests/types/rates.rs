//! Additional tests for rate types and conversions

use finstack_quant_core::types::{Bps, Rate};
use proptest::prelude::*;

fn pct(rate: Rate) -> f64 {
    rate.as_percent().expect("finite percent view")
}

#[test]
fn rate_unit_views_are_fractional_and_fallible() {
    let rate = Rate::from_decimal(0.0525).expect("valid rate fixture");
    assert!((rate.as_bp().expect("bp view") - 525.0).abs() < 1e-9);
    let rate2 = Rate::from_bp(525.0).expect("bp rate");
    assert!((rate.as_decimal() - rate2.as_decimal()).abs() < 1e-15);

    let rate = Rate::from_percent(5.25).expect("valid rate fixture");
    assert!((rate.as_bp().expect("bp view") - 525.0).abs() < 1e-9);
    assert!((pct(rate) - 5.25).abs() < 1e-12);

    // A fractional basis-point view is never rounded to a whole quote.
    let sub_bp = Rate::from_bp(62.5).expect("fractional bp");
    assert!((sub_bp.as_bp().expect("bp view") - 62.5).abs() < 1e-9);
}

#[test]
fn rate_arithmetic_operations() {
    let r1 = Rate::from_percent(3.0).expect("valid rate fixture");
    let r2 = Rate::from_percent(1.5).expect("valid rate fixture");

    assert!((pct(r1.checked_add(r2).expect("finite sum")) - 4.5).abs() < 1e-10);
    assert!((pct(r1.checked_sub(r2).expect("finite difference")) - 1.5).abs() < 1e-10);
    assert!((pct(r1.checked_mul(2.0).expect("finite product")) - 6.0).abs() < 1e-10);
    assert!((pct(r1.checked_div(2.0).expect("finite quotient")) - 1.5).abs() < 1e-10);
    assert!((pct(-r1) + 3.0).abs() < 1e-10);
}

#[test]
fn rate_predicates() {
    assert!(Rate::ZERO.is_zero());
    assert!(!Rate::ZERO.is_positive());
    assert!(!Rate::ZERO.is_negative());

    let positive = Rate::from_percent(2.5).expect("valid rate fixture");
    assert!(!positive.is_zero());
    assert!(positive.is_positive());
    assert!(!positive.is_negative());

    let negative = Rate::from_percent(-1.5).expect("valid rate fixture");
    assert!(!negative.is_zero());
    assert!(!negative.is_positive());
    assert!(negative.is_negative());
}

#[test]
fn rate_abs() {
    let negative = Rate::from_percent(-3.5).expect("valid rate fixture");
    assert!((pct(negative.abs()) - 3.5).abs() < 1e-10);
    let positive = Rate::from_percent(2.5).expect("valid rate fixture");
    assert_eq!(positive.abs(), positive);
}

#[test]
fn rate_display_is_decimal_unit_shortest_text() {
    let rate = Rate::from_decimal(0.025).expect("valid rate fixture");
    assert_eq!(format!("{rate}"), "0.025");
    let parsed: Rate = format!("{rate}").parse().expect("display round trip");
    assert_eq!(parsed, rate);
    assert_eq!(format!("{}", Bps::new(150).as_rate()), "0.015");
}

#[test]
fn rate_f64_conversions() {
    let r = Rate::from_decimal(0.035).expect("finite decimal rate");
    assert_eq!(r.as_decimal(), 0.035);
    let f: f64 = r.into();
    assert_eq!(f, 0.035);
}

#[test]
fn bp_conversions_comprehensive() {
    let bp = Bps::new(250);
    assert_eq!(bp.as_bp(), 250);
    assert!((bp.as_decimal() - 0.025).abs() < 1e-15);
    assert!((bp.as_percent() - 2.5).abs() < 1e-15);
    assert_eq!(
        bp.as_rate(),
        Rate::from_decimal(bp.as_decimal()).expect("rate")
    );
}

#[test]
fn bp_arithmetic_operations() {
    let b1 = Bps::new(100);
    let b2 = Bps::new(50);

    assert_eq!(b1.checked_add(b2).expect("integer sum"), Bps::new(150));
    assert_eq!(
        b1.checked_sub(b2).expect("integer difference"),
        Bps::new(50)
    );
    assert_eq!(b1.checked_mul(3).expect("integer product"), Bps::new(300));
    assert_eq!(b1.checked_div(2).expect("integer quotient"), Bps::new(50));
    assert!(
        b1.checked_div(3).is_err(),
        "fractional quotient is an error"
    );
    assert_eq!(b1.checked_neg().expect("integer negation"), Bps::new(-100));
}

#[test]
fn bp_predicates() {
    assert!(Bps::ZERO.is_zero());
    assert!(!Bps::ZERO.is_positive());
    assert!(!Bps::ZERO.is_negative());

    let positive = Bps::new(100);
    assert!(positive.is_positive());
    assert!(!positive.is_negative());

    let negative = Bps::new(-50);
    assert!(negative.is_negative());
    assert!(!negative.is_positive());
}

#[test]
fn bp_display_formatting() {
    assert_eq!(format!("{}", Bps::new(250)), "250bp");
    assert_eq!(format!("{}", Bps::new(-50)), "-50bp");
}

#[test]
fn bp_from_conversions() {
    let b = Bps::new(250);
    let i: i32 = b.into();
    assert_eq!(i, 250);
    // There is no `From<Bps> for f64`; use the explicit accessors.
    assert!((b.as_decimal() - 0.025).abs() < 1e-15);
    assert!((b.as_percent() - 2.5).abs() < 1e-15);
}

#[test]
fn whole_bp_round_trip_keeps_the_original_bps() {
    // A Rate view need not round-trip bit-exactly to an integer; strict Bps
    // admission of the view can therefore fail. Keep the original Bps.
    let original = Bps::new(250);
    let view = original.as_rate().as_bp().expect("bp view");
    assert!((view - 250.0).abs() < 1e-9);
    if view.fract() == 0.0 {
        assert_eq!(Bps::try_new(view).expect("whole view"), original);
    }
}

#[test]
fn non_finite_constructors_reject_input() {
    assert!(Rate::from_decimal(f64::NAN).is_err());
    assert!(Rate::from_percent(f64::NAN).is_err());
    assert!(Rate::from_percent(f64::INFINITY).is_err());
    assert!(Rate::from_percent(f64::NEG_INFINITY).is_err());
    assert!(Rate::from_bp(f64::NAN).is_err());
    assert!(Rate::from_bp(f64::INFINITY).is_err());
}

#[test]
fn checked_rate_arithmetic_rejects_non_finite_results() {
    let rate = Rate::from_percent(3.0).expect("valid rate fixture");

    assert!(rate.checked_mul(f64::NAN).is_err());
    assert!(rate.checked_div(0.0).is_err());
    let largest = Rate::from_decimal(f64::MAX).expect("finite rate");
    assert!(largest.checked_add(largest).is_err());
    assert!(largest.checked_mul(2.0).is_err());
    assert!(largest.checked_sub(-largest).is_err());
    assert!(largest.as_percent().is_err());
    let two = Rate::from_percent(2.0).expect("valid rate fixture");
    assert_eq!(pct(rate.checked_add(two).expect("sum")), 5.0);
    assert!((pct(rate.checked_sub(two).expect("difference")) - 1.0).abs() < 1e-12);
    assert_eq!(pct(-rate), -3.0);
}

#[test]
fn checked_bps_arithmetic_rejects_integer_overflow_and_zero_division() {
    assert!(Bps::new(i32::MAX).checked_add(Bps::new(1)).is_err());
    assert!(Bps::new(i32::MIN).checked_neg().is_err());
    assert!(Bps::new(10).checked_div(0).is_err());
    assert!(Bps::new(i32::MIN).checked_div(-1).is_err());
    assert!(Bps::new(i32::MIN).checked_sub(Bps::new(1)).is_err());
    assert!(Bps::new(i32::MAX).checked_mul(2).is_err());
    assert_eq!(Bps::new(10).checked_mul(3).expect("product"), Bps::new(30));
}

#[test]
fn rate_edge_cases() {
    let tiny = Rate::from_bp(1.0).expect("one bp");
    assert!((tiny.as_decimal() - 0.0001).abs() < 1e-18);

    let large = Rate::from_percent(100.0).expect("valid rate fixture");
    assert!((large.as_bp().expect("bp view") - 10_000.0).abs() < 1e-9);
    assert_eq!(large.as_decimal(), 1.0);

    let negative = Rate::from_bp(-50.0).expect("negative bp");
    assert!(negative.is_negative());
    assert!(
        Rate::from_decimal(-2.0)
            .expect("below -1 is primitive-valid")
            .is_negative()
    );
}

#[test]
fn bp_ordering() {
    let b1 = Bps::new(100);
    let b2 = Bps::new(200);
    let b3 = Bps::new(100);

    assert!(b1 < b2);
    assert!(b2 > b1);
    assert_eq!(b1, b3);
    assert!(b1 <= b3);
    assert!(b1 >= b3);
}

#[test]
fn rate_ordering() {
    let r1 = Rate::from_percent(2.0).expect("valid rate fixture");
    let r2 = Rate::from_percent(3.0).expect("valid rate fixture");
    let r3 = Rate::from_percent(2.0).expect("valid rate fixture");

    assert!(r1 < r2);
    assert!(r2 > r1);
    assert_eq!(r1, r3);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn checked_rate_division_matches_finite_scalar_division(
        decimal in -1_000.0_f64..1_000.0,
        divisor in prop_oneof![-1_000.0_f64..-1.0e-9, 1.0e-9_f64..1_000.0],
    ) {
        let rate = Rate::from_decimal(decimal).unwrap();
        let divided = rate.checked_div(divisor).unwrap();
        prop_assert_eq!(divided.as_decimal(), if decimal / divisor == 0.0 { 0.0 } else { decimal / divisor });
    }

    #[test]
    fn bp_exact_division_inverts_multiplication(
        bp in -1_000_000_i32..1_000_000,
        multiplier in -1_000_i32..1_000,
        divisor in prop_oneof![-1_000_i32..-1, 1_i32..1_000],
    ) {
        let scaled = Bps::new(bp).checked_mul(multiplier).unwrap();
        let divided = scaled.checked_div(divisor);
        prop_assert_eq!(divided.is_ok(), scaled.as_bp() % divisor == 0);
        if let Ok(quotient) = divided {
            prop_assert_eq!(quotient.as_bp() * divisor, scaled.as_bp());
        }
    }
}
