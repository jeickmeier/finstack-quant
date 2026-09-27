//! Compounding conventions for floating leg calculations in interest rate swaps.
//!
//! Defines how floating rate coupons are calculated based on the
//! underlying reference rate (LIBOR, SOFR, SONIA, etc.).
//!
//! # Implementation Notes
//!
//! ## Compounded-in-Arrears (Full Daily Compounding)
//!
//! For overnight-indexed swaps (OIS) with `CompoundedInArrears` compounding,
//! the implementation uses **full daily compounding** per ISDA 2021:
//!
//! ```text
//! Coupon = N × [∏(1 + r_i × dcf_i) - 1] + spread × accrual
//! ```
//!
//! where the product is taken over daily observations in the accrual period.
//!
//! ## Fast Path for Unseasoned Single-Curve OIS
//!
//! When all of the following conditions are met, the discount curve identity
//! is used as an optimization:
//!
//! - The contract uses `CompoundedInArrears { lookback_days: 0 }`
//! - Forward curve ID matches discount curve ID (single-curve)
//!
//! In this case:
//! ```text
//! ∏(1 + r_i × dcf_i) = DF(start) / DF(end)
//! ```
//!
//! This identity is exact and avoids iterating over daily observations.
//!
//! ## Lookback and Observation Shift
//!
//! Lookback and observation shift are distinct, mutually exclusive enum
//! variants:
//!
//! - **Lookback** (`CompoundedInArrears`): shifts observation dates backward
//!   while day-count weights remain on the original accrual dates.
//! - **Observation shift** (`CompoundedWithObservationShift`): shifts both
//!   observations and their day-count weights backward.
//!
//! Either non-zero convention disables the discount-identity fast path and
//! performs full daily compounding.
//!
//! ## Seasoned Swaps
//!
//! For seasoned swaps where `as_of` falls within an accrual period, historical
//! fixings are required for observation dates before `as_of`. Provide fixings
//! via `ScalarTimeSeries` with id `FIXING:{forward_curve_id}`.
//!
//! # References
//!
//! - **ISDA 2021 Definitions**: Compounded RFR conventions `docs/REFERENCES.md#isda-2021-definitions`
//! - **ARRC** (Alternative Reference Rates Committee): SOFR conventions `docs/REFERENCES.md#arrc-sofr-users-guide`
//! - **BoE** (Bank of England): SONIA conventions `docs/REFERENCES.md#boe-sonia-key-features`

/// Canonical floating-leg compounding enum, defined in
/// `finstack_quant_cashflows` and re-exported here for the swap family.
pub use finstack_quant_cashflows::builder::FloatingLegCompounding;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_simple() {
        assert_eq!(
            FloatingLegCompounding::default(),
            FloatingLegCompounding::Simple
        );
    }

    #[test]
    fn test_market_presets() {
        // Cleared OIS compounds plain in-arrears (payment delay only); the
        // ARRC 2bd / BoE 5bd lookbacks are FRN conventions, not OIS.
        for preset in [
            FloatingLegCompounding::sofr(),
            FloatingLegCompounding::sonia(),
            FloatingLegCompounding::estr(),
            FloatingLegCompounding::tona(),
            FloatingLegCompounding::saron(),
            FloatingLegCompounding::fedfunds(),
        ] {
            assert_eq!(
                preset,
                FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }
            );
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let methods = vec![
            FloatingLegCompounding::Simple,
            FloatingLegCompounding::sofr(),
            FloatingLegCompounding::sonia(),
        ];

        for method in methods {
            let json =
                serde_json::to_string(&method).expect("Serialization should succeed in test");
            let deserialized: FloatingLegCompounding =
                serde_json::from_str(&json).expect("Deserialization should succeed in test");
            assert_eq!(method, deserialized);
        }
    }

    #[test]
    fn rate_cutoff_roundtrips() {
        let method = FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 };
        let json = serde_json::to_string(&method).expect("serialize rate cutoff");
        let deserialized: FloatingLegCompounding =
            serde_json::from_str(&json).expect("deserialize rate cutoff");

        assert_eq!(deserialized, method);
    }
}
