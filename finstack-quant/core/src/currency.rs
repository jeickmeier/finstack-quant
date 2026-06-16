//! Currency types and operations based on ISO 4217 standard.
//!
//! This module provides the `Currency` enum representing all ISO 4217 currencies,
//! with numeric discriminants matching the official ISO currency codes. The currency
//! data is automatically generated from the official ISO 4217 currency list.
//!
//! # Features
//!
//! - Complete ISO 4217 currency enumeration with 3-letter codes
//! - Numeric currency codes as enum discriminants
//! - Decimal precision (minor units) for each currency
//! - Case-insensitive parsing from strings
//! - Serialization support with stable serde field names
//! - Zero-cost abstractions (2-byte size)
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
//! # `decimals()` fallback
//!
//! [`Currency::decimals`] returns the ISO 4217 minor-unit count from the
//! generated table (`data/iso_4217.csv`). If a currency's numeric code were
//! ever missing from that table, the accessor falls back to **2 decimal
//! places** rather than panicking; the table is generated from the full ISO
//! list precisely so this fallback never fires in practice.
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
//! // Invalid codes return error
//! assert!("XXX".parse::<Currency>().is_err());
//! ```
//!
//! ## Converting between representations
//!
//! ```
//! use finstack_quant_core::currency::Currency;
//!
//! // From numeric code
//! let currency = Currency::try_from(840u16).expect("Currency conversion should succeed");
//! assert_eq!(currency, Currency::USD);
//!
//! // To numeric code
//! let code: u16 = Currency::EUR.into();
//! assert_eq!(code, 978);
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

// ---------------------------------------------------------------------------
// Generated enum (ISO-4217)
// ---------------------------------------------------------------------------
// The build script copies the generated currency enum to OUT_DIR for inclusion.
// This allows IDEs (`rust-analyzer`) to parse the generated code for auto-completion
// and navigation. We load it and publicly re-export all items.

include!(concat!(env!("OUT_DIR"), "/currency_generated.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem;

    #[test]
    fn test_currency_size() {
        assert_eq!(mem::size_of::<Currency>(), 2);
    }

    #[test]
    fn test_currency_numeric_values() {
        assert_eq!(Currency::USD as u16, 840);
        assert_eq!(Currency::EUR as u16, 978);
        assert_eq!(Currency::GBP as u16, 826);
        assert_eq!(Currency::JPY as u16, 392);
        assert_eq!(Currency::CHF as u16, 756);
        assert_eq!(Currency::AUD as u16, 36);
        assert_eq!(Currency::CAD as u16, 124);
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
        use crate::error::{Error, InputError};
        let error: Error = InputError::Invalid.into();
        assert_eq!(format!("{}", error), "Invalid input data");
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
            let back: u16 = currency.into();
            assert_eq!(code, back);
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
    fn test_decimals_fallback_behavior() {
        // Test that the current decimals() method no longer panics and falls back to 2
        // This should work for all known currencies
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
