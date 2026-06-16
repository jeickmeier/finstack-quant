//! Foreign-exchange interfaces and a simplified FX matrix.
//!
//! Design goals:
//! - store raw FX quotes for currency pairs
//! - compute reciprocal and triangulated rates on demand
//! - provide deterministic lookups with bounded LRU caching
//!
//! The public surface remains stable:
//! - `FxProvider` trait for on-demand quotes
//! - `FxMatrix` offering `FxMatrix::rate` for consumers and `MarketContext`
//! - standard provider implementations (e.g. `SimpleFxProvider`)
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider, FxQuery};
//! use finstack_quant_core::money::fx::SimpleFxProvider;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::Date;
//! use std::sync::Arc;
//! use time::Month;
//!
//! let provider = Arc::new(SimpleFxProvider::new());
//! provider.set_quote(Currency::EUR, Currency::USD, 1.1);
//!
//! let matrix = FxMatrix::new(provider.clone());
//! let date = Date::from_calendar_date(2024, Month::January, 5).expect("Valid date");
//! let res = matrix.rate(FxQuery::new(Currency::EUR, Currency::USD, date)).expect("FX rate lookup should succeed");
//! assert_eq!(res.rate, 1.1);
//! ```

mod matrix;
mod provider;
mod providers;
mod types;

pub use matrix::FxMatrix;
pub use provider::FxProvider;
pub(crate) use provider::{reciprocal_rate_or_err, validate_fx_rate};
pub use providers::{BumpedFxProvider, SimpleFxProvider};
pub use types::{FxConfig, FxConversionPolicy, FxMatrixState, FxPolicyMeta, FxQuery, FxRateResult};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::Currency;
    use std::sync::Arc;

    fn assert_parses_to(label: &str, expected: FxConversionPolicy) {
        assert!(matches!(label.parse::<FxConversionPolicy>(), Ok(value) if value == expected));
    }
    use time::macros::date;

    #[test]
    fn with_bumped_rate_rejects_non_positive_result() {
        let provider = Arc::new(SimpleFxProvider::new());
        let _ = provider.set_quote(Currency::EUR, Currency::USD, 1.10);
        let matrix = FxMatrix::new(provider);

        let result =
            matrix.with_bumped_rate(Currency::EUR, Currency::USD, -1.0, date!(2025 - 01 - 01));

        assert!(result.is_err(), "100% negative bump should be rejected");
    }

    #[test]
    fn test_fx_conversion_policy_fromstr_display_roundtrip() {
        for (input, expected) in [
            ("cashflow_date", FxConversionPolicy::CashflowDate),
            ("cashflow", FxConversionPolicy::CashflowDate),
            ("period_end", FxConversionPolicy::PeriodEnd),
            ("end", FxConversionPolicy::PeriodEnd),
            ("period_average", FxConversionPolicy::PeriodAverage),
            ("average", FxConversionPolicy::PeriodAverage),
            ("custom", FxConversionPolicy::Custom),
        ] {
            assert_parses_to(input, expected);
        }

        for variant in [
            FxConversionPolicy::CashflowDate,
            FxConversionPolicy::PeriodEnd,
            FxConversionPolicy::PeriodAverage,
            FxConversionPolicy::Custom,
        ] {
            let display = variant.to_string();
            assert!(matches!(display.parse::<FxConversionPolicy>(), Ok(value) if value == variant));
        }
    }

    #[test]
    fn test_fx_conversion_policy_fromstr_rejects_unknown() {
        assert!("unknown".parse::<FxConversionPolicy>().is_err());
    }

    #[test]
    fn fx_conversion_policy_serde_uses_snake_case_wire_names() {
        for (variant, expected_json) in [
            (FxConversionPolicy::CashflowDate, r#""cashflow_date""#),
            (FxConversionPolicy::PeriodEnd, r#""period_end""#),
            (FxConversionPolicy::PeriodAverage, r#""period_average""#),
            (FxConversionPolicy::Custom, r#""custom""#),
        ] {
            let json = serde_json::to_string(&variant).expect("serializes");
            assert_eq!(json, expected_json);

            let restored: FxConversionPolicy =
                serde_json::from_str(expected_json).expect("deserializes");
            assert_eq!(restored, variant);
        }
    }

    #[test]
    fn fx_query_defaults_policy_and_rejects_unknown_fields() {
        let query: FxQuery = serde_json::from_str(r#"{"from":"USD","to":"EUR","on":"2025-01-01"}"#)
            .expect("policy should default");
        assert_eq!(query.policy, FxConversionPolicy::CashflowDate);

        let result: std::result::Result<FxQuery, _> = serde_json::from_str(
            r#"{"from":"USD","to":"EUR","on":"2025-01-01","unexpected":true}"#,
        );
        assert!(result.is_err());
    }
}
