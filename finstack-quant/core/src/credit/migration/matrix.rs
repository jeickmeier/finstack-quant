//! Row-stochastic transition matrix for discrete-state Markov chains.

use nalgebra::DMatrix;
use serde::{Deserialize, Serialize};

use super::{error::MigrationError, scale::RatingScale};

/// Row-stochastic N×N transition matrix representing migration probabilities
/// over a fixed time horizon.
///
/// Entry (i, j) is the probability of transitioning from state i to state j
/// over the matrix's time horizon. All entries are in \[0, 1\] and each row
/// sums to 1.
///
/// # Construction
///
/// ```
/// use finstack_quant_core::credit::migration::{RatingScale, TransitionMatrix};
///
/// // 2×2 identity (AAA stays AAA, D stays D)
/// let scale = RatingScale::custom(vec!["AAA".to_string(), "D".to_string()])
///     .expect("valid scale");
/// let matrix = TransitionMatrix::new(scale, &[1.0, 0.0, 0.0, 1.0], 1.0)
///     .expect("valid matrix");
/// assert_eq!(matrix.probability("AAA", "D").unwrap(), 0.0);
/// ```
///
/// # References
///
/// - Jarrow, R. A., Lando, D., & Turnbull, S. M. (1997). "A Markov Model for
///   the Term Structure of Credit Risk Spreads." *Review of Financial Studies*,
///   10(2), 481-523.
/// - Gupton, G. M., Finger, C. C., & Bhatia, M. (1997). *CreditMetrics —
///   Technical Document*. J.P. Morgan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionMatrix {
    pub(crate) data: DMatrix<f64>,
    pub(crate) horizon: f64,
    pub(crate) scale: RatingScale,
}

impl TransitionMatrix {
    /// Construct a transition matrix from row-major data.
    ///
    /// # Arguments
    ///
    /// * `scale` — Rating scale defining states.
    /// * `data` — Row-major probabilities; must have length `n²` where `n = scale.n_states()`.
    /// * `horizon` — Time horizon in years (must be positive).
    ///
    /// # Errors
    ///
    /// - [`MigrationError::DimensionMismatch`] if `data.len() != n²`.
    /// - [`MigrationError::EntryOutOfRange`] if any entry is outside \[0, 1\].
    /// - [`MigrationError::RowSumViolation`] if any row does not sum to 1.
    /// - [`MigrationError::NonAbsorbingDefault`] if the default state is not absorbing.
    /// - [`MigrationError::InvalidHorizon`] if `horizon <= 0`.
    pub fn new(scale: RatingScale, data: &[f64], horizon: f64) -> Result<Self, MigrationError> {
        if horizon <= 0.0 {
            return Err(MigrationError::InvalidHorizon(horizon));
        }
        let n = scale.n_states();
        if data.len() != n * n {
            return Err(MigrationError::DimensionMismatch {
                expected: n * n,
                actual: data.len(),
            });
        }
        let matrix = DMatrix::from_row_slice(n, n, data);
        validate_transition_matrix(&matrix, &scale)?;
        Ok(Self {
            data: matrix,
            horizon,
            scale,
        })
    }

    /// Transition probability P(from → to) looked up by label.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationError::UnknownState`] if either label is not in the scale.
    pub fn probability(&self, from: &str, to: &str) -> Result<f64, MigrationError> {
        let i = self.scale.index_of_required(from)?;
        let j = self.scale.index_of_required(to)?;
        Ok(self.data[(i, j)])
    }

    /// Transition probability by row/column index (no bounds check).
    #[must_use]
    pub fn probability_by_index(&self, from: usize, to: usize) -> f64 {
        self.data[(from, to)]
    }

    /// Row of transition probabilities from a given state.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationError::UnknownState`] if `from` is not in the scale.
    pub fn row(&self, from: &str) -> Result<Vec<f64>, MigrationError> {
        let i = self.scale.index_of_required(from)?;
        Ok(self.data.row(i).iter().copied().collect())
    }

    /// The underlying `nalgebra` matrix.
    #[must_use]
    pub fn as_matrix(&self) -> &DMatrix<f64> {
        &self.data
    }

    /// Time horizon in years.
    #[must_use]
    pub fn horizon(&self) -> f64 {
        self.horizon
    }

    /// The rating scale.
    #[must_use]
    pub fn scale(&self) -> &RatingScale {
        &self.scale
    }

    /// Number of states.
    #[must_use]
    pub fn n_states(&self) -> usize {
        self.scale.n_states()
    }

    /// Compose two transition matrices: P(s+t) = P(s) × P(t).
    ///
    /// Both matrices must share the same rating scale.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationError::ScaleMismatch`] if the scales differ.
    pub fn compose(&self, other: &TransitionMatrix) -> Result<TransitionMatrix, MigrationError> {
        if self.scale != other.scale {
            return Err(MigrationError::ScaleMismatch);
        }
        let composed = &self.data * &other.data;
        Ok(TransitionMatrix {
            data: composed,
            horizon: self.horizon + other.horizon,
            scale: self.scale.clone(),
        })
    }

    /// Default probability vector: probability of reaching the default state from each row.
    ///
    /// Returns `None` if no default state is defined on the scale.
    #[must_use]
    pub fn default_probabilities(&self) -> Option<Vec<f64>> {
        let d = self.scale.default_state()?;
        Some(self.data.column(d).iter().copied().collect())
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

pub(crate) fn validate_transition_matrix(
    m: &DMatrix<f64>,
    scale: &RatingScale,
) -> Result<(), MigrationError> {
    let n = scale.n_states();
    if m.nrows() != n || m.ncols() != n {
        return Err(MigrationError::DimensionMismatch {
            expected: n,
            actual: m.nrows(),
        });
    }

    const ROW_SUM_TOL: f64 = 1e-8;

    for i in 0..n {
        let mut row_sum = 0.0;
        for j in 0..n {
            let v = m[(i, j)];
            if !(-1e-12..=1.0 + 1e-12).contains(&v) {
                return Err(MigrationError::EntryOutOfRange {
                    row: i,
                    col: j,
                    value: v,
                    min: 0.0,
                    max: 1.0,
                });
            }
            row_sum += v;
        }
        if (row_sum - 1.0).abs() > ROW_SUM_TOL {
            return Err(MigrationError::RowSumViolation {
                row: i,
                sum: row_sum,
                expected: 1.0,
                tol: ROW_SUM_TOL,
            });
        }
    }

    // If a default (absorbing) state is defined, its row must be exactly absorbing.
    if let Some(d) = scale.default_state() {
        for j in 0..n {
            if j != d && m[(d, j)] > 1e-8 {
                return Err(MigrationError::NonAbsorbingDefault { state: d });
            }
        }
    }

    Ok(())
}
