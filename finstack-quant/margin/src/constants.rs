//! Constants for margin calculations.
//!
//! Financial constants used across margin computation modules,
//! following ISDA SIMM and industry standard conventions.

/// Standard days per year for year fraction calculations (ACT/365 Fixed).
///
/// Re-exported from `finstack_quant_core::dates::CALENDAR_DAYS_PER_YEAR`.
pub use finstack_quant_core::dates::CALENDAR_DAYS_PER_YEAR;

/// Duration approximation factor.
///
/// Approximates modified duration as `years_to_maturity * DURATION_FACTOR`
/// assuming reasonable yield levels (2-5% range).
pub const DURATION_APPROXIMATION_FACTOR: f64 = 0.9;

/// One basis point (0.01%).
///
/// Used for DV01 and CS01 calculations.
pub const ONE_BP: f64 = 0.0001;

/// Standard CDS maturity for SIMM bucketing.
///
/// 5Y is the most liquid CDS tenor and standard for SIMM sensitivity assignment.
pub const STANDARD_CDS_MATURITY_YEARS: f64 = 5.0;

/// Default bond index duration assumption.
///
/// Used when actual duration data is unavailable for fixed income indices.
pub const DEFAULT_BOND_INDEX_DURATION: f64 = 5.0;

/// SIMM tenor bucket boundaries in years.
pub mod tenor_buckets {
    /// 3 month bucket threshold (short-dated).
    pub const BUCKET_3M: f64 = 0.25;
    /// 6 month bucket threshold.
    pub const BUCKET_6M: f64 = 0.5;
    /// 1 year bucket threshold.
    pub const BUCKET_1Y: f64 = 1.0;
    /// 2 year bucket threshold.
    pub const BUCKET_2Y: f64 = 2.0;
    /// 3 year bucket threshold.
    pub const BUCKET_3Y: f64 = 3.0;
    /// 5 year bucket threshold.
    pub const BUCKET_5Y: f64 = 5.0;
    /// 10 year bucket threshold.
    pub const BUCKET_10Y: f64 = 10.0;
    /// 15 year bucket threshold.
    pub const BUCKET_15Y: f64 = 15.0;
    /// 20 year bucket threshold.
    pub const BUCKET_20Y: f64 = 20.0;
}

/// Default credit qualifying spread threshold in basis points.
///
/// Spreads below this level are typically considered investment grade.
pub const INVESTMENT_GRADE_SPREAD_THRESHOLD_BP: f64 = 200.0;
