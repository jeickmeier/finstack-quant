//! Factor models for correlated behavior in credit portfolios.
//!
//! **Naming note:** This module's [`LatentFactorKind`] enum is distinct from the
//! portfolio-risk factor framework in the `finstack-quant-factor` crate.
//! Here, "factor model" refers to *latent-variable models* used to generate
//! correlated default/prepayment events in Monte Carlo simulations (the
//! Gaussian copula family). The core/portfolio factor model instead provides
//! market-risk factor definitions, covariance, and sensitivity-based Euler
//! risk decomposition.
//!
//! Factor models drive the correlation between prepayment and default events
//! through common systematic factors. This module provides:
//!
//! - [`LatentFactorKind`] enum: Closed dispatch over the concrete factor models
//! - [`LatentSingleFactor`]: One-factor model (common market factor)
//! - [`LatentTwoFactor`]: Two-factor model (prepayment + credit factors)
//! - [`LatentMultiFactor`]: N-factor model with custom correlation matrix
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
//! For `LatentMultiFactor`, the correlation matrix must be:
//! - **Symmetric**: ρᵢⱼ = ρⱼᵢ
//! - **Unit diagonal**: ρᵢᵢ = 1
//! - **Positive semi-definite**: All eigenvalues ≥ 0 (verified via Cholesky)
//!
//! # References
//!
//! - Factor-model and covariance interpretation: `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
//!
//! - Portfolio-risk and dependence context: `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`
//!

use crate::correlation::{Error, Result};
use finstack_quant_core::math::linalg::{
    cholesky_correlation, validate_correlation_matrix, CorrelationError, CorrelationFactor,
};

/// Closed enum over the concrete factor models for correlated behavior.
///
/// Replaces a former `FactorModel` trait object: the set of factor models is
/// fixed and small, so a `match`-dispatched enum avoids the heap allocation
/// and dynamic dispatch of `Box<dyn FactorModel>` while keeping the same
/// six-method query surface.
#[derive(Debug, Clone)]
pub enum LatentFactorKind {
    /// Single common-market-factor model.
    Single(LatentSingleFactor),
    /// Two-factor prepayment/credit model.
    Two(LatentTwoFactor),
    /// N-factor model with a custom correlation matrix.
    Multi(LatentMultiFactor),
}

impl LatentFactorKind {
    /// Number of factors in the model.
    ///
    /// # Returns
    ///
    /// The number of systematic factors in the model.
    #[must_use]
    pub fn num_factors(&self) -> usize {
        match self {
            LatentFactorKind::Single(m) => m.num_factors(),
            LatentFactorKind::Two(m) => m.num_factors(),
            LatentFactorKind::Multi(m) => m.num_factors(),
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
            LatentFactorKind::Single(m) => &m.correlation_matrix,
            LatentFactorKind::Two(m) => &m.correlation_matrix,
            LatentFactorKind::Multi(m) => &m.correlation_matrix,
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
            LatentFactorKind::Single(m) => &m.volatilities,
            LatentFactorKind::Two(m) => &m.volatilities,
            LatentFactorKind::Multi(m) => &m.volatilities,
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
            LatentFactorKind::Single(_) => vec!["Market"],
            LatentFactorKind::Two(_) => vec!["Prepayment", "Credit"],
            LatentFactorKind::Multi(m) => vec!["Factor"; m.num_factors],
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
            LatentFactorKind::Single(_) => "Single Factor Model",
            LatentFactorKind::Two(_) => "Two-Factor Prepay-Credit Model",
            LatentFactorKind::Multi(_) => "Multi-Factor Model",
        }
    }

    /// Compute a single factor value given one standard normal draw.
    ///
    /// **Important:** For multi-factor models this returns only the *diagonal*
    /// Cholesky contribution `L[i,i] * z * vol[i]`, ignoring cross-factor
    /// correlation.  To generate properly correlated factor vectors, use
    /// [`LatentMultiFactor::generate_correlated_factors`] instead.
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
            LatentFactorKind::Single(m) => z * m.volatility,
            LatentFactorKind::Two(m) => match factor_index {
                // For the 2×2 correlation matrix: L[0,0] = 1, L[1,1] = √(1 − ρ²).
                0 => z * m.prepay_vol,
                1 => m.cholesky_l11 * z * m.credit_vol,
                _ => 0.0,
            },
            LatentFactorKind::Multi(m) => {
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    try_from = "RawLatentFactorSpec"
)]
#[allow(clippy::enum_variant_names)]
#[non_exhaustive]
pub enum LatentFactorSpec {
    /// Single factor model (common market factor).
    SingleFactor {
        /// Finite non-negative factor volatility (decimal).
        volatility: f64,
        /// Finite non-negative mean-reversion speed per year.
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

#[derive(serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RawLatentFactorSpec {
    #[serde(rename = "single_factor")]
    Single {
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::NonNegativeF64Wire")
        )]
        /// Finite non-negative decimal factor volatility.
        volatility: f64,
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::NonNegativeF64Wire")
        )]
        /// Finite non-negative mean-reversion speed per year.
        mean_reversion: f64,
    },
    #[serde(rename = "two_factor")]
    Two {
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::NonNegativeF64Wire")
        )]
        /// Finite non-negative decimal prepayment-factor volatility.
        prepay_vol: f64,
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::NonNegativeF64Wire")
        )]
        /// Finite non-negative decimal credit-factor volatility.
        credit_vol: f64,
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::CorrelationWire")
        )]
        /// Gaussian correlation in [-1, 1], including endpoints.
        correlation: f64,
    },
    #[serde(rename = "multi_factor")]
    Multi {
        #[cfg_attr(feature = "json-schema", schemars(range(min = 1)))]
        /// Positive number of factors.
        num_factors: usize,
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Vec<finstack_quant_core::wire::NonNegativeF64Wire>")
        )]
        /// One finite non-negative decimal volatility per factor.
        volatilities: Vec<f64>,
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Vec<finstack_quant_core::wire::CorrelationWire>")
        )]
        /// Row-major square Gaussian correlation matrix with entries in [-1, 1].
        correlations: Vec<f64>,
    },
}

impl TryFrom<RawLatentFactorSpec> for LatentFactorSpec {
    type Error = Error;

    fn try_from(raw: RawLatentFactorSpec) -> Result<Self> {
        let spec = match raw {
            RawLatentFactorSpec::Single {
                volatility,
                mean_reversion,
            } => Self::SingleFactor {
                volatility,
                mean_reversion,
            },
            RawLatentFactorSpec::Two {
                prepay_vol,
                credit_vol,
                correlation,
            } => Self::TwoFactor {
                prepay_vol,
                credit_vol,
                correlation,
            },
            RawLatentFactorSpec::Multi {
                num_factors,
                volatilities,
                correlations,
            } => Self::MultiFactor {
                num_factors,
                volatilities,
                correlations,
            },
        };
        spec.build()?;
        Ok(spec)
    }
}

fn validate_volatility(index: usize, value: f64) -> Result<()> {
    finstack_quant_core::wire::NonNegativeF64Wire::try_from(value)
        .map(|_| ())
        .map_err(|_| Error::InvalidVolatility { index, value })
}

fn validate_single_factor(volatility: f64, mean_reversion: f64) -> Result<()> {
    validate_volatility(0, volatility)?;
    finstack_quant_core::wire::NonNegativeF64Wire::try_from(mean_reversion)
        .map(|_| ())
        .map_err(|_| Error::InvalidMeanReversion {
            value: mean_reversion,
        })
}

fn validate_two_factor(prepay_vol: f64, credit_vol: f64, correlation: f64) -> Result<()> {
    validate_volatility(0, prepay_vol)?;
    validate_volatility(1, credit_vol)?;
    finstack_quant_core::wire::CorrelationWire::try_from(correlation)
        .map(|_| ())
        .map_err(|_| {
            CorrelationError::OutOfBounds {
                i: 0,
                j: 1,
                value: correlation,
            }
            .into()
        })
}

fn validate_factor_volatilities(num_factors: usize, volatilities: &[f64]) -> Result<()> {
    if num_factors == 0 {
        return Err(Error::InvalidFactorCount { value: num_factors });
    }
    if volatilities.len() != num_factors {
        return Err(Error::VolatilityLengthMismatch {
            expected: num_factors,
            actual: volatilities.len(),
        });
    }
    for (index, value) in volatilities.iter().copied().enumerate() {
        validate_volatility(index, value)?;
    }
    Ok(())
}

impl Default for LatentFactorSpec {
    fn default() -> Self {
        LatentFactorSpec::SingleFactor {
            volatility: 1.0,
            mean_reversion: 0.0,
        }
    }
}

impl LatentFactorSpec {
    /// Create a checked single-factor specification.
    ///
    /// # Arguments
    ///
    /// * `volatility` - Finite, non-negative decimal factor volatility; zero
    ///   disables innovations and no upper cap is applied.
    /// * `mean_reversion` - Finite, non-negative speed per year; zero disables
    ///   mean reversion and no upper cap is applied.
    ///
    /// # Errors
    ///
    /// Returns a validation error for a negative or non-finite input.
    pub fn single_factor(volatility: f64, mean_reversion: f64) -> Result<Self> {
        validate_single_factor(volatility, mean_reversion)?;
        Ok(Self::SingleFactor {
            volatility,
            mean_reversion,
        })
    }

    /// Create a checked two-factor specification.
    ///
    /// # Arguments
    ///
    /// * `prepay_vol` - Finite, non-negative decimal prepayment-factor volatility.
    /// * `credit_vol` - Finite, non-negative decimal credit-factor volatility.
    /// * `correlation` - Gaussian factor correlation in `[-1, 1]`; perfect
    ///   correlation endpoints are accepted without altering them.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an invalid volatility or correlation.
    pub fn two_factor(prepay_vol: f64, credit_vol: f64, correlation: f64) -> Result<Self> {
        validate_two_factor(prepay_vol, credit_vol, correlation)?;
        Ok(Self::TwoFactor {
            prepay_vol,
            credit_vol,
            correlation,
        })
    }

    /// Build a factor model from this specification.
    ///
    /// # Returns
    ///
    /// A [`LatentFactorKind`] variant matching the specification.
    ///
    /// # Errors
    ///
    /// Returns [`crate::correlation::Error`] if any specification contains a
    /// negative or non-finite volatility or mean-reversion speed, an invalid
    /// correlation, a zero factor count, or inconsistent vector dimensions. Call
    /// [`finstack_quant_core::math::linalg::nearest_correlation`] explicitly before
    /// construction when projection is part of the caller's policy.
    pub fn build(&self) -> Result<LatentFactorKind> {
        Ok(match self {
            LatentFactorSpec::SingleFactor {
                volatility,
                mean_reversion,
            } => LatentFactorKind::Single(LatentSingleFactor::new(*volatility, *mean_reversion)?),
            LatentFactorSpec::TwoFactor {
                prepay_vol,
                credit_vol,
                correlation,
            } => LatentFactorKind::Two(LatentTwoFactor::new(
                *prepay_vol,
                *credit_vol,
                *correlation,
            )?),
            LatentFactorSpec::MultiFactor {
                num_factors,
                volatilities,
                correlations,
            } => LatentFactorKind::Multi(LatentMultiFactor::new(
                *num_factors,
                volatilities.clone(),
                correlations.clone(),
            )?),
        })
    }

    /// Get the number of factors.
    ///
    /// # Returns
    ///
    /// The factor count implied by the specification.
    pub fn num_factors(&self) -> usize {
        match self {
            LatentFactorSpec::SingleFactor { .. } => 1,
            LatentFactorSpec::TwoFactor { .. } => 2,
            LatentFactorSpec::MultiFactor { num_factors, .. } => *num_factors,
        }
    }
}

/// Single-factor model (common market factor).
///
/// Models all correlation through a single systematic factor.
/// Suitable for simple correlation structures.
#[derive(Debug, Clone)]
pub struct LatentSingleFactor {
    volatility: f64,
    mean_reversion: f64,
    correlation_matrix: Vec<f64>,
    volatilities: Vec<f64>,
}

impl LatentSingleFactor {
    /// Create a single-factor model.
    ///
    /// # Arguments
    /// * `volatility` - Finite, non-negative decimal factor volatility; no upper cap.
    /// * `mean_reversion` - Finite non-negative mean-reversion speed per year; no upper cap.
    ///
    /// # Returns
    ///
    /// A single-factor model preserving the validated inputs exactly.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_models::correlation::LatentSingleFactor;
    ///
    /// let model = LatentSingleFactor::new(0.25, 0.10).expect("valid model inputs");
    /// assert!((model.volatility() - 0.25).abs() < 1e-10);
    /// ```
    pub fn new(volatility: f64, mean_reversion: f64) -> Result<Self> {
        validate_single_factor(volatility, mean_reversion)?;
        Ok(Self {
            volatility,
            mean_reversion,
            correlation_matrix: vec![1.0],
            volatilities: vec![volatility],
        })
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
pub struct LatentTwoFactor {
    prepay_vol: f64,
    credit_vol: f64,
    correlation: f64,
    correlation_matrix: Vec<f64>,
    volatilities: Vec<f64>,
    // Cholesky lower triangular for correlated sampling: L[1][0], L[1][1]
    cholesky_l10: f64,
    cholesky_l11: f64,
}

impl LatentTwoFactor {
    /// Create a two-factor model.
    ///
    /// # Arguments
    /// * `prepay_vol` - Finite, non-negative decimal prepayment-factor volatility.
    /// * `credit_vol` - Finite, non-negative decimal credit-factor volatility.
    /// * `correlation` - Gaussian factor correlation in [-1, 1], including degenerate endpoints.
    ///
    /// # Returns
    ///
    /// A two-factor prepayment/credit model.
    pub fn new(prepay_vol: f64, credit_vol: f64, correlation: f64) -> Result<Self> {
        validate_two_factor(prepay_vol, credit_vol, correlation)?;
        Ok(Self {
            prepay_vol,
            credit_vol,
            correlation,
            correlation_matrix: vec![1.0, correlation, correlation, 1.0],
            volatilities: vec![prepay_vol, credit_vol],
            cholesky_l10: correlation,
            cholesky_l11: (1.0 - correlation * correlation).sqrt(),
        })
    }

    /// Standard RMBS calibration with negative prepay-credit correlation.
    ///
    /// Uses: prepay_vol=0.20, credit_vol=0.25, correlation=-0.30
    ///
    /// # Returns
    ///
    /// The crate's default RMBS-oriented two-factor calibration.
    pub fn rmbs_standard() -> Result<Self> {
        Self::new(0.20, 0.25, -0.30)
    }

    /// Standard CLO calibration.
    ///
    /// Uses: prepay_vol=0.15, credit_vol=0.30, correlation=-0.20
    ///
    /// # Returns
    ///
    /// The crate's default CLO-oriented two-factor calibration.
    pub fn clo_standard() -> Result<Self> {
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
/// Invalid matrices are rejected; correlation repair is an explicit caller policy.
#[derive(Debug, Clone)]
pub struct LatentMultiFactor {
    num_factors: usize,
    volatilities: Vec<f64>,
    correlation_matrix: Vec<f64>,
    /// Pivoted Cholesky factor (lower-triangular in original variable order) for
    /// correlated sampling. Generates correlated factors from independent normals:
    /// `correlated_z = L · z`.
    cholesky_factor: CorrelationFactor,
}

impl LatentMultiFactor {
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
        let n = num_factors;
        validate_factor_volatilities(n, &volatilities)?;
        if n.checked_mul(n) != Some(correlations.len()) {
            return Err(CorrelationError::InvalidSize {
                expected: n,
                actual: correlations.len(),
            }
            .into());
        }
        for (index, value) in correlations.iter().copied().enumerate() {
            if finstack_quant_core::wire::CorrelationWire::try_from(value).is_err() {
                return Err(CorrelationError::OutOfBounds {
                    i: index / n,
                    j: index % n,
                    value,
                }
                .into());
            }
        }

        validate_correlation_matrix(&correlations, n)?;

        let cholesky = cholesky_correlation(&correlations, n)?;

        Ok(Self {
            num_factors: n,
            volatilities,
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
    /// # Arguments
    /// * `num_factors` - Positive number of factors; zero is rejected.
    /// * `volatilities` - One finite, non-negative decimal volatility per factor;
    ///   zero values and arbitrarily large finite values are preserved.
    ///
    /// # Errors
    /// Returns the same volatility-validation errors as [`Self::new`].
    pub fn uncorrelated(num_factors: usize, volatilities: Vec<f64>) -> Result<Self> {
        let n = num_factors;
        validate_factor_volatilities(n, &volatilities)?;
        let size = n.checked_mul(n).ok_or_else(|| {
            Error::Core(finstack_quant_core::Error::Validation(
                "factor matrix dimension overflows usize".into(),
            ))
        })?;
        let mut correlations = Vec::new();
        correlations.try_reserve_exact(size).map_err(|error| {
            Error::Core(finstack_quant_core::Error::Internal(format!(
                "cannot allocate factor correlation matrix: {error}"
            )))
        })?;
        correlations.resize(size, 0.0);
        for i in 0..n {
            correlations[i * n + i] = 1.0;
        }
        Self::new(num_factors, volatilities, correlations)
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

    /// Generate correlated factor values, rejecting a draw vector of the
    /// wrong length instead of panicking.
    ///
    /// # Arguments
    ///
    /// * `independent_z` - Independent standard normal draws, exactly one per
    ///   factor, in factor order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::FactorDrawLengthMismatch`] if `independent_z` does not
    /// hold exactly [`Self::num_factors`] entries.
    pub fn generate_correlated_factors(&self, independent_z: &[f64]) -> Result<Vec<f64>> {
        if independent_z.len() != self.num_factors {
            return Err(Error::FactorDrawLengthMismatch {
                expected: self.num_factors,
                actual: independent_z.len(),
            });
        }
        let mut out = vec![0.0; self.num_factors];
        let _ = self.cholesky_factor.apply(independent_z, &mut out);
        for (value, vol) in out.iter_mut().zip(self.volatilities.iter()) {
            *value *= *vol;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::math::linalg::CholeskyError;

    #[test]
    fn checked_constructors_preserve_zero_and_large_factor_inputs() {
        for volatility in [0.0, 0.001, 3.0] {
            let model = LatentSingleFactor::new(volatility, 20.0).expect("valid factors");
            assert_eq!(model.volatility(), volatility);
            assert_eq!(model.mean_reversion(), 20.0);
            let spec = LatentFactorSpec::single_factor(volatility, 20.0).expect("valid spec");
            let json = serde_json::to_value(&spec).expect("wire");
            let restored: LatentFactorSpec = serde_json::from_value(json).expect("valid wire");
            assert_eq!(restored, spec);
            let model = LatentMultiFactor::uncorrelated(1, vec![volatility]).expect("valid multi");
            assert_eq!(model.volatilities(), &[volatility]);
        }
        for correlation in [-1.0, 1.0] {
            let model = LatentTwoFactor::new(0.0, 3.0, correlation).expect("perfect correlation");
            assert_eq!(model.correlation(), correlation);
            assert_eq!(model.prepay_vol(), 0.0);
            assert_eq!(model.credit_vol(), 3.0);
            let model = LatentFactorKind::Two(model);
            assert_eq!(model.diagonal_factor_contribution(0, 2.0), 0.0);
            assert_eq!(model.diagonal_factor_contribution(1, 9.0), 0.0);
        }
    }

    #[test]
    fn invalid_factor_inputs_fail_constructors_specs_and_wire() {
        for bad in [-0.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(LatentSingleFactor::new(bad, 0.0).is_err());
            assert!(LatentSingleFactor::new(0.0, bad).is_err());
            assert!(LatentFactorSpec::single_factor(bad, 0.0).is_err());
            assert!(LatentTwoFactor::new(bad, 0.2, 0.0).is_err());
            assert!(LatentFactorSpec::two_factor(0.2, bad, 0.0).is_err());
        }
        for bad in [-1.1, 1.1, f64::NAN, f64::INFINITY] {
            assert!(LatentTwoFactor::new(0.2, 0.3, bad).is_err());
            assert!(LatentFactorSpec::two_factor(0.2, 0.3, bad).is_err());
        }
        for wire in [
            serde_json::json!({"type":"single_factor", "volatility":-0.1,"mean_reversion":0.0}),
            serde_json::json!({"type":"single_factor", "volatility":0.1,"mean_reversion":-1.0}),
            serde_json::json!({"type":"two_factor", "prepay_vol":0.1,"credit_vol":0.2,"correlation":1.1}),
            serde_json::json!({"type":"multi_factor", "num_factors":0,"volatilities":[],"correlations":[]}),
        ] {
            assert!(serde_json::from_value::<LatentFactorSpec>(wire).is_err());
        }
        assert!(LatentMultiFactor::uncorrelated(0, vec![]).is_err());
        assert!(LatentMultiFactor::new(
            2,
            vec![0.1, 0.2],
            vec![1.0, 1.0000000001, 1.0000000001, 1.0]
        )
        .is_err());
        assert!(LatentFactorSpec::SingleFactor {
            volatility: -0.1,
            mean_reversion: 0.0
        }
        .build()
        .is_err());
    }

    #[test]
    fn test_single_factor_creation() {
        let model = LatentSingleFactor::new(0.25, 0.1).expect("valid model inputs");
        let kind = LatentFactorKind::Single(model.clone());
        assert_eq!(kind.num_factors(), 1);
        assert!((model.volatility() - 0.25).abs() < 1e-10);
        assert_eq!(kind.factor_names(), vec!["Market"]);
    }

    #[test]
    fn test_two_factor_creation() {
        let model = LatentTwoFactor::new(0.20, 0.30, -0.30).expect("valid model inputs");
        assert_eq!(LatentFactorKind::Two(model.clone()).num_factors(), 2);
        assert!((model.prepay_vol() - 0.20).abs() < 1e-10);
        assert!((model.credit_vol() - 0.30).abs() < 1e-10);
        assert!((model.correlation() - (-0.30)).abs() < 1e-10);
    }

    #[test]
    fn test_diagonal_factor_contribution_single_factor() {
        let kind = LatentFactorKind::Single(
            LatentSingleFactor::new(1.0, 0.0).expect("valid model inputs"),
        );

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
        let kind = LatentFactorKind::Two(
            LatentTwoFactor::new(0.20, credit_vol, rho).expect("valid model inputs"),
        );

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
        let kind = LatentFactorKind::Two(
            LatentTwoFactor::new(0.20, 0.30, -0.30).expect("valid model inputs"),
        );
        let factor = kind.diagonal_factor_contribution(0, 1.5);
        assert!((factor - 0.30).abs() < 1e-10);
    }

    #[test]
    fn test_two_factor_correlation_matrix() {
        let kind = LatentFactorKind::Two(
            LatentTwoFactor::new(0.20, 0.30, -0.30).expect("valid model inputs"),
        );
        let corr = kind.correlation_matrix();

        assert!((corr[0] - 1.0).abs() < 1e-10);
        assert!((corr[3] - 1.0).abs() < 1e-10);

        assert!((corr[1] - (-0.30)).abs() < 1e-10);
        assert!((corr[2] - (-0.30)).abs() < 1e-10);
    }

    #[test]
    fn test_factor_spec_build() {
        let spec = LatentFactorSpec::single_factor(0.25, 0.1).expect("valid model inputs");
        let model = spec.build().expect("single-factor spec should build");
        assert_eq!(model.num_factors(), 1);
        assert!(matches!(model, LatentFactorKind::Single(_)));

        let spec = LatentFactorSpec::two_factor(0.20, 0.30, -0.30).expect("valid model inputs");
        let model = spec.build().expect("two-factor spec should build");
        assert_eq!(model.num_factors(), 2);
        assert!(matches!(model, LatentFactorKind::Two(_)));
    }

    #[test]
    fn test_standard_calibrations() {
        let rmbs = LatentTwoFactor::rmbs_standard().expect("valid model inputs");
        assert_eq!(LatentFactorKind::Two(rmbs.clone()).num_factors(), 2);
        assert!(rmbs.correlation() < 0.0); // Negative correlation

        let clo = LatentTwoFactor::clo_standard().expect("valid model inputs");
        assert_eq!(LatentFactorKind::Two(clo.clone()).num_factors(), 2);
        assert!(clo.correlation() < 0.0);
    }

    #[test]
    fn test_volatilities() {
        let kind = LatentFactorKind::Two(
            LatentTwoFactor::new(0.20, 0.30, -0.30).expect("valid model inputs"),
        );
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
        assert!(matches!(result, Err(CorrelationError::InvalidSize { .. })));
    }

    #[test]
    fn test_validate_diagonal_not_one() {
        let corr = vec![0.9, 0.5, 0.5, 1.0];
        let result = validate_correlation_matrix(&corr, 2);
        assert!(matches!(
            result,
            Err(CorrelationError::DiagonalNotOne { index: 0, .. })
        ));
    }

    #[test]
    fn test_validate_not_symmetric() {
        let corr = vec![1.0, 0.5, 0.3, 1.0]; // Off-diagonals don't match
        let result = validate_correlation_matrix(&corr, 2);
        assert!(matches!(result, Err(CorrelationError::NotSymmetric { .. })));
    }

    #[test]
    fn test_validate_not_psd() {
        // Non-PSD matrix: high correlations that violate PSD constraint
        let corr = vec![1.0, 0.9, 0.9, 0.9, 1.0, -0.5, 0.9, -0.5, 1.0];
        let result = validate_correlation_matrix(&corr, 3);
        assert!(matches!(
            result,
            Err(CorrelationError::NotPositiveSemiDefinite { .. })
        ));
    }

    #[test]
    fn test_validate_not_psd_reports_cholesky_failure_row() {
        let corr = vec![1.0, 0.9, 0.9, 0.9, 1.0, -0.5, 0.9, -0.5, 1.0];
        let result = validate_correlation_matrix(&corr, 3);
        let cholesky_result = cholesky_correlation(&corr, 3);

        match (result, cholesky_result) {
            (
                Err(CorrelationError::NotPositiveSemiDefinite { row: validate_row }),
                Err(CholeskyError::NotPositiveDefinite {
                    row: cholesky_row, ..
                }),
            ) => assert_eq!(validate_row, cholesky_row),
            other => panic!("expected matching PSD errors, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_out_of_bounds() {
        let corr = vec![1.0, 1.5, 1.5, 1.0];
        let result = validate_correlation_matrix(&corr, 2);
        assert!(matches!(result, Err(CorrelationError::OutOfBounds { .. })));
    }

    // ========== Cholesky Decomposition Tests ==========

    #[test]
    fn test_cholesky_dimension_mismatch_preserves_expected_and_actual() {
        let err = cholesky_correlation(&[1.0, 0.0, 0.0], 2)
            .expect_err("non-square flattened input should be rejected");

        assert!(matches!(
            err,
            CholeskyError::DimensionMismatch {
                expected: 2,
                actual: 3,
            }
        ));
    }

    #[test]
    fn test_cholesky_indefinite_matrix_preserves_failure_row_and_diagonal() {
        let err = cholesky_correlation(&[1.0, 0.9, 0.0, 0.9, 1.0, 0.9, 0.0, 0.9, 1.0], 3)
            .expect_err("indefinite matrix should be rejected");

        match err {
            CholeskyError::NotPositiveDefinite { diag, row } => {
                assert_eq!(row, 1);
                assert!(
                    (diag + 0.62).abs() < 1e-12,
                    "expected failing diagonal -0.62 at row 1, got {diag}"
                );
            }
            other => panic!("expected positive-definiteness error, got {other:?}"),
        }
    }

    #[test]
    fn test_cholesky_non_finite_input_preserves_location_and_value() {
        let err = cholesky_correlation(&[1.0, 0.0, f64::INFINITY, 1.0], 2)
            .expect_err("non-finite matrix entry should be rejected");

        assert!(matches!(
            err,
            CholeskyError::NonFiniteInput {
                value,
                row: 1,
                col: 0,
            } if value == f64::INFINITY
        ));
    }

    #[test]
    fn test_cholesky_identity() {
        let identity = vec![1.0, 0.0, 0.0, 1.0];
        let f = cholesky_correlation(&identity, 2).expect("identity matrix should decompose");
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
        let f = cholesky_correlation(&corr, 2).expect("2x2 correlation matrix should decompose");

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
    }

    #[test]
    fn test_cholesky_reconstructs_original() {
        let corr = vec![1.0, 0.5, 0.3, 0.5, 1.0, 0.4, 0.3, 0.4, 1.0];
        let f = cholesky_correlation(&corr, 3).expect("3x3 correlation matrix should decompose");
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

    // ========== LatentMultiFactor Tests ==========

    #[test]
    fn test_multi_factor_valid_matrix() {
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let vols = vec![0.2, 0.3];
        let model = LatentMultiFactor::new(2, vols, corr)
            .expect("valid 2x2 correlation matrix should create model");

        assert_eq!(LatentFactorKind::Multi(model.clone()).num_factors(), 2);
        assert!(model.cholesky_factor().factor_matrix().len() == 4);
    }

    #[test]
    fn test_multi_factor_invalid_matrix_returns_error() {
        // Invalid matrix (not symmetric)
        let corr = vec![1.0, 0.5, 0.3, 1.0];
        let vols = vec![0.2, 0.3];
        let err = LatentMultiFactor::new(2, vols, corr)
            .expect_err("invalid matrices should no longer silently fall back");

        assert!(matches!(
            err,
            Error::Matrix(CorrelationError::NotSymmetric { .. })
        ));
    }

    #[test]
    fn test_multi_factor_volatility_length_mismatch_returns_error() {
        // Regression: previously the caller's vols were silently replaced with
        // unit volatilities. Now it surfaces as an explicit error variant so
        // config-driven workflows can fail loudly.
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let wrong_vols = vec![0.2, 0.3, 0.4]; // len 3, but num_factors = 2
        let err = LatentMultiFactor::new(2, wrong_vols, corr)
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
    fn test_multi_factor_uncorrelated() {
        let model = LatentFactorKind::Multi(
            LatentMultiFactor::uncorrelated(3, vec![0.1, 0.2, 0.3]).expect("valid factors"),
        );

        assert_eq!(model.num_factors(), 3);

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
        let model = LatentMultiFactor::new(2, vols, corr.clone())
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
        let zeros = model
            .generate_correlated_factors(&[0.0, 0.0])
            .expect("valid draws");
        assert!(zeros[0].abs() < 1e-15);
        assert!(zeros[1].abs() < 1e-15);
    }

    #[test]
    fn test_generate_correlated_factors_with_volatility() {
        let corr = vec![1.0, 0.0, 0.0, 1.0]; // Identity
        let vols = vec![0.2, 0.3];
        let model = LatentMultiFactor::new(2, vols, corr)
            .expect("identity correlation model should create successfully");

        let factors = model
            .generate_correlated_factors(&[1.0, 1.0])
            .expect("valid draws");

        // With identity correlation: factors = z * vol
        assert!((factors[0] - 0.2).abs() < 1e-10);
        assert!((factors[1] - 0.3).abs() < 1e-10);
    }

    #[test]
    fn checked_multi_factor_inputs() {
        assert!(LatentMultiFactor::uncorrelated(2, vec![0.2]).is_err());
        for bad in [-0.1, f64::NAN, f64::INFINITY] {
            assert!(LatentMultiFactor::uncorrelated(2, vec![0.2, bad]).is_err());
        }
        let model = LatentMultiFactor::uncorrelated(2, vec![0.2, 0.3]).expect("valid factors");
        assert!(model.generate_correlated_factors(&[1.0]).is_err());
        assert_eq!(
            model
                .generate_correlated_factors(&[1.0, -1.0])
                .expect("valid draws"),
            vec![0.2, -0.3]
        );
    }

    #[test]
    fn test_diagonal_factor_contribution_multi_factor_uses_cholesky_diagonal() {
        let corr = vec![1.0, 0.6, 0.6, 1.0];
        let model = LatentMultiFactor::new(2, vec![0.2, 0.3], corr)
            .expect("correlated model should create successfully");

        let l_entry = model.cholesky_factor().factor_matrix()[3];
        let factor = LatentFactorKind::Multi(model).diagonal_factor_contribution(1, 2.0);
        let expected = 2.0 * l_entry * 0.3;
        assert!((factor - expected).abs() < 1e-10);
    }
}
