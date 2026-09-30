//! Positions × factors sensitivity matrix layout.
//!
//! [`SensitivityMatrix`] is the canonical row-major dense layout used by the
//! delta-based and full-repricing factor sensitivity engines.

use crate::factor::FactorId;

/// Positions x factors sensitivity matrix stored in row-major order.
///
/// The matrix carries no serde form of its own: its only wire form is the
/// reporting-currency-tagged row layout owned by the portfolio crate, which
/// converts back into a matrix through [`SensitivityMatrix::from_rows`].
///
/// # Invariants
///
/// - The storage holds exactly `n_positions() * n_factors()` values.
#[derive(Debug, Clone, PartialEq)]
pub struct SensitivityMatrix {
    position_ids: Vec<String>,
    factor_ids: Vec<FactorId>,
    data: Vec<f64>,
    n_factors: usize,
}

impl SensitivityMatrix {
    /// Build a matrix from one row of factor sensitivities per position.
    ///
    /// Row `i` holds the sensitivities of `position_ids[i]` to each factor in
    /// `factor_ids` order, in the same monetary units as the engine output
    /// (PV change per factor bump). Values are stored unchanged; finiteness is
    /// checked by the risk decomposers that consume the matrix.
    ///
    /// # Arguments
    ///
    /// * `position_ids` - Ordered position identifiers, one per row.
    /// * `factor_ids` - Ordered factor identifiers, one per column.
    /// * `rows` - Row-major sensitivities, `rows[position][factor]`; there must
    ///   be one row per position and each row must hold one value per factor.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when the row count
    /// differs from the number of positions, or when a row's length differs
    /// from the number of factors (the error names the offending row).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_models::factor::{FactorId, SensitivityMatrix};
    ///
    /// let matrix = SensitivityMatrix::from_rows(
    ///     vec!["A".into(), "B".into()],
    ///     vec![FactorId::new("Rates"), FactorId::new("Credit")],
    ///     vec![vec![1.0, 2.0], vec![3.0, -1.0]],
    /// )?;
    /// assert_eq!(matrix.delta(1, 0), 3.0);
    /// assert!(SensitivityMatrix::from_rows(
    ///     vec!["A".into(), "B".into()],
    ///     vec![FactorId::new("Rates")],
    ///     vec![vec![1.0]],
    /// )
    /// .is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn from_rows(
        position_ids: Vec<String>,
        factor_ids: Vec<FactorId>,
        rows: Vec<Vec<f64>>,
    ) -> finstack_quant_core::Result<Self> {
        let n_factors = factor_ids.len();
        if rows.len() != position_ids.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "sensitivity data has {} row(s) but position_ids declares {} position(s)",
                rows.len(),
                position_ids.len()
            )));
        }
        let mut data = Vec::with_capacity(rows.len().saturating_mul(n_factors));
        for (index, row) in rows.into_iter().enumerate() {
            if row.len() != n_factors {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "sensitivity data row {index} has {} element(s) but factor_ids declares \
                     {n_factors} factor(s)",
                    row.len()
                )));
            }
            data.extend(row);
        }
        Ok(Self {
            position_ids,
            factor_ids,
            data,
            n_factors,
        })
    }

    /// Create a zero-initialized matrix with the provided axes.
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
    #[test]
    fn from_rows_stores_rows_in_row_major_order() {
        let matrix = SensitivityMatrix::from_rows(
            vec!["A".into(), "B".into()],
            vec![FactorId::new("F1"), FactorId::new("F2")],
            vec![vec![1.0, 2.0], vec![3.0, -1.0]],
        )
        .expect("well-formed rows");
        assert_eq!(matrix.as_slice(), &[1.0, 2.0, 3.0, -1.0]);
        assert_eq!(matrix.n_factors(), 2);
        assert_eq!(matrix.position_deltas(1), &[3.0, -1.0]);
    }

    #[test]
    fn from_rows_accepts_the_empty_matrix() {
        let matrix = SensitivityMatrix::from_rows(vec![], vec![], vec![]).expect("empty");
        assert_eq!(matrix.n_positions(), 0);
        assert_eq!(matrix.n_factors(), 0);
    }

    #[test]
    fn from_rows_rejects_too_few_rows() {
        let err = SensitivityMatrix::from_rows(
            vec!["A".into(), "B".into()],
            vec![FactorId::new("F1")],
            vec![vec![1.0]],
        )
        .expect_err("one row for two positions");
        assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
        assert!(err.to_string().contains("1 row(s)"), "{err}");
        assert!(err.to_string().contains("2 position(s)"), "{err}");
    }

    #[test]
    fn from_rows_rejects_too_many_rows() {
        let err = SensitivityMatrix::from_rows(
            vec!["A".into()],
            vec![FactorId::new("F1")],
            vec![vec![1.0], vec![2.0]],
        )
        .expect_err("two rows for one position");
        assert!(err.to_string().contains("2 row(s)"), "{err}");
    }

    #[test]
    fn from_rows_rejects_a_ragged_row_and_names_it() {
        for bad_row in [vec![3.0], vec![3.0, 4.0, 5.0]] {
            let err = SensitivityMatrix::from_rows(
                vec!["A".into(), "B".into()],
                vec![FactorId::new("F1"), FactorId::new("F2")],
                vec![vec![1.0, 2.0], bad_row.clone()],
            )
            .expect_err("ragged row");
            let message = err.to_string();
            assert!(message.contains("row 1"), "{message}");
            assert!(
                message.contains(&format!("{} element(s)", bad_row.len())),
                "{message}"
            );
            assert!(message.contains("2 factor(s)"), "{message}");
        }
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
