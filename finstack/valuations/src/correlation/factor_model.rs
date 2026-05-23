//! Factor models for correlated behavior in credit portfolios.
//!
//! **Naming note:** This module's [`FactorModelKind`] enum is distinct from the
//! portfolio-risk factor framework in [`finstack_core::factor_model`].
//! Here, "factor model" refers to *latent-variable models* used to generate
//! correlated default/prepayment events in Monte Carlo simulations (the
//! Gaussian copula family). The core/portfolio factor model instead provides
//! market-risk factor definitions, covariance, and sensitivity-based Euler
//! risk decomposition.
//!
//! Factor models drive the correlation between prepayment and default events
//! through common systematic factors. This module provides:
//!
//! - [`FactorModelKind`] enum: Closed dispatch over the concrete factor models
//! - [`SingleFactorModel`]: One-factor model (common market factor)
//! - [`TwoFactorModel`]: Two-factor model (prepayment + credit factors)
//! - [`MultiFactorModel`]: N-factor model with custom correlation matrix
//!
//! # Mathematical Framework
//!
//! For entity i, the latent variable depends on factors:
//! ```text
//! Aᵢ = β₁·Z₁ + β₂·Z₂ + ... + γᵢ·εᵢ
//! ```
//!
//! where:
//! - Zₖ are systematic factors (standard normal)
//! - βₖ are factor loadings
//! - εᵢ is the idiosyncratic component
//! - γᵢ = √(1 - Σβₖ²) ensures Var(Aᵢ) = 1
//!
//! # Correlation Matrix Requirements
//!
//! For `MultiFactorModel`, the correlation matrix must be:
//! - **Symmetric**: ρᵢⱼ = ρⱼᵢ
//! - **Unit diagonal**: ρᵢᵢ = 1
//! - **Positive semi-definite**: All eigenvalues ≥ 0 (verified via Cholesky)
//!
//! # References
//!
//! - Factor-model and covariance interpretation:
//!   `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
//! - Portfolio-risk and dependence context:
//!   `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`

use crate::correlation::error::{Error, Result};
use finstack_analytics::correlation::validate_correlation_matrix;
use finstack_core::math::linalg::{cholesky_correlation, CholeskyError, CorrelationFactor};

/// Tolerance for correlation matrix validation.
#[allow(dead_code)]
const CORRELATION_TOLERANCE: f64 = 1e-10;

/// Perform Cholesky decomposition of a correlation matrix using diagonal pivoting.
/// Returns a [`CorrelationFactor`] that holds the lower triangular factor in the
/// **original variable ordering** and exposes the effective numerical rank. The
/// pivoted algorithm handles near-singular and positive-semidefinite matrices
/// gracefully rather than rejecting them with an absolute threshold.
///
/// # Arguments
/// * `matrix` - Flattened row-major correlation/covariance matrix
/// * `n` - Matrix dimension
///
/// # Returns
/// [`CorrelationFactor`] with unpermuted lower-triangular L, or error if the matrix
/// is indefinite.
///
/// # Errors
/// Returns [`crate::correlation::Error`] if the flattened matrix shape is wrong or
/// the matrix is not positive semidefinite.
pub fn cholesky_decompose(matrix: &[f64], n: usize) -> Result<CorrelationFactor> {
    if matrix.len() != n * n {
        return Err(Error::InvalidSize {
            expected: n,
            actual: matrix.len(),
        });
    }

    match cholesky_correlation(matrix, n) {
        Ok(factor) => Ok(factor),
        Err(CholeskyError::NotPositiveDefinite { row, .. }) => {
            Err(Error::NotPositiveSemiDefinite { row })
        }
        Err(CholeskyError::DimensionMismatch { expected, actual }) => {
            Err(Error::InvalidSize { expected, actual })
        }
        // cholesky_correlation only emits NotPositiveDefinite and DimensionMismatch.
        Err(_) => Err(Error::NotPositiveSemiDefinite { row: 0 }),
    }
}

/// Closed enum over the concrete factor models for correlated behavior.
///
/// Replaces a former `FactorModel` trait object: the set of factor models is
/// fixed and small, so a `match`-dispatched enum avoids the heap allocation
/// and dynamic dispatch of `Box<dyn FactorModel>` while keeping the same
/// six-method query surface.
#[derive(Debug, Clone)]
pub enum FactorModelKind {
    /// Single common-market-factor model.
    Single(SingleFactorModel),
    /// Two-factor prepayment/credit model.
    Two(TwoFactorModel),
    /// N-factor model with a custom correlation matrix.
    Multi(MultiFactorModel),
}

impl FactorModelKind {
    /// Number of factors in the model.
    ///
    /// # Returns
    ///
    /// The number of systematic factors in the model.
    #[must_use]
    pub fn num_factors(&self) -> usize {
        match self {
            FactorModelKind::Single(m) => m.num_factors(),
            FactorModelKind::Two(m) => m.num_factors(),
            FactorModelKind::Multi(m) => m.num_factors(),
        }
    }

    /// Get the factor correlation matrix (flattened row-major).
    ///
    /// For n factors, returns n×n values where element `matrix[i,j] = correlation(Zᵢ, Zⱼ)`.
    ///
    /// # Returns
    ///
    /// The factor correlation matrix in row-major order.
    #[must_use]
    pub fn correlation_matrix(&self) -> &[f64] {
        match self {
            FactorModelKind::Single(m) => &m.correlation_matrix,
            FactorModelKind::Two(m) => &m.correlation_matrix,
            FactorModelKind::Multi(m) => &m.correlation_matrix,
        }
    }

    /// Get factor volatilities.
    ///
    /// # Returns
    ///
    /// The factor volatilities in decimal form, aligned with the factor ordering.
    #[must_use]
    pub fn volatilities(&self) -> &[f64] {
        match self {
            FactorModelKind::Single(m) => &m.volatilities,
            FactorModelKind::Two(m) => &m.volatilities,
            FactorModelKind::Multi(m) => &m.volatilities,
        }
    }

    /// Get factor names for reporting.
    ///
    /// # Returns
    ///
    /// Human-readable factor names aligned with [`Self::volatilities`].
    #[must_use]
    pub fn factor_names(&self) -> Vec<&'static str> {
        match self {
            FactorModelKind::Single(_) => vec!["Market"],
            FactorModelKind::Two(_) => vec!["Prepayment", "Credit"],
            FactorModelKind::Multi(m) => vec!["Factor"; m.num_factors],
        }
    }

    /// Model name for diagnostics.
    ///
    /// # Returns
    ///
    /// A static human-readable model name.
    #[must_use]
    pub fn model_name(&self) -> &'static str {
        match self {
            FactorModelKind::Single(_) => "Single Factor Model",
            FactorModelKind::Two(_) => "Two-Factor Prepay-Credit Model",
            FactorModelKind::Multi(_) => "Multi-Factor Model",
        }
    }

    /// Compute a single factor value given one standard normal draw.
    ///
    /// **Important:** For multi-factor models this returns only the *diagonal*
    /// Cholesky contribution `L[i,i] * z * vol[i]`, ignoring cross-factor
    /// correlation.  To generate properly correlated factor vectors, use
    /// [`MultiFactorModel::generate_correlated_factors`] instead.
    ///
    /// # Arguments
    ///
    /// * `factor_index` - Index of the factor to evaluate.
    /// * `z` - One standard-normal draw for that diagonal contribution.
    ///
    /// # Returns
    ///
    /// The requested factor's diagonal contribution, scaled by its volatility.
    #[must_use]
    pub fn diagonal_factor_contribution(&self, factor_index: usize, z: f64) -> f64 {
        match self {
            FactorModelKind::Single(m) => z * m.volatility,
            FactorModelKind::Two(m) => match factor_index {
                // For the 2×2 correlation matrix: L[0,0] = 1, L[1,1] = √(1 − ρ²).
                0 => z * m.prepay_vol,
                1 => m.cholesky_l11 * z * m.credit_vol,
                _ => 0.0,
            },
            FactorModelKind::Multi(m) => {
                if factor_index < m.num_factors {
                    let n = m.num_factors;
                    let l_ii = m.cholesky_factor.factor_matrix()[factor_index * n + factor_index];
                    z * l_ii * m.volatilities[factor_index]
                } else {
                    0.0
                }
            }
        }
    }
}

/// Factor model specification for configuration and serialization.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", deny_unknown_fields)]
#[allow(clippy::enum_variant_names)]
#[non_exhaustive]
pub enum FactorSpec {
    /// Single factor model (common market factor).
    SingleFactor {
        /// Factor volatility (std dev of innovations)
        volatility: f64,
        /// Mean reversion speed (0 = random walk)
        mean_reversion: f64,
    },

    /// Two-factor model for prepayment and credit.
    TwoFactor {
        /// Prepayment factor volatility
        prepay_vol: f64,
        /// Credit factor volatility
        credit_vol: f64,
        /// Correlation between prepayment and credit factors
        correlation: f64,
    },

    /// Multi-factor model with custom correlation matrix.
    MultiFactor {
        /// Number of factors
        num_factors: usize,
        /// Factor volatilities
        volatilities: Vec<f64>,
        /// Correlation matrix (flattened row-major)
        correlations: Vec<f64>,
    },
}

impl Default for FactorSpec {
    fn default() -> Self {
        FactorSpec::SingleFactor {
            volatility: 1.0,
            mean_reversion: 0.0,
        }
    }
}

impl FactorSpec {
    /// Create a single factor specification.
    ///
    /// # Arguments
    /// * `volatility` - Factor volatility, clamped to [0.01, 2.0]
    /// * `mean_reversion` - Mean reversion speed, clamped to [0.0, 10.0]
    ///
    /// # Returns
    ///
    /// A [`FactorSpec::SingleFactor`] configuration.
    #[must_use]
    pub fn single_factor(volatility: f64, mean_reversion: f64) -> Self {
        FactorSpec::SingleFactor {
            volatility: volatility.clamp(0.01, 2.0),
            mean_reversion: mean_reversion.clamp(0.0, 10.0),
        }
    }

    /// Create a two-factor specification.
    ///
    /// # Arguments
    /// * `prepay_vol` - Prepayment factor volatility, clamped to [0.01, 2.0]
    /// * `credit_vol` - Credit factor volatility, clamped to [0.01, 2.0]
    /// * `correlation` - Correlation between factors, clamped to [-0.99, 0.99]
    ///
    /// # Returns
    ///
    /// A [`FactorSpec::TwoFactor`] configuration.
    #[must_use]
    pub fn two_factor(prepay_vol: f64, credit_vol: f64, correlation: f64) -> Self {
        FactorSpec::TwoFactor {
            prepay_vol: prepay_vol.clamp(0.01, 2.0),
            credit_vol: credit_vol.clamp(0.01, 2.0),
            correlation: correlation.clamp(-0.99, 0.99),
        }
    }

    /// Build a factor model from this specification.
    ///
    /// # Returns
    ///
    /// A [`FactorModelKind`] variant matching the specification.
    #[must_use]
    pub fn build(&self) -> FactorModelKind {
        match self {
            FactorSpec::SingleFactor {
                volatility,
                mean_reversion,
            } => FactorModelKind::Single(SingleFactorModel::new(*volatility, *mean_reversion)),
            FactorSpec::TwoFactor {
                prepay_vol,
                credit_vol,
                correlation,
            } => FactorModelKind::Two(TwoFactorModel::new(*prepay_vol, *credit_vol, *correlation)),
            FactorSpec::MultiFactor {
                num_factors,
                volatilities,
                correlations,
            } => FactorModelKind::Multi(MultiFactorModel::new_or_identity(
                *num_factors,
                volatilities.clone(),
                correlations.clone(),
            )),
        }
    }

    /// Get the number of factors.
    ///
    /// # Returns
    ///
    /// The factor count implied by the specification.
    pub fn num_factors(&self) -> usize {
        match self {
            FactorSpec::SingleFactor { .. } => 1,
            FactorSpec::TwoFactor { .. } => 2,
            FactorSpec::MultiFactor { num_factors, .. } => *num_factors,
        }
    }
}

/// Single-factor model (common market factor).
///
/// Models all correlation through a single systematic factor.
/// Suitable for simple correlation structures.
#[derive(Debug, Clone)]
pub struct SingleFactorModel {
    volatility: f64,
    mean_reversion: f64,
    correlation_matrix: Vec<f64>,
    volatilities: Vec<f64>,
}

impl SingleFactorModel {
    /// Create a single-factor model.
    ///
    /// # Arguments
    /// * `volatility` - Factor volatility, clamped to [0.01, 2.0]
    /// * `mean_reversion` - Mean reversion speed (0 = random walk), clamped to [0.0, 10.0]
    ///
    /// # Returns
    ///
    /// A single-factor model using the bounded volatility and mean-reversion
    /// inputs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::correlation::SingleFactorModel;
    ///
    /// let model = SingleFactorModel::new(0.25, 0.10);
    /// assert!((model.volatility() - 0.25).abs() < 1e-10);
    /// ```
    #[must_use]
    pub fn new(volatility: f64, mean_reversion: f64) -> Self {
        let vol = volatility.clamp(0.01, 2.0);
        Self {
            volatility: vol,
            mean_reversion: mean_reversion.clamp(0.0, 10.0),
            correlation_matrix: vec![1.0],
            volatilities: vec![vol],
        }
    }

    /// Number of factors in the model (always 1).
    ///
    /// # Returns
    ///
    /// The factor count, which is `1` for a single-factor model.
    #[must_use]
    pub fn num_factors(&self) -> usize {
        1
    }

    /// Get the volatility.
    ///
    /// # Returns
    ///
    /// The single-factor volatility in decimal form.
    pub fn volatility(&self) -> f64 {
        self.volatility
    }

    /// Get the mean reversion speed.
    ///
    /// # Returns
    ///
    /// The mean-reversion speed.
    pub fn mean_reversion(&self) -> f64 {
        self.mean_reversion
    }
}

/// Two-factor model for prepayment and credit.
///
/// Models prepayment and credit behavior through two correlated factors.
/// Captures the empirical negative correlation between prepayment and default.
#[derive(Debug, Clone)]
pub struct TwoFactorModel {
    prepay_vol: f64,
    credit_vol: f64,
    correlation: f64,
    correlation_matrix: Vec<f64>,
    volatilities: Vec<f64>,
    // Cholesky lower triangular for correlated sampling: L[1][0], L[1][1]
    // Used by external callers for generating correlated factors
    cholesky_l10: f64,
    cholesky_l11: f64,
}

impl TwoFactorModel {
    /// Create a two-factor model.
    ///
    /// # Arguments
    /// * `prepay_vol` - Prepayment factor volatility, clamped to [0.01, 2.0]
    /// * `credit_vol` - Credit factor volatility, clamped to [0.01, 2.0]
    /// * `correlation` - Correlation between factors (typically negative for RMBS), clamped to [-0.99, 0.99]
    ///
    /// # Returns
    ///
    /// A two-factor prepayment/credit model.
    #[must_use]
    pub fn new(prepay_vol: f64, credit_vol: f64, correlation: f64) -> Self {
        let corr = correlation.clamp(-0.99, 0.99);
        let pv = prepay_vol.clamp(0.01, 2.0);
        let cv = credit_vol.clamp(0.01, 2.0);

        // Cholesky decomposition of correlation matrix [[1, ρ], [ρ, 1]]
        // L = [[1, 0], [ρ, √(1-ρ²)]]
        let sqrt_1mr2 = (1.0 - corr * corr).sqrt();

        Self {
            prepay_vol: pv,
            credit_vol: cv,
            correlation: corr,
            // Row-major: [ρ₁₁, ρ₁₂, ρ₂₁, ρ₂₂]
            correlation_matrix: vec![1.0, corr, corr, 1.0],
            volatilities: vec![pv, cv],
            cholesky_l10: corr,
            cholesky_l11: sqrt_1mr2,
        }
    }

    /// Standard RMBS calibration with negative prepay-credit correlation.
    ///
    /// Uses: prepay_vol=0.20, credit_vol=0.25, correlation=-0.30
    ///
    /// # Returns
    ///
    /// The crate's default RMBS-oriented two-factor calibration.
    #[must_use]
    pub fn rmbs_standard() -> Self {
        Self::new(0.20, 0.25, -0.30)
    }

    /// Standard CLO calibration.
    ///
    /// Uses: prepay_vol=0.15, credit_vol=0.30, correlation=-0.20
    ///
    /// # Returns
    ///
    /// The crate's default CLO-oriented two-factor calibration.
    #[must_use]
    pub fn clo_standard() -> Self {
        Self::new(0.15, 0.30, -0.20)
    }

    /// Number of factors in the model (always 2).
    ///
    /// # Returns
    ///
    /// The factor count, which is `2` for a two-factor model.
    #[must_use]
    pub fn num_factors(&self) -> usize {
        2
    }

    /// Get prepayment factor volatility.
    ///
    /// # Returns
    ///
    /// The prepayment-factor volatility in decimal form.
    pub fn prepay_vol(&self) -> f64 {
        self.prepay_vol
    }

    /// Get credit factor volatility.
    ///
    /// # Returns
    ///
    /// The credit-factor volatility in decimal form.
    pub fn credit_vol(&self) -> f64 {
        self.credit_vol
    }

    /// Get factor correlation.
    ///
    /// # Returns
    ///
    /// The correlation between the prepayment and credit factors.
    pub fn correlation(&self) -> f64 {
        self.correlation
    }

    /// Get Cholesky `L[1][0]` coefficient for correlated factor generation.
    ///
    /// For generating correlated factors from independent normals (z1, z2):
    /// - Factor 1 = z1 * prepay_vol
    /// - Factor 2 = (l10 * z1 + l11 * z2) * credit_vol
    ///
    /// # Returns
    ///
    /// The off-diagonal Cholesky loading `L[1][0]`.
    pub fn cholesky_l10(&self) -> f64 {
        self.cholesky_l10
    }

    /// Get Cholesky `L[1][1]` coefficient for correlated factor generation.
    ///
    /// # Returns
    ///
    /// The diagonal Cholesky loading `L[1][1]`.
    pub fn cholesky_l11(&self) -> f64 {
        self.cholesky_l11
    }
}

/// Multi-factor model with custom correlation structure.
///
/// Supports arbitrary number of factors with custom correlation matrix.
/// Uses pivoted Cholesky decomposition internally for generating correlated factors,
/// which handles near-singular and positive-semidefinite correlation matrices robustly.
///
/// # Correlation Matrix Requirements
///
/// The correlation matrix must be:
/// - Symmetric: ρᵢⱼ = ρⱼᵢ
/// - Unit diagonal: ρᵢᵢ = 1
/// - Positive semi-definite: All eigenvalues ≥ 0
///
/// Invalid matrices will be replaced with the identity matrix.
/// Use [`MultiFactorModel::validated`] for explicit validation.
#[derive(Debug, Clone)]
pub struct MultiFactorModel {
    num_factors: usize,
    volatilities: Vec<f64>,
    correlation_matrix: Vec<f64>,
    /// Pivoted Cholesky factor (lower-triangular in original variable order) for
    /// correlated sampling. Generates correlated factors from independent normals:
    /// `correlated_z = L · z`.
    cholesky_factor: CorrelationFactor,
}

impl MultiFactorModel {
    /// Create a multi-factor model.
    ///
    /// # Arguments
    /// * `num_factors` - Number of factors (must be ≥ 1)
    /// * `volatilities` - Factor volatilities (one per factor)
    /// * `correlations` - Correlation matrix (flattened row-major, n×n values)
    ///
    /// # Errors
    /// Returns [`crate::correlation::Error`] if the matrix is invalid.
    ///
    /// # Returns
    ///
    /// A validated multi-factor model.
    pub fn new(num_factors: usize, volatilities: Vec<f64>, correlations: Vec<f64>) -> Result<Self> {
        Self::validated(num_factors, volatilities, correlations)
    }

    /// Create a multi-factor model, projecting near-PSD correlation matrices
    /// onto the nearest valid correlation matrix (Higham 2002) before falling
    /// back to the identity.
    ///
    /// Precedence of attempts:
    ///
    /// 1. `new` — use the matrix as provided if it is already a valid
    ///    correlation matrix.
    /// 2. [`nearest_correlation_matrix`](crate::correlation::nearest_correlation_matrix)
    ///    — repair small PSD violations (typical when the matrix comes from a
    ///    thresholded sample estimate or shrinkage) and retry validation.
    /// 3. Identity fallback — only if both of the above fail, and only with a
    ///    loud warning. This preserves backwards compatibility with existing
    ///    call sites while eliminating silent identity substitution for
    ///    matrices that were *almost* correct.
    ///
    /// Sites that want a hard error on invalid input should call
    /// [`MultiFactorModel::validated`] directly instead.
    ///
    /// # Returns
    ///
    /// A validated model from the original matrix, a model built from the
    /// Higham-projected matrix, or (last resort) an uncorrelated fallback.
    #[must_use]
    pub fn new_or_identity(
        num_factors: usize,
        volatilities: Vec<f64>,
        correlations: Vec<f64>,
    ) -> Self {
        if let Ok(model) = Self::new(num_factors, volatilities.clone(), correlations.clone()) {
            return model;
        }

        // Try Higham's nearest-correlation projection before the identity
        // fallback. This repairs small PSD violations (e.g. sample-correlation
        // estimation noise) instead of silently throwing the user's correlation
        // structure away.
        let n = num_factors.max(1);
        if correlations.len() == n * n {
            match crate::correlation::nearest_correlation::nearest_correlation_matrix(
                &correlations,
                n,
                crate::correlation::nearest_correlation::NearestCorrelationOpts::default(),
            ) {
                Ok(repaired) => {
                    if let Ok(model) = Self::new(num_factors, volatilities.clone(), repaired) {
                        tracing::warn!(
                            num_factors,
                            "Invalid correlation matrix; using Higham (2002) nearest \
                             correlation projection as the repaired input"
                        );
                        return model;
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        num_factors,
                        %err,
                        "Nearest-correlation projection rejected the input; \
                         continuing to identity fallback"
                    );
                }
            }
        }

        tracing::warn!(
            num_factors,
            "Invalid correlation matrix and projection failed; falling back to \
             uncorrelated (identity) model"
        );
        Self::uncorrelated(num_factors, volatilities)
    }

    /// Create a multi-factor model with validation.
    ///
    /// Returns an error if the correlation matrix is invalid.
    ///
    /// # Arguments
    /// * `num_factors` - Number of factors (must be ≥ 1)
    /// * `volatilities` - Factor volatilities (one per factor)
    /// * `correlations` - Correlation matrix (flattened row-major, n×n values)
    ///
    /// # Errors
    /// Returns [`crate::correlation::Error`] if the matrix is invalid.
    ///
    /// # Returns
    ///
    /// A validated multi-factor model.
    pub fn validated(
        num_factors: usize,
        volatilities: Vec<f64>,
        correlations: Vec<f64>,
    ) -> Result<Self> {
        let n = num_factors.max(1);

        // Validate volatilities length: previously a mismatch silently replaced
        // the caller's vector with unit volatilities, which hid serious
        // misconfiguration (wrong vol magnitudes flowing into pricing).
        if volatilities.len() != n {
            return Err(Error::VolatilityLengthMismatch {
                expected: n,
                actual: volatilities.len(),
            });
        }
        // Reject negative or non-finite volatilities explicitly. Silent
        // clamping of `-0.2` to `0.01` would mask sign-flip bugs upstream.
        // The upper-bound clamp (10.0) and lower-bound clamp (0.01) below
        // remain — defensive against vol-of-vol explosions and singular
        // Cholesky on legitimately-zero entries — but only after the input
        // has been verified non-negative and finite.
        if let Some((index, value)) = volatilities
            .iter()
            .copied()
            .enumerate()
            .find(|(_, v)| !v.is_finite() || *v < 0.0)
        {
            return Err(Error::InvalidVolatility { index, value });
        }
        let vols: Vec<f64> = volatilities.iter().map(|v| v.clamp(0.01, 10.0)).collect();

        // Validate correlation matrix
        if correlations.len() != n * n {
            return Err(Error::InvalidSize {
                expected: n,
                actual: correlations.len(),
            });
        }

        validate_correlation_matrix(&correlations, n)?;

        // Compute Cholesky decomposition
        let cholesky = cholesky_decompose(&correlations, n)?;

        Ok(Self {
            num_factors: n,
            volatilities: vols,
            correlation_matrix: correlations,
            cholesky_factor: cholesky,
        })
    }

    /// Create an uncorrelated (identity) multi-factor model.
    ///
    /// All factors are independent (correlation = 0 for i ≠ j).
    ///
    /// # Returns
    ///
    /// A multi-factor model with an identity correlation matrix.
    #[must_use]
    pub fn uncorrelated(num_factors: usize, volatilities: Vec<f64>) -> Self {
        let n = num_factors.max(1);

        let vols = if volatilities.len() == n {
            volatilities.iter().map(|v| v.clamp(0.01, 10.0)).collect()
        } else {
            tracing::warn!(
                expected = n,
                actual = volatilities.len(),
                "MultiFactorModel::uncorrelated: volatility length mismatch; falling back to unit volatilities"
            );
            vec![1.0; n]
        };

        // Identity correlation matrix
        let mut corrs = vec![0.0; n * n];
        for i in 0..n {
            corrs[i * n + i] = 1.0;
        }

        // Identity is its own Cholesky factor (already in original order, full rank)
        let cholesky = CorrelationFactor::from_parts(corrs.clone(), n, n);

        Self {
            num_factors: n,
            volatilities: vols,
            correlation_matrix: corrs,
            cholesky_factor: cholesky,
        }
    }

    /// Number of factors in the model.
    ///
    /// # Returns
    ///
    /// The number of systematic factors.
    #[must_use]
    pub fn num_factors(&self) -> usize {
        self.num_factors
    }

    /// Get the factor correlation matrix (flattened row-major).
    ///
    /// For n factors, returns n×n values where `matrix[i,j] = correlation(Zᵢ, Zⱼ)`.
    ///
    /// # Returns
    ///
    /// The correlation matrix in row-major order.
    #[must_use]
    pub fn correlation_matrix(&self) -> &[f64] {
        &self.correlation_matrix
    }

    /// Get factor volatilities.
    ///
    /// # Returns
    ///
    /// The factor volatilities in decimal form, aligned with the factor ordering.
    #[must_use]
    pub fn volatilities(&self) -> &[f64] {
        &self.volatilities
    }

    /// Get the Cholesky factor for correlated factor generation.
    ///
    /// Returns the [`CorrelationFactor`] holding the lower-triangular L in original
    /// variable ordering. To generate n correlated factors from n independent standard
    /// normals z:
    /// ```text
    /// correlated_z = L · z
    /// factor_i = correlated_z[i] * volatility[i]
    /// ```
    ///
    /// # Returns
    ///
    /// The cached Cholesky factor in original variable order.
    #[must_use]
    pub fn cholesky_factor(&self) -> &CorrelationFactor {
        &self.cholesky_factor
    }

    /// Generate correlated factor values from independent standard normal draws.
    ///
    /// # Arguments
    /// * `independent_z` - Vector of n independent standard normal values
    ///
    /// # Returns
    /// Vector of n correlated factor values (scaled by volatilities).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::correlation::MultiFactorModel;
    ///
    /// let model = MultiFactorModel::uncorrelated(2, vec![0.2, 0.3]);
    /// let factors = model.generate_correlated_factors(&[1.0, -1.0]);
    ///
    /// assert_eq!(factors, vec![0.2, -0.3]);
    /// ```
    #[must_use]
    pub fn generate_correlated_factors(&self, independent_z: &[f64]) -> Vec<f64> {
        let mut factors = vec![0.0; self.num_factors];
        self.generate_correlated_factors_into(independent_z, &mut factors);
        factors
    }

    /// Generate correlated factor values into a caller-provided buffer.
    ///
    /// The input slice and output buffer must both have length `num_factors()`.
    ///
    /// # Arguments
    ///
    /// * `independent_z` - Independent standard-normal draws, one per factor.
    /// * `out` - Output buffer that receives the correlated factor values.
    ///
    /// # Panics
    ///
    /// Panics if either slice length differs from `self.num_factors()`.
    pub fn generate_correlated_factors_into(&self, independent_z: &[f64], out: &mut [f64]) {
        assert_eq!(
            independent_z.len(),
            self.num_factors,
            "expected {} independent factors, got {}",
            self.num_factors,
            independent_z.len()
        );
        assert_eq!(
            out.len(),
            self.num_factors,
            "expected output buffer of length {}, got {}",
            self.num_factors,
            out.len()
        );

        let _ = self.cholesky_factor.apply(independent_z, out);
        for (value, vol) in out.iter_mut().zip(self.volatilities.iter()) {
            *value *= *vol;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_factor_creation() {
        let model = SingleFactorModel::new(0.25, 0.1);
        let kind = FactorModelKind::Single(model.clone());
        assert_eq!(kind.num_factors(), 1);
        assert!((model.volatility() - 0.25).abs() < 1e-10);
        assert_eq!(kind.factor_names(), vec!["Market"]);
    }

    #[test]
    fn test_two_factor_creation() {
        let model = TwoFactorModel::new(0.20, 0.30, -0.30);
        assert_eq!(FactorModelKind::Two(model.clone()).num_factors(), 2);
        assert!((model.prepay_vol() - 0.20).abs() < 1e-10);
        assert!((model.credit_vol() - 0.30).abs() < 1e-10);
        assert!((model.correlation() - (-0.30)).abs() < 1e-10);
    }

    #[test]
    fn test_diagonal_factor_contribution_single_factor() {
        let kind = FactorModelKind::Single(SingleFactorModel::new(1.0, 0.0));

        let factor = kind.diagonal_factor_contribution(0, 1.5);
        assert!((factor - 1.5).abs() < 1e-10);
    }

    #[test]
    fn test_diagonal_factor_contribution_two_factor_credit_uses_diagonal_loading() {
        // Regression: the diagonal credit contribution must include the L[1,1]
        // Cholesky factor √(1 − ρ²). Previously this returned z · credit_vol,
        // double-counting the systematic component when combined with the
        // correlated draw path.
        let rho: f64 = -0.30;
        let credit_vol: f64 = 0.30;
        let kind = FactorModelKind::Two(TwoFactorModel::new(0.20, credit_vol, rho));

        let z = 2.0;
        let expected = z * credit_vol * (1.0 - rho * rho).sqrt();
        let factor = kind.diagonal_factor_contribution(1, z);
        assert!(
            (factor - expected).abs() < 1e-10,
            "diagonal credit contribution: expected {expected}, got {factor}"
        );
    }

    #[test]
    fn test_diagonal_factor_contribution_two_factor_prepay_unchanged() {
        // Prepayment is factor 0; L[0,0] = 1 so the contribution is z · vol.
        let kind = FactorModelKind::Two(TwoFactorModel::new(0.20, 0.30, -0.30));
        let factor = kind.diagonal_factor_contribution(0, 1.5);
        assert!((factor - 0.30).abs() < 1e-10);
    }

    #[test]
    fn test_two_factor_correlation_matrix() {
        let kind = FactorModelKind::Two(TwoFactorModel::new(0.20, 0.30, -0.30));
        let corr = kind.correlation_matrix();

        // Check diagonal is 1
        assert!((corr[0] - 1.0).abs() < 1e-10);
        assert!((corr[3] - 1.0).abs() < 1e-10);

        // Check off-diagonal is correlation
        assert!((corr[1] - (-0.30)).abs() < 1e-10);
        assert!((corr[2] - (-0.30)).abs() < 1e-10);
    }

    #[test]
    fn test_factor_spec_build() {
        let spec = FactorSpec::single_factor(0.25, 0.1);
        let model = spec.build();
        assert_eq!(model.num_factors(), 1);
        assert!(matches!(model, FactorModelKind::Single(_)));

        let spec = FactorSpec::two_factor(0.20, 0.30, -0.30);
        let model = spec.build();
        assert_eq!(model.num_factors(), 2);
        assert!(matches!(model, FactorModelKind::Two(_)));
    }

    #[test]
    fn test_standard_calibrations() {
        let rmbs = TwoFactorModel::rmbs_standard();
        assert_eq!(FactorModelKind::Two(rmbs.clone()).num_factors(), 2);
        assert!(rmbs.correlation() < 0.0); // Negative correlation

        let clo = TwoFactorModel::clo_standard();
        assert_eq!(FactorModelKind::Two(clo.clone()).num_factors(), 2);
        assert!(clo.correlation() < 0.0);
    }

    #[test]
    fn test_volatilities() {
        let kind = FactorModelKind::Two(TwoFactorModel::new(0.20, 0.30, -0.30));
        let vols = kind.volatilities();
        assert_eq!(vols.len(), 2);
        assert!((vols[0] - 0.20).abs() < 1e-10);
        assert!((vols[1] - 0.30).abs() < 1e-10);
    }

    // ========== Correlation Matrix Validation Tests ==========

    #[test]
    fn test_validate_identity_matrix() {
        let identity = vec![1.0, 0.0, 0.0, 1.0];
        assert!(validate_correlation_matrix(&identity, 2).is_ok());
    }

    #[test]
    fn test_validate_valid_correlation_matrix() {
        // Valid 3x3 correlation matrix
        let corr = vec![1.0, 0.5, 0.3, 0.5, 1.0, 0.4, 0.3, 0.4, 1.0];
        assert!(validate_correlation_matrix(&corr, 3).is_ok());
    }

    #[test]
    fn test_validate_invalid_size() {
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let result = validate_correlation_matrix(&corr, 3);
        assert!(matches!(result, Err(Error::InvalidSize { .. })));
    }

    #[test]
    fn test_validate_diagonal_not_one() {
        let corr = vec![0.9, 0.5, 0.5, 1.0];
        let result = validate_correlation_matrix(&corr, 2);
        assert!(matches!(
            result,
            Err(Error::DiagonalNotOne { index: 0, .. })
        ));
    }

    #[test]
    fn test_validate_not_symmetric() {
        let corr = vec![1.0, 0.5, 0.3, 1.0]; // Off-diagonals don't match
        let result = validate_correlation_matrix(&corr, 2);
        assert!(matches!(result, Err(Error::NotSymmetric { .. })));
    }

    #[test]
    fn test_validate_not_psd() {
        // Non-PSD matrix: high correlations that violate PSD constraint
        let corr = vec![1.0, 0.9, 0.9, 0.9, 1.0, -0.5, 0.9, -0.5, 1.0];
        let result = validate_correlation_matrix(&corr, 3);
        assert!(matches!(result, Err(Error::NotPositiveSemiDefinite { .. })));
    }

    #[test]
    fn test_validate_not_psd_reports_cholesky_failure_row() {
        let corr = vec![1.0, 0.9, 0.9, 0.9, 1.0, -0.5, 0.9, -0.5, 1.0];
        let result = validate_correlation_matrix(&corr, 3);
        let cholesky_result = cholesky_decompose(&corr, 3);

        match (result, cholesky_result) {
            (
                Err(Error::NotPositiveSemiDefinite { row: validate_row }),
                Err(Error::NotPositiveSemiDefinite { row: cholesky_row }),
            ) => assert_eq!(validate_row, cholesky_row),
            other => panic!("expected matching PSD errors, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_out_of_bounds() {
        let corr = vec![1.0, 1.5, 1.5, 1.0];
        let result = validate_correlation_matrix(&corr, 2);
        assert!(matches!(result, Err(Error::OutOfBounds { .. })));
    }

    // ========== Cholesky Decomposition Tests ==========

    #[test]
    fn test_cholesky_identity() {
        let identity = vec![1.0, 0.0, 0.0, 1.0];
        let f = cholesky_decompose(&identity, 2).expect("identity matrix should decompose");
        let l = f.factor_matrix();

        // Identity is its own Cholesky factor
        assert!((l[0] - 1.0).abs() < 1e-10);
        assert!(l[1].abs() < 1e-10);
        assert!(l[2].abs() < 1e-10);
        assert!((l[3] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cholesky_2x2() {
        // [[1, 0.6], [0.6, 1]]
        let corr = vec![1.0, 0.6, 0.6, 1.0];
        let f = cholesky_decompose(&corr, 2).expect("2x2 correlation matrix should decompose");

        // Verify L * L^T = original (pivoting may produce different but valid factor).
        let l = f.factor_matrix();
        let n = 2;
        for i in 0..n {
            for j in 0..n {
                let mut sum = 0.0;
                for k in 0..n {
                    sum += l[i * n + k] * l[j * n + k];
                }
                assert!(
                    (sum - corr[i * n + j]).abs() < 1e-10,
                    "LLᵀ[{i},{j}] = {sum} but expected {}",
                    corr[i * n + j]
                );
            }
        }
        assert!(f.is_full_rank());
    }

    #[test]
    fn test_cholesky_reconstructs_original() {
        let corr = vec![1.0, 0.5, 0.3, 0.5, 1.0, 0.4, 0.3, 0.4, 1.0];
        let f = cholesky_decompose(&corr, 3).expect("3x3 correlation matrix should decompose");
        let l = f.factor_matrix();

        // Verify L * L^T = original
        let n = 3;
        for i in 0..n {
            for j in 0..n {
                let mut sum = 0.0;
                for k in 0..n {
                    sum += l[i * n + k] * l[j * n + k];
                }
                assert!(
                    (sum - corr[i * n + j]).abs() < 1e-10,
                    "LLᵀ[{},{}] = {} but expected {}",
                    i,
                    j,
                    sum,
                    corr[i * n + j]
                );
            }
        }
    }

    // ========== MultiFactorModel Tests ==========

    #[test]
    fn test_multi_factor_valid_matrix() {
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let vols = vec![0.2, 0.3];
        let model = MultiFactorModel::validated(2, vols, corr)
            .expect("valid 2x2 correlation matrix should create model");

        assert_eq!(FactorModelKind::Multi(model.clone()).num_factors(), 2);
        assert!(model.cholesky_factor().factor_matrix().len() == 4);
    }

    #[test]
    fn test_multi_factor_invalid_matrix_returns_error() {
        // Invalid matrix (not symmetric)
        let corr = vec![1.0, 0.5, 0.3, 1.0];
        let vols = vec![0.2, 0.3];
        let err = MultiFactorModel::new(2, vols, corr)
            .expect_err("invalid matrices should no longer silently fall back");

        assert!(matches!(err, Error::NotSymmetric { .. }));
    }

    #[test]
    fn test_multi_factor_volatility_length_mismatch_returns_error() {
        // Regression: previously the caller's vols were silently replaced with
        // unit volatilities. Now it surfaces as an explicit error variant so
        // config-driven workflows can fail loudly.
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let wrong_vols = vec![0.2, 0.3, 0.4]; // len 3, but num_factors = 2
        let err = MultiFactorModel::new(2, wrong_vols, corr)
            .expect_err("mismatched vol length should be reported");

        assert!(matches!(
            err,
            Error::VolatilityLengthMismatch {
                expected: 2,
                actual: 3,
            }
        ));
    }

    #[test]
    fn test_multi_factor_new_or_identity_fallback() {
        let corr = vec![1.0, 0.5, 0.3, 1.0];
        let vols = vec![0.2, 0.3];
        let model = FactorModelKind::Multi(MultiFactorModel::new_or_identity(2, vols, corr));

        let corr_matrix = model.correlation_matrix();
        assert!((corr_matrix[0] - 1.0).abs() < 1e-10);
        assert!(corr_matrix[1].abs() < 1e-10);
        assert!(corr_matrix[2].abs() < 1e-10);
        assert!((corr_matrix[3] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_multi_factor_uncorrelated() {
        let model = FactorModelKind::Multi(MultiFactorModel::uncorrelated(3, vec![0.1, 0.2, 0.3]));

        assert_eq!(model.num_factors(), 3);

        // Check identity correlation
        let corr = model.correlation_matrix();
        for i in 0..3 {
            for j in 0..3 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((corr[i * 3 + j] - expected).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_generate_correlated_factors() {
        let corr = vec![1.0, 0.6, 0.6, 1.0];
        let vols = vec![1.0, 1.0]; // Unit volatilities for easy verification
        let model = MultiFactorModel::validated(2, vols, corr.clone())
            .expect("correlated model should create successfully");

        // Verify L * L^T = correlation matrix (covariance structure is preserved).
        // The pivoted factor may produce different but equivalent L entries.
        let l = model.cholesky_factor().factor_matrix();
        let n = 2;
        for i in 0..n {
            for j in 0..n {
                let mut sum = 0.0;
                for k in 0..n {
                    sum += l[i * n + k] * l[j * n + k];
                }
                assert!(
                    (sum - corr[i * n + j]).abs() < 1e-10,
                    "correlation reconstruction mismatch at [{i},{j}]"
                );
            }
        }

        // generate_correlated_factors with z=[0,0] must return zeros.
        let zeros = model.generate_correlated_factors(&[0.0, 0.0]);
        assert!(zeros[0].abs() < 1e-15);
        assert!(zeros[1].abs() < 1e-15);
    }

    #[test]
    fn test_generate_correlated_factors_with_volatility() {
        let corr = vec![1.0, 0.0, 0.0, 1.0]; // Identity
        let vols = vec![0.2, 0.3];
        let model = MultiFactorModel::validated(2, vols, corr)
            .expect("identity correlation model should create successfully");

        let factors = model.generate_correlated_factors(&[1.0, 1.0]);

        // With identity correlation: factors = z * vol
        assert!((factors[0] - 0.2).abs() < 1e-10);
        assert!((factors[1] - 0.3).abs() < 1e-10);
    }

    #[test]
    fn test_generate_correlated_factors_into_matches_allocating_api() {
        let corr = vec![1.0, 0.6, 0.6, 1.0];
        let model = MultiFactorModel::validated(2, vec![0.2, 0.3], corr)
            .expect("correlated model should create successfully");

        let mut out = vec![0.0; 2];
        model.generate_correlated_factors_into(&[1.0, 0.5], &mut out);

        assert_eq!(out, model.generate_correlated_factors(&[1.0, 0.5]));
    }

    #[test]
    #[should_panic(expected = "expected 2 independent factors, got 1")]
    fn test_generate_correlated_factors_into_rejects_wrong_input_length() {
        let corr = vec![1.0, 0.6, 0.6, 1.0];
        let model = MultiFactorModel::validated(2, vec![0.2, 0.3], corr)
            .expect("correlated model should create successfully");

        let mut out = vec![0.0; 2];
        model.generate_correlated_factors_into(&[1.0], &mut out);
    }

    #[test]
    fn test_diagonal_factor_contribution_multi_factor_uses_cholesky_diagonal() {
        let corr = vec![1.0, 0.6, 0.6, 1.0];
        let model = MultiFactorModel::validated(2, vec![0.2, 0.3], corr)
            .expect("correlated model should create successfully");

        let l_entry = model.cholesky_factor().factor_matrix()[3];
        let factor = FactorModelKind::Multi(model).diagonal_factor_contribution(1, 2.0);
        let expected = 2.0 * l_entry * 0.3;
        assert!((factor - expected).abs() < 1e-10);
    }
}
