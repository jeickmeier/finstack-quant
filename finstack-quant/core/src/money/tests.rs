use super::types::*;
use crate::config::{FinstackConfig, RoundingMode};
use crate::currency::Currency;
use crate::{Error, InputError};

#[test]
fn creation_and_accessors() {
    let m = Money::from((100_i64, Currency::USD));
    assert_eq!(m.amount(), 100.0);
    assert_eq!(m.currency(), Currency::USD);
}

#[test]
fn checked_comparison_is_exact_and_requires_matching_currencies() {
    use core::cmp::Ordering;
    let lower = Money::from((9_007_199_254_740_992_u64, Currency::USD));
    let higher = Money::from((9_007_199_254_740_993_u64, Currency::USD));
    assert_eq!(lower.amount(), higher.amount());
    assert_eq!(lower.checked_cmp(higher), Ok(Ordering::Less));
    assert_eq!(higher.checked_cmp(lower), Ok(Ordering::Greater));
    assert_eq!(lower.checked_cmp(lower), Ok(Ordering::Equal));
    assert!(matches!(
        lower.checked_cmp(Money::from((1_i64, Currency::EUR))),
        Err(Error::CurrencyMismatch {
            expected: Currency::USD,
            actual: Currency::EUR
        })
    ));
}

#[test]
fn checked_ops() {
    let a = Money::from((50_i64, Currency::USD));
    let b = Money::from((25_i64, Currency::USD));
    let c = a
        .checked_add(b)
        .expect("Currency match should succeed in test");
    assert_eq!(c.amount(), 75.0);
}

#[test]
fn currency_mismatch_error() {
    let usd = Money::from((10_i64, Currency::USD));
    let eur = Money::from((10_i64, Currency::EUR));
    assert!(usd.checked_add(eur).is_err());
}

#[test]
fn tuple_from_conversions() {
    use core::convert::Into;
    let m1: Money = (100_i64, Currency::USD).into();
    assert_eq!(m1.amount(), 100.0);
    let m2: Money = (42_u64, Currency::EUR).into();
    assert_eq!(m2.amount(), 42.0);
}

#[test]
fn grouped_format_handles_negative_values() {
    let m = Money::new(-1234.56, Currency::USD).expect("valid money fixture");
    let formatted = m.format_with(FormatOpts {
        group: Some(','),
        ..FormatOpts::default()
    });
    assert!(
        formatted.starts_with("USD -1,234.56"),
        "formatted output should keep sign on integer part only: {}",
        formatted
    );
}

#[test]
fn new_rejects_non_finite_amounts() {
    assert!(Money::new(f64::NAN, Currency::USD).is_err());
}

#[test]
fn checked_mul_div_f64_accepts_finite_scalars() {
    let money = Money::from((10_i64, Currency::USD));

    assert_eq!(
        money
            .checked_mul_f64(2.5)
            .expect("finite multiplication")
            .amount(),
        25.0
    );
    assert_eq!(
        money
            .checked_div_f64(4.0)
            .expect("finite division")
            .amount(),
        2.5
    );
}

#[test]
fn checked_mul_div_f64_reject_invalid_scalars() {
    let money = Money::from((10_i64, Currency::USD));

    assert!(money.checked_mul_f64(f64::NAN).is_err());
    assert!(money.checked_mul_f64(f64::INFINITY).is_err());
    assert!(money.checked_div_f64(f64::NEG_INFINITY).is_err());
    // A zero scalar and a zero `Money` divisor report the same error.
    let zero = Money::from((0_i64, Currency::USD));
    let division_by_zero = Err(Error::Validation("division by zero".into()));
    assert_eq!(money.checked_div_f64(0.0).map(|_| ()), division_by_zero);
    assert_eq!(money.checked_div_f64(-0.0).map(|_| ()), division_by_zero);
    assert_eq!(money.ratio(zero).map(|_| ()), division_by_zero);
}

#[test]
fn checked_neg_is_exact_and_keeps_scale() {
    let exact = Money::from_decimal_str("1.2500", Currency::USD).expect("exact");
    let negated = exact.checked_neg();
    assert_eq!(negated.amount_decimal().to_string(), "-1.2500");
    assert_eq!(negated.checked_neg(), exact);
    let tiny = Money::from_decimal_str("1e-27", Currency::USD).expect("exact");
    assert_eq!(tiny.checked_neg().amount_decimal(), -tiny.amount_decimal());
    let zero = Money::zero(Currency::USD).checked_neg();
    assert!(zero.amount_decimal().is_sign_positive());
    assert_eq!(zero.to_string(), "USD 0.00");
}

#[test]
fn money_deserialize_rejects_unknown_fields() {
    let json = r#"{"amount":10.0,"currency":"USD","unexpected":true}"#;
    let result: std::result::Result<Money, _> = serde_json::from_str(json);

    assert!(result.is_err());
}

#[test]
fn money_json_field_names_are_stable() {
    let json = serde_json::to_value(Money::new(10.25, Currency::USD).expect("valid money fixture"))
        .expect("serializes");
    let object = json.as_object().expect("money serializes as an object");

    assert_eq!(object.len(), 2);
    assert!(object.contains_key("amount"));
    assert_eq!(object.get("currency"), Some(&serde_json::json!("USD")));
}

#[test]
fn convert_rejects_non_finite_rate() {
    let usd = Money::from((5_i64, Currency::USD));
    assert!(usd.convert_at_rate_f64(Currency::EUR, f64::NAN).is_err());
}

#[test]
fn amount_does_not_silently_return_zero_for_large_values() {
    // This test documents the fix: large values must NOT silently become 0.
    // Prior to the fix, conversion failure would return 0.0 silently.
    let large_amount = Money::from((1_000_000_000_000_i64, Currency::USD));
    let amount = large_amount.amount();
    assert!(
        amount > 0.0,
        "Large monetary amount must not silently become zero"
    );
    assert!(
        (amount - 1_000_000_000_000.0).abs() < 1e3,
        "Amount should preserve the large value"
    );
}

// Fallible constructor tests (Money::new / Money::new_with_config)

#[test]
fn try_new_succeeds_for_finite_values() {
    let m = Money::new(123.45, Currency::USD).expect("Finite value should succeed");
    assert!((m.amount() - 123.45).abs() < 1e-10);
    assert_eq!(m.currency(), Currency::USD);
}

#[test]
fn try_new_returns_error_for_nan() {
    let result = Money::new(f64::NAN, Currency::USD);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(crate::Error::Input(
            crate::InputError::NonFiniteValue { kind }
        )) if kind == crate::NonFiniteKind::NaN
    ));
}

#[test]
fn try_new_returns_error_for_positive_infinity() {
    let result = Money::new(f64::INFINITY, Currency::EUR);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(crate::Error::Input(
            crate::InputError::NonFiniteValue { kind }
        )) if kind == crate::NonFiniteKind::PosInfinity
    ));
}

#[test]
fn try_new_returns_error_for_negative_infinity() {
    let result = Money::new(f64::NEG_INFINITY, Currency::GBP);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(crate::Error::Input(
            crate::InputError::NonFiniteValue { kind }
        )) if kind == crate::NonFiniteKind::NegInfinity
    ));
}

#[test]
fn try_new_with_config_succeeds_for_finite_values() {
    let mut cfg = FinstackConfig::default();
    cfg.rounding.mode = Some(RoundingMode::Bankers);
    cfg.rounding
        .ingest_scale
        .set_scale(Currency::USD, 3)
        .expect("valid decimal scale");
    let m = Money::new_with_config(1.2345, Currency::USD, &cfg).expect("Finite should succeed");
    assert!((m.amount() - 1.234).abs() < 1e-9);
}

#[test]
fn try_new_preserves_internal_precision_by_default() {
    let m = Money::new(10.005, Currency::USD).expect("Finite should succeed");
    assert!((m.amount() - 10.005).abs() < 1e-12);
}

#[test]
fn try_new_with_config_honors_ingest_scale_override() {
    let mut cfg = FinstackConfig::default();
    cfg.rounding.mode = Some(RoundingMode::Bankers);
    cfg.rounding
        .ingest_scale
        .set_scale(Currency::USD, 2)
        .expect("valid decimal scale");
    let m = Money::new_with_config(10.999, Currency::USD, &cfg).expect("Finite should succeed");
    assert!((m.amount() - 11.00).abs() < 1e-12);
}

#[test]
fn configured_ingest_requires_an_override_and_an_explicit_mode() {
    // Record 012 #9 boundary rows.
    let mut cfg = FinstackConfig::default();
    let exact =
        Money::from_decimal_with_config(rust_decimal::Decimal::new(1, 3), Currency::USD, &cfg)
            .expect("no override: exact");
    assert_eq!(exact.amount_decimal().to_string(), "0.001");
    assert_eq!(
        Money::new_with_config(0.001, Currency::USD, &cfg)
            .expect("no override: exact")
            .amount_decimal()
            .to_string(),
        "0.001"
    );
    cfg.rounding
        .ingest_scale
        .set_scale(Currency::USD, 2)
        .expect("valid decimal scale");
    // An override without a mode is incomplete, never Bankers by default.
    assert!(matches!(
        Money::new_with_config(0.001, Currency::USD, &cfg),
        Err(Error::Validation(_))
    ));
    cfg.rounding.mode = Some(RoundingMode::Bankers);
    // Admission precedes rounding: configuration cannot rescue an
    // unrepresentable float.
    assert!(Money::new_with_config(1e-29, Currency::USD, &cfg).is_err());
    let rounded = Money::new_with_config(-0.001, Currency::USD, &cfg).expect("rounds");
    assert_eq!(rounded.amount_decimal().to_string(), "0");
    assert!(rounded.amount_decimal().is_sign_positive());
}

#[test]
fn explicit_round_requires_scale_and_mode() {
    let tiny = Money::from_decimal(rust_decimal::Decimal::new(1, 28), Currency::USD);
    let zero = tiny.round(2, RoundingMode::Bankers).expect("scale 2");
    assert!(zero.amount_decimal().is_zero());
    let negative = Money::from_decimal(rust_decimal::Decimal::new(-1, 3), Currency::USD);
    let rounded = negative.round(2, RoundingMode::Bankers).expect("scale 2");
    assert!(rounded.amount_decimal().is_sign_positive());
    assert_eq!(
        rounded,
        Money::from_decimal(rust_decimal::Decimal::ZERO, Currency::USD)
    );
    assert!(matches!(
        tiny.round(29, RoundingMode::Bankers),
        Err(Error::Validation(_))
    ));
    assert!(tiny.round(28, RoundingMode::Floor).is_ok());
}

#[test]
fn formatting_bounds_precision_and_drops_a_displayed_zero_sign() {
    let negative = Money::from_decimal(rust_decimal::Decimal::new(-1, 3), Currency::USD);
    assert_eq!(negative.format_with(FormatOpts::default()), "USD 0.00");
    assert_eq!(negative.to_string(), "USD 0.00");
    assert_eq!(FormatOpts::MAX_DECIMALS, 28);
    assert!(FormatOpts::new(Some(29), None, None, None).is_err());
}

#[test]
fn try_new_with_config_returns_error_for_non_finite() {
    let cfg = FinstackConfig::default();
    let result = Money::new_with_config(f64::NAN, Currency::USD, &cfg);
    assert!(result.is_err());
}

#[test]
fn try_new_handles_zero() {
    let m = Money::new(0.0, Currency::USD).expect("Zero should succeed");
    assert_eq!(m.amount(), 0.0);
}

#[test]
fn try_new_handles_negative_zero() {
    let m = Money::new(-0.0, Currency::USD).expect("Negative zero should succeed");
    // -0.0 == 0.0 in floating point
    assert_eq!(m.amount(), 0.0);
}

#[test]
fn try_new_handles_very_small_values() {
    let small = 1e-15;
    let m = Money::new(small, Currency::USD).expect("Small value should succeed");
    // Construction preserves the raw finite amount; formatting/rounding is a separate concern.
    assert_eq!(m.amount(), small);
}

#[test]
fn try_new_handles_large_finite_values() {
    let large = 1e15;
    let m = Money::new(large, Currency::USD).expect("Large finite value should succeed");
    assert_eq!(m.amount(), large);
}

#[test]
fn try_new_rejects_finite_values_outside_decimal_range() {
    let result = Money::new(1e100, Currency::USD);
    assert!(matches!(
        result,
        Err(crate::Error::Input(InputError::ConversionOverflow))
    ));
}

#[test]
fn convert_at_rate_preserves_decimal_amount_precision() {
    use core::str::FromStr;

    let amount = rust_decimal::Decimal::from_str("10000000000000000.1").expect("valid decimal");
    let eur = Money::from_decimal(amount, Currency::EUR);
    let usd = eur
        .convert_at_rate_f64(Currency::USD, 2.0)
        .expect("valid conversion");

    assert_eq!(
        usd.amount_decimal(),
        rust_decimal::Decimal::from_str("20000000000000000.2").expect("valid decimal")
    );
}

fn hash_of(money: Money) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    money.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn hash_matches_equality_across_decimal_scale() {
    let a = Money::from_decimal(rust_decimal::Decimal::new(10, 1), Currency::USD);
    let b = Money::from_decimal(rust_decimal::Decimal::new(100, 2), Currency::USD);
    assert_eq!(a, b);
    assert_eq!(hash_of(a), hash_of(b));
}

#[test]
fn money_format_from_decimal_str_accepts_exact_text() {
    let wide = Money::from_decimal_str("12345678901234567890.12345", Currency::USD).expect("exact");
    assert_eq!(
        wide.amount_decimal().to_string(),
        "12345678901234567890.12345"
    );
    assert_eq!(wide.currency(), Currency::USD);
    let scientific = Money::from_decimal_str("1.2345e3", Currency::EUR).expect("exact scientific");
    assert_eq!(scientific.amount_decimal().to_string(), "1234.5");
    assert_eq!(scientific.currency(), Currency::EUR);
    let promoted = Money::from_decimal_str("0.1e29", Currency::USD).expect("exact 1e28");
    assert_eq!(
        promoted.amount_decimal(),
        rust_decimal::Decimal::try_from_i128_with_scale(10_i128.pow(28), 0)
            .expect("1e28 is representable")
    );
    let tiny = Money::from_decimal_str("1.00e-27", Currency::USD).expect("exact 1e-27");
    assert_eq!(tiny.amount_decimal(), rust_decimal::Decimal::new(1, 27));
    assert_eq!(
        tiny.format_with(
            FormatOpts::new(Some(28), Some(false), None, Some(RoundingMode::Bankers))
                .expect("bounded opts")
        ),
        "0.0000000000000000000000000010"
    );
    let reduced = Money::from_decimal_str("2000e-31", Currency::USD).expect("exact 2e-28");
    assert_eq!(reduced.amount_decimal(), rust_decimal::Decimal::new(2, 28));
    assert_eq!(
        reduced.format_with(
            FormatOpts::new(Some(28), Some(false), None, Some(RoundingMode::Bankers))
                .expect("bounded opts")
        ),
        "0.0000000000000000000000000002"
    );
    let min = Money::from_decimal_str("-7.9228162514264337593543950335e28", Currency::USD)
        .expect("exact Decimal minimum");
    assert_eq!(min.amount_decimal(), rust_decimal::Decimal::MIN);
    let zero = Money::from_decimal_str("0e999999", Currency::USD).expect("zero exponent");
    assert_eq!(zero.amount_decimal(), rust_decimal::Decimal::ZERO);
}

#[test]
fn money_format_from_decimal_str_rejects_inexact_amounts() {
    for amount in [
        "1.24500000000000000000000000001",
        "1.23450000000000000000000000001e3",
        "NaN",
        "1e-29",
        "1e29",
        "1.23e-28",
    ] {
        let result = Money::from_decimal_str(amount, Currency::USD);
        assert!(
            matches!(result, Err(Error::Validation(_))),
            "expected validation error for {amount}: {result:?}"
        );
    }
}

#[test]
fn money_format_opts_new_bounds_precision() {
    assert!(
        FormatOpts::new(
            Some(FormatOpts::MAX_DECIMALS),
            Some(true),
            None,
            Some(RoundingMode::Bankers)
        )
        .is_ok()
    );
    let result = FormatOpts::new(
        Some(FormatOpts::MAX_DECIMALS + 1),
        Some(true),
        None,
        Some(RoundingMode::Bankers),
    );
    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(
        FormatOpts::new(None, Some(true), Some(","), Some(RoundingMode::Bankers))
            .expect("unbounded precision")
            .decimals
            .is_none()
    );
}

#[test]
fn money_format_with_honours_returned_rounding_modes() {
    let vectors: &[(&str, &str, RoundingMode, &str)] = &[
        ("1.245", "USD 1.24", RoundingMode::Bankers, "bankers"),
        ("1.255", "USD 1.26", RoundingMode::Bankers, "bankers"),
        (
            "-1.245",
            "USD -1.25",
            RoundingMode::MidpointAwayFromZero,
            "midpoint_away_from_zero",
        ),
        (
            "1.241",
            "USD 1.24",
            RoundingMode::MidpointAwayFromZero,
            "midpoint_away_from_zero",
        ),
        (
            "-1.259",
            "USD -1.25",
            RoundingMode::TowardZero,
            "toward_zero",
        ),
        ("-1.241", "USD -1.25", RoundingMode::Floor, "floor"),
        ("1.249", "USD 1.24", RoundingMode::Floor, "floor"),
        ("1.241", "USD 1.25", RoundingMode::Ceil, "ceil"),
        ("-1.249", "USD -1.24", RoundingMode::Ceil, "ceil"),
    ];
    for (amount, expected, mode, name) in vectors.iter().copied() {
        let money = Money::from_decimal_str(amount, Currency::USD).expect("exact vector amount");
        let opts = FormatOpts::new(Some(2), Some(true), Some(","), Some(mode))
            .expect("bounded formatting options");
        assert_eq!(money.format_with(opts), expected, "{name} {amount}");
        assert_eq!(
            money.amount_decimal().to_string(),
            amount,
            "formatting must not mutate the stored amount"
        );
    }
}

#[test]
fn convert_at_rate_validates_rate_on_same_currency() {
    let usd = Money::new(100.0, Currency::USD).expect("valid money");
    assert!(usd.convert_at_rate_f64(Currency::USD, 0.0).is_err());
    assert!(usd.convert_at_rate_f64(Currency::USD, -1.0).is_err());
    assert!(usd.convert_at_rate_f64(Currency::USD, f64::NAN).is_err());
    assert_eq!(
        usd.convert_at_rate_f64(Currency::USD, 1.0)
            .expect("valid same-currency rate"),
        usd
    );
}

fn usd(text: &str) -> Money {
    Money::from_decimal_str(text, Currency::USD).expect("valid decimal money")
}

#[test]
fn arithmetic_results_normalize_zero() {
    let a = usd("0.10");
    let zero = a.checked_sub(a).expect("exact cancellation");
    assert_eq!(zero.amount_decimal().to_string(), "0");
    assert!(!zero.amount_decimal().is_sign_negative());
    let product = a
        .checked_mul(rust_decimal::Decimal::new(-3, 0))
        .expect("signed factor")
        .checked_mul(rust_decimal::Decimal::ZERO)
        .expect("zero factor");
    assert_eq!(product.amount_decimal().to_string(), "0");
    assert!(!product.amount_decimal().is_sign_negative());
    assert_eq!(
        Money::zero(Currency::USD).checked_neg(),
        Money::zero(Currency::USD)
    );
}

#[test]
fn addition_never_discards_a_cent() {
    let big = usd("10000000000000000000000000000");
    assert!(matches!(
        big.checked_add(usd("0.01")),
        Err(Error::Input(InputError::ConversionOverflow))
    ));
    assert_eq!(
        usd("0.1")
            .checked_add(usd("0.2"))
            .expect("exact")
            .amount_decimal()
            .to_string(),
        "0.3"
    );
}

#[test]
fn decimal_scalar_operations_are_canonical_and_f64_adapters_delegate() {
    let m = usd("100.10");
    assert_eq!(
        m.checked_mul(rust_decimal::Decimal::new(1, 1))
            .expect("mul")
            .amount_decimal(),
        rust_decimal::Decimal::new(1001, 2)
    );
    // The f64 adapter admits 0.1 as exactly 0.1, so both paths agree.
    assert_eq!(
        m.checked_mul_f64(0.1).expect("mul f64"),
        m.checked_mul(rust_decimal::Decimal::new(1, 1))
            .expect("mul")
    );
    assert_eq!(
        m.checked_div_f64(4.0).expect("div f64"),
        m.checked_div(rust_decimal::Decimal::from(4)).expect("div")
    );
    let tiny = usd("0.0000000000000000000000000001");
    assert!(matches!(
        tiny.checked_div(rust_decimal::Decimal::from(3)),
        Err(Error::Input(InputError::DecimalUnderflow))
    ));
}

#[test]
fn ratio_is_a_direct_binary64_ratio_within_budget() {
    // Exact ratio n/d to 40 significant decimals, parsed once to binary64.
    fn reference(n: rust_decimal::Decimal, d: rust_decimal::Decimal) -> f64 {
        use num_bigint::BigInt;
        let pow = |e: u32| num_traits::pow(BigInt::from(10_u8), e as usize);
        let num = BigInt::from(n.mantissa()) * pow(d.scale()) * pow(60);
        let den = BigInt::from(d.mantissa()) * pow(n.scale());
        let q = num / den;
        format!("{q}e-60").parse::<f64>().expect("reference ratio")
    }
    let cases = [
        ("0.75959782851251844215141", "1.3"),
        ("1", "3"),
        (
            "79228162514264337593543950335",
            "0.0000000000000000000000000001",
        ),
        ("-123456789.0123456789", "987.654321"),
    ];
    for (n, d) in cases {
        let (a, b) = (usd(n), usd(d));
        let got = a.ratio(b).expect("ratio");
        let want = reference(a.amount_decimal(), b.amount_decimal());
        assert!(
            ((got - want) / want).abs() <= 4.0 * f64::EPSILON,
            "{n}/{d}: {got:e} vs {want:e}"
        );
    }
    assert_eq!(
        Money::zero(Currency::USD).ratio(usd("2")).expect("zero"),
        0.0
    );
    assert!(usd("1").ratio(Money::zero(Currency::USD)).is_err());
    assert!(usd("1").ratio(Money::from((1_i64, Currency::EUR))).is_err());
}

#[test]
fn proportional_change_preserves_decimal_difference_and_rejects_zero_baseline() {
    assert!(
        Money::zero(Currency::USD)
            .proportional_change(Money::zero(Currency::USD))
            .is_err()
    );
    assert!(
        usd("1")
            .proportional_change(Money::zero(Currency::USD))
            .is_err()
    );
    assert!(
        usd("1")
            .proportional_change(Money::from((1_i64, Currency::EUR)))
            .is_err()
    );
    let big_base = usd("10000000000000000000");
    let big_next = usd("10000000000000000001");
    let frac = big_next
        .proportional_change(big_base)
        .expect("exact small delta");
    assert!((frac - 1e-19).abs() < 1e-32);
    assert_eq!(usd("50").proportional_change(usd("50")).expect("same"), 0.0);

    let half = rust_decimal::Decimal::from(2_i32);
    let pos_extreme = Money::from_decimal(rust_decimal::Decimal::MAX / half, Currency::USD);
    let neg_extreme = Money::from_decimal(-rust_decimal::Decimal::MAX / half, Currency::USD);
    let opp = pos_extreme
        .proportional_change(neg_extreme)
        .expect("opposite-sign extreme");
    assert!((opp - (-2.0)).abs() < 1e-12);
}

#[test]
fn convert_at_rate_is_exact_and_validates_before_shortcuts() {
    let eur = Money::from_decimal_str("-100.000000000000000001", Currency::EUR).expect("eur");
    let rate = rust_decimal::Decimal::new(10825, 4);
    let usd_amount = eur.convert_at_rate(Currency::USD, rate).expect("convert");
    assert_eq!(usd_amount.currency(), Currency::USD);
    assert_eq!(
        usd_amount.amount_decimal(),
        eur.amount_decimal() * rate,
        "no target minor-unit rounding"
    );
    // Same currency: exactly one, never a tolerance.
    assert_eq!(
        eur.convert_at_rate(Currency::EUR, rust_decimal::Decimal::ONE)
            .expect("unit"),
        eur
    );
    assert!(matches!(
        eur.convert_at_rate(
            Currency::EUR,
            rust_decimal::Decimal::from_i128_with_scale(10_000_000_000_000_000_000_000_000_001, 28)
        ),
        Err(Error::Input(InputError::NonUnitSameCurrencyRate { .. }))
    ));
    assert!(
        eur.convert_at_rate_f64(Currency::EUR, 1.0000000000000002)
            .is_err()
    );
    // Validation precedes the zero-amount and same-currency shortcuts.
    let zero = Money::zero(Currency::EUR);
    assert!(
        zero.convert_at_rate(Currency::USD, rust_decimal::Decimal::ZERO)
            .is_err()
    );
    assert!(
        zero.convert_at_rate(Currency::EUR, rust_decimal::Decimal::NEGATIVE_ONE)
            .is_err()
    );
    assert!(
        zero.convert_at_rate_f64(Currency::USD, f64::INFINITY)
            .is_err()
    );
}

#[test]
fn amount_nearest_even() {
    // Money::amount() delegates to decimal_to_f64_nearest (single rounding).
    let edge = Money::from_decimal_str("9007199254740993", Currency::USD).expect("2^53+1");
    assert_eq!(edge.amount(), 9_007_199_254_740_992.0);
    let huge = Money::from_decimal_str("12345678901234567890123456789", Currency::USD)
        .expect("large exact");
    let expected: f64 = huge
        .amount_decimal()
        .to_string()
        .parse()
        .expect("f64 nearest");
    assert_eq!(huge.amount().to_bits(), expected.to_bits());
}
