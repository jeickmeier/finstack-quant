//! PCA-based dimension ordering for Sobol sequences.
//!
//! Reduces effective dimension for quasi-Monte Carlo by reordering
//! dimensions based on principal component analysis of the correlation matrix.
//!
//! # Motivation
//!
//! Sobol sequences work best when:
//! - Important dimensions come first (assigned best-quality Sobol points)
//! - Effective dimension is low
//!
//! For correlated assets, PCA identifies the most important directions
//! (eigenvectors with largest eigenvalues) and assigns them to early dimensions.
//!
//! # Algorithm
//!
//! 1. Eigen-decomposition of correlation matrix: ρ = Q Λ Q'
//! 2. Sort eigenvectors by eigenvalue (descending)
//! 3. Reorder Sobol dimensions to match sorted eigenvectors
//!
//! # Example
//!
//! ```ignore
//! use finstack_core::math::random::sobol_pca::pca_ordering;
//!
//! # fn main() -> finstack_core::Result<()> {
//! // 3-asset basket with correlation
//! let correlation = vec![
//!     1.0, 0.8, 0.6,
//!     0.8, 1.0, 0.7,
//!     0.6, 0.7, 1.0,
//! ];
//!
//! let (_eigenvalues, _eigenvectors, permutation) = pca_ordering(&correlation, 3)?;
//!
//! // permutation maps back to original asset order
//! # Ok(())
//! # }
//! ```
//!
//! Reference: Acworth et al. (1998) - "A comparison of some MC and QMC techniques"

use nalgebra::{DMatrix, SymmetricEigen};
use smallvec::SmallVec;

use crate::error::InputError;

/// Perform PCA on correlation matrix to find optimal dimension ordering.
///
/// # Arguments
///
/// * `correlation` - Correlation matrix (n x n, row-major)
/// * `num_factors` - Number of factors
///
/// # Returns
///
/// (eigenvalues, eigenvectors, permutation)
/// - eigenvalues: sorted in descending order
/// - eigenvectors: columns are eigenvectors (sorted by eigenvalue)
/// - permutation: maps original dimensions to PCA-ordered dimensions
pub fn pca_ordering(
    correlation: &[f64],
    num_factors: usize,
) -> crate::Result<(Vec<f64>, Vec<f64>, Vec<usize>)> {
    if correlation.len() != num_factors * num_factors {
        return Err(InputError::DimensionMismatch.into());
    }

    // Convert to nalgebra matrix
    let corr_matrix = DMatrix::from_row_slice(num_factors, num_factors, correlation);

    // Ensure symmetric (average with transpose for numerical stability)
    let corr_sym = (&corr_matrix + &corr_matrix.transpose()) / 2.0;

    // Eigen-decomposition
    let eigen = SymmetricEigen::new(corr_sym);

    // Extract eigenvalues and eigenvectors
    let mut eigenvalues: Vec<(usize, f64)> = eigen
        .eigenvalues
        .iter()
        .enumerate()
        .map(|(i, &val)| (i, val))
        .collect();

    // Sort by eigenvalue (descending) using total_cmp for safe float comparison
    eigenvalues.sort_by(|a, b| b.1.total_cmp(&a.1));

    // Build permutation (original index → sorted index)
    let permutation: Vec<usize> = eigenvalues.iter().map(|(i, _)| *i).collect();

    // Extract sorted eigenvalues and reorder eigenvector columns consistently.
    let sorted_eigenvalues: Vec<f64> = eigenvalues.iter().map(|(_, val)| *val).collect();
    let mut sorted_eigenvectors = vec![0.0; num_factors * num_factors];
    for (sorted_col, (original_col, _)) in eigenvalues.iter().enumerate() {
        for row in 0..num_factors {
            sorted_eigenvectors[row + sorted_col * num_factors] =
                eigen.eigenvectors[(row, *original_col)];
        }
    }

    Ok((sorted_eigenvalues, sorted_eigenvectors, permutation))
}

/// Compute effective dimension for QMC.
///
/// Effective dimension measures how many dimensions contribute significantly
/// to variance. Lower effective dimension → better QMC performance.
///
/// # Formula
///
/// d_eff = (Σ λ_i)² / Σ λ_i²
///
/// where λ_i are eigenvalues of correlation matrix.
pub fn effective_dimension(eigenvalues: &[f64]) -> f64 {
    let sum = eigenvalues.iter().sum::<f64>();
    let sum_sq = eigenvalues.iter().map(|&x| x * x).sum::<f64>();

    if sum_sq > 1e-10 {
        sum * sum / sum_sq
    } else {
        eigenvalues.len() as f64
    }
}

/// Apply PCA transformation to random shocks.
///
/// Transforms shocks from PCA space back to original asset space via
/// `z_out = Q_sorted · z_pca`. The rows of the sorted eigenvector matrix are
/// already indexed by original asset (only the *columns* were reordered by the
/// eigenvalue sort), so the eigen-sort permutation must **not** be applied to
/// the asset axis — doing so would scramble the output covariance to
/// `P·ρ·Pᵀ ≠ ρ` (see `docs/reviews/2026-06-09-core-quant-review.md`,
/// "transform_pca_to_assets scrambles the asset axis"). The permutation
/// argument is retained for API stability and dimension validation only.
///
/// # Arguments
///
/// * `z_pca` - Shocks in PCA-ordered space (component `j` scaled by `√λ_j`)
/// * `eigenvectors` - Sorted eigenvector matrix from [`pca_ordering`] (column-major)
/// * `permutation` - Permutation from [`pca_ordering`] (validated, not applied)
/// * `z_out` - Output shocks in original asset order
pub fn transform_pca_to_assets(
    z_pca: &[f64],
    eigenvectors: &[f64],
    permutation: &[usize],
    z_out: &mut [f64],
) -> crate::Result<()> {
    let n = z_pca.len();
    if eigenvectors.len() != n * n || z_out.len() != n || permutation.len() != n {
        return Err(InputError::DimensionMismatch.into());
    }

    // z_out = Q * z_pca (where Q is the sorted eigenvector matrix).
    // Rows of Q are original asset indices, so the result is already in
    // original asset order; no permutation of the asset axis is applied.
    let mut z_temp: SmallVec<[f64; 64]> = smallvec::smallvec![0.0; n];
    for i in 0..n {
        let mut sum = 0.0;
        for j in 0..n {
            sum += eigenvectors[i + j * n] * z_pca[j];
        }
        z_temp[i] = sum;
    }
    z_out.copy_from_slice(&z_temp);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pca_identity_matrix() {
        // Identity matrix: eigenvalues all 1, any order is optimal
        let correlation = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

        let (eigenvalues, _, _) = pca_ordering(&correlation, 3).expect("3x3 matrix should succeed");

        // All eigenvalues should be 1
        for &val in &eigenvalues {
            assert!((val - 1.0).abs() < 0.01);
        }

        // Effective dimension should be 3
        let d_eff = effective_dimension(&eigenvalues);
        assert!((d_eff - 3.0).abs() < 0.1);
    }

    #[test]
    fn test_pca_high_correlation() {
        // High correlation: effective dimension should be low
        let correlation = vec![1.0, 0.9, 0.9, 0.9, 1.0, 0.9, 0.9, 0.9, 1.0];

        let (eigenvalues, _, _) = pca_ordering(&correlation, 3).expect("3x3 matrix should succeed");

        println!("Eigenvalues (high correlation): {:?}", eigenvalues);

        // First eigenvalue should dominate
        assert!(eigenvalues[0] > 2.0); // Should be ~2.7
        assert!(eigenvalues[1] < 1.0); // Others smaller

        // Effective dimension should be low
        let d_eff = effective_dimension(&eigenvalues);
        println!("Effective dimension: {:.2}", d_eff);
        assert!(d_eff < 2.0);
    }

    #[test]
    fn test_effective_dimension_bounds() {
        // Effective dimension should be between 1 and n
        let eigenvalues_equal = vec![1.0, 1.0, 1.0];
        let d_eff = effective_dimension(&eigenvalues_equal);
        assert!((d_eff - 3.0).abs() < 0.1);

        // Single dominant eigenvalue
        let eigenvalues_dominant = vec![2.9, 0.05, 0.05];
        let d_eff = effective_dimension(&eigenvalues_dominant);
        assert!((1.0..=3.0).contains(&d_eff));
        assert!(d_eff < 1.5); // Should be close to 1
    }

    #[test]
    fn test_pca_transformation() {
        // Simple 2D test
        let correlation = vec![1.0, 0.5, 0.5, 1.0];

        let (_eigenvalues, eigenvectors, permutation) =
            pca_ordering(&correlation, 2).expect("2x2 matrix should succeed");

        // Transform identity shocks through PCA
        let z_pca = vec![1.0, 0.0]; // Shock in first PC direction
        let mut z_assets = vec![0.0; 2];

        transform_pca_to_assets(&z_pca, &eigenvectors, &permutation, &mut z_assets)
            .expect("matching dimensions should succeed");

        // Result should be in asset space
        println!("PCA transform: {:?} -> {:?}", z_pca, z_assets);
        assert!(z_assets[0].is_finite());
        assert!(z_assets[1].is_finite());
    }

    #[test]
    fn test_pca_ordering_reconstructs_correlation_matrix() {
        let correlation = vec![
            1.0, 0.9, 0.8, //
            0.9, 1.0, 0.7, //
            0.8, 0.7, 1.0,
        ];
        let (eigenvalues, eigenvectors, _) =
            pca_ordering(&correlation, 3).expect("3x3 matrix should succeed");

        let mut reconstructed = [0.0; 9];
        for row in 0..3 {
            for col in 0..3 {
                let mut value = 0.0;
                for factor in 0..3 {
                    let q_row = eigenvectors[row + factor * 3];
                    let q_col = eigenvectors[col + factor * 3];
                    value += q_row * eigenvalues[factor] * q_col;
                }
                reconstructed[row * 3 + col] = value;
            }
        }

        for (actual, expected) in reconstructed.iter().zip(correlation.iter()) {
            assert!(
                (actual - expected).abs() < 1e-10,
                "reconstructed correlation mismatch: {actual} vs {expected}"
            );
        }
    }

    #[test]
    fn test_pca_ordering_rejects_dimension_mismatch() {
        let result = pca_ordering(&[1.0, 0.0, 0.0], 2);
        assert!(result.is_err(), "dimension mismatch should return an error");
    }

    #[test]
    fn test_transform_pca_to_assets_ignores_permutation_on_asset_axis() {
        // Review 2026-06-09 (core math): `z_temp = Q · z_pca` is already in
        // original asset order; re-applying the eigen-sort permutation to the
        // asset axis produced Cov(z_out) = P·ρ·Pᵀ ≠ ρ. With an identity Q the
        // output must equal z_pca regardless of the permutation argument.
        let z_pca = [1.0, 2.0];
        let eigenvectors = [
            1.0, 0.0, // column 0
            0.0, 1.0, // column 1
        ];
        let permutation = [1, 0]; // non-identity eigen-sort permutation
        let mut z_out = [0.0; 2];

        transform_pca_to_assets(&z_pca, &eigenvectors, &permutation, &mut z_out)
            .expect("matching dimensions should succeed");

        assert!(
            (z_out[0] - 1.0).abs() < 1e-12 && (z_out[1] - 2.0).abs() < 1e-12,
            "output must be Q·z_pca = z_pca for identity Q, got {z_out:?}"
        );
    }

    #[test]
    fn test_transform_pca_to_assets_output_covariance_matches_correlation() {
        // Analytic covariance check through the public transform: feeding the
        // PCA-space "shock columns" √λ_j·e_j and summing the outer products of
        // the outputs reconstructs Cov(z_out) = Q·Λ·Qᵀ, which must equal the
        // input correlation matrix in original asset order.
        let n = 3;
        let correlation = vec![
            1.0, 0.9, 0.8, //
            0.9, 1.0, 0.7, //
            0.8, 0.7, 1.0,
        ];
        let (eigenvalues, eigenvectors, permutation) =
            pca_ordering(&correlation, n).expect("3x3 matrix should succeed");

        let mut covariance = vec![0.0; n * n];
        for (j, &lambda) in eigenvalues.iter().enumerate() {
            // z_pca = √λ_j · e_j (unit shock in sorted PCA dimension j)
            let mut z_pca = vec![0.0; n];
            z_pca[j] = lambda.max(0.0).sqrt();
            let mut z_out = vec![0.0; n];
            transform_pca_to_assets(&z_pca, &eigenvectors, &permutation, &mut z_out)
                .expect("matching dimensions should succeed");

            for row in 0..n {
                for col in 0..n {
                    covariance[row * n + col] += z_out[row] * z_out[col];
                }
            }
        }

        for (actual, expected) in covariance.iter().zip(correlation.iter()) {
            assert!(
                (actual - expected).abs() < 1e-10,
                "output covariance mismatch: {actual} vs {expected}"
            );
        }
    }

    #[test]
    fn test_transform_pca_to_assets_rejects_dimension_mismatch() {
        let z_pca = [1.0, 0.0];
        let eigenvectors = [1.0, 0.0, 0.0];
        let permutation = [0, 1];
        let mut z_out = [0.0; 2];

        let result = transform_pca_to_assets(&z_pca, &eigenvectors, &permutation, &mut z_out);
        assert!(result.is_err(), "dimension mismatch should return an error");
    }
}
