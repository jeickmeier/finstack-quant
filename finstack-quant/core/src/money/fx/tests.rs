use super::*;
use crate::currency::Currency;
use std::sync::Arc;

use time::macros::date;

#[test]
fn with_bumped_rate_rejects_non_positive_result() {
    let provider = Arc::new(SimpleFxProvider::new());
    let _ = provider.set_quotes(&[(Currency::EUR, Currency::USD, 1.10)]);
    let matrix = FxMatrix::new(provider, None);

    let result = matrix.with_bumped_rate(
        crate::money::fx::FxQuery::new(
            Currency::EUR,
            Currency::USD,
            date!(2025 - 01 - 01),
            crate::money::fx::FxConversionPolicy::CashflowDate,
        ),
        -1.0,
    );

    assert!(result.is_err(), "100% negative bump should be rejected");
}

#[test]
fn currency_pair_parses_both_forms_and_rejects_multibyte_keys() {
    for text in ["EUR/USD", "EURUSD", "eurusd", "eur/usd"] {
        let pair: CurrencyPair = text.parse().expect("valid pair");
        assert_eq!((pair.base, pair.quote), (Currency::EUR, Currency::USD));
    }
    // Six bytes but not six characters: must error, never slice mid-char.
    for text in ["é€x", "EURUS", "EURUSDX", "", "EUR-USD"] {
        let err = text.parse::<CurrencyPair>().expect_err("malformed pair");
        assert_eq!(err.kind(), crate::ErrorKind::Validation);
    }
    let err = "EUR/XYZ".parse::<CurrencyPair>().expect_err("bad quote");
    assert!(err.to_string().contains("\"XYZ\""), "{err}");
}

#[test]
fn test_fx_conversion_policy_fromstr_display_roundtrip() {
    for (input, expected) in [
        ("cashflow_date", FxConversionPolicy::CashflowDate),
        ("period_end", FxConversionPolicy::PeriodEnd),
    ] {
        assert!(matches!(input.parse::<FxConversionPolicy>(), Ok(value) if value == expected));
    }

    for variant in [
        FxConversionPolicy::CashflowDate,
        FxConversionPolicy::PeriodEnd,
    ] {
        let display = variant.to_string();
        assert!(matches!(display.parse::<FxConversionPolicy>(), Ok(value) if value == variant));
    }
}

#[test]
fn test_fx_conversion_policy_fromstr_rejects_unknown() {
    for rejected in [
        "cashflow",
        "end",
        "average",
        "CashflowDate",
        "period-end",
        "period_average",
    ] {
        assert!(rejected.parse::<FxConversionPolicy>().is_err());
    }
}

#[test]
fn fx_conversion_policy_serde_uses_snake_case_wire_names() {
    for (variant, expected_json) in [
        (FxConversionPolicy::CashflowDate, r#""cashflow_date""#),
        (FxConversionPolicy::PeriodEnd, r#""period_end""#),
    ] {
        let json = serde_json::to_string(&variant).expect("serializes");
        assert_eq!(json, expected_json);

        let restored: FxConversionPolicy =
            serde_json::from_str(expected_json).expect("deserializes");
        assert_eq!(restored, variant);
    }
}

#[test]
fn fx_query_requires_policy_and_rejects_unknown_fields() {
    let missing: std::result::Result<FxQuery, _> =
        serde_json::from_str(r#"{"from":"USD","to":"EUR","on":"2025-01-01"}"#);
    assert!(
        missing
            .expect_err("policy has no default")
            .to_string()
            .contains("missing field `policy`")
    );

    let query: FxQuery = serde_json::from_str(
        r#"{"from":"USD","to":"EUR","on":"2025-01-01","policy":"period_end"}"#,
    )
    .expect("explicit policy deserializes");
    assert_eq!(query.policy, FxConversionPolicy::PeriodEnd);

    let result: std::result::Result<FxQuery, _> =
        serde_json::from_str(r#"{"from":"USD","to":"EUR","on":"2025-01-01","unexpected":true}"#);
    assert!(result.is_err());
}
