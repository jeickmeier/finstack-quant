//! Neutral credit-rating types, parsing, and ordering.
//!
//! This module provides fundamental credit rating types used throughout the
//! financial system, including:
//!
//! - [`CreditRating`]: A unified credit rating scale with notch-level precision
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::types::CreditRating;
//!
//! // Check investment grade status
//! assert!(CreditRating::BBB.is_investment_grade());
//! assert!(!CreditRating::BB.is_investment_grade());
//!
//! ```

use serde::{Deserialize, Serialize};

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
/// use finstack_quant_core::types::CreditRating;
///
/// assert!(CreditRating::A.is_investment_grade());
/// assert!(CreditRating::BBBMinus.is_investment_grade());
/// assert!(!CreditRating::BBPlus.is_investment_grade());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum CreditRating {
    /// AAA / Aaa — Highest quality, minimal credit risk
    #[serde(rename = "AAA")]
    AAA,
    /// AA+ / Aa1
    #[serde(rename = "AA+")]
    AAPlus,
    /// AA / Aa2 — Very high quality, very low credit risk
    #[serde(rename = "AA")]
    AA,
    /// AA- / Aa3
    #[serde(rename = "AA-")]
    AAMinus,
    /// A+ / A1
    #[serde(rename = "A+")]
    APlus,
    /// A / A2 — High quality, low credit risk
    #[serde(rename = "A")]
    A,
    /// A- / A3
    #[serde(rename = "A-")]
    AMinus,
    /// BBB+ / Baa1
    #[serde(rename = "BBB+")]
    BBBPlus,
    /// BBB / Baa2 — Medium grade, moderate credit risk (lowest investment grade)
    #[serde(rename = "BBB")]
    BBB,
    /// BBB- / Baa3
    #[serde(rename = "BBB-")]
    BBBMinus,
    /// BB+ / Ba1
    #[serde(rename = "BB+")]
    BBPlus,
    /// BB / Ba2 — Speculative, substantial credit risk
    #[serde(rename = "BB")]
    BB,
    /// BB- / Ba3
    #[serde(rename = "BB-")]
    BBMinus,
    /// B+ / B1
    #[serde(rename = "B+")]
    BPlus,
    /// B / B2 — Highly speculative, high credit risk
    #[serde(rename = "B")]
    B,
    /// B- / B3
    #[serde(rename = "B-")]
    BMinus,
    /// CCC+ / Caa1
    #[serde(rename = "CCC+")]
    CCCPlus,
    /// CCC / Caa2 — Poor standing, very high credit risk
    #[serde(rename = "CCC")]
    CCC,
    /// CCC- / Caa3
    #[serde(rename = "CCC-")]
    CCCMinus,
    /// CC / Ca — Highly vulnerable, near default
    #[serde(rename = "CC")]
    CC,
    /// C — Lowest rated, typically in default
    #[serde(rename = "C")]
    C,
    /// D — In default
    #[serde(rename = "D")]
    D,
    /// NR — Not rated
    #[serde(rename = "NR")]
    NR,
}

impl CreditRating {
    /// Letter bucket of the rating with the `+`/`-` modifier folded away:
    /// `BPlus`, `B` and `BMinus` all map to `B`; unmodified ratings map to
    /// themselves.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::CreditRating;
    ///
    /// assert_eq!(CreditRating::BMinus.bucket(), CreditRating::B);
    /// assert_eq!(CreditRating::AAA.bucket(), CreditRating::AAA);
    /// ```
    pub fn bucket(self) -> Self {
        match self {
            Self::AAPlus | Self::AA | Self::AAMinus => Self::AA,
            Self::APlus | Self::A | Self::AMinus => Self::A,
            Self::BBBPlus | Self::BBB | Self::BBBMinus => Self::BBB,
            Self::BBPlus | Self::BB | Self::BBMinus => Self::BB,
            Self::BPlus | Self::B | Self::BMinus => Self::B,
            Self::CCCPlus | Self::CCC | Self::CCCMinus => Self::CCC,
            other => other,
        }
    }

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
    /// use finstack_quant_core::types::CreditRating;
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
    /// use finstack_quant_core::types::CreditRating;
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
    /// use finstack_quant_core::types::CreditRating;
    ///
    /// assert!(CreditRating::D.is_default());
    /// assert!(!CreditRating::CCC.is_default());
    /// ```
    pub fn is_default(&self) -> bool {
        matches!(self, Self::D)
    }

    /// Signed notch distance from `self` to `other` on the 23-step scale.
    ///
    /// Positive when `other` is weaker (further down the scale), negative when
    /// it is stronger, zero when equal. `NR` sits between `C` and `D`, so
    /// distances involving `NR` follow that ordinal placement.
    ///
    /// # Arguments
    ///
    /// * `other` - Rating to measure against; `self.notches_to(other)` equals
    ///   `-(other.notches_to(self))`
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::types::CreditRating;
    ///
    /// assert_eq!(CreditRating::BBB.notches_to(CreditRating::BB), 3);
    /// assert_eq!(CreditRating::BB.notches_to(CreditRating::BBB), -3);
    /// ```
    pub fn notches_to(self, other: Self) -> i32 {
        i32::from(other.ordinal()) - i32::from(self.ordinal())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notches_to_is_signed_and_antisymmetric() {
        assert_eq!(CreditRating::BBB.notches_to(CreditRating::BB), 3);
        assert_eq!(CreditRating::BB.notches_to(CreditRating::BBB), -3);
        assert_eq!(CreditRating::AAA.notches_to(CreditRating::AAA), 0);
        assert_eq!(CreditRating::C.notches_to(CreditRating::NR), 1);
        assert_eq!(CreditRating::NR.notches_to(CreditRating::D), 1);
    }

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
    fn credit_rating_serde_wire_name_is_stable() {
        let json = serde_json::to_string(&CreditRating::BBBMinus).expect("rating serializes");

        assert_eq!(json, r#""BBB-""#);
        let restored: CreditRating = serde_json::from_str(&json).expect("rating deserializes");
        assert_eq!(restored, CreditRating::BBBMinus);

        for noncanonical in [r#""BBBMinus""#, r#""bbb-""#, r#""Baa3""#] {
            assert!(
                serde_json::from_str::<CreditRating>(noncanonical).is_err(),
                "noncanonical persisted rating {noncanonical} must be rejected"
            );
        }
    }
}
