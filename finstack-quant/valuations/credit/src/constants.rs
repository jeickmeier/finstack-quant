//! Credit-derivative numerical, ISDA, and Bloomberg CDSO constants.

/// Numerical tolerance constants for credit spread comparisons.
pub mod numerical {
    /// Tolerance for comparing floating-point rates and spreads.
    ///
    /// Used when checking if two rates or spreads are equal (1e-12).
    pub const RATE_COMPARISON_TOLERANCE: f64 = 1e-12;
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
}

/// ISDA 2014 standard recovery conventions.
pub mod isda {
    /// Standard recovery rate for senior unsecured debt (40%).
    pub const STANDARD_RECOVERY_SENIOR: f64 = 0.40;
}

/// Credit-derivative pricing numerical thresholds.
pub mod credit {
    /// Survival probability floor below which a reference entity is treated as defaulted.
    pub const SURVIVAL_PROBABILITY_FLOOR: f64 = 1e-15;

    /// Par spread risky-annuity denominator tolerance below which par spread is undefined.
    pub const PAR_SPREAD_DENOM_TOLERANCE: f64 = 1e-12;
}
