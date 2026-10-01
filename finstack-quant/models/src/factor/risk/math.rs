//! Shared numerical helpers for factor-model risk decomposition.
//!
//! These helpers live here (rather than in each decomposer) so that the
//! parametric portfolio and position decomposers reuse bit-identical
//! constants and formulas. Raw position covariances use core's pivoted
//! Cholesky validation. The `normal_quantile` implementation is Peter Acklam's
//! rational approximation
//! to the inverse standard-normal CDF (sometimes misattributed to
//! Beasley–Springer–Moro, a different algorithm); `normal_pdf` is the
//! standard-normal density. Keep the constants byte-identical — changing
//! them is a numerical-behaviour change, not a cleanup.

/// Rational approximation for the inverse standard-normal CDF (probit).
pub(super) fn normal_quantile(probability: f64) -> f64 {
    const A1: f64 = -3.969_683_028_665_376e1;
    const A2: f64 = 2.209_460_984_245_205e2;
    const A3: f64 = -2.759_285_104_469_687e2;
    const A4: f64 = 1.383_577_518_672_69e2;
    const A5: f64 = -3.066_479_806_614_716e1;
    const A6: f64 = 2.506_628_277_459_239;
    const B1: f64 = -5.447_609_879_822_406e1;
    const B2: f64 = 1.615_858_368_580_409e2;
    const B3: f64 = -1.556_989_798_598_866e2;
    const B4: f64 = 6.680_131_188_771_972e1;
    const B5: f64 = -1.328_068_155_288_572e1;
    const C1: f64 = -7.784_894_002_430_293e-3;
    const C2: f64 = -3.223_964_580_411_365e-1;
    const C3: f64 = -2.400_758_277_161_838;
    const C4: f64 = -2.549_732_539_343_734;
    const C5: f64 = 4.374_664_141_464_968;
    const C6: f64 = 2.938_163_982_698_783;
    const D1: f64 = 7.784_695_709_041_462e-3;
    const D2: f64 = 3.224_671_290_700_398e-1;
    const D3: f64 = 2.445_134_137_142_996;
    const D4: f64 = 3.754_408_661_907_416;
    const P_LOW: f64 = 0.024_25;
    const P_HIGH: f64 = 1.0 - P_LOW;

    if probability < P_LOW {
        let q = (-2.0 * probability.ln()).sqrt();
        (((((C1 * q + C2) * q + C3) * q + C4) * q + C5) * q + C6)
            / ((((D1 * q + D2) * q + D3) * q + D4) * q + 1.0)
    } else if probability > P_HIGH {
        let q = (-2.0 * (1.0 - probability).ln()).sqrt();
        -(((((C1 * q + C2) * q + C3) * q + C4) * q + C5) * q + C6)
            / ((((D1 * q + D2) * q + D3) * q + D4) * q + 1.0)
    } else {
        let q = probability - 0.5;
        let r = q * q;
        (((((A1 * r + A2) * r + A3) * r + A4) * r + A5) * r + A6) * q
            / (((((B1 * r + B2) * r + B3) * r + B4) * r + B5) * r + 1.0)
    }
}

/// Standard-normal probability density function.
pub(super) fn normal_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt()
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
pub(super) fn validate_covariance(data: &[f64], n: usize) -> finstack_quant_core::Result<()> {
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
