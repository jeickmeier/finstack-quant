//! Shared least-squares solver for LSMC regression.
//!
//! Provides a robust SVD-based solver used by both equity and swaption LSMC pricers.
//! Consolidates duplicate regression logic to ensure consistent results and easier testing.

use crate::pricer::basis::BasisFunctions;
use finstack_core::Result;

/// Solve least squares problem using SVD (Singular Value Decomposition).
///
/// Solves: min || Xβ - y ||²
///
/// where X is n x k design matrix (row-major).
///
/// Uses nalgebra's SVD decomposition which is numerically more stable
/// than normal equations (Cholesky) or QR for ill-conditioned systems.
///
/// # Numerical Stability
///
/// SVD is the most robust method for least squares:
/// - Avoids forming X'X which squares the condition number: cond(X'X) ≈ cond(X)²
/// - Handles rank-deficient matrices gracefully
/// - Uses threshold-based pseudo-inverse for numerical stability
///
/// This is critical for LSMC with high-degree polynomials or extreme spot/rate ranges
/// where regression matrices can be ill-conditioned.
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
pub fn solve_least_squares(design: &[f64], y: &[f64], n: usize, k: usize) -> Result<Vec<f64>> {
    use nalgebra::{DMatrix, DVector};

    // Check for degenerate cases
    if n < k {
        return Err(finstack_core::Error::internal(
            "LSMC regression requires at least as many observations as basis functions",
        ));
    }

    // Convert to nalgebra matrices
    let x_matrix = DMatrix::from_row_slice(n, k, design);
    let y_vector = DVector::from_column_slice(y);

    // Solve least squares problem using SVD (more robust than QR for overdetermined systems)
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
        Ok(beta) => {
            // Convert back to Vec<f64>
            Ok(beta.as_slice().to_vec())
        }
        Err(_) => {
            // SVD decomposition failed (singular or near-singular matrix)
            // This can happen with:
            // - Linearly dependent basis functions
            // - Too few ITM paths for regression
            // - Numerical issues with extreme values
            tracing::warn!("LSMC regression failed (singular matrix)");
            Err(finstack_core::Error::internal(
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
pub fn regression_coefficients_with_basis<B>(x: &[f64], y: &[f64], basis: &B) -> Result<Vec<f64>>
where
    B: BasisFunctions + ?Sized,
{
    let n = x.len();
    let k = basis.num_basis();

    // Build design matrix X (n x k). One reusable basis_vals buffer.
    let mut design = vec![0.0; n * k];
    let mut basis_vals = vec![0.0; k];

    for (i, &x_val) in x.iter().enumerate() {
        basis.evaluate(x_val, &mut basis_vals);
        let row_start = i * k;
        design[row_start..row_start + k].copy_from_slice(&basis_vals);
    }

    solve_least_squares(&design, y, n, k)
}

/// Perform LSMC regression with basis functions.
///
/// This is the complete regression workflow used by both equity and swaption LSMC:
/// 1. Build design matrix from basis function evaluations
/// 2. Solve least squares using SVD
/// 3. Predict continuation values for all inputs
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
pub fn regression_with_basis<B>(x: &[f64], y: &[f64], basis: &B) -> Result<Vec<f64>>
where
    B: BasisFunctions + ?Sized,
{
    let coeffs = regression_coefficients_with_basis(x, y, basis)?;
    let k = basis.num_basis();

    // Predict continuation values
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
    use crate::pricer::basis::PolynomialBasis;

    #[test]
    fn test_solve_least_squares_simple() {
        // Simple regression: y = 2 + 3x
        // Design matrix: [1, x_i] for each observation
        let design = vec![
            1.0, 1.0, // observation 1: x=1
            1.0, 2.0, // observation 2: x=2
            1.0, 3.0, // observation 3: x=3
        ];
        let y = vec![5.0, 8.0, 11.0]; // y = 2 + 3x

        let solution = solve_least_squares(&design, &y, 3, 2).expect("should succeed");

        // Should recover β₀=2, β₁=3
        assert!((solution[0] - 2.0).abs() < 1e-10);
        assert!((solution[1] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_solve_least_squares_singular() {
        // Test with singular matrix (linearly dependent columns)
        let design = vec![
            1.0, 1.0, 2.0, // Column 3 = 2 * Column 2
            1.0, 2.0, 4.0, 1.0, 3.0, 6.0,
        ];
        let y = vec![1.0, 2.0, 3.0];

        let solution = solve_least_squares(&design, &y, 3, 3).expect("should succeed");

        // Should return fallback zero vector or a valid solution
        assert!(solution.len() == 3);
        assert!(solution.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_solve_least_squares_ill_conditioned() {
        // Test with ill-conditioned polynomial design matrix
        // (narrow x range with high-degree polynomial)
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

        // SVD should handle ill-conditioning gracefully
        assert!(solution.is_ok());
        let beta = solution.expect("should succeed");
        assert_eq!(beta.len(), 4);
        assert!(beta.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_regression_with_basis_polynomial() {
        // Test the complete regression workflow with polynomial basis
        // True function: y = 1 + 2x
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![3.0, 5.0, 7.0, 9.0, 11.0];

        let basis = PolynomialBasis::new(1); // Linear: {1, x}

        let predictions = regression_with_basis(&x, &y, &basis).expect("should succeed");

        // Check predictions match observed values (perfect fit for linear data)
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
        // Test with quadratic basis and quadratic data
        // True function: y = 1 + 2x + 3x²
        let x = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let y = vec![1.0, 6.0, 17.0, 34.0, 57.0];

        let basis = PolynomialBasis::new(2); // {1, x, x²}

        let predictions = regression_with_basis(&x, &y, &basis).expect("should succeed");

        // Check predictions match observed values (perfect fit for quadratic data)
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
        // Test numerical stability with high-degree polynomial and wide x range
        let x = vec![10.0, 50.0, 100.0, 200.0, 500.0, 1000.0];
        let y = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];

        let basis = PolynomialBasis::new(3); // Cubic basis

        let result = regression_with_basis(&x, &y, &basis);

        // Should succeed despite potentially ill-conditioned matrix
        assert!(result.is_ok());
        let predictions = result.expect("should succeed");
        assert_eq!(predictions.len(), x.len());
        assert!(predictions.iter().all(|&p| p.is_finite()));
    }
}
