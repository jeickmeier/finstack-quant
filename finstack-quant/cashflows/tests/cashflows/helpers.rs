//! Shared test utilities for cashflow tests.
//!
//! # Tolerance Conventions
//!
//! - `RATE_TOLERANCE` (1e-10): For rate/factor comparisons
//! - `FACTOR_TOLERANCE` (1e-12): For year fractions
//! - `financial_tolerance(notional)`: For money amounts
//!
//! # Test Curve Conventions
//!
//! - `FlatRateCurve`: Time-dependent DF = exp(-r*t), DF(0) = 1.0
//! - `FlatHazardRateCurve`: Time-dependent SP = exp(-lambda*t), SP(0) = 1.0

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::traits::{Discounting, Survival, TermStructure};
use finstack_quant_core::types::CurveId;
use time::Month;

/// Tolerance for rate and factor comparisons (e.g., CPR, SMM, DF, SP).
pub const RATE_TOLERANCE: f64 = 1e-10;

/// Tolerance for year fraction comparisons.
pub const FACTOR_TOLERANCE: f64 = 1e-12;

/// Calculate appropriate tolerance for financial amounts based on notional.
pub fn financial_tolerance(notional: f64) -> f64 {
    (notional.abs() * 1e-8).max(0.01)
}

/// Convenience date constructor for tests.
pub fn d(year: i32, month: u8, day: u8) -> Date {
    let month_enum = Month::try_from(month).unwrap_or_else(|_| {
        panic!(
            "Invalid month {} in date {}-{:02}-{:02}",
            month, year, month, day
        )
    });
    Date::from_calendar_date(year, month_enum, day)
        .unwrap_or_else(|_| panic!("Invalid date: {}-{:02}-{:02}", year, month, day))
}

/// Flat-rate discount curve with proper time-dependent discount factors.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FlatRateCurve {
    pub id: CurveId,
    pub base: Date,
    pub rate: f64,
}

impl FlatRateCurve {
    /// Create a new flat rate curve.
    #[allow(dead_code)]
    pub fn new(id: impl Into<String>, base: Date, rate: f64) -> Self {
        Self {
            id: CurveId::new(id),
            base,
            rate,
        }
    }
}

impl TermStructure for FlatRateCurve {
    fn id(&self) -> &CurveId {
        &self.id
    }
}

impl Discounting for FlatRateCurve {
    fn base_date(&self) -> Date {
        self.base
    }

    fn df(&self, t: f64) -> f64 {
        if t <= 0.0 {
            1.0
        } else {
            (-self.rate * t).exp()
        }
    }
}

/// Flat hazard rate curve with proper time-dependent survival probabilities.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FlatHazardRateCurve {
    pub id: CurveId,
    pub lambda: f64,
}

impl FlatHazardRateCurve {
    /// Create a new flat hazard rate curve.
    #[allow(dead_code)]
    pub fn new(id: impl Into<String>, lambda: f64) -> Self {
        Self {
            id: CurveId::new(id),
            lambda,
        }
    }
}

impl TermStructure for FlatHazardRateCurve {
    fn id(&self) -> &CurveId {
        &self.id
    }
}

impl Survival for FlatHazardRateCurve {
    fn sp(&self, t: f64) -> f64 {
        if t <= 0.0 {
            1.0
        } else {
            (-self.lambda * t).exp()
        }
    }
}
