//! Trait abstractions for portfolio factor risk decomposition engines.

use super::types::RiskDecomposition;
use crate::sensitivity::SensitivityMatrix;
use finstack_factor_model::{FactorCovarianceMatrix, RiskMeasure};

/// Common interface for engines that decompose portfolio risk into factor contributions.
pub trait RiskDecomposer: Send + Sync {
    /// Decompose the requested portfolio risk measure into factor and residual components.
    ///
    /// `sensitivities` must already reflect any position sizing or weighting applied by the
    /// upstream sensitivity engine. Implementations should therefore treat each row as aligned
    /// with `sensitivities.position_ids()` and must not re-apply portfolio weights.
    ///
    /// # Arguments
    ///
    /// * `sensitivities` - Weighted position-factor sensitivity matrix.
    /// * `covariance` - Factor covariance matrix aligned to the same factor order.
    /// * `measure` - Risk measure to decompose.
    ///
    /// # Returns
    ///
    /// Factor and residual risk decomposition in the units implied by `measure`.
    ///
    /// # Errors
    ///
    /// Implementations should return an error when factor axes are inconsistent,
    /// the covariance matrix is invalid, or the requested measure cannot be evaluated.
    fn decompose(
        &self,
        sensitivities: &SensitivityMatrix,
        covariance: &FactorCovarianceMatrix,
        measure: &RiskMeasure,
    ) -> finstack_core::Result<RiskDecomposition>;
}
