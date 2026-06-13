//! Credit rating types and rating factor tables.
//!
//! This module provides fundamental credit rating types used throughout the
//! financial system, including:
//!
//! - [`CreditRating`]: A unified credit rating scale with notch-level precision
//! - [`RatingFactorTable`]: Rating-to-factor mappings (e.g., Moody's WARF)
//! - [`RatingLabel`]: Stable, display-ready labels sourced from `CreditRating`
//!
//! # Rating Factors
//!
//! Rating factors are numerical values that correspond to expected default
//! probabilities for each rating category. The most commonly used is the
//! Moody's Weighted Average Rating Factor (WARF) system used in CLO analysis.
//!
//! # Examples
//!
//! ```rust
//! use finstack_core::types::CreditRating;
//!
//! // Check investment grade status
//! assert!(CreditRating::BBB.is_investment_grade());
//! assert!(!CreditRating::BB.is_investment_grade());
//!
//! // Get WARF factor for a rating
//! assert_eq!(CreditRating::B.warf().unwrap(), 2720.0);
//! assert_eq!(CreditRating::BPlus.warf().unwrap(), 2220.0);
//! ```

use crate::collections::HashMap;
use crate::credit::registry::{embedded_registry, RatingFactorTableParts};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

// ============================================================================
// CREDIT RATING
// ============================================================================

/// Unified credit rating scale with notch-level precision (agency-agnostic).
///
/// Each variant represents a specific notch in the rating scale. Ratings
/// without a `Plus`/`Minus` suffix represent the "flat" (middle) notch.
///
/// | Variant     | S&P/Fitch | Moody's |
/// |-------------|-----------|---------|
/// | `AAA`       | AAA       | Aaa     |
/// | `AAPlus`    | AA+       | Aa1     |
/// | `AA`        | AA        | Aa2     |
/// | `AAMinus`   | AA-       | Aa3     |
/// | `APlus`     | A+        | A1      |
/// | `A`         | A         | A2      |
/// | `AMinus`    | A-        | A3      |
/// | `BBBPlus`   | BBB+      | Baa1    |
/// | `BBB`       | BBB       | Baa2    |
/// | `BBBMinus`  | BBB-      | Baa3    |
/// | `BBPlus`    | BB+       | Ba1     |
/// | `BB`        | BB        | Ba2     |
/// | `BBMinus`   | BB-       | Ba3     |
/// | `BPlus`     | B+        | B1      |
/// | `B`         | B         | B2      |
/// | `BMinus`    | B-        | B3      |
/// | `CCCPlus`   | CCC+      | Caa1    |
/// | `CCC`       | CCC       | Caa2    |
/// | `CCCMinus`  | CCC-      | Caa3    |
/// | `CC`        | CC        | Ca      |
/// | `C`         | C         | C       |
/// | `D`         | D         | D       |
/// | `NR`        | NR        | NR      |
///
/// # Investment Grade
///
/// Ratings of `BBBMinus` and above are considered "investment grade":
///
/// ```rust
/// use finstack_core::types::CreditRating;
///
/// assert!(CreditRating::A.is_investment_grade());
/// assert!(CreditRating::BBBMinus.is_investment_grade());
/// assert!(!CreditRating::BBPlus.is_investment_grade());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum CreditRating {
    /// AAA / Aaa — Highest quality, minimal credit risk
    AAA,
    /// AA+ / Aa1
    AAPlus,
    /// AA / Aa2 — Very high quality, very low credit risk
    AA,
    /// AA- / Aa3
    AAMinus,
    /// A+ / A1
    APlus,
    /// A / A2 — High quality, low credit risk
    A,
    /// A- / A3
    AMinus,
    /// BBB+ / Baa1
    BBBPlus,
    /// BBB / Baa2 — Medium grade, moderate credit risk (lowest investment grade)
    BBB,
    /// BBB- / Baa3
    BBBMinus,
    /// BB+ / Ba1
    BBPlus,
    /// BB / Ba2 — Speculative, substantial credit risk
    BB,
    /// BB- / Ba3
    BBMinus,
    /// B+ / B1
    BPlus,
    /// B / B2 — Highly speculative, high credit risk
    B,
    /// B- / B3
    BMinus,
    /// CCC+ / Caa1
    CCCPlus,
    /// CCC / Caa2 — Poor standing, very high credit risk
    CCC,
    /// CCC- / Caa3
    CCCMinus,
    /// CC / Ca — Highly vulnerable, near default
    CC,
    /// C — Lowest rated, typically in default
    C,
    /// D — In default
    D,
    /// NR — Not rated
    NR,
}

impl CreditRating {
    /// Numeric ordinal for ordering. Lower values indicate higher credit quality.
    /// NR is placed between C and D in the ordering.
    fn ordinal(self) -> u8 {
        match self {
            Self::AAA => 0,
            Self::AAPlus => 1,
            Self::AA => 2,
            Self::AAMinus => 3,
            Self::APlus => 4,
            Self::A => 5,
            Self::AMinus => 6,
            Self::BBBPlus => 7,
            Self::BBB => 8,
            Self::BBBMinus => 9,
            Self::BBPlus => 10,
            Self::BB => 11,
            Self::BBMinus => 12,
            Self::BPlus => 13,
            Self::B => 14,
            Self::BMinus => 15,
            Self::CCCPlus => 16,
            Self::CCC => 17,
            Self::CCCMinus => 18,
            Self::CC => 19,
            Self::C => 20,
            Self::NR => 21,
            Self::D => 22,
        }
    }
}

impl PartialOrd for CreditRating {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CreditRating {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ordinal().cmp(&other.ordinal())
    }
}

impl CreditRating {
    /// Check if rating is investment grade (BBB- and above).
    ///
    /// Investment grade ratings indicate lower credit risk and are often
    /// required for institutional investors with regulatory constraints.
    ///
    /// # Example
    /// ```rust
    /// use finstack_core::types::CreditRating;
    ///
    /// assert!(CreditRating::BBB.is_investment_grade());
    /// assert!(CreditRating::BBBMinus.is_investment_grade());
    /// assert!(!CreditRating::BBPlus.is_investment_grade());
    /// ```
    pub fn is_investment_grade(&self) -> bool {
        matches!(
            self,
            Self::AAA
                | Self::AAPlus
                | Self::AA
                | Self::AAMinus
                | Self::APlus
                | Self::A
                | Self::AMinus
                | Self::BBBPlus
                | Self::BBB
                | Self::BBBMinus
        )
    }

    /// Check if rating is speculative grade (below BBB-).
    ///
    /// Speculative grade (or "junk") ratings indicate higher credit risk.
    ///
    /// # Example
    /// ```rust
    /// use finstack_core::types::CreditRating;
    ///
    /// assert!(CreditRating::BB.is_speculative_grade());
    /// assert!(!CreditRating::A.is_speculative_grade());
    /// ```
    pub fn is_speculative_grade(&self) -> bool {
        !self.is_investment_grade() && *self != Self::NR
    }

    /// Check if the rating indicates default status.
    ///
    /// # Example
    /// ```rust
    /// use finstack_core::types::CreditRating;
    ///
    /// assert!(CreditRating::D.is_default());
    /// assert!(!CreditRating::CCC.is_default());
    /// ```
    pub fn is_default(&self) -> bool {
        matches!(self, Self::D)
    }

    /// Returns the Moody's WARF (Weighted Average Rating Factor) for this rating.
    ///
    /// # Errors
    ///
    /// Returns an error if no WARF factor is defined for this rating.
    ///
    /// # Example
    /// ```rust
    /// use finstack_core::types::CreditRating;
    ///
    /// assert_eq!(CreditRating::AAA.warf().unwrap(), 1.0);
    /// assert_eq!(CreditRating::B.warf().unwrap(), 2720.0);
    /// assert_eq!(CreditRating::BPlus.warf().unwrap(), 2220.0);
    /// ```
    pub fn warf(self) -> crate::Result<f64> {
        moodys_warf_factor(self)
    }

    /// S&P/Fitch-style string representation.
    fn to_generic_string(self) -> &'static str {
        match self {
            Self::AAA => "AAA",
            Self::AAPlus => "AA+",
            Self::AA => "AA",
            Self::AAMinus => "AA-",
            Self::APlus => "A+",
            Self::A => "A",
            Self::AMinus => "A-",
            Self::BBBPlus => "BBB+",
            Self::BBB => "BBB",
            Self::BBBMinus => "BBB-",
            Self::BBPlus => "BB+",
            Self::BB => "BB",
            Self::BBMinus => "BB-",
            Self::BPlus => "B+",
            Self::B => "B",
            Self::BMinus => "B-",
            Self::CCCPlus => "CCC+",
            Self::CCC => "CCC",
            Self::CCCMinus => "CCC-",
            Self::CC => "CC",
            Self::C => "C",
            Self::D => "D",
            Self::NR => "NR",
        }
    }

    /// Moody's-style string representation (e.g., `Aa1`, `Ba2`, `B3`).
    pub fn to_moodys_string(self) -> &'static str {
        match self {
            Self::AAA => "Aaa",
            Self::AAPlus => "Aa1",
            Self::AA => "Aa2",
            Self::AAMinus => "Aa3",
            Self::APlus => "A1",
            Self::A => "A2",
            Self::AMinus => "A3",
            Self::BBBPlus => "Baa1",
            Self::BBB => "Baa2",
            Self::BBBMinus => "Baa3",
            Self::BBPlus => "Ba1",
            Self::BB => "Ba2",
            Self::BBMinus => "Ba3",
            Self::BPlus => "B1",
            Self::B => "B2",
            Self::BMinus => "B3",
            Self::CCCPlus => "Caa1",
            Self::CCC => "Caa2",
            Self::CCCMinus => "Caa3",
            Self::CC => "Ca",
            Self::C => "C",
            Self::D => "D",
            Self::NR => "NR",
        }
    }
}

impl core::fmt::Display for CreditRating {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.to_generic_string())
    }
}

// ============================================================================
// RATING LABEL
// ============================================================================

/// Stable label for referring to ratings (curve names, exports, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RatingLabel(String);

impl RatingLabel {
    /// Create a label using the generic (S&P/Fitch-style) string.
    pub fn generic(rating: CreditRating) -> Self {
        Self(rating.to_string())
    }

    /// Create a label using Moody's naming convention.
    pub fn moodys(rating: CreditRating) -> Self {
        Self(rating.to_moodys_string().to_owned())
    }

    /// Access the underlying string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the label and return the owned string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<CreditRating> for RatingLabel {
    fn from(value: CreditRating) -> Self {
        Self::generic(value)
    }
}

impl core::fmt::Display for RatingLabel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::str::FromStr for CreditRating {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_credit_rating(s)
    }
}

fn parse_credit_rating(value: &str) -> Result<CreditRating, crate::Error> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(crate::error::InputError::InvalidRating {
            value: value.to_string(),
        }
        .into());
    }

    let normalized = trimmed.replace(' ', "");
    if normalized.is_empty() {
        return Err(crate::error::InputError::InvalidRating {
            value: value.to_string(),
        }
        .into());
    }

    let upper = normalized.to_uppercase();

    if matches!(upper.as_str(), "NR" | "NOTRATED" | "UNRATED") {
        return Ok(CreditRating::NR);
    }

    if matches!(upper.as_str(), "D" | "DEFAULT") {
        return Ok(CreditRating::D);
    }

    // Detect notch suffix: +/-, or trailing digit 1/2/3 (Moody's style)
    enum Notch {
        Plus,
        Flat,
        Minus,
    }
    let mut notch = Notch::Flat;
    let mut base_slice = upper.as_str();

    if base_slice.ends_with('+') {
        notch = Notch::Plus;
        base_slice = &base_slice[..base_slice.len() - 1];
    } else if base_slice.ends_with('-') {
        notch = Notch::Minus;
        base_slice = &base_slice[..base_slice.len() - 1];
    } else if let Some(last) = base_slice.chars().last() {
        if last.is_ascii_digit() {
            notch = match last {
                '1' => Notch::Plus,
                '2' => Notch::Flat,
                '3' => Notch::Minus,
                _ => {
                    return Err(crate::error::InputError::InvalidRating {
                        value: value.to_string(),
                    }
                    .into())
                }
            };
            base_slice = &base_slice[..base_slice.len() - 1];
        }
    }

    if base_slice.is_empty() {
        return Err(crate::error::InputError::InvalidRating {
            value: value.to_string(),
        }
        .into());
    }

    // Ratings that don't support notches always resolve to the flat variant
    let supports_notches = matches!(
        base_slice,
        "AA" | "A" | "BBB" | "BAA" | "BB" | "BA" | "B" | "CCC" | "CAA"
    );

    let rating = match (base_slice, supports_notches) {
        ("AAA", _) => CreditRating::AAA,
        ("AA", true) => match notch {
            Notch::Plus => CreditRating::AAPlus,
            Notch::Flat => CreditRating::AA,
            Notch::Minus => CreditRating::AAMinus,
        },
        ("A", true) => match notch {
            Notch::Plus => CreditRating::APlus,
            Notch::Flat => CreditRating::A,
            Notch::Minus => CreditRating::AMinus,
        },
        ("BBB" | "BAA", true) => match notch {
            Notch::Plus => CreditRating::BBBPlus,
            Notch::Flat => CreditRating::BBB,
            Notch::Minus => CreditRating::BBBMinus,
        },
        ("BB" | "BA", true) => match notch {
            Notch::Plus => CreditRating::BBPlus,
            Notch::Flat => CreditRating::BB,
            Notch::Minus => CreditRating::BBMinus,
        },
        ("B", true) => match notch {
            Notch::Plus => CreditRating::BPlus,
            Notch::Flat => CreditRating::B,
            Notch::Minus => CreditRating::BMinus,
        },
        ("CCC" | "CAA", true) => match notch {
            Notch::Plus => CreditRating::CCCPlus,
            Notch::Flat => CreditRating::CCC,
            Notch::Minus => CreditRating::CCCMinus,
        },
        ("CC" | "CA", _) => CreditRating::CC,
        ("C", _) => CreditRating::C,
        ("D", _) => CreditRating::D,
        ("NR", _) => CreditRating::NR,
        _ => {
            return Err(crate::error::InputError::InvalidRating {
                value: value.to_string(),
            }
            .into())
        }
    };

    Ok(rating)
}

// ============================================================================
// RATING FACTOR TABLE
// ============================================================================

/// Rating factor table for a specific rating agency methodology.
///
/// Rating factors are numerical values that correspond to expected cumulative
/// default rates over the life of an asset. They are used extensively in
/// structured credit analysis, particularly for CLO WARF calculations.
///
/// # Example
///
/// ```rust
/// use finstack_core::types::{CreditRating, RatingFactorTable};
///
/// let table = RatingFactorTable::moodys_standard().unwrap();
/// let factor = table.get_factor(CreditRating::B).unwrap();
/// assert_eq!(factor, 2720.0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingFactorTable {
    /// Factors by rating
    factors: HashMap<CreditRating, f64>,
    /// Agency name
    agency: String,
    /// Methodology description
    methodology: String,
    /// Default factor when a rating is missing
    default_factor: f64,
}

impl RatingFactorTable {
    /// Create Moody's standard WARF table.
    ///
    /// Based on Moody's IDEALIZED DEFAULT RATES table used in CLO analysis.
    /// The table includes every notch published by Moody's as of September
    /// 2024.
    ///
    /// Source: Moody's "Approach to Rating Collateralized Loan Obligations"
    ///
    /// # Example
    /// ```rust
    /// use finstack_core::types::{CreditRating, RatingFactorTable};
    ///
    /// let table = RatingFactorTable::moodys_standard().unwrap();
    /// assert_eq!(table.get_factor(CreditRating::AAA).unwrap(), 1.0);
    /// assert_eq!(table.get_factor(CreditRating::B).unwrap(), 2720.0);
    /// ```
    pub fn moodys_standard() -> crate::Result<Self> {
        Self::from_registry_id(embedded_registry()?.default_rating_factor_table_id())
    }

    /// Load a rating factor table from the credit assumptions registry.
    pub fn from_registry_id(id: &str) -> crate::Result<Self> {
        Ok(Self::from_registry_parts(
            embedded_registry()?.rating_factor_table(id)?,
        ))
    }

    fn from_registry_parts(parts: RatingFactorTableParts) -> Self {
        Self {
            factors: parts.factors,
            agency: parts.agency,
            methodology: parts.methodology,
            default_factor: parts.default_factor,
        }
    }

    /// Get factor for a specific rating.
    ///
    /// If no entry exists, returns an error instead of silently substituting
    /// the table default.
    pub fn get_factor(&self, rating: CreditRating) -> crate::Result<f64> {
        self.factors.get(&rating).copied().ok_or_else(|| {
            crate::Error::Input(crate::error::InputError::NotFound {
                id: format!("rating factor for {}", rating),
            })
        })
    }

    /// Get factor for a specific rating, falling back to the table default when missing.
    pub fn get_factor_or_default(&self, rating: CreditRating) -> f64 {
        self.get_factor(rating).unwrap_or(self.default_factor)
    }

    /// Get agency name.
    pub fn agency(&self) -> &str {
        &self.agency
    }

    /// Get methodology description.
    pub fn methodology(&self) -> &str {
        &self.methodology
    }

    /// Get the table default factor used for missing entries.
    pub fn default_factor(&self) -> f64 {
        self.default_factor
    }
}

// ============================================================================
// CONVENIENCE FUNCTIONS
// ============================================================================

/// Lazily initialized Moody's WARF rating factor table
static MOODYS_WARF_TABLE: OnceLock<crate::Result<RatingFactorTable>> = OnceLock::new();

/// Get Moody's WARF factor for a rating (convenience function).
///
/// This is the standard function that should be used throughout the codebase
/// for consistent WARF calculations. The table is lazily initialized on first
/// call and cached for subsequent calls.
///
/// # Example
/// ```rust
/// use finstack_core::types::{CreditRating, moodys_warf_factor};
///
/// assert_eq!(moodys_warf_factor(CreditRating::AAA).unwrap(), 1.0);
/// assert_eq!(moodys_warf_factor(CreditRating::B).unwrap(), 2720.0);
/// assert_eq!(moodys_warf_factor(CreditRating::BBBPlus).unwrap(), 260.0);
/// ```
pub fn moodys_warf_factor(rating: CreditRating) -> crate::Result<f64> {
    match MOODYS_WARF_TABLE.get_or_init(RatingFactorTable::moodys_standard) {
        Ok(table) => table.get_factor(rating),
        Err(err) => Err(err.clone()),
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credit_rating_investment_grade() {
        assert!(CreditRating::AAA.is_investment_grade());
        assert!(CreditRating::AAPlus.is_investment_grade());
        assert!(CreditRating::AA.is_investment_grade());
        assert!(CreditRating::AAMinus.is_investment_grade());
        assert!(CreditRating::APlus.is_investment_grade());
        assert!(CreditRating::A.is_investment_grade());
        assert!(CreditRating::AMinus.is_investment_grade());
        assert!(CreditRating::BBBPlus.is_investment_grade());
        assert!(CreditRating::BBB.is_investment_grade());
        assert!(CreditRating::BBBMinus.is_investment_grade());
        assert!(!CreditRating::BBPlus.is_investment_grade());
        assert!(!CreditRating::BB.is_investment_grade());
        assert!(!CreditRating::B.is_investment_grade());
        assert!(!CreditRating::CCC.is_investment_grade());
        assert!(!CreditRating::D.is_investment_grade());
        assert!(!CreditRating::NR.is_investment_grade());
    }

    #[test]
    fn test_credit_rating_speculative_grade() {
        assert!(!CreditRating::AAA.is_speculative_grade());
        assert!(!CreditRating::BBB.is_speculative_grade());
        assert!(!CreditRating::BBBMinus.is_speculative_grade());
        assert!(CreditRating::BBPlus.is_speculative_grade());
        assert!(CreditRating::BB.is_speculative_grade());
        assert!(CreditRating::B.is_speculative_grade());
        assert!(CreditRating::CCC.is_speculative_grade());
        assert!(CreditRating::D.is_speculative_grade());
        assert!(!CreditRating::NR.is_speculative_grade());
    }

    #[test]
    fn test_credit_rating_default() {
        assert!(!CreditRating::CCC.is_default());
        assert!(CreditRating::D.is_default());
    }

    #[test]
    fn test_credit_rating_display() {
        assert_eq!(format!("{}", CreditRating::AAA), "AAA");
        assert_eq!(format!("{}", CreditRating::BBPlus), "BB+");
        assert_eq!(format!("{}", CreditRating::BB), "BB");
        assert_eq!(format!("{}", CreditRating::BBMinus), "BB-");
        assert_eq!(format!("{}", CreditRating::NR), "NR");
    }

    #[test]
    fn test_credit_rating_ordering() {
        assert!(CreditRating::AAA < CreditRating::AAPlus);
        assert!(CreditRating::AAPlus < CreditRating::AA);
        assert!(CreditRating::AA < CreditRating::AAMinus);
        assert!(CreditRating::AAMinus < CreditRating::APlus);
        assert!(CreditRating::BBBMinus < CreditRating::BBPlus);
        assert!(CreditRating::B < CreditRating::D);
    }

    #[test]
    fn test_credit_rating_from_str() {
        assert_eq!(
            "AAA".parse::<CreditRating>().expect("AAA"),
            CreditRating::AAA
        );
        assert_eq!(
            "aaa".parse::<CreditRating>().expect("aaa"),
            CreditRating::AAA
        );
        assert_eq!(
            "BB+".parse::<CreditRating>().expect("BB+"),
            CreditRating::BBPlus
        );
        assert_eq!("BB".parse::<CreditRating>().expect("BB"), CreditRating::BB);
        assert_eq!(
            "BB-".parse::<CreditRating>().expect("BB-"),
            CreditRating::BBMinus
        );
        assert_eq!(
            "B1".parse::<CreditRating>().expect("B1"),
            CreditRating::BPlus
        );
        assert_eq!("B2".parse::<CreditRating>().expect("B2"), CreditRating::B);
        assert_eq!(
            "B3".parse::<CreditRating>().expect("B3"),
            CreditRating::BMinus
        );
        assert_eq!(
            "Ba2".parse::<CreditRating>().expect("Ba2"),
            CreditRating::BB
        );
        assert_eq!(
            "Baa3".parse::<CreditRating>().expect("Baa3"),
            CreditRating::BBBMinus
        );
        assert_eq!("NR".parse::<CreditRating>().expect("NR"), CreditRating::NR);
        assert_eq!(
            "Not Rated".parse::<CreditRating>().expect("Not Rated"),
            CreditRating::NR
        );
        assert!("XYZ".parse::<CreditRating>().is_err());
    }

    #[test]
    fn test_moodys_string() {
        assert_eq!(CreditRating::AAA.to_moodys_string(), "Aaa");
        assert_eq!(CreditRating::AAPlus.to_moodys_string(), "Aa1");
        assert_eq!(CreditRating::BBBMinus.to_moodys_string(), "Baa3");
        assert_eq!(CreditRating::BPlus.to_moodys_string(), "B1");
        assert_eq!(CreditRating::CC.to_moodys_string(), "Ca");
    }

    #[test]
    fn test_rating_labels() {
        let generic = RatingLabel::generic(CreditRating::BBBMinus);
        let moodys = RatingLabel::moodys(CreditRating::BBBMinus);

        assert_eq!(generic.as_str(), "BBB-");
        assert_eq!(moodys.as_str(), "Baa3");
        assert_eq!(generic.to_string(), "BBB-");
        assert_eq!(moodys.into_inner(), "Baa3".to_string());
    }

    #[test]
    fn test_moodys_warf_factors() {
        let table = RatingFactorTable::moodys_standard().expect("registry table");

        assert_eq!(table.get_factor(CreditRating::AAA).expect("AAA"), 1.0);
        assert_eq!(table.get_factor(CreditRating::AAPlus).expect("AA+"), 10.0);
        assert_eq!(table.get_factor(CreditRating::A).expect("A"), 120.0);
        assert_eq!(table.get_factor(CreditRating::BMinus).expect("B-"), 3490.0);
        assert_eq!(table.get_factor(CreditRating::CCC).expect("CCC"), 6500.0);

        //  pin published
        // Moody's values — Ba3 (BB-) = 1766 (was 1760) and CC = 10000
        // (Ca/C bucket; the previous 9550 was unpublished).
        assert_eq!(
            table.get_factor(CreditRating::BBMinus).expect("BB-"),
            1766.0
        );
        assert_eq!(table.get_factor(CreditRating::CC).expect("CC"), 10000.0);
        assert_eq!(table.get_factor(CreditRating::C).expect("C"), 10000.0);
        assert_eq!(table.get_factor(CreditRating::D).expect("D"), 10000.0);
    }

    #[test]
    fn test_rating_factor_table_errors_when_missing() {
        let table = RatingFactorTable {
            factors: HashMap::default(),
            agency: "Test".to_string(),
            methodology: "Test".to_string(),
            default_factor: 42.0,
        };

        assert!(table.get_factor(CreditRating::AA).is_err());
        assert_eq!(table.get_factor_or_default(CreditRating::AA), 42.0);
    }

    #[test]
    fn test_convenience_function_matches_table() {
        let table = RatingFactorTable::moodys_standard().expect("registry table");

        assert_eq!(
            moodys_warf_factor(CreditRating::AAA).expect("AAA convenience"),
            table.get_factor(CreditRating::AAA).expect("AAA table")
        );
        assert_eq!(
            moodys_warf_factor(CreditRating::AAMinus).expect("AA- convenience"),
            table.get_factor(CreditRating::AAMinus).expect("AA- table")
        );
        assert_eq!(
            moodys_warf_factor(CreditRating::BBB).expect("BBB convenience"),
            table.get_factor(CreditRating::BBB).expect("BBB table")
        );
        assert_eq!(
            moodys_warf_factor(CreditRating::BPlus).expect("B+ convenience"),
            table.get_factor(CreditRating::BPlus).expect("B+ table")
        );
    }

    #[test]
    fn test_rating_factor_table_metadata() {
        let table = RatingFactorTable::moodys_standard().expect("registry table");
        assert_eq!(table.agency(), "Moody's");
        assert_eq!(table.methodology(), "IDEALIZED DEFAULT RATES");
        assert_eq!(table.default_factor(), 3650.0);
    }

    #[test]
    fn test_warf_on_rating() {
        assert_eq!(CreditRating::AAA.warf().expect("AAA"), 1.0);
        assert_eq!(CreditRating::B.warf().expect("B"), 2720.0);
        assert_eq!(CreditRating::BPlus.warf().expect("B+"), 2220.0);
        assert_eq!(CreditRating::BBBMinus.warf().expect("BBB-"), 610.0);
    }
}
