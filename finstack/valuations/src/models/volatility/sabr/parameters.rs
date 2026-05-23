use finstack_core::{Error, Result};

/// SABR model parameters
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SABRParameters {
    /// Initial volatility (alpha)
    pub alpha: f64,
    /// CEV exponent (beta) - typically 0 to 1
    pub beta: f64,
    /// Volatility of volatility (nu/volvol)
    pub nu: f64,
    /// Correlation between asset and volatility (rho)
    pub rho: f64,
    /// Shift parameter for handling negative rates (optional)
    pub shift: Option<f64>,
}

impl SABRParameters {
    /// Create new SABR parameters with validation.
    ///
    /// Enforces market-standard parameter bounds:
    /// - α (alpha) > 0: Initial volatility must be positive
    /// - β (beta) ∈ [0, 1]: CEV exponent (0=normal, 1=lognormal)
    /// - ν (nu) ≥ 0: Volatility of volatility must be non-negative
    /// - ρ (rho) ∈ [-1, 1]: Correlation must be valid
    pub fn new(alpha: f64, beta: f64, nu: f64, rho: f64) -> Result<Self> {
        // Validate parameters with descriptive error messages
        if alpha <= 0.0 {
            return Err(Error::Validation(format!(
                "SABR parameter α (alpha) must be positive, got: {:.6}",
                alpha
            )));
        }
        if !(0.0..=1.0).contains(&beta) {
            return Err(Error::Validation(format!(
                "SABR parameter β (beta) must be in [0, 1], got: {:.6}",
                beta
            )));
        }
        if nu < 0.0 {
            return Err(Error::Validation(format!(
                "SABR parameter ν (nu) must be non-negative, got: {:.6}",
                nu
            )));
        }
        if !(-1.0..=1.0).contains(&rho) {
            return Err(Error::Validation(format!(
                "SABR parameter ρ (rho) must be in [-1, 1], got: {:.6}",
                rho
            )));
        }

        Ok(Self {
            alpha,
            beta,
            nu,
            rho,
            shift: None,
        })
    }

    /// Equity market standard (beta = 1.0).
    ///
    /// # Errors
    ///
    /// Returns an error if any SABR parameter is invalid.
    pub fn equity_standard(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        Self::new(alpha, 1.0, nu, rho)
    }

    /// Rates market standard (beta = 0.5).
    ///
    /// # Errors
    ///
    /// Returns an error if any SABR parameter is invalid.
    pub fn rates_standard(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        Self::new(alpha, 0.5, nu, rho)
    }

    /// Create new SABR parameters with shift for negative rates.
    ///
    /// Same validation as `new()` plus shift validation:
    /// - shift > 0: Shift must be positive for negative rate support
    pub fn new_with_shift(alpha: f64, beta: f64, nu: f64, rho: f64, shift: f64) -> Result<Self> {
        // Validate base parameters via new(), then set shift
        let mut params = Self::new(alpha, beta, nu, rho)?;

        // Shift should be positive to handle negative rates
        if shift <= 0.0 {
            return Err(Error::Validation(format!(
                "SABR shift parameter must be positive for negative rate support, got: {:.6}",
                shift
            )));
        }

        params.shift = Some(shift);
        Ok(params)
    }

    /// Create parameters for normal SABR (beta = 0)
    pub fn normal(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        Self::new(alpha, 0.0, nu, rho)
    }

    /// Create parameters for lognormal SABR (beta = 1)
    pub fn lognormal(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        Self::new(alpha, 1.0, nu, rho)
    }

    /// Create parameters for shifted normal SABR (beta = 0, with shift)
    pub fn shifted_normal(alpha: f64, nu: f64, rho: f64, shift: f64) -> Result<Self> {
        Self::new_with_shift(alpha, 0.0, nu, rho, shift)
    }

    /// Create parameters for shifted lognormal SABR (beta = 1, with shift)
    pub fn shifted_lognormal(alpha: f64, nu: f64, rho: f64, shift: f64) -> Result<Self> {
        Self::new_with_shift(alpha, 1.0, nu, rho, shift)
    }

    /// Create default equity-standard SABR parameters.
    ///
    /// Returns parameters suitable as a starting point for equity options:
    /// - α = 0.20 (20% initial volatility)
    /// - β = 1.0 (lognormal, standard for equities)
    /// - ν = 0.30 (30% vol-of-vol)
    /// - ρ = -0.20 (mild negative correlation, typical equity skew)
    ///
    /// These are reasonable defaults for calibration initialization but
    /// should always be calibrated to market data for production use.
    #[must_use]
    pub const fn equity_default() -> Self {
        Self {
            alpha: 0.20,
            beta: 1.0,
            nu: 0.30,
            rho: -0.20,
            shift: None,
        }
    }

    /// Create default rates-standard SABR parameters.
    ///
    /// Returns parameters suitable as a starting point for rates options:
    /// - α = 0.02 (2% normal vol level, typical for rates)
    /// - β = 0.5 (mixed normal/lognormal, common for rates)
    /// - ν = 0.30 (30% vol-of-vol)
    /// - ρ = 0.0 (neutral correlation as starting point)
    ///
    /// These are reasonable defaults for calibration initialization but
    /// should always be calibrated to market data for production use.
    #[must_use]
    pub const fn rates_default() -> Self {
        Self {
            alpha: 0.02,
            beta: 0.5,
            nu: 0.30,
            rho: 0.0,
            shift: None,
        }
    }

    /// Get the shift parameter
    pub fn shift(&self) -> Option<f64> {
        self.shift
    }

    /// Check if this is a shifted SABR model
    pub fn is_shifted(&self) -> bool {
        self.shift.is_some()
    }
}
