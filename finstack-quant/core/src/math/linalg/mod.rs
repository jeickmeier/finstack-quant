//! Linear algebra utilities for correlation and covariance matrices.
//!
//! Provides essential matrix operations for financial modeling, particularly
//! Cholesky decomposition for generating correlated random variables in Monte
//! Carlo simulations and portfolio risk calculations.
//!
//! # Algorithms
//!
//! - **Cholesky decomposition** (unpivoted): Factorize Σ = L L^T for positive definite
//!   matrices. Used for solver normal equations via [`cholesky_decomposition`] and
//!   [`cholesky_solve`].
//! - **Pivoted Cholesky for correlation matrices**: Numerically robust factorization via
//!   [`cholesky_correlation`] using relative-tolerance diagonal pivoting. Handles
//!   near-singular and positive-semidefinite correlation matrices safely. The permutation
//!   is internalized; the returned [`CorrelationFactor`] applies shocks in original
//!   variable order.
//! - **Correlation application**: Transform independent normals to correlated via L
//! - **Matrix validation**: Check positive-definiteness and correlation properties via
//!   [`validate_correlation_matrix`], with located [`CorrelationError`] diagnostics
//! - **Nearest correlation repair**: Higham (2002) alternating projections via
//!   [`nearest_correlation()`] for matrices with small PSD / unit-diagonal defects
//! - **Ledoit-Wolf shrinkage**: Well-conditioned covariance estimation via [`ledoit_wolf_shrinkage`]
//!   (Ledoit & Wolf 2004, identity-scaled target, analytic optimal intensity)
//!
//! # Use Cases
//!
//! - **Monte Carlo**: Generate correlated asset paths — use [`cholesky_correlation`]
//! - **Portfolio risk**: Covariance matrix factorization for VaR
//! - **Factor models**: Decompose returns into systematic factors
//! - **Copula models**: Correlation structure in credit derivatives
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::math::linalg::cholesky_correlation;
//!
//! // 2x2 correlation matrix: [[1.0, 0.5], [0.5, 1.0]]
//! let corr = vec![1.0, 0.5, 0.5, 1.0];
//! let factor = cholesky_correlation(&corr, 2).expect("valid correlation matrix");
//!
//! // Transform independent standard normals to correlated
//! let z = vec![1.0, 0.0]; // Independent N(0,1) shocks
//! let mut z_corr = vec![0.0; 2];
//! factor.apply(&z, &mut z_corr).unwrap();
//! // z_corr now contains correlated shocks with correlation 0.5
//! ```
//!
//! # References
//!
//! - **Pivoted Cholesky**:
//! - Higham, N. J. (2002). *Accuracy and Stability of Numerical Algorithms* (2nd ed.).
//!   SIAM. Algorithm 10.2 (Cholesky with complete pivoting). `docs/REFERENCES.md#higham-accuracy-and-stability`
//! - Golub, G. H., & Van Loan, C. F. (2013). *Matrix Computations* (4th ed.).
//!   Johns Hopkins University Press. Algorithm 4.2.5. `docs/REFERENCES.md#golub-van-loan-matrix-computations`
//!
//! - **Correlation Matrices**:
//! - Rebonato, R., & Jäckel, P. (2000). "The Most General Methodology to Create
//!   a Valid Correlation Matrix for Risk Management and Option Pricing Purposes."
//!   *Journal of Risk*, 2(2), 17-27. `docs/REFERENCES.md#rebonato-2004-volatility-correlation`
//!
//! - **Monte Carlo Applications**:
//! - Glasserman, P. (2003). *Monte Carlo Methods in Financial Engineering*.
//!   Springer. Section 2.4 (Generating multivariate samples). `docs/REFERENCES.md#glasserman-2004-monte-carlo`

mod cholesky;
mod errors;
mod factor;
mod nearest_correlation;
mod shrinkage;
#[cfg(test)]
mod tests;

pub use cholesky::{
    apply_lower_triangular, apply_lower_triangular_many, cholesky_decomposition, cholesky_solve,
    flatten_square_matrix, rows_of, symmetric_eigen, unflatten_square, validate_correlation_matrix,
};
use errors::square_len;
pub use errors::{CholeskyError, CorrelationError, SYMMETRY_TOLERANCE};
pub use factor::{CorrelationFactor, cholesky_correlation};
pub use nearest_correlation::{NearestCorrelationOpts, nearest_correlation};
pub use shrinkage::{LedoitWolfResult, ledoit_wolf_shrinkage};
