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
/// use finstack_quant_valuations::calibration::RateBounds;
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
    /// Returns an error if `min_rate > max_rate`.
    pub fn validate(&self) -> Result<()> {
        if self.min_rate > self.max_rate {
            return Err(Error::Validation(format!(
                "RateBounds invalid: min_rate ({}) must be <= max_rate ({})",
                self.min_rate, self.max_rate
            )));
        }
        Ok(())
    }

    /// Construct explicit bounds with validation.
    ///
    /// # Errors
    ///
    /// Returns an error if `min_rate > max_rate`.
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

    /// Rate bounds for negative rate environments.
    ///
    /// Optimized for EUR/JPY/CHF where deeply negative rates are common.
    pub fn negative_rate_environment() -> Self {
        Self {
            min_rate: -0.10, // -10%
            max_rate: 0.20,  // 20%
        }
    }

    /// Rate bounds for stress testing scenarios.
    ///
    /// Very wide bounds to allow extreme scenarios.
    pub fn stress_test() -> Self {
        Self {
            min_rate: -0.20, // -20%
            max_rate: 5.00,  // 500%
        }
    }

    /// Check if a rate is within bounds.
    #[inline]
    pub fn contains(&self, rate: f64) -> bool {
        rate >= self.min_rate && rate <= self.max_rate
    }

    /// Clamp a rate to be within bounds.
    #[inline]
    pub fn clamp(&self, rate: f64) -> f64 {
        rate.clamp(self.min_rate, self.max_rate)
    }
}

/// How `CalibrationConfig` obtains rate bounds.
///
/// Market-standard bounds depend on currency/market regime. `AutoCurrency` makes this choice
/// explicit and avoids relying on `RateBounds::default()` as an implicit assumption.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
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

impl std::str::FromStr for RateBoundsPolicy {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "auto_currency" | "autocurrency" | "auto" => Ok(Self::AutoCurrency),
            "explicit" => Ok(Self::Explicit),
            other => Err(format!(
                "Unknown rate bounds policy: '{}'. Valid: auto_currency, auto, explicit",
                other
            )),
        }
    }
}

/// Runtime validation behavior for arbitrage/consistency checks.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

impl std::str::FromStr for ValidationMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "warn" | "warning" => Ok(Self::Warn),
            "error" | "strict" => Ok(Self::Error),
            other => Err(format!(
                "Unknown validation mode: '{}'. Valid: warn, warning, error, strict",
                other
            )),
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
    /// Create a strict validation config that enforces monotonicity
    /// even in potentially negative rate environments.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            allow_negative_rates: false,
            lenient_arbitrage: false,
            ..Default::default()
        }
    }

    /// Create a permissive validation config for negative rate environments
    /// (e.g., EUR/JPY/CHF) where discount factors > 1.0 at short tenors is valid.
    #[must_use]
    pub fn negative_rates() -> Self {
        Self {
            allow_negative_rates: true,
            ..Default::default()
        }
    }

    /// Create a lenient configuration that warns but does not fail on arbitrage.
    ///
    /// Use this only for exploratory analysis or when strict arbitrage-free
    /// surfaces are not required. Calendar spread and butterfly arbitrage
    /// violations will log warnings instead of returning errors.
    #[must_use]
    pub fn lenient() -> Self {
        Self {
            lenient_arbitrage: true,
            ..Default::default()
        }
    }

    /// Set whether arbitrage violations should warn (lenient) or error (strict).
    ///
    /// By default, arbitrage violations fail validation. Set `lenient = true`
    /// only for exploratory analysis or when arbitrage-free constraints are
    /// not required.
    #[must_use]
    pub fn with_lenient_arbitrage(mut self, lenient: bool) -> Self {
        self.lenient_arbitrage = lenient;
        self
    }

    /// Validate configuration invariants.
    ///
    /// This is intentionally strict so that UI/binding layers can be thin and rely on
    /// core validation for consistent behavior across Rust/Python/WASM.
    ///
    /// # Errors
    ///
    /// Returns an error if any constraints are violated (e.g. min > max, non-positive tolerances).
    pub fn validate(&self) -> Result<()> {
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
        if self.butterfly_upper_ratio < self.butterfly_lower_ratio {
            return Err(Error::Validation(format!(
                "ValidationConfig invalid: butterfly_upper_ratio ({}) must be >= butterfly_lower_ratio ({})",
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn validation_mode_fromstr_display_roundtrip() {
        fn assert_validation_mode(label: &str, expected: ValidationMode) {
            assert!(matches!(ValidationMode::from_str(label), Ok(value) if value == expected));
        }

        let variants = [ValidationMode::Warn, ValidationMode::Error];
        for v in variants {
            let s = v.to_string();
            let parsed = ValidationMode::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        // Test aliases
        assert_validation_mode("warning", ValidationMode::Warn);
        assert_validation_mode("strict", ValidationMode::Error);
        assert!(ValidationMode::from_str("invalid").is_err());
    }

    #[test]
    fn rate_bounds_policy_fromstr_display_roundtrip() {
        fn assert_rate_bounds_policy(label: &str, expected: RateBoundsPolicy) {
            assert!(matches!(RateBoundsPolicy::from_str(label), Ok(value) if value == expected));
        }

        let variants = [RateBoundsPolicy::AutoCurrency, RateBoundsPolicy::Explicit];
        for v in variants {
            let s = v.to_string();
            let parsed = RateBoundsPolicy::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        // Test aliases
        assert_rate_bounds_policy("auto", RateBoundsPolicy::AutoCurrency);
        assert!(RateBoundsPolicy::from_str("invalid").is_err());
    }
}
