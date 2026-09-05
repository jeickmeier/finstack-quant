//! Polymorphic term-structure trait implementations.

use super::DiscountCurve;
use crate::dates::{Date, DayCount};
use crate::market_data::traits::Discounting;
use crate::types::CurveId;

impl Discounting for DiscountCurve {
    #[inline]
    fn id(&self) -> &CurveId {
        &self.id
    }

    #[inline]
    fn base_date(&self) -> Date {
        self.base
    }

    #[inline]
    fn df(&self, t: f64) -> f64 {
        DiscountCurve::df(self, t)
    }

    #[inline]
    fn day_count(&self) -> DayCount {
        self.day_count
    }
}
