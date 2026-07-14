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
//! - Swap is unseasoned (`as_of <= accrual_start`)
//! - No lookback or observation shift (`lookback_days = 0`, `observation_shift = None`)
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
//! The `lookback_days` and `observation_shift` parameters are fully supported:
//!
//! - **Lookback**: Shifts observation dates back from the accrual period. For example,
//!   with `lookback_days = 2`, observations for the period Jan 1-Apr 1 would be
//!   taken from Dec 28-Mar 28 (2 business days earlier).
//!
//! - **Observation Shift**: Additional adjustment to observation dates. The total
//!   shift is computed as `-lookback_days + observation_shift`.
//!
//! When lookback/shift is non-zero, the fast path is disabled and full daily
//! compounding is performed with shifted observation dates.
//!
//! ## Seasoned Swaps
//!
//! For seasoned swaps where `as_of` falls within an accrual period, historical
//! fixings are required for observation dates before `as_of`. Provide fixings
//! via `ScalarTimeSeries` with id `FIXING:{forward_curve_id}`.
//!
//! # References
//!
//! - **ISDA 2021 Definitions**: Compounded RFR conventions
//! - **ARRC** (Alternative Reference Rates Committee): SOFR conventions
//! - **BoE** (Bank of England): SONIA conventions

/// Method for calculating floating leg coupon payments.
///
/// Different reference rates require different compounding conventions:
/// - **Term rates (SOFR 3M, EURIBOR, historical LIBOR)**: Simple interest
/// - **Overnight rates (SOFR, SONIA, €STR, TONA)**: Compounded in arrears
///
/// # Market Standards
///
/// ## Simple (LIBOR-style)
/// - **Formula**: `Coupon = Notional × (Forward_Rate + Spread) × DCF`
/// - **Use for**: USD LIBOR, EUR EURIBOR, GBP LIBOR (historical)
/// - **Standard**: ISDA 2006 Definitions
///
/// ## Compounded In Arrears (RFR-style)
/// - **Formula**: `Coupon = Notional × [∏(1 + r_i × dcf_i) - 1]`
/// - **Use for**: USD SOFR, GBP SONIA, EUR €STR, JPY TONA
/// - **Standard**: ISDA 2021 Definitions
/// - **Lookback**: Typically 2-5 business days before period end
/// - **Observation shift**: Optional shift for operational convenience
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding;
///
/// // LIBOR-style swap (simple compounding)
/// let simple = FloatingLegCompounding::Simple;
/// assert_eq!(simple, FloatingLegCompounding::default());
///
/// // SOFR OIS swap: plain compounded in arrears (no lookback)
/// let sofr = FloatingLegCompounding::CompoundedInArrears {
///     lookback_days: 0,
///     observation_shift: None,
/// };
/// assert_eq!(sofr, FloatingLegCompounding::sofr());
///
/// // SONIA FRN-style leg with the BoE 5-day lookback (explicit, not the OIS preset)
/// let sonia_frn = FloatingLegCompounding::CompoundedInArrears {
///     lookback_days: 5,
///     observation_shift: None,
/// };
/// assert_ne!(sonia_frn, FloatingLegCompounding::sonia());
/// ```
///
/// # References
///
/// - **ISDA 2021 Definitions**: Compounded RFR conventions
/// - **ARRC** (Alternative Reference Rates Committee): SOFR conventions
/// - **BoE** (Bank of England): SONIA conventions
/// - **ECB**: €STR conventions
///
/// In the IRS instrument implementation, the RFR-style variant
/// (`CompoundedInArrears`) is also used to classify swaps as OIS for
/// discount-only float-leg pricing; see `InterestRateSwap::is_single_curve_ois` for details.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum FloatingLegCompounding {
    /// Simple interest compounding (term-rate style).
    ///
    /// Coupon = Notional × (Forward_Rate + Spread) × Day_Count_Fraction
    ///
    /// Use for:
    /// - Term SOFR / EURIBOR-style swaps with fixed-tenor indices
    /// - Legacy USD/EUR/GBP LIBOR swaps (for back-testing only)
    ///
    /// This is the current default behavior for vanilla IRS and matches
    /// ISDA 2006 term-rate conventions.
    Simple,

    /// Compounded in arrears (overnight RFR rates).
    ///
    /// Coupon = Notional × [∏(1 + r_i × dcf_i) - 1] where the product
    /// is taken over daily observations in the accrual period.
    ///
    /// Use for:
    /// - USD SOFR (Secured Overnight Financing Rate)
    /// - GBP SONIA (Sterling Overnight Index Average)
    /// - EUR €STR (Euro Short-Term Rate)
    /// - JPY TONA (Tokyo Overnight Average Rate)
    ///
    /// # Fields
    ///
    /// - `lookback_days`: Days to shift observation end date before period end (typically 2-5)
    /// - `observation_shift`: Optional additional shift for operational convenience
    ///
    /// # Market Conventions
    ///
    /// - **SOFR**: 2-day lookback (ARRC recommended)
    /// - **SONIA**: 5-day lookback (BoE recommended)
    /// - **€STR**: 2-day shift (ECB convention)
    /// - **TONA**: 2-day lag (JSCC convention)
    CompoundedInArrears {
        /// Number of business days to shift observation dates back from the accrual
        /// period (lookback).  Typically 2–5 days depending on market convention.
        ///
        /// The observation dates are shifted while the day-count-fraction (DCF)
        /// weights remain anchored to the **original** accrual period dates.
        /// This is consistent with "lookback without observation shift" as
        /// described in the ISDA 2021 Definitions and ARRC SOFR conventions.
        #[schemars(range(min = 0, max = 31))]
        lookback_days: i32,

        /// Optional ISDA 2021 observation shift (in business days), as an
        /// **alternative** to a lookback.
        ///
        /// When set (non-zero), observation dates are shifted *backward* by
        /// this many business days and the day-count-fraction weights are
        /// shifted with them — identical semantics to
        /// [`FloatingLegCompounding::CompoundedWithObservationShift`].
        ///
        /// Combining a non-zero `lookback_days` with a non-zero
        /// `observation_shift` is **rejected** at pricing time on every
        /// pricing path: the two conventions are mutually exclusive, and the
        /// previous behavior (the shift silently cancelling the lookback on
        /// one path while erroring on the other) was a defect.
        #[schemars(range(min = 0, max = 31))]
        observation_shift: Option<i32>,
    },

    /// Compounded in arrears with true ISDA 2021 observation shift.
    ///
    /// Unlike `CompoundedInArrears` (lookback semantics), this variant shifts
    /// **both** the observation dates AND the day-count-fraction (DCF) weights.
    /// This matches ISDA 2021 Definitions Section 4.5(c).
    ///
    /// ```text
    /// Lookback:           DCF(d, d+1)           × rate(d - shift, d+1 - shift)
    /// Observation Shift:  DCF(d - shift, d+1 - shift) × rate(d - shift, d+1 - shift)
    /// ```
    CompoundedWithObservationShift {
        /// Number of business days to shift both observation dates and DCF weights.
        #[schemars(range(min = 0, max = 31))]
        shift_days: i32,
    },

    /// Compounded in arrears with a rate cut-off near the period end.
    ///
    /// This freezes the last observed overnight rate for the final `cutoff_days`
    /// business days of the accrual period. Bloomberg SWPM labels this
    /// convention as "Rate Cut-Off Days".
    CompoundedWithRateCutoff {
        /// Number of business days before period end to freeze the overnight rate.
        #[schemars(range(min = 0, max = 31))]
        cutoff_days: i32,
    },
}

impl Default for FloatingLegCompounding {
    /// Default to simple compounding (LIBOR-style, most conservative).
    fn default() -> Self {
        Self::Simple
    }
}

/// Market-standard compounding presets for common RFR **swaps** (cleared OIS).
///
/// Cleared OIS swaps compound the overnight rate plain in-arrears over the
/// accrual period, with only a payment delay — no observation lookback or
/// shift. The ARRC 2-business-day and BoE 5-business-day lookbacks are *FRN
/// coupon* conventions, not OIS swap conventions; using them on OIS both
/// introduces a small basis (sub-bp to ~1bp) and disables the exact `1/DF`
/// telescoping fast path. FRN-style legs should use
/// [`FloatingLegCompounding::CompoundedInArrears`] with an explicit lookback or
/// the `*_observation_shift` presets.
impl FloatingLegCompounding {
    /// USD SOFR OIS convention (plain compounded in arrears, payment delay only).
    pub fn sofr() -> Self {
        Self::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        }
    }

    /// USD Fed Funds / EFFR-style overnight convention (no lookback).
    ///
    /// Bloomberg `FEDL01 Index` OIS conventions typically do **not** apply the SOFR-style
    /// observation lookback. We model that as `lookback_days = 0`.
    pub fn fedfunds() -> Self {
        Self::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        }
    }

    /// GBP SONIA OIS convention (plain compounded in arrears).
    ///
    /// The BoE 5-business-day lookback applies to SONIA FRNs, not OIS swaps.
    pub fn sonia() -> Self {
        Self::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        }
    }

    /// EUR €STR OIS convention (plain compounded in arrears).
    pub fn estr() -> Self {
        Self::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        }
    }

    /// JPY TONA OIS convention (plain compounded in arrears).
    pub fn tona() -> Self {
        Self::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        }
    }

    /// CHF SARON OIS convention (plain compounded in arrears).
    pub fn saron() -> Self {
        Self::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        }
    }

    /// USD SOFR with ISDA 2021 observation shift (2-day shift).
    pub fn sofr_observation_shift() -> Self {
        Self::CompoundedWithObservationShift { shift_days: 2 }
    }

    /// GBP SONIA with ISDA 2021 observation shift (5-day shift).
    pub fn sonia_observation_shift() -> Self {
        Self::CompoundedWithObservationShift { shift_days: 5 }
    }

    /// Compounded RFR with an end-of-period rate cut-off.
    pub fn rate_cutoff(cutoff_days: i32) -> Self {
        Self::CompoundedWithRateCutoff { cutoff_days }
    }
}

impl std::fmt::Display for FloatingLegCompounding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FloatingLegCompounding::Simple => write!(f, "simple"),
            FloatingLegCompounding::CompoundedInArrears { .. } => {
                write!(f, "compounded_in_arrears")
            }
            FloatingLegCompounding::CompoundedWithObservationShift { .. } => {
                write!(f, "compounded_observation_shift")
            }
            FloatingLegCompounding::CompoundedWithRateCutoff { .. } => {
                write!(f, "compounded_rate_cutoff")
            }
        }
    }
}

impl std::str::FromStr for FloatingLegCompounding {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "simple" => Ok(Self::Simple),
            "sofr" => Ok(Self::sofr()),
            "sonia" => Ok(Self::sonia()),
            "estr" | "€str" | "ester" => Ok(Self::estr()),
            "tona" => Ok(Self::tona()),
            "fedfunds" | "fed_funds" | "effr" => Ok(Self::fedfunds()),
            "compounded_in_arrears" | "compounded" => Ok(Self::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None,
            }),
            "compounded_observation_shift" | "observation_shift" => {
                Ok(Self::CompoundedWithObservationShift { shift_days: 0 })
            }
            "sofr_observation_shift" => Ok(Self::sofr_observation_shift()),
            "sonia_observation_shift" => Ok(Self::sonia_observation_shift()),
            "rate_cutoff" | "rate_cut_off" | "compounded_rate_cutoff" | "compounded_lockout" => {
                Ok(Self::CompoundedWithRateCutoff { cutoff_days: 0 })
            }
            other => Err(format!(
                "Unknown floating leg compounding: '{}'. Valid: simple, sofr, sonia, estr, tona, \
                 fedfunds, compounded_in_arrears, compounded, compounded_observation_shift, \
                 sofr_observation_shift, sonia_observation_shift, rate_cutoff",
                other
            )),
        }
    }
}

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
                FloatingLegCompounding::CompoundedInArrears {
                    lookback_days: 0,
                    observation_shift: None,
                }
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
    fn rate_cutoff_roundtrips_and_parses() {
        let method = FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 };
        let json = serde_json::to_string(&method).expect("serialize rate cutoff");
        let deserialized: FloatingLegCompounding =
            serde_json::from_str(&json).expect("deserialize rate cutoff");

        assert_eq!(deserialized, method);
        assert_eq!(
            "rate_cutoff".parse::<FloatingLegCompounding>(),
            Ok(FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 0 })
        );
    }
}
