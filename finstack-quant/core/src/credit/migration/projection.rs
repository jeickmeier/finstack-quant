//! Matrix exponentiation: P(t) = exp(Q · t).
//!
//! Implements the scaling-and-squaring method with \[13/13\] Padé approximation
//! as described in Higham (2005). For small matrices (N ≤ 20, i.e. all credit
//! rating scales), this method is both fast and highly accurate.
//!
//! # Algorithm
//!
//! Given a generator matrix Q and time horizon t:
//!
//! 1. Form A = Q · t.
//! 2. Choose scaling factor s = max(0, ⌈log₂(‖A‖₁ / θ₁₃)⌉) where θ₁₃ = 5.371920…
//! 3. Compute the \[13/13\] Padé approximant r₁₃(A / 2^s) ≈ exp(A / 2^s).
//! 4. Square s times: P = r₁₃(A / 2^s)^{2^s}.
//! 5. Post-process: clamp negative entries to 0, re-normalize rows to 1.
//!
//! # References
//!
//! - Higham, N. J. (2005). "The Scaling and Squaring Method for the Matrix
//!   Exponential Revisited." *SIAM Journal on Matrix Analysis and Applications*,
//!   26(4), 1179-1193.
//! - Moler, C., & Van Loan, C. (2003). "Nineteen Dubious Ways to Compute the
//!   Exponential of a Matrix, Twenty-Five Years Later." *SIAM Review*, 45(1), 3-49.

use nalgebra::DMatrix;

use super::{
    error::MigrationError,
    generator::GeneratorMatrix,
    matrix::{validate_transition_matrix, TransitionMatrix},
};

/// Compute P(t) = exp(Q · t) using the default algorithm.
///
/// For matrices up to 20×20 (all standard rating scales), uses the \[13/13\]
/// Padé scaling-and-squaring method. The result is a validated [`TransitionMatrix`].
///
/// # Errors
///
/// Returns [`MigrationError::InvalidHorizon`] if `t <= 0`.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::credit::migration::{RatingScale, GeneratorMatrix, projection};
///
/// // 2-state generator: AAA→D at rate 0.1
/// let scale = RatingScale::custom(vec!["AAA".to_string(), "D".to_string()])
///     .expect("valid scale");
/// let gen = GeneratorMatrix::new(scale, &[-0.1, 0.1, 0.0, 0.0])
///     .expect("valid generator");
///
/// // 1-year projection
/// let p1 = projection::project(&gen, 1.0).expect("valid projection");
/// assert!(p1.probability("AAA", "D").unwrap() > 0.0);
/// ```
pub fn project(generator: &GeneratorMatrix, t: f64) -> Result<TransitionMatrix, MigrationError> {
    if !t.is_finite() || t <= 0.0 {
        return Err(MigrationError::InvalidHorizon(t));
    }
    let a = generator.data.scale(t);
    let result = pade_expm(&a)?;
    let result = post_process(result);
    let tm = TransitionMatrix {
        data: result,
        horizon: t,
        scale: generator.scale.clone(),
    };
    validate_transition_matrix(&tm.data, &tm.scale)?;
    Ok(tm)
}

/// Compute P(t) = exp(Q · t) using the \[13/13\] Padé scaling-and-squaring method.
///
/// Explicit algorithm selection; equivalent to [`project`].
///
/// # Errors
///
/// Returns [`MigrationError::InvalidHorizon`] if `t <= 0`.
pub fn project_pade(
    generator: &GeneratorMatrix,
    t: f64,
) -> Result<TransitionMatrix, MigrationError> {
    project(generator, t)
}

// ---------------------------------------------------------------------------
// Padé [13/13] scaling-and-squaring
// ---------------------------------------------------------------------------

/// Pade [13/13] threshold from Higham (2005), Table 1.
const THETA_13: f64 = 5.371_920_351_148_152;

/// Padé [13/13] numerator/denominator coefficients.
///
/// `b[k]` is the coefficient of `A^k` in the Padé numerator p(A).
/// The denominator uses `(-1)^k b[k]`.
///
/// Source: Higham (2005), Table 1.
#[rustfmt::skip]
const PADE_COEFF: [f64; 14] = [
    64_764_752_532_480_000.0,
    32_382_376_266_240_000.0,
     7_771_770_303_897_600.0,
     1_187_353_796_428_800.0,
       129_060_195_264_000.0,
        10_559_470_521_600.0,
           670_442_572_800.0,
            33_522_128_640.0,
             1_323_241_920.0,
                40_840_800.0,
                   960_960.0,
                    16_380.0,
                       182.0,
                         1.0,
];

/// Compute exp(A) via Padé \[13/13\] scaling and squaring.
///
/// Exposed as `pub(crate)` so `generator.rs` can call it for round-trip
/// validation without going through the full `project()` pipeline.
pub(crate) fn pade_expm(a: &DMatrix<f64>) -> Result<DMatrix<f64>, MigrationError> {
    let n = a.nrows();
    let norm1 = one_norm(a);

    // Scaling: choose s such that ||A/2^s||_1 <= theta_13.
    let s = if norm1 <= THETA_13 {
        0i32
    } else {
        let raw = (norm1 / THETA_13).log2().ceil();
        raw.max(0.0) as i32
    };

    let scale = (2.0_f64).powi(s);
    let a_scaled = if s > 0 { a / scale } else { a.clone() };

    // Compute matrix powers.
    let identity = DMatrix::identity(n, n);
    let a2 = &a_scaled * &a_scaled;
    let a4 = &a2 * &a2;
    let a6 = &a2 * &a4;

    let b = &PADE_COEFF;

    // U = A_scaled * (A6*(b13*A6 + b11*A4 + b9*A2) + b7*A6 + b5*A4 + b3*A2 + b1*I)
    let u_inner = &a6 * (b[13] * &a6 + b[11] * &a4 + b[9] * &a2)
        + b[7] * &a6
        + b[5] * &a4
        + b[3] * &a2
        + b[1] * &identity;
    let u = &a_scaled * u_inner;

    // V = A6*(b12*A6 + b10*A4 + b8*A2) + b6*A6 + b4*A4 + b2*A2 + b0*I
    let v = &a6 * (b[12] * &a6 + b[10] * &a4 + b[8] * &a2)
        + b[6] * &a6
        + b[4] * &a4
        + b[2] * &a2
        + b[0] * &identity;

    // Padé approximant: r = (V - U)^{-1} (V + U)
    let p = &v + &u;
    let q = &v - &u;

    let r = q
        .clone()
        .lu()
        .solve(&p)
        .or_else(|| q.full_piv_lu().solve(&p))
        .ok_or(MigrationError::SingularMatrix)?;

    // Squaring: r = r^{2^s}
    let mut result = r;
    for _ in 0..s {
        result = &result * &result;
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Post-processing
// ---------------------------------------------------------------------------

/// Clamp negative entries to 0 and re-normalize rows to sum to 1.
///
/// Necessary because floating-point arithmetic in the Padé approximant can
/// produce small negative values or rows that do not sum to exactly 1.
fn post_process(mut m: DMatrix<f64>) -> DMatrix<f64> {
    let n = m.nrows();
    for i in 0..n {
        let mut row_sum = 0.0;
        for j in 0..n {
            if m[(i, j)] < 0.0 {
                m[(i, j)] = 0.0;
            }
            row_sum += m[(i, j)];
        }
        // Re-normalize. Guard against zero row (degenerate).
        if row_sum > 1e-15 {
            for j in 0..n {
                m[(i, j)] /= row_sum;
            }
        }
    }
    m
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Column-sum norm (‖A‖₁ = max_j Σ_i |a_ij|).
fn one_norm(m: &DMatrix<f64>) -> f64 {
    m.column_iter()
        .map(|col| col.iter().map(|x| x.abs()).sum::<f64>())
        .fold(0.0_f64, f64::max)
}
