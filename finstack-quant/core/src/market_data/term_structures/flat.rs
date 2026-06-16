//! Flat forward/discount curve implementation.
//!
//! A simple curve where the discount rate (continuously compounded) is constant
//! across all tenors. Useful for:
//! - Approximate valuations
//! - Performance metrics (NPV/IRR) where a single rate is assumed
//! - Testing and validation
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::market_data::term_structures::FlatCurve;
//! use finstack_quant_core::market_data::traits::Discounting;
//! use finstack_quant_core::dates::{Date, DayCount};
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
//! let curve = FlatCurve::new(0.05, base, DayCount::Act365F, "FLAT-5%");
//!
//! // Discount factor at 1 year = e^(-0.05 * 1.0) ≈ 0.9512
//! let df = curve.df(1.0);
//! assert!((df - (-0.05_f64).exp()).abs() < 1e-10);
//! ```

use crate::dates::{Date, DayCount};
use crate::market_data::traits::{Discounting, TermStructure};
use crate::types::CurveId;

/// A term structure with a constant continuously compounded rate.
#[derive(Debug, Clone)]
pub struct FlatCurve {
    id: CurveId,
    rate: f64,
    base_date: Date,
    day_count: DayCount,
}

impl FlatCurve {
    /// Create a new flat curve with constant rate.
    ///
    /// # Arguments
    /// * `rate` - Continuously compounded annual rate (decimal, e.g. 0.05)
    /// * `base_date` - Reference date for the curve
    /// * `day_count` - Day count convention for year fractions
    /// * `id` - Identifier for the curve
    pub fn new(rate: f64, base_date: Date, day_count: DayCount, id: impl Into<String>) -> Self {
        Self {
            id: CurveId::new(id),
            rate,
            base_date,
            day_count,
        }
    }

    /// Update the constant rate.
    pub fn set_rate(&mut self, rate: f64) {
        self.rate = rate;
    }

    /// Get the current rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }
}

impl TermStructure for FlatCurve {
    fn id(&self) -> &CurveId {
        &self.id
    }
}

impl Discounting for FlatCurve {
    fn base_date(&self) -> Date {
        self.base_date
    }

    fn day_count(&self) -> DayCount {
        self.day_count
    }

    fn df(&self, t: f64) -> f64 {
        (-self.rate * t).exp()
    }
}
