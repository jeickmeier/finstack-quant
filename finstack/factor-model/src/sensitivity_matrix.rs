//! Positions × factors sensitivity matrix layout.
//!
//! [`SensitivityMatrix`] is the canonical row-major dense layout used by the
//! delta-based and full-repricing factor sensitivity engines.

use finstack_core::factor_model::FactorId;

/// Positions x factors sensitivity matrix stored in row-major order.
#[derive(Debug, Clone, PartialEq)]
pub struct SensitivityMatrix {
    position_ids: Vec<String>,
    factor_ids: Vec<FactorId>,
    data: Vec<f64>,
    n_factors: usize,
}

impl SensitivityMatrix {
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
    pub fn delta(&self, position_idx: usize, factor_idx: usize) -> f64 {
        debug_assert!(
            position_idx < self.n_positions(),
            "position_idx {position_idx} out of bounds for {} positions",
            self.n_positions()
        );
        debug_assert!(
            factor_idx < self.n_factors,
            "factor_idx {factor_idx} out of bounds for {} factors",
            self.n_factors
        );
        self.data[position_idx * self.n_factors + factor_idx]
    }

    /// Set a matrix element.
    pub fn set_delta(&mut self, position_idx: usize, factor_idx: usize, value: f64) {
        debug_assert!(
            position_idx < self.n_positions(),
            "position_idx {position_idx} out of bounds for {} positions",
            self.n_positions()
        );
        debug_assert!(
            factor_idx < self.n_factors,
            "factor_idx {factor_idx} out of bounds for {} factors",
            self.n_factors
        );
        self.data[position_idx * self.n_factors + factor_idx] = value;
    }

    /// Return the contiguous row slice for a position.
    #[must_use]
    pub fn position_deltas(&self, position_idx: usize) -> &[f64] {
        debug_assert!(
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
}
