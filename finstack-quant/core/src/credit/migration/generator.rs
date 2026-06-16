//! Continuous-time generator (intensity) matrix for a CTMC.
//!
//! Provides the [`GeneratorMatrix`] type and extraction via matrix logarithm
//! of an annual transition matrix.
//!
//! # Matrix Logarithm Algorithm
//!
//! Generator extraction uses the real Schur decomposition: P = Q T Q^T where
//! T is upper-triangular (all eigenvalues real). The logarithm is then:
//! log(P) = Q · log(T) · Q^T, where log(T) is computed via Parlett's recurrence
//! on the upper-triangular structure. Kreinin-Sidenius post-processing clamps
//! any negative off-diagonal entries to zero and re-normalizes the diagonal.
//!
//! # References
//!
//! - Israel, R., Rosenthal, J., & Wei, J. (2001). "Finding Generators for Markov
//!   Chains via Empirical Transition Matrices." *Mathematical Finance*, 11(2), 245-265.
//! - Kreinin, A., & Sidenius, J. (2001). "Regularization Algorithms for Transition
//!   Matrices." *Algo Research Quarterly*, 4(1/2), 23-40.
//! - Higham, N. J. (2008). *Functions of Matrices: Theory and Computation*. SIAM.
//!   Chapter 11 (Matrix Logarithm).

use nalgebra::{linalg::Schur, DMatrix};
use serde::{Deserialize, Serialize};

use super::{
    error::MigrationError, matrix::TransitionMatrix, projection::pade_expm, scale::RatingScale,
};

/// Continuous-time generator (intensity) matrix for a CTMC.
///
/// Off-diagonal entry `q_ij` (i ≠ j) is the instantaneous rate of transitioning
/// from state i to state j. Diagonal entry `q_ii = -Σ_{j≠i} q_ij` so rows sum
/// to zero.
///
/// # Validation
///
/// - Off-diagonal entries ≥ 0
/// - Diagonal entries ≤ 0
/// - Each row sums to 0 (tolerance: 1e-8)
/// - If a default state is set, its row must be zero (absorbing)
///
/// # References
///
/// - Lando, D., & Skodeberg, T. M. (2002). "Analyzing Rating Transitions and
///   Rating Drift with Continuous Observations." *Journal of Banking & Finance*,
///   26(2-3), 423-444.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorMatrix {
    pub(crate) data: DMatrix<f64>,
    pub(crate) scale: RatingScale,
    /// Total negative off-diagonal mass clamped to zero by Kreinin-Sidenius
    /// regularization, summed over the whole matrix (L1 norm of the clamped
    /// entries). Zero for directly constructed generators.
    ///
    /// Stamped per the policy-visibility invariant (2026-06-09 core quant
    /// review): K-S regularization changes the economics of the generator,
    /// so the magnitude of the adjustment must be observable.
    #[serde(default)]
    pub(crate) regularization_l1: f64,
    /// Round-trip reconstruction error ‖exp(Q) − P‖∞ measured during
    /// extraction from a transition matrix. Zero for directly constructed
    /// generators.
    #[serde(default)]
    pub(crate) round_trip_error: f64,
}

impl GeneratorMatrix {
    /// Construct a generator matrix directly from row-major data.
    ///
    /// # Arguments
    ///
    /// * `scale` — Rating scale defining states.
    /// * `data` — Row-major entries; must have length `n²`.
    ///
    /// # Errors
    ///
    /// - [`MigrationError::DimensionMismatch`] if `data.len() != n²`.
    /// - [`MigrationError::EntryOutOfRange`] if any off-diagonal entry is negative.
    /// - [`MigrationError::RowSumViolation`] if any row does not sum to 0.
    /// - [`MigrationError::NonAbsorbingDefault`] if the default state is not absorbing.
    pub fn new(scale: RatingScale, data: &[f64]) -> Result<Self, MigrationError> {
        let n = scale.n_states();
        if data.len() != n * n {
            return Err(MigrationError::DimensionMismatch {
                expected: n * n,
                actual: data.len(),
            });
        }
        let matrix = DMatrix::from_row_slice(n, n, data);
        validate_generator(&matrix, &scale)?;
        Ok(Self {
            data: matrix,
            scale,
            regularization_l1: 0.0,
            round_trip_error: 0.0,
        })
    }

    /// Extract a generator from an annual transition matrix via matrix logarithm.
    ///
    /// Applies the real Schur decomposition to compute log(P), followed by
    /// Kreinin-Sidenius post-processing to ensure a valid Q-matrix.
    ///
    /// The default round-trip tolerance is `1e-2`. For a matrix with 4-digit
    /// precision such as a published annual transition table, K-S regularization
    /// introduces errors on the order of 1e-3 to 1e-2 (the matrix itself only
    /// has 4-digit accuracy). Use [`from_transition_matrix_with_tol`](Self::from_transition_matrix_with_tol)
    /// to tighten or loosen this threshold.
    ///
    /// # Errors
    ///
    /// - [`MigrationError::ComplexEigenvalues`] if P has complex eigenvalues.
    /// - [`MigrationError::NoValidGenerator`] if any eigenvalue is ≤ 0.
    /// - [`MigrationError::RoundTripError`] if ‖exp(Q) − P‖∞ exceeds the default
    ///   tolerance of `1e-2`.
    pub fn from_transition_matrix(p: &TransitionMatrix) -> Result<Self, MigrationError> {
        Self::from_transition_matrix_with_tol(p, 1e-2)
    }

    /// Like [`from_transition_matrix`](Self::from_transition_matrix) but with a
    /// configurable round-trip tolerance.
    ///
    /// # Errors
    ///
    /// - [`MigrationError::ComplexEigenvalues`] if P has complex eigenvalues.
    /// - [`MigrationError::NoValidGenerator`] if any eigenvalue is ≤ 0.
    /// - [`MigrationError::RoundTripError`] if ‖exp(Q) − P‖∞ exceeds `round_trip_tol`.
    pub fn from_transition_matrix_with_tol(
        p: &TransitionMatrix,
        round_trip_tol: f64,
    ) -> Result<Self, MigrationError> {
        let q_data = matrix_log(&p.data)?;

        // Kreinin-Sidenius post-processing: clamp negative off-diagonals.
        // The total clamped mass is stamped on the result for policy
        // visibility (regularization changes the generator's economics).
        let (q_corrected, regularization_l1) = kreinin_sidenius(q_data, &p.scale);

        let mut gen = GeneratorMatrix {
            data: q_corrected,
            scale: p.scale.clone(),
            regularization_l1,
            round_trip_error: 0.0,
        };

        // Round-trip validation: ||exp(Q) - P||_inf < tol
        let p_reconstructed = pade_expm(&gen.data)?;
        let inf_err = inf_norm_diff(&p_reconstructed, &p.data);
        if inf_err > round_trip_tol {
            return Err(MigrationError::RoundTripError {
                error: inf_err,
                tolerance: round_trip_tol,
            });
        }
        gen.round_trip_error = inf_err;

        Ok(gen)
    }

    /// Transition intensity q_ij looked up by state labels.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationError::UnknownState`] if either label is not in the scale.
    pub fn intensity(&self, from: &str, to: &str) -> Result<f64, MigrationError> {
        let i = self.scale.index_of_required(from)?;
        let j = self.scale.index_of_required(to)?;
        Ok(self.data[(i, j)])
    }

    /// Total exit rate from a state: `-q_ii`.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationError::UnknownState`] if `state` is not in the scale.
    pub fn exit_rate(&self, state: &str) -> Result<f64, MigrationError> {
        let i = self.scale.index_of_required(state)?;
        Ok(-self.data[(i, i)])
    }

    /// The underlying `nalgebra` matrix.
    #[must_use]
    pub fn as_matrix(&self) -> &DMatrix<f64> {
        &self.data
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

    /// Total negative off-diagonal mass clamped to zero by Kreinin-Sidenius
    /// regularization during extraction (L1 norm of the clamped entries,
    /// summed over the whole matrix).
    ///
    /// Returns `0.0` for generators constructed directly via
    /// [`GeneratorMatrix::new`]. A non-zero value means the extracted
    /// generator does not exactly reproduce the input transition matrix;
    /// see also [`round_trip_error`](Self::round_trip_error).
    #[must_use]
    pub fn regularization_l1(&self) -> f64 {
        self.regularization_l1
    }

    /// Round-trip reconstruction error `‖exp(Q) − P‖∞` measured against the
    /// source transition matrix during extraction.
    ///
    /// Returns `0.0` for generators constructed directly via
    /// [`GeneratorMatrix::new`].
    #[must_use]
    pub fn round_trip_error(&self) -> f64 {
        self.round_trip_error
    }
}

// ---------------------------------------------------------------------------
// Matrix logarithm via real Schur decomposition + Parlett's recurrence
// ---------------------------------------------------------------------------

/// Compute log(M) for a matrix with all real positive eigenvalues.
///
/// Uses the real Schur decomposition M = Q T Q^T, then applies Parlett's
/// recurrence to compute log(T) for the upper-triangular T.
///
/// Returns `Err` if:
/// - The Schur form has complex eigenvalues (2×2 blocks remain after decomposition).
/// - Any eigenvalue is ≤ 0 (logarithm undefined).
pub(crate) fn matrix_log(m: &DMatrix<f64>) -> Result<DMatrix<f64>, MigrationError> {
    let schur = Schur::new(m.clone());

    // `eigenvalues()` returns Some only when all eigenvalues are real.
    let eigenvalues = schur
        .eigenvalues()
        .ok_or(MigrationError::ComplexEigenvalues)?;

    // Check all eigenvalues are strictly positive.
    for (idx, &ev) in eigenvalues.iter().enumerate() {
        if ev <= 0.0 {
            return Err(MigrationError::NoValidGenerator {
                index: idx,
                value: ev,
            });
        }
    }

    let (q, t) = schur.unpack();

    // Compute log(T) using Parlett's recurrence for upper-triangular matrices.
    let log_t = upper_triangular_log(&t, &eigenvalues)?;

    // log(M) = Q * log(T) * Q^T
    Ok(q.clone() * log_t * q.transpose())
}

/// Parlett's recurrence for the logarithm of an upper-triangular matrix.
///
/// For f(T) = log(T), commutativity L·T = T·L gives, for the $(i,j)$ entry
/// (i < j):
///
/// $$L_{ij} = T_{ij} \cdot \frac{L_{jj} - L_{ii}}{T_{jj} - T_{ii}}
///           + \frac{\sum_{k=i+1}^{j-1}(T_{ik} L_{kj} - L_{ik} T_{kj})}{T_{jj} - T_{ii}}$$
///
/// (Cross-term sign verified against `scipy.linalg.logm`; it was previously
/// implemented negated, which produced spurious round-trip error and masked
/// genuine negative off-diagonals from Kreinin-Sidenius clamping —
/// )
///
/// Reference: Higham, N. J. (2008). *Functions of Matrices: Theory and Computation*.
/// SIAM. Equation (4.19).
fn upper_triangular_log(
    t: &DMatrix<f64>,
    eigenvalues: &nalgebra::DVector<f64>,
) -> Result<DMatrix<f64>, MigrationError> {
    let n = t.nrows();
    let mut l = DMatrix::zeros(n, n);

    // Diagonal: L_ii = log(T_ii)
    for i in 0..n {
        l[(i, i)] = eigenvalues[i].ln();
    }

    // Superdiagonals: process by increasing offset k = j - i.
    for k in 1..n {
        for i in 0..(n - k) {
            let j = i + k;
            let denom = eigenvalues[j] - eigenvalues[i];

            // Accumulate off-diagonal cross terms: Σ (T_ik L_kj − L_ik T_kj).
            let mut cross = 0.0;
            for m in (i + 1)..j {
                cross += t[(i, m)] * l[(m, j)] - l[(i, m)] * t[(m, j)];
            }

            if denom.abs() < 1e-12 {
                // Degenerate (repeated eigenvalue): limit gives L_ij = T_ij / T_ii + cross.
                //
                // Limitations of this branch (doc-only note, 2026-06-09 core
                //
                // - It is exact only for the leading term of the log series.
                //   For repeated eigenvalues with nontrivial nilpotent
                //   structure (Jordan blocks of size > 2), higher-order terms
                //   T_ij²/(2 T_ii²)… are dropped, so log(T) is approximate.
                // - For *near*-repeated eigenvalues just above the 1e-12
                //   threshold, the standard Parlett formula divides by a tiny
                //   `denom` and suffers catastrophic cancellation; accuracy
                //   degrades smoothly as eigenvalues coalesce. Higham (2008,
                //   §11.6) recommends blocked Schur-Parlett with
                //   inverse-scaling-and-squaring per block for such cases.
                // Empirical annual transition matrices have well-separated
                // eigenvalues, so this branch is rarely exercised; the
                // round-trip ‖exp(Q) − P‖∞ check guards the final result.
                l[(i, j)] = t[(i, j)] / eigenvalues[i] + cross / eigenvalues[i];
            } else {
                // Standard Parlett formula.
                l[(i, j)] = t[(i, j)] * (l[(j, j)] - l[(i, i)]) / denom + cross / denom;
            }
        }
    }

    Ok(l)
}

// ---------------------------------------------------------------------------
// Kreinin-Sidenius post-processing
// ---------------------------------------------------------------------------

/// Apply Kreinin-Sidenius post-processing to produce a valid Q-matrix:
/// 1. Set any negative off-diagonal entry to zero.
/// 2. Recompute diagonal as -Σ_{j≠i} q_ij.
/// 3. If a default state exists and is absorbing, zero its entire row.
///
/// Returns the corrected matrix together with the total clamped mass
/// (L1 norm of the negative off-diagonal entries set to zero), which is
/// stamped on the resulting [`GeneratorMatrix`] for policy visibility.
fn kreinin_sidenius(mut q: DMatrix<f64>, scale: &RatingScale) -> (DMatrix<f64>, f64) {
    let n = q.nrows();

    // If default state row should be all-zero, enforce it first.
    if let Some(d) = scale.default_state() {
        for j in 0..n {
            q[(d, j)] = 0.0;
        }
    }

    let mut clamped_l1 = 0.0;
    for i in 0..n {
        let mut row_sum = 0.0;
        for j in 0..n {
            if j != i {
                if q[(i, j)] < 0.0 {
                    clamped_l1 += -q[(i, j)];
                    q[(i, j)] = 0.0;
                }
                row_sum += q[(i, j)];
            }
        }
        q[(i, i)] = -row_sum;
    }

    (q, clamped_l1)
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

pub(crate) fn validate_generator(
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
            if j != i && v < -1e-12 {
                return Err(MigrationError::EntryOutOfRange {
                    row: i,
                    col: j,
                    value: v,
                    min: 0.0,
                    max: f64::INFINITY,
                });
            }
            if j == i && v > 1e-12 {
                return Err(MigrationError::EntryOutOfRange {
                    row: i,
                    col: j,
                    value: v,
                    min: f64::NEG_INFINITY,
                    max: 0.0,
                });
            }
            row_sum += v;
        }
        if row_sum.abs() > ROW_SUM_TOL {
            return Err(MigrationError::RowSumViolation {
                row: i,
                sum: row_sum,
                expected: 0.0,
                tol: ROW_SUM_TOL,
            });
        }
    }

    // Default state row must be all zero.
    if let Some(d) = scale.default_state() {
        for j in 0..n {
            if j != d && m[(d, j)] > 1e-8 {
                return Err(MigrationError::NonAbsorbingDefault { state: d });
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Infinity norm of (A - B), i.e., max row-sum of absolute differences.
pub(crate) fn inf_norm_diff(a: &DMatrix<f64>, b: &DMatrix<f64>) -> f64 {
    let diff = a - b;
    diff.row_iter()
        .map(|row| row.iter().map(|x| x.abs()).sum::<f64>())
        .fold(0.0_f64, f64::max)
}
