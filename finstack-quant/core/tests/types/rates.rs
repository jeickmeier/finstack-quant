//! Additional tests for rate types and conversions

use finstack_quant_core::types::{Bps, Rate};
use proptest::prelude::*;
use std::panic::catch_unwind;

#[test]
fn rate_conversions_roundtrip() {
    // Test decimal -> bp -> decimal
    let rate = Rate::from_decimal(0.0525).expect("valid rate fixture");
    assert_eq!(rate.as_bp(), 525);
    let rate2 = Rate::from_bp(525);
    assert!((rate.as_decimal() - rate2.as_decimal()).abs() < 1e-10);

    // Test percent -> bp -> percent
    let rate = Rate::from_percent(5.25).expect("valid rate fixture");
    assert_eq!(rate.as_bp(), 525);
    assert!((rate.as_percent() - 5.25).abs() < 1e-10);
}

#[test]
fn rate_arithmetic_operations() {
    let r1 = Rate::from_percent(3.0).expect("valid rate fixture");
    let r2 = Rate::from_percent(1.5).expect("valid rate fixture");

    // Addition
    let sum = r1 + r2;
    assert!((sum.as_percent() - 4.5).abs() < 1e-10);

    // Subtraction
    let diff = r1 - r2;
    assert!((diff.as_percent() - 1.5).abs() < 1e-10);

    // Multiplication
    let doubled = r1 * 2.0;
    assert!((doubled.as_percent() - 6.0).abs() < 1e-10);

    let halved = r1 / 2.0;
    assert!((halved.as_percent() - 1.5).abs() < 1e-10);

    // Negation
    let neg = -r1;
    assert!((neg.as_percent() + 3.0).abs() < 1e-10);
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
    let abs_val = negative.abs();
    assert!((abs_val.as_percent() - 3.5).abs() < 1e-10);

    let positive = Rate::from_percent(2.5).expect("valid rate fixture");
    let abs_val = positive.abs();
    assert_eq!(abs_val.as_percent(), 2.5);
}

#[test]
fn rate_display_formatting() {
    let rate = Rate::from_percent(2.5).expect("valid rate fixture");
    assert_eq!(format!("{}", rate), "2.5000%");

    let rate = Rate::from_bp(150);
    assert_eq!(format!("{}", rate), "1.5000%");
}

#[test]
fn rate_f64_conversions() {
    let r = Rate::from_decimal(0.035).expect("finite decimal rate");
    assert!((r.as_decimal() - 0.035).abs() < 1e-10);

    let f: f64 = r.into();
    assert!((f - 0.035).abs() < 1e-10);
}

#[test]
fn bp_conversions_comprehensive() {
    let bp = Bps::new(250);

    assert_eq!(bp.as_bp(), 250);
    assert!((bp.as_decimal() - 0.025).abs() < 1e-10);
    assert!((bp.as_percent() - 2.5).abs() < 1e-10);

    let rate = bp.as_rate();
    assert_eq!(rate.as_bp(), 250);
}

#[test]
fn bp_arithmetic_operations() {
    let b1 = Bps::new(100);
    let b2 = Bps::new(50);

    assert_eq!(b1 + b2, Bps::new(150));
    assert_eq!(b1 - b2, Bps::new(50));
    assert_eq!(b1 * 3, Bps::new(300));
    assert_eq!(b1 / 2, Bps::new(50));
    assert_eq!(-b1, Bps::new(-100));
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
    assert_eq!(b.as_bp(), 250);

    let i: i32 = b.into();
    assert_eq!(i, 250);

    // `From<Bps> for f64` was removed. `TryFrom<f64>` reads a basis-point
    // count, so a decimal-valued `From<Bps> for f64` turned a 25bp round-trip
    // through f64 into 0bp. Use the explicit accessors instead.
    assert!((b.as_decimal() - 0.025).abs() < 1e-10);
    assert!((b.as_percent() - 2.5).abs() < 1e-10);
}

#[test]
fn cross_type_conversions() {
    // Rate <-> Bps
    let rate = Rate::from_percent(2.5).expect("valid rate fixture");
    let bp: Bps = rate.into();
    assert_eq!(bp.as_bp(), 250);
    let rate_back = bp.as_rate();
    assert_eq!(rate.as_bp(), rate_back.as_bp());
}

#[test]
fn non_finite_constructors_reject_input() {
    assert!(Rate::from_decimal(f64::NAN).is_err());
    assert!(Rate::from_percent(f64::NAN).is_err());
    assert!(Rate::from_percent(f64::INFINITY).is_err());
    assert!(Rate::from_percent(f64::NEG_INFINITY).is_err());
    assert!(Rate::from_decimal(f64::NAN).is_err());
    assert!(Rate::try_from_bp(f64::NAN).is_err());
    assert!(Rate::try_from_bp(62.5).is_err());
    assert_eq!(
        Rate::try_from_bp(250.0).expect("whole bp"),
        Rate::from_bp(250)
    );
}

#[test]
fn non_finite_rate_arithmetic_panics() {
    let rate = Rate::from_percent(3.0).expect("valid rate fixture");

    assert!(catch_unwind(|| {
        let _ = rate * f64::NAN;
    })
    .is_err());
    assert!(catch_unwind(|| {
        let large = Rate::from_decimal(f64::MAX).expect("finite rate");
        let _ = large + large;
    })
    .is_err());
    assert!(catch_unwind(|| {
        let _ = rate / 0.0;
    })
    .is_err());
}

#[test]
fn checked_rate_arithmetic_rejects_non_finite_results() {
    let rate = Rate::from_percent(3.0).expect("valid rate fixture");

    assert!(rate.checked_mul(f64::NAN).is_err());
    assert!(rate.checked_div(0.0).is_err());
    assert_eq!(
        rate.checked_add(Rate::from_percent(2.0).expect("valid rate fixture"))
            .unwrap()
            .as_percent(),
        5.0
    );
    assert!(
        (rate
            .checked_sub(Rate::from_percent(2.0).expect("valid rate fixture"))
            .unwrap()
            .as_percent()
            - 1.0)
            .abs()
            < 1e-12
    );
    assert_eq!((-rate).as_percent(), -3.0);
}

#[test]
fn checked_bps_arithmetic_rejects_integer_overflow_and_zero_division() {
    assert!(Bps::new(i32::MAX).checked_add(Bps::new(1)).is_err());
    assert!(Bps::new(i32::MIN).checked_neg().is_err());
    assert!(Bps::new(10).checked_div(0).is_err());
    assert_eq!(Bps::new(10).checked_mul(3).unwrap(), Bps::new(30));
    assert!(catch_unwind(|| {
        let _ = Bps::new(i32::MAX) + Bps::new(1);
    })
    .is_err());
    assert!(catch_unwind(|| {
        let _ = -Bps::new(i32::MIN);
    })
    .is_err());
    assert!(catch_unwind(|| {
        let _ = Bps::new(10) / 0;
    })
    .is_err());
}

#[test]
fn rate_edge_cases() {
    // Very small rates
    let tiny = Rate::from_bp(1);
    assert_eq!(tiny.as_bp(), 1);
    assert!((tiny.as_decimal() - 0.0001).abs() < 1e-10);

    // Large rates
    let large = Rate::from_percent(100.0).expect("valid rate fixture");
    assert_eq!(large.as_bp(), 10000);
    assert!((large.as_decimal() - 1.0).abs() < 1e-10);

    // Negative rates
    let negative = Rate::from_bp(-50);
    assert_eq!(negative.as_bp(), -50);
    assert!(negative.is_negative());
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

        prop_assert!(divided.as_decimal().is_finite());
        prop_assert!((divided.as_decimal() - decimal / divisor).abs() < 1e-10);
    }

    #[test]
    fn bp_checked_arithmetic_round_trips_without_overflow(
        bp in -1_000_000_i32..1_000_000,
        multiplier in -1_000_i32..1_000,
        divisor in prop_oneof![-1_000_i32..-1, 1_i32..1_000],
    ) {
        let value = Bps::new(bp);
        let scaled = value.checked_mul(multiplier).unwrap();
        let divided = scaled.checked_div(divisor).unwrap();

        prop_assert_eq!(divided.as_bp(), bp * multiplier / divisor);
    }
}
