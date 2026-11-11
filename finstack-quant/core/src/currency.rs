//! Currency types and operations based on ISO 4217 standard.
//!
//! This module provides the `Currency` enum of supported ISO 4217 currencies,
//! generated from the maintained `data/iso_4217.csv` selection. A currency's
//! identity is its alphabetic code (record CORE-CONVENTIONS-012 #12): equality,
//! hashing, ordering and the wire form all follow the code, and the ISO
//! numeric code is metadata that distinct currencies may share.
//!
//! # Features
//!
//! - Closed catalogue of supported currencies with 3-letter codes
//! - Ordering by alphabetic code, so currency-keyed maps and output are
//!   deterministic and independent of numeric codes
//! - ISO numeric code and decimal precision (minor units) as metadata
//! - Case-insensitive parsing from strings (no trimming)
//! - Numeric lookup only for a unique supported match
//! - Serialization support with stable serde field names
//!
//! # Scope: deliverable currencies only
//!
//! The enum covers ISO 4217 **deliverable national currencies** (including
//! indexed units of account such as CLF, the Chilean Unidad de Fomento).
//! Non-currency ISO 4217 codes are intentionally excluded:
//!
//! - `XXX` (no currency) — use an `Option<Currency>` instead of a sentinel
//! - `XAU`/`XAG`/`XPT`/`XPD` (precious metals) — commodities, not money
//! - `XDR` (IMF Special Drawing Rights) — a basket claim, not a deliverable
//!   settlement currency
//!
//! Parsing these codes returns an error by design.
//!
//! # `decimals()`
//!
//! `Currency::decimals()` returns the maintained ISO 4217 minor-unit exponent
//! from the generated metadata table (`data/iso_4217.csv`) for every supported
//! currency; there is no fallback value.
//!
//! # Examples
//!
//! ## Creating and using currencies
//!
//! ```
//! use finstack_quant_core::currency::Currency;
//!
//! // Create currencies directly
//! let usd = Currency::USD;
//! let eur = Currency::EUR;
//!
//! // Get ISO numeric code
//! assert_eq!(usd.numeric(), 840);
//! assert_eq!(eur.numeric(), 978);
//!
//! // Get decimal precision
//! assert_eq!(usd.decimals(), 2);  // Most currencies have 2 decimals
//! assert_eq!(Currency::JPY.decimals(), 0);  // Japanese Yen has no decimals
//! assert_eq!(Currency::BHD.decimals(), 3);  // Bahraini Dinar has 3 decimals
//! ```
//!
//! ## Parsing from strings
//!
//! ```
//! use finstack_quant_core::currency::Currency;
//! use std::str::FromStr;
//!
//! // Parse from uppercase
//! let currency = Currency::from_str("USD").expect("Currency parsing should succeed");
//! assert_eq!(currency, Currency::USD);
//!
//! // Parse case-insensitive
//! let currency = "eur".parse::<Currency>().expect("Currency parsing should succeed");
//! assert_eq!(currency, Currency::EUR);
//!
//! // Invalid codes return an error naming the rejected text; surrounding
//! // whitespace is not trimmed.
//! let err = "XXX".parse::<Currency>().expect_err("XXX is not a deliverable currency");
//! assert!(err.to_string().contains("\"XXX\""));
//! assert!(" USD ".parse::<Currency>().is_err());
//! ```
//!
//! ## Converting between representations
//!
//! ```
//! use finstack_quant_core::currency::Currency;
//!
//! // From a numeric code with a unique supported match
//! let currency = Currency::try_from(840u16).expect("Currency conversion should succeed");
//! assert_eq!(currency, Currency::USD);
//! assert!(Currency::try_from(0u16).is_err());
//!
//! // To numeric code
//! assert_eq!(Currency::EUR.numeric(), 978);
//!
//! // Display formatting
//! assert_eq!(format!("{}", Currency::GBP), "GBP");
//! ```
//!
//! ## Iterating over all currencies
//!
//! ```
//! use finstack_quant_core::currency::Currency;
//! use strum::IntoEnumIterator;
//!
//! // Count total currencies
//! let count = Currency::iter().count();
//! assert!(count > 150);
//!
//! // Find currencies with 3 decimal places
//! let three_decimal_currencies: Vec<_> = Currency::iter()
//!     .filter(|c| c.decimals() == 3)
//!     .collect();
//! assert!(three_decimal_currencies.contains(&Currency::KWD));
//! ```

// Generated enum (ISO-4217), included by path so IDEs (`rust-analyzer`) can
// parse it for auto-completion and navigation.
include!("generated/currency_generated.rs");

/// Parse-error constructor used by the `FromStr` implementation.
///
/// This is the only currency-text parser: `FromStr`, `TryFrom<&str>` and
/// every host binding go through it, so a bad code always reports
/// [`InputError::UnknownCurrency`](crate::InputError::UnknownCurrency)
/// naming the rejected text.
fn unknown_currency_code(code: &str) -> crate::Error {
    crate::InputError::UnknownCurrency {
        code: code.to_string(),
    }
    .into()
}

impl std::fmt::Display for Currency {
    /// Writes the ISO 4217 alphabetic code, such as `USD`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl AsRef<str> for Currency {
    fn as_ref(&self) -> &str {
        self.code()
    }
}

/// Parses an ISO 4217 alphabetic code, ignoring ASCII case.
impl std::str::FromStr for Currency {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        ALL_CURRENCIES
            .iter()
            .copied()
            .find(|currency| currency.code().eq_ignore_ascii_case(s))
            .ok_or_else(|| unknown_currency_code(s))
    }
}

impl TryFrom<&str> for Currency {
    type Error = crate::Error;

    fn try_from(code: &str) -> crate::Result<Self> {
        code.parse()
    }
}

impl strum::IntoEnumIterator for Currency {
    type Iterator = std::iter::Copied<std::slice::Iter<'static, Currency>>;

    fn iter() -> Self::Iterator {
        ALL_CURRENCIES.iter().copied()
    }
}

/// Alphabetic code order (record CORE-CONVENTIONS-012 #12, resolution O10),
/// independent of ISO numeric codes.
impl Ord for Currency {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.code().cmp(other.code())
    }
}

impl PartialOrd for Currency {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem;

    #[test]
    fn test_currency_size() {
        assert_eq!(mem::size_of::<Currency>(), 1);
    }

    #[test]
    fn test_currency_numeric_values() {
        assert_eq!(Currency::USD.numeric(), 840);
        assert_eq!(Currency::EUR.numeric(), 978);
        assert_eq!(Currency::GBP.numeric(), 826);
        assert_eq!(Currency::JPY.numeric(), 392);
        assert_eq!(Currency::CHF.numeric(), 756);
        assert_eq!(Currency::AUD.numeric(), 36);
        assert_eq!(Currency::CAD.numeric(), 124);
    }

    #[test]
    fn identity_and_order_follow_the_alphabetic_code() {
        use std::collections::BTreeMap;
        use strum::IntoEnumIterator;
        // AUD (036) sorts before USD (840) and CHF (756) before JPY (392).
        assert!(Currency::AUD < Currency::USD);
        assert!(Currency::CHF < Currency::JPY);
        let codes: Vec<&str> = Currency::iter().map(Currency::code).collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        assert_eq!(codes, sorted);
        for (position, currency) in Currency::iter().enumerate() {
            // Discriminants are positions, never numeric codes.
            assert_eq!(currency as usize, position);
        }
        let map = BTreeMap::from([(Currency::USD, 1), (Currency::JPY, 2), (Currency::AUD, 3)]);
        let json = serde_json::to_string(&map).expect("map JSON");
        assert_eq!(json, r#"{"AUD":3,"JPY":2,"USD":1}"#);
    }

    #[test]
    fn numeric_lookup_requires_a_unique_supported_match() {
        use strum::IntoEnumIterator;
        let mut by_numeric = std::collections::BTreeMap::<u16, Vec<Currency>>::new();
        for currency in Currency::iter() {
            by_numeric
                .entry(currency.numeric())
                .or_default()
                .push(currency);
        }
        for (numeric, currencies) in by_numeric {
            match currencies.as_slice() {
                [only] => assert_eq!(Currency::try_from(numeric), Ok(*only)),
                many => {
                    let err = Currency::try_from(numeric).expect_err("ambiguous");
                    for currency in many {
                        assert!(err.to_string().contains(currency.code()), "{err}");
                    }
                }
            }
        }
        let err = Currency::try_from(999u16).expect_err("unknown numeric code");
        assert!(err.to_string().contains("999"), "{err}");
    }

    #[test]
    fn test_currency_display() {
        assert_eq!(format!("{}", Currency::USD), "USD");
        assert_eq!(format!("{}", Currency::EUR), "EUR");
        assert_eq!(format!("{}", Currency::GBP), "GBP");
        assert_eq!(format!("{}", Currency::JPY), "JPY");
    }

    #[test]
    fn test_currency_from_str() {
        assert_eq!(
            "USD"
                .parse::<Currency>()
                .expect("USD should parse successfully"),
            Currency::USD
        );
        assert_eq!(
            "EUR"
                .parse::<Currency>()
                .expect("EUR should parse successfully"),
            Currency::EUR
        );
        assert_eq!(
            "GBP"
                .parse::<Currency>()
                .expect("GBP should parse successfully"),
            Currency::GBP
        );
        assert_eq!(
            "JPY"
                .parse::<Currency>()
                .expect("JPY should parse successfully"),
            Currency::JPY
        );

        // Test case insensitive
        assert_eq!(
            "usd"
                .parse::<Currency>()
                .expect("usd should parse successfully"),
            Currency::USD
        );
        assert_eq!(
            "eur"
                .parse::<Currency>()
                .expect("eur should parse successfully"),
            Currency::EUR
        );
        assert_eq!(
            "gbp"
                .parse::<Currency>()
                .expect("gbp should parse successfully"),
            Currency::GBP
        );
    }

    #[test]
    fn test_currency_from_str_invalid() {
        assert!("INVALID".parse::<Currency>().is_err());
        assert!("XXX".parse::<Currency>().is_err());
        assert!("".parse::<Currency>().is_err());
    }

    #[test]
    fn test_currency_parse_error_names_code_and_does_not_trim() {
        use crate::{ErrorKind, InputError};
        for code in [" USD ", "USD\n", "XXX", "é€x"] {
            let err = code.parse::<Currency>().expect_err("rejected code");
            assert_eq!(err.kind(), ErrorKind::Validation);
            assert_eq!(
                err,
                crate::Error::Input(InputError::UnknownCurrency {
                    code: code.to_string()
                })
            );
            assert!(err.to_string().starts_with("Invalid currency code"));
            assert!(err.to_string().contains(&format!("{code:?}")));
        }
        assert_eq!(Currency::try_from("eur"), Ok(Currency::EUR));
    }

    #[test]
    fn test_currency_round_trip() {
        let currencies = [
            Currency::USD,
            Currency::EUR,
            Currency::GBP,
            Currency::JPY,
            Currency::CHF,
            Currency::AUD,
            Currency::CAD,
            Currency::CNY,
            Currency::SEK,
            Currency::NOK,
            Currency::DKK,
            Currency::PLN,
        ];

        for currency in &currencies {
            let formatted = format!("{}", currency);
            let parsed: Currency = formatted
                .parse()
                .expect("Currency parsing should succeed in test");
            assert_eq!(*currency, parsed);
        }
    }

    #[test]
    fn test_currency_decimals() {
        assert_eq!(Currency::USD.decimals(), 2);
        assert_eq!(Currency::EUR.decimals(), 2);
        assert_eq!(Currency::JPY.decimals(), 0);
        assert_eq!(Currency::BHD.decimals(), 3);
        assert_eq!(Currency::KWD.decimals(), 3);
        assert_eq!(Currency::JOD.decimals(), 3);
        // Most currencies have 2 decimals

        // Currencies with 0 decimals
        assert_eq!(Currency::BIF.decimals(), 0);
        assert_eq!(Currency::CLP.decimals(), 0);
        assert_eq!(Currency::DJF.decimals(), 0);
        assert_eq!(Currency::GNF.decimals(), 0);
        assert_eq!(Currency::ISK.decimals(), 0);
        assert_eq!(Currency::KMF.decimals(), 0);
        assert_eq!(Currency::KRW.decimals(), 0);
        assert_eq!(Currency::PYG.decimals(), 0);
        assert_eq!(Currency::RWF.decimals(), 0);
        assert_eq!(Currency::UGX.decimals(), 0);
        assert_eq!(Currency::VND.decimals(), 0);
        assert_eq!(Currency::VUV.decimals(), 0);
        assert_eq!(Currency::XAF.decimals(), 0);
        assert_eq!(Currency::XOF.decimals(), 0);
        assert_eq!(Currency::XPF.decimals(), 0);
    }

    #[test]
    fn test_currency_error_display() {
        use crate::{Error, InputError};
        let error: Error = InputError::NegativeValue.into();
        assert_eq!(format!("{}", error), "Values must be non-negative");
    }

    #[test]
    fn test_currency_serde() {
        let currency = Currency::USD;
        let serialized =
            serde_json::to_string(&currency).expect("JSON serialization should succeed in test");
        let deserialized: Currency =
            serde_json::from_str(&serialized).expect("JSON deserialization should succeed in test");
        assert_eq!(currency, deserialized);
    }

    #[test]
    fn test_currency_numeric_conversion_roundtrip() {
        let codes = [840u16, 978, 826, 392, 756, 36, 124];

        for &code in &codes {
            let currency =
                Currency::try_from(code).expect("Currency conversion should succeed in test");
            assert_eq!(currency.numeric(), code);
        }

        // invalid code
        assert!(Currency::try_from(0u16).is_err());
    }

    #[test]
    fn test_currency_iter() {
        use strum::IntoEnumIterator;

        // Ensure iterator produces some known currencies
        assert!(Currency::iter().any(|c| c == Currency::USD));
        assert!(Currency::iter().any(|c| c == Currency::EUR));

        // Length should be substantial (simple sanity)
        let enum_count = Currency::iter().count();
        assert!(enum_count > 100); // at least 100 currencies expected
    }

    #[test]
    fn test_decimals_are_maintained_metadata() {
        use strum::IntoEnumIterator;
        for currency in Currency::iter() {
            let decimals = currency.decimals();
            assert!(
                decimals <= 4,
                "Decimal places should be reasonable (<= 4) for {:?}",
                currency
            );
        }
    }
}
