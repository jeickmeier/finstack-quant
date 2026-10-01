//! Continuous-time generator (intensity) matrix for a CTMC.
//!
//! Provides the [`GeneratorMatrix`] type and extraction via matrix logarithm
//! of an annual transition matrix.
//!
//! # Matrix Logarithm Algorithm
//!
//! Generator extraction uses the real Schur decomposition: P = Q T Q^T where
//! T is upper-triangular (all eigenvalues real). The logarithm is then:
//! log(P) = Q · log(T) · Q^T, where log(T) is computed by inverse scaling and
//! squaring with a convergent full matrix series. This also handles repeated
//! and clustered eigenvalues. Kreinin-Sidenius post-processing clamps
//! any negative off-diagonal entries to zero and re-normalizes the diagonal.
//!
//! # References
//!
//! - Israel, R., Rosenthal, J., & Wei, J. (2001). "Finding Generators for Markov
//!   Chains via Empirical Transition Matrices." *Mathematical Finance*, 11(2), 245-265. `docs/REFERENCES.md#israel-rosenthal-wei-2001`
//! - Kreinin, A., & Sidenius, J. (2001). "Regularization Algorithms for Transition
//!   Matrices." *Algo Research Quarterly*, 4(1/2), 23-40.
//! - Higham, N. J. (2008). *Functions of Matrices: Theory and Computation*. SIAM.
//!   Chapter 11 (Matrix Logarithm). `docs/REFERENCES.md#higham-accuracy-and-stability`

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
///   26(2-3), 423-444. `docs/REFERENCES.md#lando-skodeberg-2002`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "GeneratorMatrixWire")]
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratorMatrixWire {
    data: DMatrix<f64>,
    scale: RatingScale,
    #[serde(default)]
    regularization_l1: f64,
    #[serde(default)]
    round_trip_error: f64,
}

impl TryFrom<GeneratorMatrixWire> for GeneratorMatrix {
    type Error = MigrationError;

    fn try_from(wire: GeneratorMatrixWire) -> Result<Self, Self::Error> {
        for (name, value) in [
            ("regularization_l1", wire.regularization_l1),
            ("round_trip_error", wire.round_trip_error),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(MigrationError::InvalidDiagnostic {
                    name: name.to_string(),
                    value,
                });
            }
        }
        let mut data = Vec::with_capacity(wire.data.len());
        for row in 0..wire.data.nrows() {
            for col in 0..wire.data.ncols() {
                data.push(wire.data[(row, col)]);
            }
        }
        let mut generator = GeneratorMatrix::new(wire.scale, &data)?;
        generator.regularization_l1 = wire.regularization_l1;
        generator.round_trip_error = wire.round_trip_error;
        Ok(generator)
    }
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
    ///
    /// # Arguments
    ///
    /// * `p` - Row-stochastic transition probabilities over `p.horizon()` years;
    ///   the extracted generator is annualized by that horizon.
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
    /// - [`MigrationError::InvalidTolerance`] if `round_trip_tol` is negative or non-finite.
    ///
    /// # Arguments
    ///
    /// * `p` - Row-stochastic transition probabilities over `p.horizon()` years;
    ///   the extracted generator is annualized by that horizon.
    /// * `round_trip_tol` - Finite, non-negative upper bound on the maximum
    ///   absolute row-sum difference between `exp(Q * p.horizon())` and `p`.
    pub fn from_transition_matrix_with_tol(
        p: &TransitionMatrix,
        round_trip_tol: f64,
    ) -> Result<Self, MigrationError> {
        if !round_trip_tol.is_finite() || round_trip_tol < 0.0 {
            return Err(MigrationError::InvalidTolerance(round_trip_tol));
        }
        // `matrix_log(P)` is the generator over the transition matrix's own
        // horizon. GeneratorMatrix is annualized, so recover Q from
        // P(h) = exp(Q * h) by dividing by h.
        let q_data = matrix_log(&p.data)?.scale(1.0 / p.horizon());

        // Kreinin-Sidenius post-processing: clamp negative off-diagonals.
        // The total clamped mass is stamped on the result for policy
        // visibility (regularization changes the generator's economics).
        let (q_corrected, regularization_l1) = kreinin_sidenius(q_data, &p.scale);
        validate_generator(&q_corrected, &p.scale)?;

        let mut gen = GeneratorMatrix {
            data: q_corrected,
            scale: p.scale.clone(),
            regularization_l1,
            round_trip_error: 0.0,
        };

        // Round-trip validation at the source matrix horizon:
        // ||exp(Q * h) - P(h)||_inf < tol.
        let p_reconstructed = pade_expm(&gen.data.scale(p.horizon()))?;
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

// Matrix logarithm via real Schur decomposition and inverse scaling and squaring.

/// Compute log(M) for a matrix with all real positive eigenvalues.
///
/// Uses the real Schur decomposition M = Q T Q^T, then computes log(T) with
/// triangular square roots and a full matrix series near the identity.
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

    for (idx, &ev) in eigenvalues.iter().enumerate() {
        if ev <= 0.0 {
            return Err(MigrationError::NoValidGenerator {
                index: idx,
                value: ev,
            });
        }
    }

    let (q, t) = schur.unpack();

    let log_t = upper_triangular_log(&t)?;

    // log(M) = Q * log(T) * Q^T
    Ok(q.clone() * log_t * q.transpose())
}

/// Inverse scaling and squaring for an upper-triangular matrix with a positive diagonal.
///
/// Repeated square roots bring `T` close to the identity, where
/// `log(I + X) = X - X²/2 + X³/3 - ...` converges in matrix norm. Keeping
/// whole matrix powers retains the nilpotent terms of repeated eigenvalues;
/// no divided difference of nearly equal eigenvalues is needed.
///
/// Reference: Higham (2008), Chapter 11, inverse scaling and squaring.
fn upper_triangular_log(t: &DMatrix<f64>) -> Result<DMatrix<f64>, MigrationError> {
    let n = t.nrows();
    let identity = DMatrix::identity(n, n);
    let mut reduced = t.clone();
    let mut multiplier = 1.0;
    let mut delta = &reduced - &identity;
    let mut delta_norm = infinity_norm(&delta);
    for _ in 0..64 {
        if delta_norm <= 0.25 {
            break;
        }
        reduced = upper_triangular_sqrt(&reduced);
        multiplier *= 2.0;
        delta = &reduced - &identity;
        delta_norm = infinity_norm(&delta);
    }
    if !delta_norm.is_finite() || delta_norm > 0.25 {
        return Err(MigrationError::MatrixLogConvergence);
    }

    let mut logarithm = DMatrix::zeros(n, n);
    let mut power = delta.clone();
    for order in 1..=64 {
        let coefficient = if order % 2 == 1 { 1.0 } else { -1.0 } / f64::from(order);
        logarithm += power.scale(coefficient);
        power = &power * &delta;
        // Bound the entire uncomputed tail using submultiplicativity:
        // sum_{j=k+1}∞ ||X^j||/j <= ||X^(k+1)|| / ((k+1)(1-||X||)).
        let tail_bound = infinity_norm(&power) / (f64::from(order + 1) * (1.0 - delta_norm));
        if tail_bound <= f64::EPSILON * infinity_norm(&logarithm) {
            logarithm *= multiplier;
            // Avoid accumulated diagonal rounding from repeated square roots.
            for i in 0..n {
                logarithm[(i, i)] = t[(i, i)].ln();
            }
            return Ok(logarithm);
        }
    }
    Err(MigrationError::MatrixLogConvergence)
}

/// Principal triangular square root, solving `R² = T` by superdiagonals.
fn upper_triangular_sqrt(t: &DMatrix<f64>) -> DMatrix<f64> {
    let n = t.nrows();
    let mut root = DMatrix::zeros(n, n);
    for i in 0..n {
        root[(i, i)] = t[(i, i)].sqrt();
    }
    for offset in 1..n {
        for i in 0..n - offset {
            let j = i + offset;
            let mut cross = 0.0;
            for k in i + 1..j {
                cross += root[(i, k)] * root[(k, j)];
            }
            root[(i, j)] = (t[(i, j)] - cross) / (root[(i, i)] + root[(j, j)]);
        }
    }
    root
}

fn infinity_norm(m: &DMatrix<f64>) -> f64 {
    m.row_iter()
        .map(|row| row.iter().map(|value| value.abs()).sum::<f64>())
        .fold(0.0, |norm, row_sum| {
            if row_sum.is_finite() {
                norm.max(row_sum)
            } else {
                f64::INFINITY
            }
        })
}

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
            if !v.is_finite() {
                return Err(MigrationError::EntryOutOfRange {
                    row: i,
                    col: j,
                    value: v,
                    min: if i == j { f64::NEG_INFINITY } else { 0.0 },
                    max: if i == j { 0.0 } else { f64::INFINITY },
                });
            }
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

/// Infinity norm of (A - B), i.e., max row-sum of absolute differences.
pub(crate) fn inf_norm_diff(a: &DMatrix<f64>, b: &DMatrix<f64>) -> f64 {
    let diff = a - b;
    diff.row_iter()
        .map(|row| row.iter().map(|x| x.abs()).sum::<f64>())
        .fold(0.0_f64, f64::max)
}
