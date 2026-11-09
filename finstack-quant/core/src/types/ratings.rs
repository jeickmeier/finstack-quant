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
/// | `D`         | D         | (none)  |
/// | `NR`        | NR        | NR      |
///
/// # Ladder, ordering and grades
///
/// The rated ladder is consecutive, `AAA` = 0 through `C` = 20 and `D` = 21:
/// library notch positions, not equally spaced risks. `NR` is explicit
/// unrated status outside the ladder (CORE-CONVENTIONS-019). Ratings are
/// partially ordered: rated values compare by ladder position (`<` means
/// better quality), `NR` is incomparable with every rated value and equal to
/// itself. There is no total order, so a rating cannot key an ordered map.
///
/// Investment grade is `AAA` through `BBB-`; speculative grade is `BB+`
/// through `D`, so `D` is both speculative and the generic default marker.
/// `NR` is neither.
///
/// ```rust
/// use finstack_quant_core::types::CreditRating;
///
/// assert!(CreditRating::A.is_investment_grade());
/// assert!(CreditRating::BBBMinus.is_investment_grade());
/// assert!(!CreditRating::BBPlus.is_investment_grade());
/// assert!(CreditRating::C < CreditRating::D);
/// assert_eq!(CreditRating::C.partial_cmp(&CreditRating::NR), None);
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
    /// BBB / Baa2 — Medium grade, moderate credit risk
    #[serde(rename = "BBB")]
    BBB,
    /// BBB- / Baa3 — lowest investment grade
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
    /// C — lowest rated quality
    #[serde(rename = "C")]
    C,
    /// D — the generic default marker; ladder position 21, after `C`.
    #[serde(rename = "D")]
    D,
    /// NR — explicit unrated status, outside the rated ladder.
    #[serde(rename = "NR")]
    NR,
}

/// Every rated variant in ladder order (`AAA` = 0 … `D` = 21).
const LADDER: [CreditRating; 22] = [
    CreditRating::AAA,
    CreditRating::AAPlus,
    CreditRating::AA,
    CreditRating::AAMinus,
    CreditRating::APlus,
    CreditRating::A,
    CreditRating::AMinus,
    CreditRating::BBBPlus,
    CreditRating::BBB,
    CreditRating::BBBMinus,
    CreditRating::BBPlus,
    CreditRating::BB,
    CreditRating::BBMinus,
    CreditRating::BPlus,
    CreditRating::B,
    CreditRating::BMinus,
    CreditRating::CCCPlus,
    CreditRating::CCC,
    CreditRating::CCCMinus,
    CreditRating::CC,
    CreditRating::C,
    CreditRating::D,
];

impl PartialOrd for CreditRating {
    /// Rated values compare by ladder position (smaller is better quality);
    /// `NR` is equal to itself and incomparable with any rated value.
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        match (self.ladder_position(), other.ladder_position()) {
            (Some(left), Some(right)) => Some(left.cmp(&right)),
            (None, None) => Some(core::cmp::Ordering::Equal),
            _ => None,
        }
    }
}

impl CreditRating {
    /// Ladder position `AAA` = 0 … `C` = 20, `D` = 21; `None` for `NR`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::CreditRating;
    ///
    /// assert_eq!(CreditRating::AAA.ladder_position(), Some(0));
    /// assert_eq!(CreditRating::D.ladder_position(), Some(21));
    /// assert_eq!(CreditRating::NR.ladder_position(), None);
    /// ```
    #[must_use]
    pub fn ladder_position(self) -> Option<u8> {
        LADDER
            .iter()
            .position(|rating| *rating == self)
            .and_then(|position| u8::try_from(position).ok())
    }

    /// Letter bucket of the rating with the `+`/`-` modifier folded away:
    /// `BPlus`, `B` and `BMinus` all map to `B`; `AAA`, `CC`, `C`, `D` and
    /// `NR` map to themselves. Not a mean or midpoint estimate.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::CreditRating;
    ///
    /// assert_eq!(CreditRating::BMinus.bucket(), CreditRating::B);
    /// assert_eq!(CreditRating::AAA.bucket(), CreditRating::AAA);
    /// ```
    #[must_use]
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

    /// Investment grade: `AAA` through `BBB-`. `NR` is not investment grade.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::types::CreditRating;
    ///
    /// assert!(CreditRating::BBB.is_investment_grade());
    /// assert!(CreditRating::BBBMinus.is_investment_grade());
    /// assert!(!CreditRating::BBPlus.is_investment_grade());
    /// assert!(!CreditRating::NR.is_investment_grade());
    /// ```
    #[must_use]
    pub fn is_investment_grade(&self) -> bool {
        self.ladder_position().is_some_and(|position| position <= 9)
    }

    /// Speculative grade: `BB+` through `D`. `NR` is not speculative grade.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::types::CreditRating;
    ///
    /// assert!(CreditRating::BB.is_speculative_grade());
    /// assert!(CreditRating::D.is_speculative_grade());
    /// assert!(!CreditRating::A.is_speculative_grade());
    /// assert!(!CreditRating::NR.is_speculative_grade());
    /// ```
    #[must_use]
    pub fn is_speculative_grade(&self) -> bool {
        self.ladder_position()
            .is_some_and(|position| position >= 10)
    }

    /// Whether this is the generic `D` default marker. `false` is not proof
    /// that an obligor or obligation is performing.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::types::CreditRating;
    ///
    /// assert!(CreditRating::D.is_default());
    /// assert!(!CreditRating::CCC.is_default());
    /// ```
    #[must_use]
    pub fn is_default(&self) -> bool {
        matches!(self, Self::D)
    }

    /// Signed ladder distance from `self` to `other`.
    ///
    /// Positive when `other` is weaker (a downgrade), negative when it is
    /// stronger, zero when equal; `C` → `D` is one step.
    ///
    /// # Arguments
    ///
    /// * `other` - Rated value to measure against; `self.notches_to(other)`
    ///   equals `-(other.notches_to(self))`
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidRating` when either side is `NR`
    /// (including `NR`/`NR`): unrated status has no ladder distance.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::types::CreditRating;
    ///
    /// assert_eq!(CreditRating::BBB.notches_to(CreditRating::BB)?, 3);
    /// assert_eq!(CreditRating::BB.notches_to(CreditRating::BBB)?, -3);
    /// assert!(CreditRating::BBB.notches_to(CreditRating::NR).is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn notches_to(self, other: Self) -> crate::Result<i32> {
        match (self.ladder_position(), other.ladder_position()) {
            (Some(from), Some(to)) => Ok(i32::from(to) - i32::from(from)),
            _ => Err(crate::InputError::InvalidRating {
                value: "NR has no ladder distance".to_string(),
            }
            .into()),
        }
    }

    /// Canonical generic string (`AAA`, `BB+`, `D`, `NR`).
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

    /// Moody's-style long-term symbol (`Aaa` … `C`); `NR` renders as explicit
    /// unrated status text.
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidRating` for `D`: Moody's long-term scale
    /// has no D grade, and `D` is not coerced to `C`.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::types::CreditRating;
    ///
    /// assert_eq!(CreditRating::BBBMinus.to_moodys_string()?, "Baa3");
    /// assert!(CreditRating::D.to_moodys_string().is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn to_moodys_string(self) -> crate::Result<&'static str> {
        Ok(match self {
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
            Self::NR => "NR",
            Self::D => {
                return Err(crate::InputError::InvalidRating {
                    value: "D has no Moody's long-term equivalent".to_string(),
                }
                .into());
            }
        })
    }
}

impl core::fmt::Display for CreditRating {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.to_generic_string())
    }
}

/// Moody's-style long-term spellings accepted by human parsing (compared in
/// ASCII upper case). Bare `Baa`/`Ba`/`Caa` are not listed: they would invent a
/// middle notch.
const MOODYS_ALIASES: [(&str, CreditRating); 21] = [
    ("AAA", CreditRating::AAA),
    ("AA1", CreditRating::AAPlus),
    ("AA2", CreditRating::AA),
    ("AA3", CreditRating::AAMinus),
    ("A1", CreditRating::APlus),
    ("A2", CreditRating::A),
    ("A3", CreditRating::AMinus),
    ("BAA1", CreditRating::BBBPlus),
    ("BAA2", CreditRating::BBB),
    ("BAA3", CreditRating::BBBMinus),
    ("BA1", CreditRating::BBPlus),
    ("BA2", CreditRating::BB),
    ("BA3", CreditRating::BBMinus),
    ("B1", CreditRating::BPlus),
    ("B2", CreditRating::B),
    ("B3", CreditRating::BMinus),
    ("CAA1", CreditRating::CCCPlus),
    ("CAA2", CreditRating::CCC),
    ("CAA3", CreditRating::CCCMinus),
    ("CA", CreditRating::CC),
    ("C", CreditRating::C),
];

/// Explicit project status aliases (compared in ASCII upper case).
const STATUS_ALIASES: [(&str, CreditRating); 6] = [
    ("NR", CreditRating::NR),
    ("NOTRATED", CreditRating::NR),
    ("NOT RATED", CreditRating::NR),
    ("UNRATED", CreditRating::NR),
    ("D", CreditRating::D),
    ("DEFAULT", CreditRating::D),
];

impl core::str::FromStr for CreditRating {
    type Err = crate::Error;

    /// Parses a canonical generic label, a Moody's-style long-term spelling
    /// or an explicit status alias, after trimming outer whitespace, ASCII
    /// case-insensitively. Anything else (`AAA+`, `C3`, `BBB2`, `SD`, bare
    /// `Baa`, internal spaces, modifiers) is `InputError::InvalidRating`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let upper = s.trim().to_ascii_uppercase();
        LADDER
            .iter()
            .copied()
            .chain([CreditRating::NR])
            .find(|rating| rating.to_generic_string() == upper)
            .or_else(|| {
                MOODYS_ALIASES
                    .iter()
                    .chain(STATUS_ALIASES.iter())
                    .find(|(alias, _)| *alias == upper)
                    .map(|(_, rating)| *rating)
            })
            .ok_or_else(|| {
                crate::InputError::InvalidRating {
                    value: s.to_string(),
                }
                .into()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cmp::Ordering;

    #[test]
    fn notches_are_ladder_distances_and_reject_nr() {
        assert_eq!(
            CreditRating::BBB
                .notches_to(CreditRating::BB)
                .expect("rated"),
            3
        );
        assert_eq!(
            CreditRating::BB
                .notches_to(CreditRating::BBB)
                .expect("rated"),
            -3
        );
        assert_eq!(
            CreditRating::AAA
                .notches_to(CreditRating::AAA)
                .expect("rated"),
            0
        );
        assert_eq!(
            CreditRating::C.notches_to(CreditRating::D).expect("rated"),
            1
        );
        assert_eq!(
            CreditRating::AAA
                .notches_to(CreditRating::D)
                .expect("rated"),
            21
        );
        for (left, right) in [
            (CreditRating::C, CreditRating::NR),
            (CreditRating::NR, CreditRating::D),
            (CreditRating::NR, CreditRating::NR),
        ] {
            assert!(left.notches_to(right).is_err());
        }
    }

    #[test]
    fn grades_are_literal() {
        for (index, rating) in LADDER.iter().enumerate() {
            assert_eq!(rating.is_investment_grade(), index <= 9, "{rating}");
            assert_eq!(rating.is_speculative_grade(), index >= 10, "{rating}");
        }
        assert!(!CreditRating::NR.is_investment_grade());
        assert!(!CreditRating::NR.is_speculative_grade());
        assert!(CreditRating::D.is_default() && CreditRating::D.is_speculative_grade());
        assert!(!CreditRating::CCC.is_default());
    }

    #[test]
    fn ordering_is_partial_with_nr_incomparable() {
        assert!(CreditRating::AAA < CreditRating::AAPlus);
        assert!(CreditRating::BBBMinus < CreditRating::BBPlus);
        assert!(CreditRating::C < CreditRating::D);
        assert!(CreditRating::B < CreditRating::D);
        assert_eq!(CreditRating::C.partial_cmp(&CreditRating::NR), None);
        assert_eq!(CreditRating::NR.partial_cmp(&CreditRating::D), None);
        assert_eq!(
            CreditRating::NR.partial_cmp(&CreditRating::NR),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn display_and_wire_use_canonical_labels() {
        for rating in LADDER.iter().copied().chain([CreditRating::NR]) {
            let label = rating.to_string();
            assert_eq!(label.parse::<CreditRating>().expect("canonical"), rating);
            let json = serde_json::to_string(&rating).expect("wire");
            assert_eq!(json, format!("\"{label}\""));
            assert_eq!(
                serde_json::from_str::<CreditRating>(&json).expect("restore"),
                rating
            );
        }
        // Wire restoration takes canonical labels only, not human aliases.
        assert!(serde_json::from_str::<CreditRating>("\"Baa3\"").is_err());
        assert!(serde_json::from_str::<CreditRating>("\"unrated\"").is_err());
    }

    #[test]
    fn human_parsing_uses_finite_alias_tables() {
        for (text, rating) in [
            ("AAA", CreditRating::AAA),
            (" aaa ", CreditRating::AAA),
            ("BB+", CreditRating::BBPlus),
            ("Aa", CreditRating::AA),
            ("Aa1", CreditRating::AAPlus),
            ("B1", CreditRating::BPlus),
            ("B2", CreditRating::B),
            ("B3", CreditRating::BMinus),
            ("Ba2", CreditRating::BB),
            ("Baa3", CreditRating::BBBMinus),
            ("Caa1", CreditRating::CCCPlus),
            ("Ca", CreditRating::CC),
            ("C", CreditRating::C),
            ("NR", CreditRating::NR),
            ("not rated", CreditRating::NR),
            ("NotRated", CreditRating::NR),
            ("unrated", CreditRating::NR),
            ("D", CreditRating::D),
            ("default", CreditRating::D),
        ] {
            assert_eq!(text.parse::<CreditRating>().expect(text), rating, "{text}");
        }
        for text in [
            "",
            "AAA+",
            "C3",
            "D1",
            "NR-",
            "BBB2",
            "CAA+",
            "SD",
            "RD",
            "Baa",
            "Ba",
            "Caa",
            "B B+",
            "BBB- (sf)",
            "A-1",
            "withdrawn",
        ] {
            assert!(text.parse::<CreditRating>().is_err(), "{text}");
        }
    }

    #[test]
    fn moodys_rendering_rejects_d() {
        assert_eq!(CreditRating::AAA.to_moodys_string().expect("rated"), "Aaa");
        assert_eq!(CreditRating::CC.to_moodys_string().expect("rated"), "Ca");
        assert_eq!(CreditRating::NR.to_moodys_string().expect("status"), "NR");
        assert!(CreditRating::D.to_moodys_string().is_err());
    }
}
