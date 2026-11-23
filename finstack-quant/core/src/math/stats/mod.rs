//! Statistical functions for financial time series analysis.
//!
//! Provides numerically stabilized statistical estimators using shifted,
//! power-of-two scaled Welford/Chan online recurrences and two-pass
//! compensated summation. All functions are deterministic for a fixed
//! evaluation order on IEEE-754 binary64 platforms.
//!
//! # Algorithms
//!
//! - **Mean & Two-Pass Moments**: Shifted and power-of-two scaled coordinates
//!   with [`neumaier_sum`](super::summation::neumaier_sum) and Chan's residual
//!   correction
//! - **Online Variance**: Shifted and power-of-two scaled Welford single-pass
//!   recurrence with parallel merge (`Chan, Golub & LeVeque 1979`)
//! - **Quantiles**: Exact binary64 rational R-7 (`np.quantile(..., method='linear')`)
//!   linear interpolation
//!
//! # Use Cases
//!
//! - Portfolio risk metrics (volatility, correlation matrices)
//! - Time series analysis (returns, volatility estimation)
//! - Factor model estimation (principal components)
//! - Monte Carlo variance reduction (control variates)
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::math::stats::{mean, variance, covariance, correlation};
//! # fn main() -> finstack_quant_core::Result<()> {
//! let xs = [1.0, 2.0, 3.0, 4.0];
//! let ys = [2.0, 4.0, 6.0, 8.0];
//! assert!((mean(&xs)?.unwrap() - 2.5).abs() < 1e-12);
//! assert!(variance(&xs)?.unwrap() > 0.0);
//! assert!(covariance(&xs, &ys)?.unwrap() > 0.0);
//! assert!((correlation(&xs, &ys)?.unwrap() - 1.0).abs() < 1e-12);
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! - **Welford's Algorithm**: `docs/REFERENCES.md#welford-1962`
//! - Welford, B. P. (1962). "Note on a Method for Calculating Corrected Sums of
//!   Squares and Products." *Technometrics*, 4(3), 419-420. `docs/REFERENCES.md#welford-1962`
//!
//! - **Chan's Parallel Algorithm**:
//! - Chan, T. F., Golub, G. H., & LeVeque, R. J. (1983). "Algorithms for Computing
//!   the Sample Variance: Analysis and Recommendations." *The American Statistician*,
//!   37(3), 242-247. `docs/REFERENCES.md#golub-van-loan-matrix-computations`
//!
//! - **Neumaier Summation**:
//! - Neumaier, A. (1974). "Rundungsfehleranalyse einiger Verfahren zur Summation
//!   endlicher Summen." *Zeitschrift für Angewandte Mathematik und Mechanik*,
//!   54(1), 39-51.

mod moments;
mod numerics;
mod ols;
pub use ols::{OlsResult, ols};
mod online;
mod quantile;
mod realized;
#[cfg(test)]
mod tests;

pub use moments::{
    correlation, covariance, finite_count, finite_max, finite_min, mean, mean_var,
    population_variance, std_dev, variance,
};
pub(crate) use numerics::{MAX_EXACT_COUNT, frexp, frexp_exp, ldexp};
use numerics::{check_constant_slice, normalize_zero, validate_finite_slice};
pub use online::{OnlineCovariance, OnlineStats};
pub use quantile::{
    empirical_tail_mean, expected_shortfall, median, quantile, quantile_ratio, quantile_sorted,
};
pub use realized::{RealizedVarMethod, log_returns, realized_variance, realized_variance_ohlc};
