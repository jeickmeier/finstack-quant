//! Rating-factor tables and Moody's WARF lookup.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use finstack_quant_core::types::CreditRating;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

use super::registry::{embedded_registry, RatingFactorTableParts};

/// Rating factor table for a specific rating-agency methodology.
///
/// Rating factors are ordinal credit-quality inputs used in structured-credit
/// tests. They are not probabilities and must not be averaged or annualized as
/// if they were default probabilities.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::types::CreditRating;
/// use finstack_quant_models::credit::RatingFactorTable;
///
/// let table = RatingFactorTable::moodys_standard().expect("embedded table");
/// assert_eq!(table.get_factor(CreditRating::B).expect("B factor"), 2720.0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct RatingFactorTable {
    factors: BTreeMap<CreditRating, f64>,
    agency: String,
    methodology: String,
    default_factor: f64,
}

impl RatingFactorTable {
    /// Load the embedded Moody's standard WARF table.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded credit-assumptions registry is invalid
    /// or its configured default rating-factor table is missing.
    pub fn moodys_standard() -> Result<Self> {
        Self::from_registry_id(embedded_registry()?.default_rating_factor_table_id())
    }

    /// Load a named rating-factor table from the embedded registry.
    ///
    /// # Arguments
    ///
    /// * `id` - Exact registry identifier for the required methodology. An
    ///   unknown ID is rejected and never falls back to the default table.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded registry is invalid or `id` is absent.
    pub fn from_registry_id(id: &str) -> Result<Self> {
        Ok(Self::from_registry_parts(
            embedded_registry()?.rating_factor_table(id)?,
        ))
    }

    fn from_registry_parts(parts: RatingFactorTableParts) -> Self {
        Self {
            factors: parts.factors.into_iter().collect(),
            agency: parts.agency,
            methodology: parts.methodology,
            default_factor: parts.default_factor,
        }
    }

    /// Return the factor for one canonical rating.
    ///
    /// # Arguments
    ///
    /// * `rating` - Canonical credit rating whose methodology factor is
    ///   required. No fallback is applied when the rating is absent.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::error::InputError::NotFound`] when the
    /// table contains no factor for `rating`.
    pub fn get_factor(&self, rating: CreditRating) -> Result<f64> {
        self.factors.get(&rating).copied().ok_or_else(|| {
            Error::Input(finstack_quant_core::error::InputError::NotFound {
                id: format!("rating factor for {rating}"),
            })
        })
    }

    /// Return the rating agency that owns this methodology.
    #[must_use]
    pub fn agency(&self) -> &str {
        &self.agency
    }

    /// Return the methodology description stored in the registry.
    #[must_use]
    pub fn methodology(&self) -> &str {
        &self.methodology
    }

    /// Return the methodology's explicit fallback factor.
    #[must_use]
    pub fn default_factor(&self) -> f64 {
        self.default_factor
    }
}

static MOODYS_WARF_TABLE: OnceLock<Result<RatingFactorTable>> = OnceLock::new();

/// Return the Moody's WARF factor for one canonical rating.
///
/// The embedded table is parsed and validated once. A missing rating is an
/// error; this function never substitutes the table's default factor.
///
/// # Arguments
///
/// * `rating` - Canonical credit rating whose Moody's WARF factor is required.
///
/// # Errors
///
/// Returns an error if the embedded registry is invalid or the default table
/// has no factor for `rating`.
pub fn moodys_warf_factor(rating: CreditRating) -> Result<f64> {
    match MOODYS_WARF_TABLE.get_or_init(RatingFactorTable::moodys_standard) {
        Ok(table) => table.get_factor(rating),
        Err(err) => Err(err.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_moodys_factors_are_stable() {
        let table = RatingFactorTable::moodys_standard().expect("registry table");
        assert_eq!(table.get_factor(CreditRating::AAA).expect("AAA"), 1.0);
        assert_eq!(table.get_factor(CreditRating::AAPlus).expect("AA+"), 10.0);
        assert_eq!(table.get_factor(CreditRating::A).expect("A"), 120.0);
        assert_eq!(
            table.get_factor(CreditRating::BBMinus).expect("BB-"),
            1766.0
        );
        assert_eq!(table.get_factor(CreditRating::B).expect("B"), 2720.0);
        assert_eq!(table.get_factor(CreditRating::CCC).expect("CCC"), 6500.0);
        assert_eq!(table.get_factor(CreditRating::D).expect("D"), 10000.0);
    }

    #[test]
    fn metadata_and_serde_round_trip_are_stable() {
        let table = RatingFactorTable::moodys_standard().expect("registry table");
        assert_eq!(table.agency(), "Moody's");
        assert_eq!(table.methodology(), "IDEALIZED DEFAULT RATES");
        assert_eq!(table.default_factor(), 3650.0);

        let json = serde_json::to_string(&table).expect("serialize");
        let restored: RatingFactorTable = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.get_factor(CreditRating::B).expect("B"), 2720.0);
    }

    #[test]
    fn convenience_lookup_matches_table() {
        let table = RatingFactorTable::moodys_standard().expect("registry table");
        for rating in [
            CreditRating::AAA,
            CreditRating::AAMinus,
            CreditRating::BBB,
            CreditRating::BPlus,
        ] {
            assert_eq!(
                moodys_warf_factor(rating).expect("convenience factor"),
                table.get_factor(rating).expect("table factor")
            );
        }
    }
}
