//! Validation configuration for curves and surfaces.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

pub(crate) fn default_rate_bounds_policy_for_serde() -> RateBoundsPolicy {
    // plan-driven default: choose currency-aware bounds unless explicitly overridden.
    RateBoundsPolicy::AutoCurrency
}

/// Configurable bounds for forward/zero rates during calibration.
///
/// Different market regimes require different rate bounds:
/// - Developed markets (USD, EUR, GBP): typically [-2%, 50%]
/// - Negative rate environments (EUR, JPY, CHF): [-5%, 20%]
/// - Emerging markets (TRY, ARS, BRL): [-5%, 200%]
///
/// # Examples
///
/// ```
/// use finstack_quant_calibration::RateBounds;
/// use finstack_quant_core::currency::Currency;
///
/// // Use currency-specific defaults
/// let usd_bounds = RateBounds::for_currency(Currency::USD);
/// assert!(usd_bounds.min_rate < 0.0);
///
/// // Or customize for specific scenarios
/// let em_bounds = RateBounds::emerging_markets();
/// assert!(em_bounds.max_rate > 1.0);
/// ```
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RateBounds {
    /// Minimum allowed rate (decimal, e.g., -0.02 for -2%)
    pub min_rate: f64,
    /// Maximum allowed rate (decimal, e.g., 0.50 for 50%)
    pub max_rate: f64,
}

impl Default for RateBounds {
    fn default() -> Self {
        Self {
            min_rate: -0.02,
            max_rate: 0.50,
        }
    }
}

impl RateBounds {
    /// Validate bounds for consistency.
    ///
    /// # Errors
    ///
    /// Returns an error if either bound is non-finite or `min_rate > max_rate`.
    pub fn validate(&self) -> Result<()> {
        if !self.min_rate.is_finite() || !self.max_rate.is_finite() || self.min_rate > self.max_rate
        {
            return Err(Error::Validation(format!(
                "RateBounds invalid: min_rate ({}) and max_rate ({}) must be finite and min_rate <= max_rate",
                self.min_rate, self.max_rate
            )));
        }
        Ok(())
    }

    /// Construct explicit bounds with validation.
    ///
    /// # Errors
    ///
    /// Returns an error if either bound is non-finite or `min_rate > max_rate`.
    pub fn new(min_rate: f64, max_rate: f64) -> Result<Self> {
        let bounds = Self { min_rate, max_rate };
        bounds.validate()?;
        Ok(bounds)
    }

    /// Create rate bounds for a specific currency based on market conventions.
    ///
    /// - USD/CAD/AUD: Standard developed market bounds [-2%, 50%]
    /// - EUR/JPY/CHF: Extended negative rate support [-5%, 30%]
    /// - GBP: Standard with slightly wider negative [-3%, 50%]
    /// - TRY/ARS/BRL/ZAR: Emerging market bounds [-5%, 200%]
    /// - Other: Conservative developed market defaults
    pub fn for_currency(currency: Currency) -> Self {
        match currency {
            // Deep negative rate environments
            Currency::EUR | Currency::JPY | Currency::CHF => Self {
                min_rate: -0.05,
                max_rate: 0.30,
            },
            // Standard developed markets
            Currency::USD | Currency::CAD | Currency::AUD | Currency::NZD => Self {
                min_rate: -0.02,
                max_rate: 0.50,
            },
            // GBP slightly wider negative
            Currency::GBP => Self {
                min_rate: -0.03,
                max_rate: 0.50,
            },
            // Emerging markets with potential for high rates
            Currency::TRY | Currency::ARS | Currency::BRL | Currency::ZAR | Currency::MXN => {
                Self::emerging_markets()
            }
            // Default: conservative developed market
            _ => Self::default(),
        }
    }

    /// Rate bounds for emerging markets with potential hyperinflation.
    ///
    /// Allows rates up to 200% to accommodate countries like Turkey and Argentina.
    pub fn emerging_markets() -> Self {
        Self {
            min_rate: -0.05,
            max_rate: 2.00, // 200%
        }
    }
}

/// How `CalibrationConfig` obtains rate bounds.
///
/// Market-standard bounds depend on currency/market regime. `AutoCurrency` makes this choice
/// explicit and avoids relying on `RateBounds::default()` as an implicit assumption.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RateBoundsPolicy {
    /// Pick currency-specific bounds via `RateBounds::for_currency(currency)`.
    #[default]
    AutoCurrency,
    /// Use the explicit `CalibrationConfig.rate_bounds` values.
    Explicit,
}

impl std::fmt::Display for RateBoundsPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AutoCurrency => write!(f, "auto_currency"),
            Self::Explicit => write!(f, "explicit"),
        }
    }
}

/// Runtime validation behavior for arbitrage/consistency checks.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ValidationMode {
    /// Emit warnings (non-fatal) when validations fail.
    /// Useful for exploratory analysis or legacy data.
    Warn,
    /// Treat validation failures as hard errors.
    /// Recommended for production and strict pricing.
    Error,
}

impl std::fmt::Display for ValidationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Validation configuration for curve and surface sanity checks.
///
/// This structure defines the limits for various financial metrics
/// (forward rates, hazard rates, inflation growth) and toggles
/// for specific arbitrage and monotonicity checks.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ValidationConfig {
    /// Enable forward rate positivity check
    pub check_forward_positivity: bool,
    /// Minimum allowed forward rate (can be slightly negative)
    pub min_forward_rate: f64,
    /// Maximum allowed forward rate
    pub max_forward_rate: f64,
    /// Enable monotonicity checks
    pub check_monotonicity: bool,
    /// Enable arbitrage checks
    pub check_arbitrage: bool,
    /// Numerical tolerance for comparisons
    pub tolerance: f64,
    /// Maximum allowed hazard rate (default 0.5 = 50%)
    pub max_hazard_rate: f64,
    /// Minimum allowed annual CPI growth (default -0.10 = -10%)
    pub min_cpi_growth: f64,
    /// Maximum allowed annual CPI growth (default 0.50 = 50%)
    pub max_cpi_growth: f64,
    /// Minimum allowed forward inflation (default -0.20 = -20%)
    pub min_fwd_inflation: f64,
    /// Maximum allowed forward inflation (default 0.50 = 50%)
    pub max_fwd_inflation: f64,
    /// Maximum allowed volatility (default 5.0 = 500%)
    pub max_volatility: f64,
    /// Allow negative rate environments (DF > 1.0 at short end)
    #[serde(default)]
    pub allow_negative_rates: bool,
    /// When true, arbitrage violations (calendar/butterfly) produce warnings instead of errors.
    /// Default is false - arbitrage violations fail validation.
    /// Set to true only for exploratory analysis or when arbitrage-free fitting is not required.
    #[serde(default)]
    pub lenient_arbitrage: bool,
    /// Butterfly spread convexity tolerance ratio (upper bound).
    /// Actual variance must be <= interpolated * this ratio to pass.
    /// Default 1.10 (10% tolerance); use values closer to 1.0 for stricter checking.
    #[serde(default = "default_butterfly_upper_ratio")]
    pub butterfly_upper_ratio: f64,
    /// Butterfly spread convexity tolerance ratio (lower bound).
    /// Actual variance must be >= interpolated * this ratio to pass.
    /// Default 0.90 (10% tolerance); use values closer to 1.0 for stricter checking.
    #[serde(default = "default_butterfly_lower_ratio")]
    pub butterfly_lower_ratio: f64,
    /// Absolute tolerance for comparing configured and quoted recovery rates.
    #[serde(default = "default_recovery_rate_abs_tolerance")]
    pub recovery_rate_abs_tolerance: f64,
    /// Minimum LGD denominator used for hazard-rate initial guesses.
    #[serde(default = "default_minimum_lgd_for_hazard_guess")]
    pub minimum_lgd_for_hazard_guess: f64,
}

fn default_butterfly_upper_ratio() -> f64 {
    1.10 // Variance must be at most 10% above linear interpolation
}

fn default_butterfly_lower_ratio() -> f64 {
    0.90 // Variance must be at least 10% below linear interpolation
}

fn default_recovery_rate_abs_tolerance() -> f64 {
    1e-12
}

fn default_minimum_lgd_for_hazard_guess() -> f64 {
    1e-6
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            check_forward_positivity: true,
            min_forward_rate: -0.01, // Allow 1% negative
            max_forward_rate: 0.50,  // 50% cap
            check_monotonicity: true,
            check_arbitrage: true,
            tolerance: 1e-10,
            max_hazard_rate: 0.50,
            min_cpi_growth: -0.10,
            max_cpi_growth: 0.50,
            min_fwd_inflation: -0.20,
            max_fwd_inflation: 0.50,
            max_volatility: 5.0,
            // Default to strict mode: enforce monotonicity in positive-rate regimes.
            // Set to true for EUR/JPY/CHF negative-rate environments where DFs > 1.0 is valid.
            allow_negative_rates: false,
            // Default to strict mode: arbitrage violations fail validation.
            // Set to true only for exploratory analysis.
            lenient_arbitrage: false,
            butterfly_upper_ratio: default_butterfly_upper_ratio(),
            butterfly_lower_ratio: default_butterfly_lower_ratio(),
            recovery_rate_abs_tolerance: default_recovery_rate_abs_tolerance(),
            minimum_lgd_for_hazard_guess: default_minimum_lgd_for_hazard_guess(),
        }
    }
}

impl ValidationConfig {
    /// Validate configuration invariants.
    ///
    /// This is intentionally strict so that UI/binding layers can be thin and rely on
    /// core validation for consistent behavior across Rust/Python/WASM.
    ///
    /// # Errors
    ///
    /// Returns an error if any constraints are violated (e.g. min > max, non-positive tolerances).
    pub fn validate(&self) -> Result<()> {
        for (label, value) in [
            ("min_forward_rate", self.min_forward_rate),
            ("max_forward_rate", self.max_forward_rate),
            ("tolerance", self.tolerance),
            ("max_hazard_rate", self.max_hazard_rate),
            ("min_cpi_growth", self.min_cpi_growth),
            ("max_cpi_growth", self.max_cpi_growth),
            ("min_fwd_inflation", self.min_fwd_inflation),
            ("max_fwd_inflation", self.max_fwd_inflation),
            ("max_volatility", self.max_volatility),
            ("butterfly_upper_ratio", self.butterfly_upper_ratio),
            ("butterfly_lower_ratio", self.butterfly_lower_ratio),
        ] {
            if !value.is_finite() {
                return Err(Error::Validation(format!(
                    "ValidationConfig invalid: {label} must be finite, got {value}"
                )));
            }
        }
        if self.min_forward_rate > 0.0 {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: min_forward_rate must be <= 0.0, got {}",
                self.min_forward_rate
            )));
        }
        if self.max_forward_rate <= 0.0 {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: max_forward_rate must be > 0.0, got {}",
                self.max_forward_rate
            )));
        }
        if self.min_forward_rate > self.max_forward_rate {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: min_forward_rate ({}) must be <= max_forward_rate ({})",
                self.min_forward_rate, self.max_forward_rate
            )));
        }
        if self.tolerance <= 0.0 {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: tolerance must be > 0.0, got {}",
                self.tolerance
            )));
        }
        if self.max_hazard_rate <= 0.0 {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: max_hazard_rate must be > 0.0, got {}",
                self.max_hazard_rate
            )));
        }
        if self.min_cpi_growth > self.max_cpi_growth {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: min_cpi_growth ({}) must be <= max_cpi_growth ({})",
                self.min_cpi_growth, self.max_cpi_growth
            )));
        }
        if self.min_fwd_inflation > self.max_fwd_inflation {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: min_fwd_inflation ({}) must be <= max_fwd_inflation ({})",
                self.min_fwd_inflation, self.max_fwd_inflation
            )));
        }
        if self.max_volatility <= 0.0 {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: max_volatility must be > 0.0, got {}",
                self.max_volatility
            )));
        }
        if self.butterfly_lower_ratio < 0.0
            || self.butterfly_upper_ratio <= 0.0
            || self.butterfly_upper_ratio < self.butterfly_lower_ratio
        {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: butterfly_upper_ratio ({}) must be positive and >= non-negative butterfly_lower_ratio ({})",
                self.butterfly_upper_ratio, self.butterfly_lower_ratio
            )));
        }
        if !self.recovery_rate_abs_tolerance.is_finite() || self.recovery_rate_abs_tolerance < 0.0 {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: recovery_rate_abs_tolerance must be finite and non-negative, got {}",
                self.recovery_rate_abs_tolerance
            )));
        }
        if !self.minimum_lgd_for_hazard_guess.is_finite()
            || self.minimum_lgd_for_hazard_guess <= 0.0
        {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: minimum_lgd_for_hazard_guess must be finite and positive, got {}",
                self.minimum_lgd_for_hazard_guess
            )));
        }
        Ok(())
    }
}
