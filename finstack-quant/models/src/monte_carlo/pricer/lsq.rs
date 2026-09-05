//! Shared SVD least-squares solver for LSMC regression.

use crate::monte_carlo::pricer::basis::BasisFunctions;
use finstack_quant_core::Result;

/// Least squares `min || Xβ - y ||²` via SVD.
///
/// `X` is an `n × k` design matrix in row-major order. SVD avoids forming
/// `X'X` (which squares the condition number) and truncates near-rank-deficient
/// directions — important for high-degree LSMC polynomials.
///
/// # Arguments
///
/// * `design` - Design matrix X in row-major order (n x k)
/// * `y` - Response vector (n elements)
/// * `n` - Number of observations (rows)
/// * `k` - Number of basis functions (columns)
///
/// # Returns
///
/// Coefficient vector β (k elements)
///
/// The SVD cutoff is relative to the largest singular value and matrix size,
/// so near-rank-deficient directions are truncated rather than amplified. The
/// returned coefficients are in the units implied by `y` per basis-function
/// unit.
///
/// # Errors
///
/// Returns an error if `n < k` or the SVD solver cannot produce a least-squares
/// solution (for example, a numerically singular design matrix).
///
/// # Panics
///
/// Panics if `design.len()` is not exactly `n * k`; callers must also pass a
/// response vector compatible with `n` observations.
pub fn solve_least_squares(design: &[f64], y: &[f64], n: usize, k: usize) -> Result<Vec<f64>> {
    use nalgebra::{DMatrix, DVector};

    if n < k {
        return Err(finstack_quant_core::Error::internal(
            "LSMC regression requires at least as many observations as basis functions",
        ));
    }

    let x_matrix = DMatrix::from_row_slice(n, k, design);
    let y_vector = DVector::from_column_slice(y);
    let svd = x_matrix.svd(true, true);

    // nalgebra's `eps` is an ABSOLUTE cutoff on singular values, so it must
    // scale with σ_max: with raw polynomial bases of spot ≈ 100 the design
    // entries reach 1e10 and a fixed 1e-10 never truncates, letting
    // near-rank-deficient directions amplify regression noise into the
    // exercise boundary. The standard relative cutoff is
    // σ_max · ε_machine · max(n, k).
    let sigma_max = svd.singular_values.iter().copied().fold(0.0_f64, f64::max);
    let eps = sigma_max * f64::EPSILON * n.max(k) as f64;

    match svd.solve(&y_vector, eps) {
        Ok(beta) => Ok(beta.as_slice().to_vec()),
        Err(_) => {
            tracing::warn!("LSMC regression failed (singular matrix)");
            Err(finstack_quant_core::Error::internal(
                "LSMC regression SVD solve failed for the design matrix",
            ))
        }
    }
}

/// Fit basis-function regression coefficients (no prediction loop).
///
/// Use this when the caller wants to retain the regression coefficients
/// — for example, to apply a frozen exercise policy to an independent path
/// set in two-pass LSMC.
///
/// # Arguments
///
/// * `x` - State variables (spot prices or swap rates)
/// * `y` - Discounted continuation values to fit
/// * `basis` - Basis functions evaluated at each x value
///
/// # Returns
///
/// Coefficient vector β with length `basis.num_basis()`.
///
/// `x` and `y` represent paired observations; basis values are evaluated in
/// `x` order to form a row-major design matrix. Coefficients can be reused for
/// an out-of-sample continuation-value policy only with the same basis and
/// state-variable convention.
///
/// # Errors
///
/// Propagates the least-squares error when there are fewer observations than
/// basis functions or the SVD solve fails. A response vector with an
/// incompatible length is rejected by the underlying solver.
pub fn regression_coefficients_with_basis<B>(x: &[f64], y: &[f64], basis: &B) -> Result<Vec<f64>>
where
    B: BasisFunctions + ?Sized,
{
    let n = x.len();
    let k = basis.num_basis();

    let mut design = vec![0.0; n * k];
    let mut basis_vals = vec![0.0; k];

    for (i, &x_val) in x.iter().enumerate() {
        basis.evaluate(x_val, &mut basis_vals);
        let row_start = i * k;
        design[row_start..row_start + k].copy_from_slice(&basis_vals);
    }

    solve_least_squares(&design, y, n, k)
}

/// Fit continuation values in-sample: design matrix, SVD, then predict.
///
/// # Arguments
///
/// * `x` - State variables (spot prices or swap rates)
/// * `y` - Discounted continuation values to fit
/// * `basis` - Basis functions to evaluate at each x value
///
/// # Returns
///
/// Predicted continuation values for each x value (same length as x)
///
/// This fits and predicts on the same sample; it is an in-sample continuation
/// estimate, not an independently validated exercise policy.
///
/// # Errors
///
/// Propagates the coefficient-fit errors from
/// [`regression_coefficients_with_basis`], including insufficient observations
/// for the selected basis or an unsuccessful SVD solve.
pub fn regression_with_basis<B>(x: &[f64], y: &[f64], basis: &B) -> Result<Vec<f64>>
where
    B: BasisFunctions + ?Sized,
{
    let coeffs = regression_coefficients_with_basis(x, y, basis)?;
    let k = basis.num_basis();

    let mut basis_vals = vec![0.0; k];
    let mut predictions = vec![0.0; x.len()];
    for (i, &x_val) in x.iter().enumerate() {
        basis.evaluate(x_val, &mut basis_vals);
        let mut pred = 0.0;
        for j in 0..k {
            pred += coeffs[j] * basis_vals[j];
        }
        predictions[i] = pred;
    }

    Ok(predictions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monte_carlo::pricer::basis::PolynomialBasis;

    #[test]
    fn test_solve_least_squares_simple() {
        let design = vec![1.0, 1.0, 1.0, 2.0, 1.0, 3.0];
        let y = vec![5.0, 8.0, 11.0];

        let solution = solve_least_squares(&design, &y, 3, 2).expect("should succeed");

        assert!((solution[0] - 2.0).abs() < 1e-10);
        assert!((solution[1] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_solve_least_squares_singular() {
        let design = vec![1.0, 1.0, 2.0, 1.0, 2.0, 4.0, 1.0, 3.0, 6.0];
        let y = vec![1.0, 2.0, 3.0];

        let solution = solve_least_squares(&design, &y, 3, 3).expect("should succeed");

        assert!(solution.len() == 3);
        assert!(solution.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_solve_least_squares_ill_conditioned() {
        let x_values = vec![1.0, 1.1, 1.2, 1.3, 1.4];
        let mut design = Vec::new();

        for &x in &x_values {
            design.push(1.0);
            design.push(x);
            design.push(x * x);
            design.push(x * x * x);
        }

        let y = vec![1.0, 1.2, 1.5, 1.8, 2.0];

        let solution = solve_least_squares(&design, &y, 5, 4);

        assert!(solution.is_ok());
        let beta = solution.expect("should succeed");
        assert_eq!(beta.len(), 4);
        assert!(beta.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_regression_with_basis_polynomial() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![3.0, 5.0, 7.0, 9.0, 11.0];

        let basis = PolynomialBasis::new(1);

        let predictions = regression_with_basis(&x, &y, &basis).expect("should succeed");

        for (i, &pred) in predictions.iter().enumerate() {
            assert!(
                (pred - y[i]).abs() < 1e-6,
                "Prediction {} differs from y[{}]: {} vs {}",
                i,
                i,
                pred,
                y[i]
            );
        }
    }

    #[test]
    fn test_regression_with_basis_quadratic() {
        let x = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let y = vec![1.0, 6.0, 17.0, 34.0, 57.0];

        let basis = PolynomialBasis::new(2);

        let predictions = regression_with_basis(&x, &y, &basis).expect("should succeed");

        for (i, &pred) in predictions.iter().enumerate() {
            assert!(
                (pred - y[i]).abs() < 1e-6,
                "Prediction {} differs from y[{}]: {} vs {}",
                i,
                i,
                pred,
                y[i]
            );
        }
    }

    #[test]
    fn test_regression_with_basis_stability() {
        let x = vec![10.0, 50.0, 100.0, 200.0, 500.0, 1000.0];
        let y = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];

        let basis = PolynomialBasis::new(3);

        let result = regression_with_basis(&x, &y, &basis);

        assert!(result.is_ok());
        let predictions = result.expect("should succeed");
        assert_eq!(predictions.len(), x.len());
        assert!(predictions.iter().all(|&p| p.is_finite()));
    }
}
