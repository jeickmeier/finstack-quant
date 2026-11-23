use super::*;

#[test]
fn zero_denom_ratio_signs_zero_denominators() {
    assert_eq!(zero_denom_ratio(0.15), f64::INFINITY);
    assert_eq!(zero_denom_ratio(-0.05), f64::NEG_INFINITY);
    assert!(zero_denom_ratio(0.0).is_nan());
    assert!(zero_denom_ratio(f64::NAN).is_nan());
    assert!(zero_denom_ratio(f64::INFINITY).is_nan());
}
