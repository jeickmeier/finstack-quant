//! Polymorphic term-structure trait implementations.

use super::DiscountCurve;
use crate::dates::{Date, DayCount, DayCountContext};
use crate::market_data::traits::Discounting;
use crate::types::CurveId;

impl Discounting for DiscountCurve {
    #[inline]
    fn id(&self) -> &CurveId {
        &self.id
    }

    #[inline]
    fn base_date(&self) -> Date {
        self.core.base
    }

    #[inline]
    fn df(&self, t: f64) -> crate::Result<f64> {
        DiscountCurve::df(self, t)
    }

    #[inline]
    fn day_count(&self) -> DayCount {
        self.core.day_count
    }

    #[inline]
    fn day_count_context(&self) -> DayCountContext<'_> {
        DiscountCurve::day_count_context(self)
    }

    #[inline]
    fn df_on_date(&self, date: Date) -> crate::Result<f64> {
        DiscountCurve::df_on_date(self, date)
    }

    #[inline]
    fn instantaneous_forward(&self, t: f64) -> crate::Result<f64> {
        DiscountCurve::instantaneous_forward(self, t)
    }
}
