//! Positions × factors sensitivity matrix layout.
//!
//! [`SensitivityMatrix`] is the canonical row-major dense layout used by the
//! delta-based and full-repricing factor sensitivity engines.

use crate::factor::FactorId;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Positions x factors sensitivity matrix stored in row-major order.
///
/// Serializes as `{position_ids, factor_ids, data}`, where `data` contains one
/// nested row per position and one finite entry per factor. Deserialization
/// validates the complete shape before flattening rows into engine storage.
#[derive(Debug, Clone, PartialEq)]
pub struct SensitivityMatrix {
    position_ids: Vec<String>,
    factor_ids: Vec<FactorId>,
    data: Vec<f64>,
    n_factors: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SensitivityMatrixWire {
    position_ids: Vec<String>,
    factor_ids: Vec<FactorId>,
    data: Vec<Vec<f64>>,
}

impl Serialize for SensitivityMatrix {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let rows: Vec<&[f64]> = (0..self.n_positions())
            .map(|index| self.position_deltas(index))
            .collect();
        let mut state = serializer.serialize_struct("SensitivityMatrix", 3)?;
        state.serialize_field("position_ids", &self.position_ids)?;
        state.serialize_field("factor_ids", &self.factor_ids)?;
        state.serialize_field("data", &rows)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SensitivityMatrix {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = SensitivityMatrixWire::deserialize(deserializer)?;
        Self::from_rows(wire.position_ids, wire.factor_ids, wire.data)
            .map_err(serde::de::Error::custom)
    }
}

impl SensitivityMatrix {
    /// Create a zero-initialized matrix with the provided axes.
    ///
    /// # Arguments
    ///
    /// * `position_ids` - Ordered position identifiers defining the matrix rows.
    /// * `factor_ids` - Ordered factor identifiers defining the matrix columns.
    #[must_use]
    pub fn zeros(position_ids: Vec<String>, factor_ids: Vec<FactorId>) -> Self {
        let n_positions = position_ids.len();
        let n_factors = factor_ids.len();
        Self {
            position_ids,
            factor_ids,
            data: vec![0.0; n_positions * n_factors],
            n_factors,
        }
    }

    /// Construct a sensitivity matrix from validated nested rows.
    ///
    /// # Arguments
    ///
    /// * `position_ids` - Ordered position identifiers; one is required per row.
    /// * `factor_ids` - Ordered factor identifiers; every row must contain one
    ///   entry per factor in this order.
    /// * `data` - Position-by-factor rows containing finite sensitivities in the
    ///   exposure units selected by the caller. Empty factor axes require empty
    ///   rows, and empty position axes require no rows.
    ///
    /// # Errors
    ///
    /// Returns a validation error for inconsistent row or column counts,
    /// non-finite entries, or dimensions whose product overflows `usize`.
    pub fn from_rows(
        position_ids: Vec<String>,
        factor_ids: Vec<FactorId>,
        data: Vec<Vec<f64>>,
    ) -> finstack_quant_core::Result<Self> {
        let n_positions = position_ids.len();
        let n_factors = factor_ids.len();
        let length = n_positions.checked_mul(n_factors).ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "sensitivity matrix dimensions overflow usize".to_string(),
            )
        })?;
        if data.len() != n_positions {
            return Err(finstack_quant_core::Error::Validation(format!(
                "sensitivity data has {} rows but position_ids declares {n_positions} positions",
                data.len()
            )));
        }
        for (position_index, row) in data.iter().enumerate() {
            if row.len() != n_factors {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "sensitivity data row {position_index} has {} entries but factor_ids declares {n_factors} factors",
                    row.len()
                )));
            }
            if let Some(factor_index) = row.iter().position(|value| !value.is_finite()) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "sensitivity for position '{}' on factor '{}' must be finite",
                    position_ids[position_index], factor_ids[factor_index]
                )));
            }
        }
        let mut flat = Vec::with_capacity(length);
        flat.extend(data.into_iter().flatten());
        Ok(Self {
            position_ids,
            factor_ids,
            data: flat,
            n_factors,
        })
    }

    /// Return the ordered position identifiers.
    #[must_use]
    pub fn position_ids(&self) -> &[String] {
        &self.position_ids
    }

    /// Return the ordered factor identifiers.
    #[must_use]
    pub fn factor_ids(&self) -> &[FactorId] {
        &self.factor_ids
    }

    /// Return the number of positions.
    #[must_use]
    pub fn n_positions(&self) -> usize {
        self.position_ids.len()
    }

    /// Return the number of factors.
    #[must_use]
    pub fn n_factors(&self) -> usize {
        self.n_factors
    }

    /// Read a matrix element.
    #[must_use]
    /// # Panics
    ///
    /// Panics when either index is out of bounds. The checks are hard
    /// asserts (not `debug_assert!`): with row-major storage an
    /// out-of-range `factor_idx` can land inside another position's row,
    /// and a silent wrong read in release builds is a risk bug.
    ///
    /// # Arguments
    ///
    /// * `position_idx` - Position idx used by the algorithm, subject to the enclosing type invariants and documented units.
    /// * `factor_idx` - Zero-based factor column index.
    pub fn delta(&self, position_idx: usize, factor_idx: usize) -> f64 {
        assert!(
            position_idx < self.n_positions(),
            "position_idx {position_idx} out of bounds for {} positions",
            self.n_positions()
        );
        assert!(
            factor_idx < self.n_factors,
            "factor_idx {factor_idx} out of bounds for {} factors",
            self.n_factors
        );
        self.data[position_idx * self.n_factors + factor_idx]
    }

    /// Set a matrix element.
    ///
    /// # Arguments
    ///
    /// * `position_idx` - Zero-based position row index.
    /// * `factor_idx` - Zero-based factor column index.
    /// * `value` - Sensitivity in the matrix's exposure and factor bump units.
    ///
    /// # Panics
    ///
    /// Panics when either index is out of bounds (hard assert; see
    /// [`Self::delta`]).
    pub fn set_delta(&mut self, position_idx: usize, factor_idx: usize, value: f64) {
        assert!(
            position_idx < self.n_positions(),
            "position_idx {position_idx} out of bounds for {} positions",
            self.n_positions()
        );
        assert!(
            factor_idx < self.n_factors,
            "factor_idx {factor_idx} out of bounds for {} factors",
            self.n_factors
        );
        self.data[position_idx * self.n_factors + factor_idx] = value;
    }

    /// Return the contiguous row slice for a position.
    ///
    /// # Arguments
    ///
    /// * `position_idx` - Zero-based index of the position whose factor
    ///   exposure row is returned. Must be strictly less than
    ///   [`Self::n_positions`].
    ///
    /// # Panics
    ///
    /// Panics when `position_idx` is out of bounds (hard assert, matching
    /// [`Self::delta`]: with `n_factors == 0` the slice arithmetic would
    /// otherwise return an empty slice for *any* index in release builds
    /// instead of failing).
    #[must_use]
    pub fn position_deltas(&self, position_idx: usize) -> &[f64] {
        assert!(
            position_idx < self.n_positions(),
            "position_idx {position_idx} out of bounds for {} positions",
            self.n_positions()
        );
        let start = position_idx * self.n_factors;
        &self.data[start..start + self.n_factors]
    }

    /// Return a materialized column for a factor.
    ///
    /// # Arguments
    ///
    /// * `factor_idx` - Zero-based index of the factor column to copy.
    #[must_use]
    pub fn factor_deltas(&self, factor_idx: usize) -> Vec<f64> {
        (0..self.n_positions())
            .map(|position_idx| self.delta(position_idx, factor_idx))
            .collect()
    }

    /// Return the underlying row-major storage.
    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip_preserves_axes_and_nested_rows() {
        let matrix = SensitivityMatrix::from_rows(
            vec!["A".into(), "B".into()],
            vec![FactorId::new("Rates"), FactorId::new("Credit")],
            vec![vec![2.0, -3.0], vec![4.0, 5.0]],
        )
        .expect("valid rows");
        let value = serde_json::to_value(&matrix).expect("serialize");
        assert_eq!(value["data"], serde_json::json!([[2.0, -3.0], [4.0, 5.0]]));
        assert!(value.get("n_factors").is_none());
        let restored: SensitivityMatrix = serde_json::from_value(value).expect("deserialize");
        assert_eq!(restored, matrix);
        assert_eq!(restored.delta(1, 0), 4.0);
    }

    #[test]
    fn deserialization_rejects_inconsistent_shape_and_obsolete_storage_fields() {
        for data in [
            serde_json::json!([]),
            serde_json::json!([[], []]),
            serde_json::json!([[]]),
            serde_json::json!([[2.0, 3.0]]),
            serde_json::json!([2.0]),
        ] {
            let value = serde_json::json!({
                "position_ids": ["A"], "factor_ids": ["Rates"], "data": data,
            });
            assert!(serde_json::from_value::<SensitivityMatrix>(value).is_err());
        }
        let value = serde_json::json!({
            "position_ids": ["A"], "factor_ids": ["Rates"], "data": [[2.0]],
            "n_factors": 1,
        });
        assert!(serde_json::from_value::<SensitivityMatrix>(value).is_err());
    }

    #[test]
    fn from_rows_rejects_nonfinite_entries_before_risk_evaluation() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let result = SensitivityMatrix::from_rows(
                vec!["A".into()],
                vec![FactorId::new("Rates")],
                vec![vec![value]],
            );
            assert!(result.is_err());
        }
    }

    #[test]
    fn zero_factor_rows_survive_serde_roundtrip() {
        let matrix = SensitivityMatrix::zeros(vec!["A".into(), "B".into()], vec![]);
        let value = serde_json::to_value(&matrix).expect("serialize");
        assert_eq!(value["data"], serde_json::json!([[], []]));
        assert_eq!(
            serde_json::from_value::<SensitivityMatrix>(value).expect("deserialize"),
            matrix
        );
    }

    #[test]
    fn test_matrix_construction() {
        let matrix = SensitivityMatrix::zeros(
            vec!["pos-1".into(), "pos-2".into()],
            vec![FactorId::new("Rates"), FactorId::new("Credit")],
        );
        assert_eq!(matrix.n_positions(), 2);
        assert_eq!(matrix.n_factors(), 2);
        assert!((matrix.delta(0, 0)).abs() < 1e-15);
    }

    #[test]
    fn test_matrix_set_and_get() {
        let mut matrix = SensitivityMatrix::zeros(
            vec!["pos-1".into()],
            vec![FactorId::new("Rates"), FactorId::new("Credit")],
        );
        matrix.set_delta(0, 0, 100.0);
        matrix.set_delta(0, 1, -50.0);

        assert!((matrix.delta(0, 0) - 100.0).abs() < 1e-12);
        assert!((matrix.delta(0, 1) - (-50.0)).abs() < 1e-12);
    }

    #[test]
    fn test_position_deltas_slice() {
        let mut matrix = SensitivityMatrix::zeros(
            vec!["pos-1".into()],
            vec![FactorId::new("Rates"), FactorId::new("Credit")],
        );
        matrix.set_delta(0, 0, 100.0);
        matrix.set_delta(0, 1, -50.0);

        let row = matrix.position_deltas(0);
        assert_eq!(row.len(), 2);
        assert!((row[0] - 100.0).abs() < 1e-12);
        assert!((row[1] - (-50.0)).abs() < 1e-12);
    }

    #[test]
    fn test_factor_deltas_column() {
        let mut matrix = SensitivityMatrix::zeros(
            vec!["pos-1".into(), "pos-2".into()],
            vec![FactorId::new("Rates")],
        );
        matrix.set_delta(0, 0, 100.0);
        matrix.set_delta(1, 0, 200.0);

        let column = matrix.factor_deltas(0);
        assert_eq!(column.len(), 2);
        assert!((column[0] - 100.0).abs() < 1e-12);
        assert!((column[1] - 200.0).abs() < 1e-12);
    }
    /// The zero-factor edge case is exactly where a `debug_assert!` would
    /// have silently returned an empty slice for any out-of-range index in
    /// release builds.
    #[test]
    #[should_panic(expected = "out of bounds")]
    fn position_deltas_panics_on_out_of_range_index_with_zero_factors() {
        let matrix = SensitivityMatrix::zeros(vec![], vec![]);
        let _ = matrix.position_deltas(7);
    }
}
