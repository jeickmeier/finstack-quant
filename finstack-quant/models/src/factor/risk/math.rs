//! Shared numerical helpers for factor-model risk decomposition.
//!
//! These helpers live here (rather than in each decomposer) so that the
//! parametric portfolio and position decomposers reuse canonical core
//! normal distribution helpers and pivoted Cholesky covariance validation.

/// Inverse standard-normal CDF (probit) delegated to `finstack_quant_core::math`.
#[inline]
pub(super) fn normal_quantile(probability: f64) -> f64 {
    finstack_quant_core::math::standard_normal_inv_cdf(probability)
}

/// Standard-normal probability density function delegated to `finstack_quant_core::math`.
#[inline]
pub(super) fn normal_pdf(x: f64) -> f64 {
    finstack_quant_core::math::norm_pdf(x)
}

/// Validate a raw position covariance using the shared core PSD factorization.
///
/// # Arguments
///
/// * `data` - Finite symmetric covariance entries in row-major order.
/// * `n` - Number of position axes; storage must contain exactly `n * n` entries.
///
/// # Errors
///
/// Returns a validation error for a malformed, asymmetric or indefinite matrix.
pub(crate) fn validate_covariance(data: &[f64], n: usize) -> finstack_quant_core::Result<()> {
    if n.checked_mul(n) != Some(data.len()) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Covariance storage length {} does not match matrix dimension {n}x{n}",
            data.len()
        )));
    }
    let scale = (0..n)
        .map(|i| data[i * n + i].abs())
        .fold(0.0_f64, f64::max);
    let tolerance = finstack_quant_core::math::linalg::SYMMETRY_TOLERANCE * scale;
    for i in 0..n {
        for j in (i + 1)..n {
            if (data[i * n + j] - data[j * n + i]).abs() > tolerance {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Covariance matrix must be symmetric at ({i}, {j})"
                )));
            }
        }
    }
    finstack_quant_core::math::linalg::cholesky_correlation(data, n)
        .map(|_| ())
        .map_err(|error| {
            finstack_quant_core::Error::Validation(format!(
                "Covariance matrix is not positive semi-definite: {error}"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::validate_covariance;

    #[test]
    fn covariance_validation_is_scale_and_axis_order_invariant() {
        for scale in [1e-8_f64, 1e-4, 1.0, 1e4, 1e8] {
            for base in [
                [1.0, 2.0, 2.0, 4.0],
                [1e-6, 0.1, 0.1, 1e4],
                [1e4, 0.1, 0.1, 1e-6],
            ] {
                let data: Vec<f64> = base.iter().map(|v| v * scale).collect();
                assert!(validate_covariance(&data, 2).is_ok());
            }
            let indefinite: Vec<f64> = [1.0, 3.0, 3.0, 1.0].iter().map(|v| v * scale).collect();
            assert!(validate_covariance(&indefinite, 2).is_err());
        }
    }

    #[test]
    fn covariance_validation_rejects_invalid_storage_and_non_finite_entries() {
        assert!(validate_covariance(&[1.0, 0.0, 1.0], 2).is_err());
        assert!(validate_covariance(&[1.0, f64::NAN, 0.0, 1.0], 2).is_err());
        assert!(validate_covariance(&[f64::INFINITY], 1).is_err());
        assert!(validate_covariance(&[], usize::MAX).is_err());
    }

    #[test]
    fn covariance_symmetry_check_is_relative() {
        assert!(validate_covariance(&[1e8, 2e8, 2e8 + 1e-4, 4e8], 2).is_ok());
        assert!(validate_covariance(&[1e-8, 2e-8, 2e-8 + 1e-9, 4e-8], 2).is_err());
    }
}
