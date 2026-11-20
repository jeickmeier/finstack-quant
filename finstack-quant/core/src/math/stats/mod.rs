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
pub use online::{OnlineCovariance, OnlineStats};
pub use quantile::{
    empirical_tail_mean, expected_shortfall, median, quantile, quantile_checked, quantile_ratio,
    quantile_sorted,
};
pub use realized::{RealizedVarMethod, log_returns, realized_variance, realized_variance_ohlc};

/// Maximum observation count whose integer conversion to IEEE-754 `f64`
/// remains exact (`2^53` on 64-bit targets, or `usize::MAX` on 32-bit targets).
pub(crate) const MAX_EXACT_COUNT: usize = match 1usize.checked_shl(53) {
    Some(limit) => limit,
    None => usize::MAX,
};

#[inline]
fn normalize_zero(x: f64) -> f64 {
    if x == 0.0 { 0.0 } else { x }
}

/// Binary exponent `exp` for finite nonzero `x` such that
/// `0.5 <= |ldexp(x, -exp)| < 1.0` (matches C `frexp`).
#[inline]
pub(crate) fn frexp_exp(x: f64) -> i32 {
    debug_assert!(x.is_finite() && x != 0.0);
    let abs = x.abs();
    let biased = ((abs.to_bits() >> 52) & 0x7ff) as i32;
    if biased > 0 {
        biased - 1022
    } else {
        // Normalize subnormals by multiplying by 2^54 (exact power of two).
        let scaled = abs * f64::from_bits(0x4350_0000_0000_0000);
        let s_biased = ((scaled.to_bits() >> 52) & 0x7ff) as i32;
        s_biased - 1022 - 54
    }
}

/// Exact power-of-two scaling `x * 2^shift` with a single IEEE-754 rounding.
#[inline]
pub(crate) fn ldexp(x: f64, shift: i32) -> f64 {
    if x == 0.0 || !x.is_finite() || shift == 0 {
        return x;
    }
    let e = frexp_exp(x);
    let Some(target_exp) = e.checked_add(shift) else {
        return if shift > 0 {
            x.signum() * f64::INFINITY
        } else {
            normalize_zero(0.0)
        };
    };
    if target_exp > 1024 {
        return x.signum() * f64::INFINITY;
    }
    if target_exp < -1074 {
        return 0.0;
    }
    // Bring x without rounding into [0.5, 1.0) (with x's sign):
    // For normal |x|, |e| <= 1024; for subnormal |x|, x * 2^54 is normal in [2^-1020, 2^-968).
    let (mant, rem_shift) = if ((x.abs().to_bits() >> 52) & 0x7ff) > 0 {
        let mut y = x;
        let mut s = -e;
        if s < -1022 {
            // e in 1023..=1024 -> multiply by 2^-512 twice (both exact normal shifts).
            y *= f64::from_bits(((1023 - 512) as u64) << 52);
            s += 512;
        }
        y *= f64::from_bits(((1023 + s) as u64) << 52);
        (y, target_exp)
    } else {
        // Subnormal: x * 2^54 is exact and normal; then shift cleanly to [0.5, 1.0).
        let mut y = x * f64::from_bits(0x4350_0000_0000_0000);
        let y_exp = e + 54;
        y *= f64::from_bits(((1023 - y_exp) as u64) << 52);
        (y, target_exp)
    };
    if rem_shift >= -1021 {
        if rem_shift <= 1023 {
            mant * f64::from_bits(((1023 + rem_shift) as u64) << 52)
        } else {
            // rem_shift == 1024 -> mant * 2^1023 * 2.0 (overflows cleanly if >= f64::MAX).
            mant * f64::from_bits(((1023 + 1023) as u64) << 52) * 2.0
        }
    } else {
        // Final result is subnormal or zero: 0.5 <= |mant| < 1.0 and -1074 <= rem_shift <= -1022.
        let p = rem_shift - 1;
        if p >= -1074 {
            let z = mant * 2.0; // exact in [1.0, 2.0)
            let pow2 = f64::from_bits(1u64 << (p + 1074) as u32);
            normalize_zero(z * pow2)
        } else {
            // p == -1075 (since target_exp >= -1074 => rem_shift >= -1074):
            // mant * 2^-1074 has single rounding (0.5 -> 0.0 even, >0.5 -> 2^-1074).
            normalize_zero(mant * f64::from_bits(1))
        }
    }
}

fn validate_finite_slice(xs: &[f64]) -> crate::Result<()> {
    if xs.len() > MAX_EXACT_COUNT {
        return Err(crate::Error::Validation(format!(
            "Sample count {} exceeds maximum exact count {MAX_EXACT_COUNT}",
            xs.len()
        )));
    }
    for &x in xs {
        if !x.is_finite() {
            return Err(crate::InputError::NonFiniteValue {
                kind: crate::NonFiniteKind::classify(x),
            }
            .into());
        }
    }
    Ok(())
}

fn check_constant_slice(xs: &[f64]) -> Option<f64> {
    let first = *xs.first()?;
    if xs
        .iter()
        .all(|&x| crate::math::interp::exact_f64_eq(x, first))
    {
        Some(normalize_zero(first))
    } else {
        None
    }
}
