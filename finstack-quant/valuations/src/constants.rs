//! Common constants used throughout the valuations crate.
//!
//! This module centralizes numerical constants to improve maintainability
//! and clarity across the codebase.
//!
//! # Rate Conversion: Type-Safe vs Raw Constants
//!
//! For rate/spread conversions, prefer the type-safe types from `finstack_quant_core::types`:
//! - [`finstack_quant_core::types::Rate`] — `Rate::from_percent(5.0).expect("valid rate fixture")`, `Rate::from_bp(500)`
//! - [`finstack_quant_core::types::Bps`] — `Bps::new(500).as_decimal()`
//! - [`finstack_quant_core::types::Percentage`] — `Percentage::new(5.0)?.as_decimal()`
//!
//! The raw `f64` constants below (`ONE_BASIS_POINT`, `BASIS_POINTS_PER_UNIT`, and
//! `DECIMAL_TO_PERCENT`) exist for use in **performance-sensitive inner loops** where:
//! - Values are already `f64` (e.g., from `Decimal::to_f64()` or curve interpolation)
//! - The `Rate`/`Bps` types are not in scope or would require `i32` truncation
//!   (e.g., fractional basis point spreads)
//! - Avoiding newtype construction overhead matters (tight pricing loops)
//!
//! When writing new code outside hot paths, prefer the core types for clarity and
//! compile-time unit safety.

/// One basis point (0.01%) as a decimal.
///
/// Use this constant instead of hardcoded `0.0001` or `1e-4` for clarity
/// when calculating sensitivity to 1bp changes in rates, spreads, etc.
///
/// **Prefer [`finstack_quant_core::types::Bps`]** for type-safe basis point arithmetic
/// outside hot paths. This raw constant is appropriate for sensitivity bumps
/// (DV01, CS01) and conversions in tight pricing loops where values are already `f64`.
///
/// # Examples
/// ```rust
/// use finstack_quant_valuations::constants::ONE_BASIS_POINT;
///
/// let rate_change = 100.0 * ONE_BASIS_POINT; // 1% or 100bp
/// let notional = 1_000_000.0;
/// let duration = 5.0;
/// let dv01 = notional * duration * ONE_BASIS_POINT;
/// assert_eq!(dv01, 500.0);
/// ```
pub const ONE_BASIS_POINT: f64 = 0.0001;

/// Basis points per unit (inverse of ONE_BASIS_POINT).
///
/// Use this to convert decimals to basis points in performance-sensitive code.
///
/// **Prefer [`finstack_quant_core::types::Rate::as_bp()`]** or
/// [`finstack_quant_core::types::Bps`] for type-safe conversions outside hot paths.
///
/// # Examples
/// ```rust
/// use finstack_quant_valuations::constants::BASIS_POINTS_PER_UNIT;
///
/// let spread_decimal = 0.0025;
/// let spread_bp = spread_decimal * BASIS_POINTS_PER_UNIT; // 25bp
/// ```
pub const BASIS_POINTS_PER_UNIT: f64 = 10_000.0;

/// Convert decimal to percentage (0.01 = 1%).
///
/// **Prefer [`finstack_quant_core::types::Rate::as_percent()`]** or
/// [`finstack_quant_core::types::Percentage`] for type-safe conversions outside hot paths.
///
/// # Examples
/// ```rust
/// use finstack_quant_valuations::constants::DECIMAL_TO_PERCENT;
///
/// let rate_decimal = 0.05;
/// let rate_pct = rate_decimal * DECIMAL_TO_PERCENT; // 5.0%
/// ```
pub const DECIMAL_TO_PERCENT: f64 = 100.0;

/// Numerical constants for floating-point comparisons and integration.
///
/// These constants replace magic numbers scattered throughout the codebase,
/// providing consistent tolerances and step sizes with documented rationale.
pub mod numerical {
    /// Tolerance for checking if a value is effectively zero (1e-10).
    ///
    /// Re-exported from [`finstack_quant_core::math::ZERO_TOLERANCE`].
    pub use finstack_quant_core::math::ZERO_TOLERANCE;

    /// Tolerance for iterative solver convergence (bootstrap, calibration).
    ///
    /// Used as the convergence criterion for root-finding algorithms like
    /// Brent's method: stop when |f(x)| < SOLVER_TOLERANCE.
    ///
    /// Value: 1e-8 (tight enough for financial precision while avoiding
    /// excessive iterations for well-conditioned problems).
    pub const SOLVER_TOLERANCE: f64 = 1e-8;

    /// Tolerance for comparing floating-point rates and spreads.
    ///
    /// Used when checking if two rates are "equal" for purposes like
    /// detecting unchanged spreads or matching calibration targets.
    ///
    /// Value: 1e-12 (tighter than ZERO_TOLERANCE because rates are typically
    /// O(0.01) to O(0.1), so relative precision matters more).
    pub const RATE_COMPARISON_TOLERANCE: f64 = 1e-12;

    // NOTE: A `DIVISION_EPSILON` constant was intentionally removed. The
    // `numerator / (denominator + epsilon)` idiom it encouraged is a weak
    // guard — for a denominator of exactly 0 it still yields `numerator * 1e15`
    // (garbage) rather than a clean error. Guard small denominators with an
    // explicit `if denom.abs() < tol { ... }` check instead.

    /// Minimum threshold for discount factor values to avoid numerical instability.
    ///
    /// Set to 1e-10 to protect against division by near-zero discount factors
    /// that can arise from extreme rate scenarios or very long time horizons.
    ///
    /// # Numerical Justification
    ///
    /// For extreme rate scenarios:
    /// - At +50% rates over 50 years: DF ≈ exp(-0.50 × 50) = exp(-25) ≈ 1.4e-11
    /// - At +60% rates over 50 years: DF ≈ exp(-0.60 × 50) = exp(-30) ≈ 9.4e-14
    ///
    /// The threshold of 1e-10 catches pathological cases while allowing reasonable
    /// stress testing up to ~48% rates over 50 years or ~96% over 25 years.
    /// This aligns with ISDA stress testing requirements for rates ranging
    /// from -10% to +50%.
    pub const DF_EPSILON: f64 = 1e-10;
}

/// Bloomberg CDSO numerical-quadrature conventions.
pub mod bloomberg_cdso {
    /// Bloomberg CDSO calendar-day denominator (DOCS 2055833 §2.1).
    pub const G_DAYS_IN_YEAR: f64 = 365.0;

    /// Bloomberg CDSO theta day basis (DOCS 2055833 §2.5).
    ///
    /// The 0.25-day inconsistency with [`G_DAYS_IN_YEAR`] is faithful to the
    /// published convention: theta shortens exercise time by `1/365.25`.
    pub const THETA_DAYS_IN_YEAR: f64 = 365.25;

    /// Minimum standard-normal quadrature half-width for the Bloomberg CDSO
    /// driver grid.
    pub const MIN_Z_LIMIT: f64 = 6.0;

    /// Step size for the Bloomberg CDSO standard-normal quadrature grid.
    pub const Z_STEP: f64 = 0.05;

    /// CDX option front-end-protection fallback start lag in business days.
    pub const INDEX_OPTION_FEP_START_LAG_BD: i32 = 2;
}

/// ISDA 2014 standard recovery constant used by the engine.
pub mod isda {
    /// Standard recovery rate for senior unsecured (40%)
    pub const STANDARD_RECOVERY_SENIOR: f64 = 0.40;
}

/// US-market business-days-per-year convention.
pub mod time {
    /// Business days per year for North America (US markets)
    pub const BUSINESS_DAYS_PER_YEAR_US: f64 = 252.0;
}

/// Credit derivatives specific constants
pub mod credit {
    /// Survival probability floor for numerical stability.
    ///
    /// When computing conditional survival probabilities (S(t)/S(t0)), if S(t0)
    /// falls below this threshold, we treat the entity as already defaulted.
    /// This prevents division by near-zero values producing inf/NaN.
    ///
    /// Value: 1e-15 (well above f64 machine epsilon ~2.2e-16, allowing for
    /// cumulative multiplication errors in survival probability calculations).
    pub const SURVIVAL_PROBABILITY_FLOOR: f64 = 1e-15;

    /// Par spread denominator tolerance.
    ///
    /// If the risky annuity (denominator) is below this, par spread is undefined.
    pub const PAR_SPREAD_DENOM_TOLERANCE: f64 = 1e-12;

    /// Pool-size threshold for exact convolution vs the moment-matched
    /// normal approximation in heterogeneous CDS-tranche pricing.
    ///
    /// Portfolios with this many or fewer constituents use exact convolution;
    /// larger pools use the moment-matched normal (CLT) approximation of the
    /// conditional loss distribution.
    ///
    /// The threshold is set from a measured bias study (2026-07
    /// credit-derivatives audit, junior \[3,7\] tranche on dispersed-hazard
    /// pools, normal approx vs exact-convolution PV):
    ///
    /// | Pool size | Relative PV bias |
    /// |-----------|------------------|
    /// | 24        | 1.55%            |
    /// | 40        | 1.22%            |
    /// | 64        | 0.20%            |
    /// | 100       | 0.11%            |
    /// | 125       | 0.03%            |
    ///
    /// At 64 names the exact convolution remains cheap (O(n · loss-buckets)
    /// per quadrature node) while the CLT error is already inside typical
    /// quoting precision; below it the approximation error is material.
    pub const SMALL_POOL_THRESHOLD: usize = 64;

    /// Calendar days per year for settlement delay calculations.
    ///
    /// Re-exported from `finstack_quant_core::dates::CALENDAR_DAYS_PER_YEAR` (ACT/365 Fixed).
    pub use finstack_quant_core::dates::CALENDAR_DAYS_PER_YEAR;
}

/// Standard number of trading/business days per year (US equity markets).
///
/// Alias for [`time::BUSINESS_DAYS_PER_YEAR_US`] (252). This is the conventional
/// denominator for annualizing daily equity and FX option statistics (realized vol,
/// Sharpe ratio, theta decay, etc.).
pub const TRADING_DAYS_PER_YEAR: f64 = time::BUSINESS_DAYS_PER_YEAR_US;

/// Default day-count denominator for per-day theta in the closed-form Greeks.
///
/// ACT/365 calendar-day theta is the market default for equity and FX options;
/// pass [`TRADING_DAYS_PER_YEAR`] (252) instead for business-day-scaled theta.
/// Both host bindings' `theta_days` parameters default to this constant, so
/// the value has a single home in Rust.
pub const DEFAULT_THETA_DAYS_PER_YEAR: f64 = 365.0;
