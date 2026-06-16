//! Correlation structure specification.
//!
//! This module provides correlation specifications that capture:
//! - Asset correlation (intra-pool default correlation)
//! - Prepay-default correlation (typically negative)
//! - Sector correlation (intra vs inter-sector)
//!
//! # Industry Standard Calibrations
//!
//! ## RMBS
//! - Asset correlation: 5-10% (diversified mortgage pools)
//! - Prepay-default correlation: -20% to -40% (refi incentive vs credit)
//!
//! ## CLO
//! - Intra-sector correlation: 25-35%
//! - Inter-sector correlation: 10-15%
//! - Prepay-default correlation: -15% to -25%
//!
//! ## CMBS
//! - Asset correlation: 15-25% (concentrated property types)
//! - Prepay-default correlation: -10% to -20%

use crate::instruments::fixed_income::structured_credit::pricing::stochastic::calibrations::{
    clo_standard, cmbs_standard, rmbs_standard,
};

/// Correlation structure specification.
///
/// Captures the various correlation parameters needed for
/// stochastic structured credit modeling.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "structure", deny_unknown_fields)]
#[non_exhaustive]
pub enum CorrelationStructure {
    /// Flat (homogeneous) correlation structure.
    ///
    /// All assets have the same pairwise correlation.
    Flat {
        /// Asset (default) correlation
        asset_correlation: f64,
        /// Correlation between prepayment and default factors
        prepay_default_correlation: f64,
    },

    /// Sectored correlation structure.
    ///
    /// Different correlation within vs. between sectors.
    Sectored {
        /// Intra-sector asset correlation
        intra_sector: f64,
        /// Inter-sector asset correlation
        inter_sector: f64,
        /// Prepay-default correlation
        prepay_default: f64,
    },

    /// Full correlation matrix.
    ///
    /// Custom correlation structure for bespoke deals.
    Matrix {
        /// Flattened correlation matrix (row-major)
        correlations: Vec<f64>,
        /// Labels for matrix rows/columns
        labels: Vec<String>,
    },
}

impl Default for CorrelationStructure {
    fn default() -> Self {
        CorrelationStructure::Flat {
            asset_correlation: 0.10,
            prepay_default_correlation: -0.20,
        }
    }
}

impl CorrelationStructure {
    /// Create a flat correlation structure without altering supplied values.
    ///
    /// Use [`Self::try_flat`] when ingesting calibration or market-data input
    /// that must fail fast on invalid correlations.
    pub fn flat(asset_correlation: f64, prepay_default_correlation: f64) -> Self {
        CorrelationStructure::Flat {
            asset_correlation,
            prepay_default_correlation,
        }
    }

    /// Create a flat correlation structure and validate supplied values.
    pub fn try_flat(
        asset_correlation: f64,
        prepay_default_correlation: f64,
    ) -> Result<Self, String> {
        let structure = Self::flat(asset_correlation, prepay_default_correlation);
        structure.validate()?;
        Ok(structure)
    }

    /// Create a sectored correlation structure without altering supplied values.
    ///
    /// Use [`Self::try_sectored`] when ingesting calibration or market-data
    /// input that must fail fast on invalid correlations.
    pub fn sectored(intra_sector: f64, inter_sector: f64, prepay_default: f64) -> Self {
        CorrelationStructure::Sectored {
            intra_sector,
            inter_sector,
            prepay_default,
        }
    }

    /// Create a sectored correlation structure and validate supplied values.
    pub fn try_sectored(
        intra_sector: f64,
        inter_sector: f64,
        prepay_default: f64,
    ) -> Result<Self, String> {
        let structure = Self::sectored(intra_sector, inter_sector, prepay_default);
        structure.validate()?;
        Ok(structure)
    }

    /// Create a custom matrix correlation structure without altering supplied values.
    ///
    /// Use [`Self::try_matrix`] when ingesting calibration or market-data input
    /// that must fail fast on invalid matrices.
    pub fn matrix(correlations: Vec<f64>, labels: Vec<String>) -> Self {
        CorrelationStructure::Matrix {
            correlations,
            labels,
        }
    }

    /// Create a custom matrix correlation structure and validate supplied values.
    pub fn try_matrix(correlations: Vec<f64>, labels: Vec<String>) -> Result<Self, String> {
        let structure = Self::matrix(correlations, labels);
        structure.validate()?;
        Ok(structure)
    }

    /// RMBS standard correlation structure.
    ///
    /// Low asset correlation (diversified pool), moderate negative
    /// prepay-default correlation (refi vs credit).
    pub fn rmbs_standard() -> Self {
        let calibration = rmbs_standard();
        CorrelationStructure::Flat {
            asset_correlation: calibration.default_correlation,
            prepay_default_correlation: -0.30,
        }
    }

    /// CLO standard correlation structure.
    ///
    /// Sectored structure with higher intra-sector correlation.
    pub fn clo_standard() -> Self {
        let calibration = clo_standard();
        CorrelationStructure::Sectored {
            intra_sector: calibration.default_correlation + 0.10,
            inter_sector: 0.10,
            prepay_default: -0.20,
        }
    }

    /// CMBS standard correlation structure.
    ///
    /// Moderate correlation (concentrated property types).
    pub fn cmbs_standard() -> Self {
        let calibration = cmbs_standard();
        CorrelationStructure::Flat {
            asset_correlation: calibration.default_correlation + 0.05,
            prepay_default_correlation: -0.15,
        }
    }

    /// ABS auto loan standard correlation structure.
    pub fn abs_auto_standard() -> Self {
        CorrelationStructure::Flat {
            asset_correlation: 0.08,
            prepay_default_correlation: -0.10,
        }
    }

    /// Get the asset (default) correlation.
    ///
    /// For sectored structures, returns the average correlation.
    pub fn asset_correlation(&self) -> f64 {
        match self {
            CorrelationStructure::Flat {
                asset_correlation, ..
            } => *asset_correlation,
            CorrelationStructure::Sectored {
                intra_sector,
                inter_sector,
                ..
            } => {
                // Average correlation (simplified)
                0.5 * intra_sector + 0.5 * inter_sector
            }
            CorrelationStructure::Matrix {
                correlations,
                labels,
            } => {
                // Average off-diagonal correlation
                let n = labels.len();
                if n < 2 {
                    return 0.0;
                }
                let mut sum = 0.0;
                let mut count = 0;
                for i in 0..n {
                    for j in 0..n {
                        if i != j {
                            if let Some(value) = correlations.get(i * n + j) {
                                sum += *value;
                                count += 1;
                            }
                        }
                    }
                }
                if count > 0 {
                    sum / count as f64
                } else {
                    0.0
                }
            }
        }
    }

    /// Get the prepay-default correlation.
    pub fn prepay_default_correlation(&self) -> f64 {
        match self {
            CorrelationStructure::Flat {
                prepay_default_correlation,
                ..
            } => *prepay_default_correlation,
            CorrelationStructure::Sectored { prepay_default, .. } => *prepay_default,
            CorrelationStructure::Matrix { .. } => -0.20, // Default assumption
        }
    }

    /// Get the intra-sector correlation (for sectored structures).
    pub fn intra_sector_correlation(&self) -> Option<f64> {
        match self {
            CorrelationStructure::Sectored { intra_sector, .. } => Some(*intra_sector),
            _ => None,
        }
    }

    /// Get the inter-sector correlation (for sectored structures).
    pub fn inter_sector_correlation(&self) -> Option<f64> {
        match self {
            CorrelationStructure::Sectored { inter_sector, .. } => Some(*inter_sector),
            _ => None,
        }
    }

    /// Check if this is a flat correlation structure.
    pub fn is_flat(&self) -> bool {
        matches!(self, CorrelationStructure::Flat { .. })
    }

    /// Check if this is a sectored correlation structure.
    pub fn is_sectored(&self) -> bool {
        matches!(self, CorrelationStructure::Sectored { .. })
    }

    /// Get correlation between two assets.
    ///
    /// # Arguments
    /// * `i` - First asset index
    /// * `j` - Second asset index
    /// * `same_sector` - Whether assets are in the same sector
    pub fn pairwise_correlation(&self, i: usize, j: usize, same_sector: bool) -> f64 {
        if i == j {
            return 1.0;
        }

        match self {
            CorrelationStructure::Flat {
                asset_correlation, ..
            } => *asset_correlation,
            CorrelationStructure::Sectored {
                intra_sector,
                inter_sector,
                ..
            } => {
                if same_sector {
                    *intra_sector
                } else {
                    *inter_sector
                }
            }
            CorrelationStructure::Matrix {
                correlations,
                labels,
            } => {
                let n = labels.len();
                if i < n && j < n {
                    correlations.get(i * n + j).copied().unwrap_or(0.0)
                } else {
                    0.0
                }
            }
        }
    }

    /// Get the prepayment factor loading.
    ///
    /// Derived from prepay-default correlation and asset correlation.
    /// Factor loading represents how much prepayment responds to systematic factor.
    pub fn prepay_factor_loading(&self) -> f64 {
        let asset_corr = self.asset_correlation();
        let prepay_default_corr = self.prepay_default_correlation();

        // Factor loading from correlation:
        // ρ = β_prepay * β_default (approximately)
        // Assuming default factor loading ≈ sqrt(asset_correlation)
        // prepay_factor_loading = prepay_default_correlation / sqrt(asset_correlation)
        let default_loading = asset_corr.sqrt();
        if default_loading > 0.001 {
            // Prepay loading with opposite sign (prepays typically anti-correlate with defaults)
            (prepay_default_corr / default_loading).clamp(-1.0, 1.0)
        } else {
            // Low default correlation → default to weak prepay factor loading
            -0.3
        }
    }

    /// Get the default factor loading.
    ///
    /// Derived from asset correlation using single-factor model assumption.
    /// Factor loading β such that ρ ≈ β².
    pub fn default_factor_loading(&self) -> f64 {
        // For Gaussian copula: correlation ≈ factor_loading²
        let asset_corr = self.asset_correlation();
        asset_corr.sqrt()
    }

    /// Bump asset correlation by the given amount.
    ///
    /// For flat structures, bumps the single asset correlation.
    /// For sectored structures, bumps intra-sector by `delta` and inter-sector by `delta * 0.5`.
    /// For matrix structures, bumps all off-diagonal elements.
    ///
    /// # Arguments
    /// * `delta` - Amount to add to correlation (clamped to valid range)
    pub fn bump_asset(&self, delta: f64) -> Self {
        self.bump_asset_with_clamp_info(delta).0
    }

    /// Bump asset correlation and report whether any underlying field was
    /// clamped.
    ///
    /// This is the primitive used by scenario adapters that need to surface a
    /// clamp warning. The clamp is detected at the underlying field level
    /// (`asset_correlation`, `intra_sector`, `inter_sector`, or each off-diagonal
    /// matrix entry) rather than at the aggregated
    /// [`Self::asset_correlation`] value, which for `Sectored` averages
    /// intra/inter and would not move by the requested `delta` even without
    /// clamping.
    ///
    /// # Returns
    ///
    /// `(new_structure, clamp_info)`. `clamp_info` is `Some` iff at least one
    /// underlying field required clamping; the string describes which field(s)
    /// were clamped and to what value.
    pub fn bump_asset_with_clamp_info(&self, delta: f64) -> (Self, Option<String>) {
        const LO: f64 = 0.0;
        const HI: f64 = 0.99;
        match self {
            CorrelationStructure::Flat {
                asset_correlation,
                prepay_default_correlation,
            } => {
                let target = asset_correlation + delta;
                let new_asset = target.clamp(LO, HI);
                let clamp_info = if (new_asset - target).abs() > 1e-12 {
                    Some(format!(
                        "asset_correlation clamped to {new_asset:.4} (requested {target:.4})"
                    ))
                } else {
                    None
                };
                (
                    CorrelationStructure::flat(new_asset, *prepay_default_correlation),
                    clamp_info,
                )
            }
            CorrelationStructure::Sectored {
                intra_sector,
                inter_sector,
                prepay_default,
            } => {
                let intra_target = intra_sector + delta;
                let inter_target = inter_sector + delta * 0.5;
                let new_intra = intra_target.clamp(LO, HI);
                let new_inter = inter_target.clamp(LO, HI);
                let intra_clamped = (new_intra - intra_target).abs() > 1e-12;
                let inter_clamped = (new_inter - inter_target).abs() > 1e-12;
                let clamp_info = match (intra_clamped, inter_clamped) {
                    (false, false) => None,
                    (true, false) => Some(format!(
                        "intra_sector clamped to {new_intra:.4} (requested {intra_target:.4})"
                    )),
                    (false, true) => Some(format!(
                        "inter_sector clamped to {new_inter:.4} (requested {inter_target:.4})"
                    )),
                    (true, true) => Some(format!(
                        "intra_sector clamped to {new_intra:.4} (requested {intra_target:.4}) \
                         and inter_sector clamped to {new_inter:.4} (requested {inter_target:.4})"
                    )),
                };
                (
                    CorrelationStructure::sectored(new_intra, new_inter, *prepay_default),
                    clamp_info,
                )
            }
            CorrelationStructure::Matrix {
                correlations,
                labels,
            } => {
                let n = labels.len();
                let mut new_corrs = correlations.clone();
                let mut clamp_count: usize = 0;
                for i in 0..n {
                    for j in 0..n {
                        if i != j {
                            let idx = i * n + j;
                            if let Some(rho) = new_corrs.get_mut(idx) {
                                let target = *rho + delta;
                                let clamped = target.clamp(LO, HI);
                                if (clamped - target).abs() > 1e-12 {
                                    clamp_count += 1;
                                }
                                *rho = clamped;
                            }
                        }
                    }
                }
                let clamp_info = if clamp_count > 0 {
                    Some(format!(
                        "{clamp_count} matrix off-diagonal entr{} clamped to [{LO}, {HI}]",
                        if clamp_count == 1 { "y" } else { "ies" }
                    ))
                } else {
                    None
                };
                (
                    CorrelationStructure::Matrix {
                        correlations: new_corrs,
                        labels: labels.clone(),
                    },
                    clamp_info,
                )
            }
        }
    }

    /// Validate the correlation structure.
    ///
    /// For `Flat` and `Sectored` variants, checks that asset correlations are
    /// within \[0, 0.99\] and prepay-default correlations are within
    /// \[-0.99, 0.99\]. For `Matrix`, verifies that:
    /// - diagonal elements are 1.0
    /// - the matrix is symmetric
    /// - all entries are within \[-1, 1\]
    ///
    /// # Errors
    ///
    /// Returns a description of the first validation failure found.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            CorrelationStructure::Flat {
                asset_correlation,
                prepay_default_correlation,
            } => {
                validate_correlation_value(*asset_correlation, 0.0, 0.99, "asset_correlation")?;
                validate_correlation_value(
                    *prepay_default_correlation,
                    -0.99,
                    0.99,
                    "prepay_default_correlation",
                )?;
                Ok(())
            }
            CorrelationStructure::Sectored {
                intra_sector,
                inter_sector,
                prepay_default,
            } => {
                validate_correlation_value(*intra_sector, 0.0, 0.99, "intra_sector")?;
                validate_correlation_value(*inter_sector, 0.0, 0.99, "inter_sector")?;
                validate_correlation_value(*prepay_default, -0.99, 0.99, "prepay_default")?;
                Ok(())
            }
            CorrelationStructure::Matrix {
                correlations,
                labels,
            } => {
                let n = labels.len();
                let expected = n * n;
                if correlations.len() != expected {
                    return Err(format!(
                        "Correlation matrix size mismatch: expected {}×{}={}, got {}",
                        n,
                        n,
                        expected,
                        correlations.len()
                    ));
                }
                for i in 0..n {
                    let diag = correlations[i * n + i];
                    if (diag - 1.0).abs() > 1e-10 {
                        return Err(format!("Diagonal element [{i},{i}] = {diag}, expected 1.0"));
                    }
                    for j in (i + 1)..n {
                        let rho_ij = correlations[i * n + j];
                        let rho_ji = correlations[j * n + i];
                        validate_correlation_value(rho_ij, -1.0, 1.0, &format!("[{i},{j}]"))?;
                        if (rho_ij - rho_ji).abs() > 1e-10 {
                            return Err(format!(
                                "Matrix not symmetric: [{i},{j}]={rho_ij} != [{j},{i}]={rho_ji}"
                            ));
                        }
                    }
                }

                // Verify positive semi-definiteness via Cholesky decomposition.
                // If the matrix is not PSD, the Cholesky factor will encounter a
                // negative diagonal, indicating the matrix cannot represent a valid
                // correlation structure.
                let mut l = vec![0.0_f64; n * n];
                for j in 0..n {
                    let mut sum = 0.0;
                    for k in 0..j {
                        sum += l[j * n + k] * l[j * n + k];
                    }
                    let diag = correlations[j * n + j] - sum;
                    if diag < -1e-10 {
                        return Err(format!(
                            "Correlation matrix is not positive semi-definite \
                             (negative diagonal {diag:.2e} at [{j},{j}])"
                        ));
                    }
                    l[j * n + j] = diag.max(0.0).sqrt();
                    for i in (j + 1)..n {
                        let mut s = 0.0;
                        for k in 0..j {
                            s += l[i * n + k] * l[j * n + k];
                        }
                        l[i * n + j] = if l[j * n + j] > 1e-15 {
                            (correlations[i * n + j] - s) / l[j * n + j]
                        } else {
                            0.0
                        };
                    }
                }

                Ok(())
            }
        }
    }

    /// Bump prepay-default correlation by the given amount.
    ///
    /// # Arguments
    /// * `delta` - Amount to add to correlation (clamped to [-0.99, 0.99])
    pub fn bump_prepay_default(&self, delta: f64) -> Self {
        self.bump_prepay_default_with_clamp_info(delta).0
    }

    /// Bump prepay-default correlation and report whether the underlying
    /// field was clamped. See [`Self::bump_asset_with_clamp_info`] for
    /// rationale.
    pub fn bump_prepay_default_with_clamp_info(&self, delta: f64) -> (Self, Option<String>) {
        const LO: f64 = -0.99;
        const HI: f64 = 0.99;
        match self {
            CorrelationStructure::Flat {
                asset_correlation,
                prepay_default_correlation,
            } => {
                let target = prepay_default_correlation + delta;
                let new_pd = target.clamp(LO, HI);
                let clamp_info = if (new_pd - target).abs() > 1e-12 {
                    Some(format!(
                        "prepay_default_correlation clamped to {new_pd:.4} \
                         (requested {target:.4})"
                    ))
                } else {
                    None
                };
                (
                    CorrelationStructure::flat(*asset_correlation, new_pd),
                    clamp_info,
                )
            }
            CorrelationStructure::Sectored {
                intra_sector,
                inter_sector,
                prepay_default,
            } => {
                let target = prepay_default + delta;
                let new_pd = target.clamp(LO, HI);
                let clamp_info = if (new_pd - target).abs() > 1e-12 {
                    Some(format!(
                        "prepay_default clamped to {new_pd:.4} (requested {target:.4})"
                    ))
                } else {
                    None
                };
                (
                    CorrelationStructure::sectored(*intra_sector, *inter_sector, new_pd),
                    clamp_info,
                )
            }
            CorrelationStructure::Matrix { .. } => {
                // Matrix structure doesn't have explicit prepay-default correlation;
                // the shock is a no-op rather than a clamp. Surface it to the caller
                // so the scenario report can distinguish "applied" from "no-op".
                (
                    self.clone(),
                    Some(
                        "CorrelationStructure::Matrix has no prepay_default field; \
                         bump_prepay_default is a no-op for matrix structures"
                            .into(),
                    ),
                )
            }
        }
    }
}

/// Validate that a single correlation value is within an inclusive range.
fn validate_correlation_value(rho: f64, lo: f64, hi: f64, name: &str) -> Result<(), String> {
    if !(lo..=hi).contains(&rho) {
        return Err(format!(
            "Correlation {name} = {rho} outside valid range [{lo}, {hi}]"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_correlation() {
        let corr = CorrelationStructure::flat(0.10, -0.30);

        assert!((corr.asset_correlation() - 0.10).abs() < 1e-10);
        assert!((corr.prepay_default_correlation() - (-0.30)).abs() < 1e-10);
        assert!(corr.is_flat());
    }

    #[test]
    fn test_sectored_correlation() {
        let corr = CorrelationStructure::sectored(0.30, 0.10, -0.20);

        assert!(corr.is_sectored());
        assert_eq!(corr.intra_sector_correlation(), Some(0.30));
        assert_eq!(corr.inter_sector_correlation(), Some(0.10));
    }

    #[test]
    fn test_pairwise_correlation() {
        let sectored = CorrelationStructure::sectored(0.30, 0.10, -0.20);

        // Same sector
        assert!((sectored.pairwise_correlation(0, 1, true) - 0.30).abs() < 1e-10);
        // Different sector
        assert!((sectored.pairwise_correlation(0, 1, false) - 0.10).abs() < 1e-10);
        // Same asset
        assert!((sectored.pairwise_correlation(0, 0, true) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_standard_calibrations() {
        let rmbs = CorrelationStructure::rmbs_standard();
        assert!((rmbs.asset_correlation() - 0.05).abs() < 1e-10);
        assert!(rmbs.prepay_default_correlation() < 0.0);

        let clo = CorrelationStructure::clo_standard();
        assert!(clo.is_sectored());
        let intra = clo
            .intra_sector_correlation()
            .expect("CLO should have intra-sector correlation");
        let inter = clo
            .inter_sector_correlation()
            .expect("CLO should have inter-sector correlation");
        assert!(intra > inter);

        let cmbs = CorrelationStructure::cmbs_standard();
        assert!(cmbs.asset_correlation() > rmbs.asset_correlation());
    }

    #[test]
    fn test_constructors_preserve_invalid_input() {
        let corr = CorrelationStructure::flat(1.5, -1.5);

        assert_eq!(corr.asset_correlation(), 1.5);
        assert_eq!(corr.prepay_default_correlation(), -1.5);
        assert!(corr.validate().is_err());

        let sectored = CorrelationStructure::sectored(1.5, -0.1, -1.5);
        assert_eq!(sectored.intra_sector_correlation(), Some(1.5));
        assert_eq!(sectored.inter_sector_correlation(), Some(-0.1));
        assert!(sectored.validate().is_err());

        let matrix = CorrelationStructure::matrix(
            vec![1.0, 0.2, 0.2],
            vec!["A".to_string(), "B".to_string()],
        );
        let result = matrix.validate();
        assert!(result.is_err());
        assert!(result
            .expect_err("expected validation error for wrong-sized matrix")
            .contains("size mismatch"));
        assert!(matrix.bump_asset(0.1).validate().is_err());
    }

    #[test]
    fn test_try_constructors_reject_invalid_input() {
        assert!(CorrelationStructure::try_flat(1.5, -0.3).is_err());
        assert!(CorrelationStructure::try_sectored(0.3, -0.1, -0.2).is_err());
        assert!(CorrelationStructure::try_matrix(
            vec![1.0, 0.2, 0.2],
            vec!["A".to_string(), "B".to_string()],
        )
        .is_err());
    }

    #[test]
    fn test_default() {
        let corr = CorrelationStructure::default();
        assert!(corr.is_flat());
        assert!(corr.asset_correlation() > 0.0);
    }

    #[test]
    fn test_bump_asset_flat() {
        let corr = CorrelationStructure::flat(0.20, -0.30);
        let bumped = corr.bump_asset(0.05);

        assert!((bumped.asset_correlation() - 0.25).abs() < 1e-10);
        assert!((bumped.prepay_default_correlation() - (-0.30)).abs() < 1e-10);
    }

    #[test]
    fn test_bump_asset_sectored() {
        let corr = CorrelationStructure::sectored(0.30, 0.10, -0.20);
        let bumped = corr.bump_asset(0.10);

        // Intra bumps by full delta
        assert!(
            (bumped
                .intra_sector_correlation()
                .expect("should be sectored")
                - 0.40)
                .abs()
                < 1e-10
        );
        // Inter bumps by half delta
        assert!(
            (bumped
                .inter_sector_correlation()
                .expect("should be sectored")
                - 0.15)
                .abs()
                < 1e-10
        );
    }

    #[test]
    fn test_bump_asset_clamping() {
        let corr = CorrelationStructure::flat(0.95, -0.30);
        let bumped = corr.bump_asset(0.10);

        // Should clamp to 0.99
        assert!(bumped.asset_correlation() <= 0.99);
    }

    #[test]
    fn test_bump_prepay_default() {
        let corr = CorrelationStructure::flat(0.20, -0.30);
        let bumped = corr.bump_prepay_default(0.10);

        assert!((bumped.asset_correlation() - 0.20).abs() < 1e-10);
        assert!((bumped.prepay_default_correlation() - (-0.20)).abs() < 1e-10);
    }

    #[test]
    fn test_bump_prepay_default_clamping() {
        let corr = CorrelationStructure::flat(0.20, -0.95);
        let bumped = corr.bump_prepay_default(-0.10);

        // Should clamp to -0.99
        assert!(bumped.prepay_default_correlation() >= -0.99);
    }

    #[test]
    fn test_validate_flat_ok() {
        let corr = CorrelationStructure::flat(0.20, -0.30);
        assert!(corr.validate().is_ok());
    }

    #[test]
    fn test_validate_matrix_ok() {
        let corr = CorrelationStructure::matrix(
            vec![1.0, 0.5, 0.5, 1.0],
            vec!["A".to_string(), "B".to_string()],
        );
        assert!(corr.validate().is_ok());
    }

    #[test]
    fn test_validate_matrix_diagonal_not_one() {
        // Bypass constructor fallback by building directly
        let corr = CorrelationStructure::Matrix {
            correlations: vec![0.9, 0.5, 0.5, 1.0],
            labels: vec!["A".to_string(), "B".to_string()],
        };
        let result = corr.validate();
        assert!(result.is_err());
        assert!(result
            .expect_err("expected validation error for non-unit diagonal")
            .contains("Diagonal"));
    }

    #[test]
    fn test_validate_matrix_not_symmetric() {
        let corr = CorrelationStructure::Matrix {
            correlations: vec![1.0, 0.5, 0.3, 1.0],
            labels: vec!["A".to_string(), "B".to_string()],
        };
        let result = corr.validate();
        assert!(result.is_err());
        assert!(result
            .expect_err("expected validation error for non-symmetric matrix")
            .contains("symmetric"));
    }

    #[test]
    fn test_matrix_asset_correlation_uses_label_count() {
        let corr = CorrelationStructure::Matrix {
            correlations: vec![1.0, 0.3, 0.5],
            labels: vec!["A".to_string(), "B".to_string()],
        };

        assert!((corr.asset_correlation() - 0.4).abs() < 1e-10);
    }
}
