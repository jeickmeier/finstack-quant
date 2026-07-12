//! Ordered state set defining transition matrix dimensions and label mapping.

use serde::{Deserialize, Serialize};

use super::error::MigrationError;
use crate::types::{moodys_warf_factor, CreditRating};
use crate::HashMap;

/// An ordered set of states defining a transition matrix's row/column layout.
///
/// States are identified by string labels for flexibility across rating
/// granularities (coarse, notched, with/without NR, or custom). The scale
/// defines which index is the absorbing default state (if any).
///
/// # Examples
///
/// ```
/// use finstack_quant_core::credit::migration::RatingScale;
///
/// let scale = RatingScale::standard();
/// assert_eq!(scale.n_states(), 10);
/// assert_eq!(scale.index_of("BBB"), Some(3));
/// assert_eq!(scale.default_state(), Some(9)); // D
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RatingScaleWire")]
pub struct RatingScale {
    labels: Vec<String>,
    index_map: HashMap<String, usize>,
    default_state: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RatingScaleWire {
    labels: Vec<String>,
    index_map: HashMap<String, usize>,
    default_state: Option<usize>,
}

impl TryFrom<RatingScaleWire> for RatingScale {
    type Error = MigrationError;

    fn try_from(wire: RatingScaleWire) -> Result<Self, Self::Error> {
        let default_label = match wire.default_state {
            Some(index) => Some(wire.labels.get(index).cloned().ok_or(
                MigrationError::InvalidState {
                    state: index,
                    n_states: wire.labels.len(),
                },
            )?),
            None => None,
        };
        let rebuilt = RatingScale::build(wire.labels, default_label)?;
        if rebuilt.index_map != wire.index_map {
            return Err(MigrationError::Internal(
                "serialized rating-scale index map does not match labels".to_string(),
            ));
        }
        Ok(rebuilt)
    }
}

impl RatingScale {
    /// Standard 10-state S&P/Fitch rating scale: AAA, AA, A, BBB, BB, B, CCC, CC, C, D.
    ///
    /// Default (absorbing) state is D at index 9.
    #[must_use]
    pub fn standard() -> Self {
        let labels = vec!["AAA", "AA", "A", "BBB", "BB", "B", "CCC", "CC", "C", "D"];
        Self::from_static_labels(&labels, Some("D"))
    }

    /// 11-state scale with NR (not rated): AAA, AA, A, BBB, BB, B, CCC, CC, C, NR, D.
    ///
    /// Default (absorbing) state is D at index 10.
    #[must_use]
    pub fn standard_with_nr() -> Self {
        let labels = vec![
            "AAA", "AA", "A", "BBB", "BB", "B", "CCC", "CC", "C", "NR", "D",
        ];
        Self::from_static_labels(&labels, Some("D"))
    }

    /// 22-state notched scale: AAA, AA+, AA, AA-, A+, A, A-, BBB+, BBB, BBB-, BB+, BB,
    /// BB-, B+, B, B-, CCC+, CCC, CCC-, CC, C, D.
    ///
    /// Default (absorbing) state is D at index 21.
    #[must_use]
    pub fn notched() -> Self {
        let labels = vec![
            "AAA", "AA+", "AA", "AA-", "A+", "A", "A-", "BBB+", "BBB", "BBB-", "BB+", "BB", "BB-",
            "B+", "B", "B-", "CCC+", "CCC", "CCC-", "CC", "C", "D",
        ];
        Self::from_static_labels(&labels, Some("D"))
    }

    /// Custom scale with the last label treated as the default (absorbing) state.
    ///
    /// Returns an error if labels are empty, fewer than 2, or contain duplicates.
    ///
    /// # Errors
    ///
    /// - [`MigrationError::InsufficientStates`] if fewer than 2 labels are provided.
    /// - [`MigrationError::DuplicateLabel`] if any label appears more than once.
    pub fn custom(labels: Vec<String>) -> Result<Self, MigrationError> {
        let n = labels.len();
        if n < 2 {
            return Err(MigrationError::InsufficientStates);
        }
        let default_label = labels[n - 1].clone();
        Self::build(labels, Some(default_label))
    }

    /// Custom scale with an explicit default (absorbing) state label.
    ///
    /// # Errors
    ///
    /// - [`MigrationError::InsufficientStates`] if fewer than 2 labels are provided.
    /// - [`MigrationError::DuplicateLabel`] if any label appears more than once.
    /// - [`MigrationError::UnknownState`] if `default_label` is not in `labels`.
    pub fn custom_with_default(
        labels: Vec<String>,
        default_label: impl Into<String>,
    ) -> Result<Self, MigrationError> {
        if labels.len() < 2 {
            return Err(MigrationError::InsufficientStates);
        }
        Self::build(labels, Some(default_label.into()))
    }

    /// Number of states in the scale.
    #[must_use]
    pub fn n_states(&self) -> usize {
        self.labels.len()
    }

    /// Returns the index of a label, or `None` if not found.
    #[must_use]
    pub fn index_of(&self, label: &str) -> Option<usize> {
        self.index_map.get(label).copied()
    }

    /// Returns the index of a label.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationError::UnknownState`] if the label is not in the scale.
    pub fn index_of_required(&self, label: &str) -> Result<usize, MigrationError> {
        self.index_map
            .get(label)
            .copied()
            .ok_or_else(|| MigrationError::UnknownState {
                label: label.to_owned(),
            })
    }

    /// Returns the label for a given state index.
    ///
    /// Returns `None` if the index is out of range.
    #[must_use]
    pub fn label_of(&self, index: usize) -> Option<&str> {
        self.labels.get(index).map(String::as_str)
    }

    /// Returns the index of the absorbing default state, if one is defined.
    #[must_use]
    pub fn default_state(&self) -> Option<usize> {
        self.default_state
    }

    /// Returns all state labels in order.
    #[must_use]
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Returns the Moody's WARF (Weighted Average Rating Factor) for a label
    /// in this scale.
    ///
    /// The label must belong to the scale and must be parseable as a credit
    /// rating (S&P/Fitch or Moody's notation).
    ///
    /// # Errors
    ///
    /// - [`MigrationError::UnknownState`] if `label` is not in the scale.
    /// - [`MigrationError::NoWarfFactor`] if the label cannot be mapped to a WARF.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::credit::migration::RatingScale;
    ///
    /// let scale = RatingScale::standard();
    /// assert_eq!(scale.warf("AAA").unwrap(), 1.0);
    /// assert_eq!(scale.warf("B").unwrap(), 2720.0);
    /// ```
    pub fn warf(&self, label: &str) -> Result<f64, MigrationError> {
        if !self.index_map.contains_key(label) {
            return Err(MigrationError::UnknownState {
                label: label.to_owned(),
            });
        }
        label
            .parse::<CreditRating>()
            .ok()
            .and_then(|r| moodys_warf_factor(r).ok())
            .ok_or_else(|| MigrationError::NoWarfFactor {
                label: label.to_owned(),
            })
    }

    /// Returns the scale label whose Moody's WARF factor is closest to the
    /// given value.
    ///
    /// Only labels that can be parsed as credit ratings participate in the
    /// lookup. When a WARF falls exactly between two ratings, the lower-quality
    /// (higher WARF) rating is returned (conservative rounding).
    ///
    /// # Errors
    ///
    /// - [`MigrationError::NoWarfMapping`] if no label in the scale maps to a
    ///   known WARF factor.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::credit::migration::RatingScale;
    ///
    /// let scale = RatingScale::standard();
    /// assert_eq!(scale.rating_from_warf(360.0).unwrap(), "BBB");
    /// assert_eq!(scale.rating_from_warf(400.0).unwrap(), "BBB");
    /// ```
    pub fn rating_from_warf(&self, warf: f64) -> Result<&str, MigrationError> {
        if !warf.is_finite() {
            return Err(MigrationError::InvalidWarf(warf));
        }
        let mut best: Option<(f64, f64, usize)> = None; // (abs_distance, factor, index)
        for (i, label) in self.labels.iter().enumerate() {
            if let Ok(r) = label.parse::<CreditRating>() {
                if let Ok(factor) = moodys_warf_factor(r) {
                    let dist = (factor - warf).abs();
                    let is_better = match best {
                        None => true,
                        Some((best_dist, best_factor, _)) => {
                            dist < best_dist
                                || ((dist - best_dist).abs() < f64::EPSILON && factor > best_factor)
                        }
                    };
                    if is_better {
                        best = Some((dist, factor, i));
                    }
                }
            }
        }
        match best {
            Some((_, _, idx)) => self
                .labels
                .get(idx)
                .map(String::as_str)
                .ok_or(MigrationError::NoWarfMapping),
            None => Err(MigrationError::NoWarfMapping),
        }
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    /// Builds a `RatingScale` from `&[&str]` labels for hardcoded presets.
    fn from_static_labels(labels: &[&str], default_label: Option<&str>) -> Self {
        let labels: Vec<String> = labels.iter().map(|s| (*s).to_owned()).collect();
        let mut index_map: HashMap<String, usize> =
            HashMap::with_capacity_and_hasher(labels.len(), Default::default());
        for (i, label) in labels.iter().enumerate() {
            index_map.insert(label.clone(), i);
        }
        let default_state = default_label.and_then(|label| index_map.get(label).copied());
        Self {
            labels,
            index_map,
            default_state,
        }
    }

    fn build(labels: Vec<String>, default_label: Option<String>) -> Result<Self, MigrationError> {
        if labels.len() < 2 {
            return Err(MigrationError::InsufficientStates);
        }

        let mut index_map: HashMap<String, usize> =
            HashMap::with_capacity_and_hasher(labels.len(), Default::default());
        for (i, label) in labels.iter().enumerate() {
            if index_map.insert(label.clone(), i).is_some() {
                return Err(MigrationError::DuplicateLabel {
                    label: label.clone(),
                });
            }
        }

        let default_state = match default_label {
            None => None,
            Some(ref lbl) => Some(
                *index_map
                    .get(lbl.as_str())
                    .ok_or_else(|| MigrationError::UnknownState { label: lbl.clone() })?,
            ),
        };

        Ok(Self {
            labels,
            index_map,
            default_state,
        })
    }
}
