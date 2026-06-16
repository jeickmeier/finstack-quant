//! PCA decomposition of yield curve changes.
//!
//! Extracts orthogonal principal components from historical yield changes,
//! typically interpreted as level, slope, and curvature shocks. Enables
//! scenario generation, P&L attribution, and risk decomposition along
//! independent curve dimensions.
//!
//! # Model
//!
//! Given a T x N matrix of yield changes Delta_y, PCA finds orthogonal
//! directions (loadings) such that:
//! ```text
//! Delta_y(t) ~ sum_{k=1}^{K} score_k(t) * loading_k
//! ```
//!
//! where loadings are eigenvectors of the covariance matrix of Delta_y,
//! ordered by eigenvalue (variance explained, descending).
//!
//! # References
//!
//! - Litterman, R., & Scheinkman, J. (1991). "Common Factors Affecting
//!   Bond Returns." *Journal of Fixed Income*, 1(1), 54-61.
//! - Rebonato, R. (2018). *Bond Pricing and Yield Curve Modeling:
//!   A Structural Approach*. Cambridge UP. Ch. 4.

use nalgebra::{DMatrix, DVector, SymmetricEigen};
use serde::{Deserialize, Serialize};

use super::types::YieldPanel;

// ---------------------------------------------------------------------------
// YieldPca
// ---------------------------------------------------------------------------

/// PCA decomposition of yield curve changes.
///
/// See module-level documentation for details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YieldPca {
    /// Eigenvalues in descending order (length min(T-1, N)).
    eigenvalues: Vec<f64>,
    /// Loadings matrix: N tenors x K components (columns are eigenvectors).
    loadings: DMatrix<f64>,
    /// Scores matrix: (T-1) dates x K components.
    scores: DMatrix<f64>,
    /// Tenor grid (length N).
    tenors: Vec<f64>,
    /// Fraction of total variance explained by each component.
    variance_explained: Vec<f64>,
    /// Cumulative fraction of variance explained.
    cumulative_variance: Vec<f64>,
    /// Mean yield change vector (length N), subtracted before PCA.
    mean_change: DVector<f64>,
}

impl YieldPca {
    /// Fit PCA to a yield panel.
    ///
    /// Computes first differences of the yield panel, then performs
    /// eigendecomposition of the sample covariance matrix.
    ///
    /// # Errors
    /// - Fewer than 3 observations (need at least 2 yield changes)
    /// - Fewer than 2 tenors
    /// - Covariance matrix is degenerate (all-zero yield changes)
    pub fn fit(panel: &YieldPanel) -> crate::Result<Self> {
        let n = panel.num_tenors();
        let t = panel.num_dates();

        if n < 2 {
            return Err(crate::Error::Validation(format!(
                "Need at least 2 tenors for PCA, got {n}"
            )));
        }
        if t < 3 {
            return Err(crate::Error::Validation(format!(
                "Need at least 3 observations for PCA (to get 2 yield changes), got {t}"
            )));
        }

        // Compute yield changes: (T-1) x N
        let changes = panel.yield_changes();
        let m = changes.nrows(); // T - 1

        // Column means
        let mut mean_change = DVector::zeros(n);
        for j in 0..n {
            let mut sum = 0.0;
            for i in 0..m {
                sum += changes[(i, j)];
            }
            mean_change[j] = sum / m as f64;
        }

        // De-mean the changes
        let mut centered = changes;
        for i in 0..m {
            for j in 0..n {
                centered[(i, j)] -= mean_change[j];
            }
        }

        // Sample covariance matrix: (1/(m-1)) * centered' * centered
        let ct = centered.transpose();
        let cov = (&ct * &centered) / (m as f64 - 1.0).max(1.0);

        // Check for degenerate covariance
        let trace = (0..n).map(|i| cov[(i, i)]).sum::<f64>();
        if trace < 1e-30 {
            return Err(crate::Error::Validation(
                "Covariance matrix is degenerate (all-zero yield changes)".into(),
            ));
        }

        // Symmetric eigendecomposition
        let eigen = SymmetricEigen::new(cov);

        // nalgebra returns eigenvalues in ascending order; we want descending
        let k = n; // number of components = number of tenors
        let raw_eigenvalues = eigen.eigenvalues;
        let raw_eigenvectors = eigen.eigenvectors;

        // Sort indices by eigenvalue descending
        let mut indices: Vec<usize> = (0..k).collect();
        indices.sort_by(|&a, &b| {
            raw_eigenvalues[b]
                .partial_cmp(&raw_eigenvalues[a])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut eigenvalues = Vec::with_capacity(k);
        let mut loadings = DMatrix::zeros(n, k);

        for (new_col, &old_col) in indices.iter().enumerate() {
            let ev = raw_eigenvalues[old_col].max(0.0); // clamp negative eigenvalues
            eigenvalues.push(ev);
            for row in 0..n {
                loadings[(row, new_col)] = raw_eigenvectors[(row, old_col)];
            }
        }

        // Enforce sign convention: first non-trivial element of each loading
        // should be positive (convention for interpretability)
        for col in 0..k {
            let first_nonzero = (0..n)
                .find(|&row| loadings[(row, col)].abs() > 1e-12)
                .unwrap_or(0);
            if loadings[(first_nonzero, col)] < 0.0 {
                for row in 0..n {
                    loadings[(row, col)] = -loadings[(row, col)];
                }
            }
        }

        // Compute scores: (T-1) x K = centered * loadings
        let scores = &centered * &loadings;

        // Variance explained
        let total_var: f64 = eigenvalues.iter().sum();
        let variance_explained: Vec<f64> = eigenvalues
            .iter()
            .map(|&ev| if total_var > 0.0 { ev / total_var } else { 0.0 })
            .collect();

        let mut cumulative_variance = Vec::with_capacity(k);
        let mut cum = 0.0;
        for &ve in &variance_explained {
            cum += ve;
            cumulative_variance.push(cum);
        }

        Ok(Self {
            eigenvalues,
            loadings,
            scores,
            tenors: panel.tenors.clone(),
            variance_explained,
            cumulative_variance,
            mean_change,
        })
    }

    /// Fit PCA directly from row-major yield changes.
    ///
    /// This is equivalent to reconstructing a pseudo-panel with
    /// [`YieldPanel::from_yield_changes`] and then calling [`Self::fit`].
    ///
    /// # Errors
    /// - Yield-change rows are empty or ragged
    /// - Fewer than two yield-change rows or fewer than two tenors are supplied
    /// - The covariance matrix is degenerate
    pub fn fit_yield_changes(yield_changes: Vec<Vec<f64>>) -> crate::Result<Self> {
        let panel = YieldPanel::from_yield_changes(yield_changes)?;
        Self::fit(&panel)
    }

    /// Number of components extracted (min(T-1, N)).
    #[must_use]
    pub fn num_components(&self) -> usize {
        self.eigenvalues.len()
    }

    /// Eigenvalues in descending order.
    #[must_use]
    pub fn eigenvalues(&self) -> &[f64] {
        &self.eigenvalues
    }

    /// Loading vectors: N x K matrix. Column k is the k-th loading vector.
    #[must_use]
    pub fn loadings(&self) -> &DMatrix<f64> {
        &self.loadings
    }

    /// Loading vector for component k (0-indexed). Length N.
    ///
    /// # Errors
    /// - k >= num_components
    pub fn loading(&self, k: usize) -> crate::Result<DVector<f64>> {
        if k >= self.num_components() {
            return Err(crate::Error::Validation(format!(
                "Component index {k} out of range (have {} components)",
                self.num_components()
            )));
        }
        Ok(self.loadings.column(k).into_owned())
    }

    /// Score time series: (T-1) x K matrix. Column k is the k-th score series.
    #[must_use]
    pub fn scores(&self) -> &DMatrix<f64> {
        &self.scores
    }

    /// Fraction of variance explained by each component.
    #[must_use]
    pub fn variance_explained(&self) -> &[f64] {
        &self.variance_explained
    }

    /// Cumulative fraction of variance explained.
    #[must_use]
    pub fn cumulative_variance(&self) -> &[f64] {
        &self.cumulative_variance
    }

    /// Number of components needed to explain at least `threshold` fraction
    /// of total variance.
    #[must_use]
    pub fn components_for_threshold(&self, threshold: f64) -> usize {
        for (i, &cv) in self.cumulative_variance.iter().enumerate() {
            if cv >= threshold {
                return i + 1;
            }
        }
        self.num_components()
    }

    /// Generate a yield change scenario by shocking principal components.
    ///
    /// Reconstructs a yield change vector as:
    ///   Delta_y = sum_k shocks\[k\] * sqrt(eigenvalue_k) * loading_k
    ///
    /// where shocks are in units of standard deviations.
    ///
    /// # Errors
    /// - shocks length exceeds num_components
    pub fn scenario(&self, shocks: &[f64]) -> crate::Result<Vec<f64>> {
        if shocks.len() > self.num_components() {
            return Err(crate::Error::Validation(format!(
                "Shocks length {} exceeds number of components {}",
                shocks.len(),
                self.num_components()
            )));
        }

        let n = self.tenors.len();
        let mut result = vec![0.0; n];

        for (k, &shock) in shocks.iter().enumerate() {
            let sigma_k = self.eigenvalues[k].max(0.0).sqrt();
            for (i, value) in result.iter_mut().enumerate() {
                *value += shock * sigma_k * self.loadings[(i, k)];
            }
        }

        Ok(result)
    }

    /// Fit PCA from row-major yield changes and shock one component.
    ///
    /// Returns `sigma_shock * sqrt(eigenvalue_k) * loading_k` for the selected
    /// component, preserving the same component-bound checks as the standard
    /// scenario path.
    ///
    /// # Errors
    /// - `n_components` is zero or exceeds the fitted component count
    /// - `component_index >= n_components`
    /// - Any invariant enforced by [`Self::fit_yield_changes`] fails
    pub fn scenario_from_yield_changes(
        yield_changes: Vec<Vec<f64>>,
        component_index: usize,
        sigma_shock: f64,
        n_components: usize,
    ) -> crate::Result<Vec<f64>> {
        let pca = Self::fit_yield_changes(yield_changes)?;
        if n_components == 0 || n_components > pca.num_components() {
            return Err(crate::Error::Validation(format!(
                "n_components must be in [1, {}], got {n_components}",
                pca.num_components()
            )));
        }
        if component_index >= n_components {
            return Err(crate::Error::Validation(format!(
                "component_index {component_index} must be < n_components {n_components}"
            )));
        }

        let mut shocks = vec![0.0_f64; n_components];
        shocks[component_index] = sigma_shock;
        pca.scenario(&shocks)
    }

    /// Reconstruct yield changes from a truncated set of K components.
    ///
    /// # Errors
    /// - num_components == 0 or exceeds available components
    pub fn reconstruct(&self, num_components: usize) -> crate::Result<DMatrix<f64>> {
        if num_components == 0 || num_components > self.num_components() {
            return Err(crate::Error::Validation(format!(
                "num_components must be in [1, {}], got {num_components}",
                self.num_components()
            )));
        }

        let m = self.scores.nrows();
        let n = self.tenors.len();

        // Reconstruct: scores[:, :K] * loadings[:, :K]'
        let scores_k = self.scores.columns(0, num_components);
        let loadings_k = self.loadings.columns(0, num_components);
        let reconstructed = scores_k * loadings_k.transpose();

        // Add back the mean
        let mut result = DMatrix::zeros(m, n);
        for i in 0..m {
            for j in 0..n {
                result[(i, j)] = reconstructed[(i, j)] + self.mean_change[j];
            }
        }

        Ok(result)
    }

    /// Apply a scenario shock to a base yield curve, producing shifted yields.
    ///
    /// # Errors
    /// - base_yields length != N
    /// - shocks length exceeds num_components
    pub fn apply_scenario(&self, base_yields: &[f64], shocks: &[f64]) -> crate::Result<Vec<f64>> {
        let n = self.tenors.len();
        if base_yields.len() != n {
            return Err(crate::Error::Validation(format!(
                "base_yields length {} does not match number of tenors {n}",
                base_yields.len()
            )));
        }

        let delta = self.scenario(shocks)?;
        Ok(base_yields
            .iter()
            .zip(delta.iter())
            .map(|(b, d)| b + d)
            .collect())
    }

    /// Tenor grid.
    #[must_use]
    pub fn tenors(&self) -> &[f64] {
        &self.tenors
    }

    /// Mean yield change vector (length N).
    #[must_use]
    pub fn mean_change(&self) -> &DVector<f64> {
        &self.mean_change
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::DMatrix;

    /// Generate synthetic yield changes from known orthogonal factors.
    ///
    /// Creates a T x N yield level matrix whose first differences are
    /// driven by `num_factors` orthogonal loadings with specified variances.
    fn make_synthetic_panel(
        num_dates: usize,
        tenors: &[f64],
        factor_variances: &[f64],
    ) -> YieldPanel {
        let n = tenors.len();
        let k = factor_variances.len();
        let m = num_dates - 1; // number of yield changes

        // Deterministic "random" scores using sin/cos patterns
        let mut scores = DMatrix::zeros(m, k);
        for t in 0..m {
            for f in 0..k {
                scores[(t, f)] =
                    ((t as f64 + 1.0) * (f as f64 + 1.0) * 0.7).sin() * factor_variances[f].sqrt();
            }
        }

        // Construct orthogonal loadings via a simple scheme
        let mut loadings = DMatrix::zeros(n, k);
        for f in 0..k {
            for i in 0..n {
                loadings[(i, f)] = match f {
                    0 => 1.0 / (n as f64).sqrt(), // level: flat
                    1 => {
                        // slope: linearly decreasing
                        let x = 2.0 * (i as f64) / (n as f64 - 1.0) - 1.0;
                        -x / (n as f64).sqrt()
                    }
                    2 => {
                        // curvature: quadratic
                        let x = 2.0 * (i as f64) / (n as f64 - 1.0) - 1.0;
                        (1.0 - 3.0 * x * x) / (n as f64).sqrt()
                    }
                    _ => 0.0,
                };
            }
        }

        // yield changes = scores * loadings'
        let changes = &scores * loadings.transpose();

        // Integrate to levels (cumsum + base)
        let mut yields = DMatrix::zeros(num_dates, n);
        for j in 0..n {
            yields[(0, j)] = 0.03 + 0.001 * (j as f64); // base curve
        }
        for i in 0..m {
            for j in 0..n {
                yields[(i + 1, j)] = yields[(i, j)] + changes[(i, j)];
            }
        }

        YieldPanel::new(yields, tenors.to_vec(), None).unwrap()
    }

    fn standard_tenors() -> Vec<f64> {
        vec![0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0, 30.0]
    }

    fn matrix_rows(matrix: &DMatrix<f64>) -> Vec<Vec<f64>> {
        (0..matrix.nrows())
            .map(|i| (0..matrix.ncols()).map(|j| matrix[(i, j)]).collect())
            .collect()
    }

    #[test]
    fn pca_fit_basic() {
        let tenors = standard_tenors();
        let panel = make_synthetic_panel(52, &tenors, &[0.01, 0.005, 0.002]);
        let pca = YieldPca::fit(&panel).unwrap();

        assert_eq!(pca.num_components(), tenors.len());
        assert_eq!(pca.tenors().len(), tenors.len());
        assert_eq!(pca.eigenvalues().len(), tenors.len());

        // Eigenvalues should be in descending order
        for i in 1..pca.eigenvalues().len() {
            assert!(
                pca.eigenvalues()[i] <= pca.eigenvalues()[i - 1] + 1e-15,
                "Eigenvalues not descending at index {i}"
            );
        }

        // Variance explained should sum to 1
        let total: f64 = pca.variance_explained().iter().sum();
        assert!((total - 1.0).abs() < 1e-10);

        // Cumulative variance should be monotone
        for i in 1..pca.cumulative_variance().len() {
            assert!(pca.cumulative_variance()[i] >= pca.cumulative_variance()[i - 1] - 1e-15);
        }
    }

    #[test]
    fn fit_yield_changes_matches_panel_fit() {
        let tenors = standard_tenors();
        let panel = make_synthetic_panel(52, &tenors, &[0.01, 0.005, 0.002]);
        let rows = matrix_rows(&panel.yield_changes());

        let from_panel = YieldPca::fit(&panel).unwrap();
        let from_changes = YieldPca::fit_yield_changes(rows).unwrap();

        for (a, b) in from_panel
            .eigenvalues()
            .iter()
            .zip(from_changes.eigenvalues())
        {
            assert!((a - b).abs() < 1e-14);
        }
    }

    #[test]
    fn scenario_from_yield_changes_matches_manual_shock() {
        let tenors = standard_tenors();
        let panel = make_synthetic_panel(52, &tenors, &[0.01, 0.005, 0.002]);
        let rows = matrix_rows(&panel.yield_changes());

        let pca = YieldPca::fit_yield_changes(rows.clone()).unwrap();
        let expected = pca.scenario(&[2.0, 0.0, 0.0]).unwrap();
        let actual = YieldPca::scenario_from_yield_changes(rows, 0, 2.0, 3).unwrap();

        for (a, b) in expected.iter().zip(actual.iter()) {
            assert!((a - b).abs() < 1e-14);
        }
    }

    #[test]
    fn pca_three_factor_explains_most_variance() {
        let tenors = standard_tenors();
        // Generate data with 3 dominant factors
        let panel = make_synthetic_panel(100, &tenors, &[0.01, 0.005, 0.002]);
        let pca = YieldPca::fit(&panel).unwrap();

        // First 3 PCs should explain vast majority
        assert!(
            pca.cumulative_variance()[2] > 0.90,
            "First 3 PCs explain only {:.1}% of variance",
            pca.cumulative_variance()[2] * 100.0
        );
    }

    #[test]
    fn pca_components_for_threshold() {
        let tenors = standard_tenors();
        let panel = make_synthetic_panel(100, &tenors, &[0.01, 0.005, 0.002]);
        let pca = YieldPca::fit(&panel).unwrap();

        let k = pca.components_for_threshold(0.99);
        assert!(k <= pca.num_components());
        assert!(k >= 1);
    }

    #[test]
    fn pca_reconstruction_fidelity() {
        let tenors = standard_tenors();
        let panel = make_synthetic_panel(50, &tenors, &[0.01, 0.005, 0.002]);
        let pca = YieldPca::fit(&panel).unwrap();

        // Full reconstruction should match original changes
        let changes = panel.yield_changes();
        let reconstructed = pca.reconstruct(pca.num_components()).unwrap();

        let m = changes.nrows();
        let n = changes.ncols();
        for i in 0..m {
            for j in 0..n {
                assert!(
                    (changes[(i, j)] - reconstructed[(i, j)]).abs() < 1e-8,
                    "Reconstruction error at ({i}, {j}): original={}, reconstructed={}",
                    changes[(i, j)],
                    reconstructed[(i, j)]
                );
            }
        }
    }

    #[test]
    fn pca_scenario_round_trip() {
        let tenors = standard_tenors();
        let panel = make_synthetic_panel(50, &tenors, &[0.01, 0.005, 0.002]);
        let pca = YieldPca::fit(&panel).unwrap();

        // +1 sigma PC1 shock
        let delta = pca.scenario(&[1.0, 0.0, 0.0]).unwrap();
        assert_eq!(delta.len(), tenors.len());

        // The magnitude should be approximately sqrt(eigenvalue_1)
        let magnitude: f64 = delta.iter().map(|d| d * d).sum::<f64>().sqrt();
        let expected_magnitude = pca.eigenvalues()[0].sqrt();
        assert!(
            (magnitude - expected_magnitude).abs() / expected_magnitude.max(1e-15) < 0.1,
            "Scenario magnitude {magnitude} far from expected {expected_magnitude}"
        );
    }

    #[test]
    fn pca_apply_scenario() {
        let tenors = standard_tenors();
        let panel = make_synthetic_panel(50, &tenors, &[0.01, 0.005, 0.002]);
        let pca = YieldPca::fit(&panel).unwrap();

        let base: Vec<f64> = tenors.iter().map(|&t| 0.03 + 0.002 * t).collect();
        let shocked = pca.apply_scenario(&base, &[2.0, -1.0]).unwrap();

        assert_eq!(shocked.len(), tenors.len());
        // Shocked yields should differ from base
        let max_diff: f64 = base
            .iter()
            .zip(shocked.iter())
            .map(|(b, s)| (b - s).abs())
            .fold(0.0_f64, f64::max);
        assert!(max_diff > 0.0, "Scenario had no effect");
    }

    #[test]
    fn pca_loading_access() {
        let tenors = standard_tenors();
        let panel = make_synthetic_panel(50, &tenors, &[0.01, 0.005, 0.002]);
        let pca = YieldPca::fit(&panel).unwrap();

        let l0 = pca.loading(0).unwrap();
        assert_eq!(l0.len(), tenors.len());

        // Out-of-range should error
        assert!(pca.loading(tenors.len()).is_err());
    }

    #[test]
    fn pca_too_few_observations() {
        let data = DMatrix::from_row_slice(2, 3, &[0.01, 0.02, 0.03, 0.02, 0.03, 0.04]);
        let panel = YieldPanel::new(data, vec![1.0, 2.0, 3.0], None).unwrap();
        assert!(YieldPca::fit(&panel).is_err());
    }

    #[test]
    fn pca_too_few_tenors() {
        let data = DMatrix::from_row_slice(5, 1, &[0.01, 0.02, 0.03, 0.04, 0.05]);
        // Should fail at YieldPanel::new since we need sorted ascending > 0
        // But we also need at least 2 tenors for PCA
        let panel = YieldPanel::new(data, vec![1.0], None);
        // If panel construction succeeds, PCA should fail
        if let Ok(p) = panel {
            assert!(YieldPca::fit(&p).is_err());
        }
    }

    #[test]
    fn pca_scenario_too_many_shocks() {
        let tenors = vec![1.0, 2.0, 5.0];
        let data = DMatrix::from_fn(10, 3, |i, j| {
            0.03 + 0.001 * ((i as f64) * (j as f64 + 1.0) * 0.7).sin()
                + 0.002 * (j as f64)
                + 0.001 * (i as f64)
        });
        let panel = YieldPanel::new(data, tenors, None).unwrap();
        let pca = YieldPca::fit(&panel).unwrap();

        // More shocks than components should error
        assert!(pca.scenario(&[1.0, 2.0, 3.0, 4.0]).is_err());
    }

    #[test]
    fn pca_reconstruct_invalid_components() {
        let tenors = vec![1.0, 2.0, 5.0];
        let data = DMatrix::from_fn(10, 3, |i, j| {
            0.03 + 0.001 * ((i as f64) * (j as f64 + 1.0) * 0.7).sin()
                + 0.002 * (j as f64)
                + 0.001 * (i as f64)
        });
        let panel = YieldPanel::new(data, tenors, None).unwrap();
        let pca = YieldPca::fit(&panel).unwrap();

        assert!(pca.reconstruct(0).is_err());
        assert!(pca.reconstruct(pca.num_components() + 1).is_err());
    }
}
