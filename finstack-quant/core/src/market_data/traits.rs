//! Minimal traits for market data polymorphism.
//!
//! Defines lightweight trait interfaces for discount curves, forward curves,
//! and survival curves. These traits enable polymorphic pricing code while
//! keeping most functionality as concrete methods for better discoverability.
//!
//! # Design Philosophy
//!
//! - **Minimal trait surface**: Only methods needed for polymorphism
//! - **Concrete types preferred**: Most methods live on concrete curve types
//! - **Zero-cost abstraction**: Trait objects use dynamic dispatch only when needed

use crate::dates::{Date, DayCount, DayCountContext};
use crate::math::interp::exact_f64_eq;

// Minimal traits for polymorphism only

/// Minimal trait for discount curve polymorphism.
///
/// Only implement this where you need to accept different discount curve types.
/// Most code should use concrete curve types directly for better discoverability.
///
/// # Required Methods
///
/// - [`id`](Self::id) - Unique curve identifier
/// - [`base_date`](Self::base_date) - Valuation anchor date
/// - [`df`](Self::df) - Discount factor at source-clock time `t`
/// - [`instantaneous_forward`](Self::instantaneous_forward) - Analytic `-D'(t) / D(t)`
///
/// # Provided Methods
///
/// - [`day_count`](Self::day_count) - Day count convention (default: Act365F)
/// - [`day_count_context`](Self::day_count_context) - Day-count context
/// - [`df_on_date`](Self::df_on_date) - Discount factor on a calendar date
/// - [`df_between_dates`](Self::df_between_dates) - Relative discount factor between dates
/// - [`df_between_times`](Self::df_between_times) - Relative discount factor between times
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::traits::Discounting;
/// use finstack_quant_core::types::CurveId;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::Result;
/// use time::macros::date;
///
/// struct FlatCurve {
///     id: CurveId,
///     rate: f64,
/// }
///
/// impl FlatCurve {
///     fn new(id: &str, rate: f64) -> Self {
///         Self { id: CurveId::from(id), rate }
///     }
/// }
///
/// impl Discounting for FlatCurve {
///     fn id(&self) -> &CurveId { &self.id }
///     fn base_date(&self) -> Date {
///         date!(2025 - 01 - 01)
///     }
///     fn df(&self, t: f64) -> Result<f64> {
///         Ok((-self.rate * t).exp())
///     }
///     fn instantaneous_forward(&self, _t: f64) -> Result<f64> {
///         Ok(self.rate)
///     }
/// }
///
/// let curve = FlatCurve::new("USD", 0.03);
/// assert!(curve.df(1.0)? < 1.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub trait Discounting: Send + Sync {
    /// Unique identifier of the curve.
    fn id(&self) -> &crate::types::CurveId;

    /// Base (valuation) date of the curve.
    fn base_date(&self) -> Date;

    /// Discount factor at time `t` (year fraction from [`base_date`](Self::base_date)).
    ///
    /// # Arguments
    ///
    /// * `t` - Finite curve time coordinate in years.
    ///
    /// # Errors
    ///
    /// Returns an error if `t` is non-finite, outside the curve's domain or
    /// tail policy, or evaluates to a non-finite or non-positive factor.
    fn df(&self, t: f64) -> crate::Result<f64>;

    /// Day count convention used by the curve for time-to-maturity calculations.
    ///
    /// **Important**: For consistent pricing, code that discounts cashflows should
    /// use the curve's day count (via this method) rather than the instrument's
    /// accrual day count.
    fn day_count(&self) -> DayCount {
        DayCount::Act365F
    }

    /// Day-count context of the curve's own time axis.
    ///
    /// Supplies the holiday calendar a business-day day count (Bus/252) needs
    /// to map dates to curve time. The context belongs to the curve, never to
    /// the instrument being discounted. Defaults to no calendar and no coupon
    /// metadata.
    fn day_count_context(&self) -> DayCountContext<'_> {
        DayCountContext::default()
    }

    /// Discount factor on a specific calendar date using the curve's clock.
    ///
    /// Conditioned views override this method to map `date` on their retained
    /// original source clock.
    ///
    /// # Arguments
    ///
    /// * `date` - Target calendar date evaluated under the curve's day-count and
    ///   context.
    ///
    /// # Errors
    ///
    /// Returns an error if day-count calculation fails or the factor at the
    /// mapped coordinate cannot be admitted.
    fn df_on_date(&self, date: Date) -> crate::Result<f64> {
        let t = self.day_count().signed_year_fraction(
            self.base_date(),
            date,
            self.day_count_context(),
        )?;
        self.df(t)
    }

    /// Discount factor from `from` to `to` using the curve's day-count.
    ///
    /// Evaluates and admits both endpoint discount factors before checking the
    /// equal-date identity, returning `DF(0->to) / DF(0->from)`.
    ///
    /// # Arguments
    ///
    /// * `from` - Start date of the relative discounting interval.
    /// * `to` - End date of the interval; an earlier date inverts the ratio.
    ///
    /// # Errors
    ///
    /// Returns an error if day-count calculation fails, either endpoint factor
    /// cannot be admitted, or their quotient overflows or underflows to zero.
    #[inline]
    fn df_between_dates(&self, from: Date, to: Date) -> crate::Result<f64> {
        let df_from = self.df_on_date(from)?;
        if !df_from.is_finite() || df_from <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor on 'from' date ({from}): {df_from}"
            )));
        }

        let df_to = self.df_on_date(to)?;
        if !df_to.is_finite() || df_to <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor on 'to' date ({to}): {df_to}"
            )));
        }

        if from == to {
            return Ok(1.0);
        }

        let relative_df = df_to / df_from;
        if !relative_df.is_finite() || relative_df <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid relative discount factor from {from} to {to}: {relative_df}"
            )));
        }
        Ok(relative_df)
    }

    /// Discount factor from `from_t` to `to_t` where `t` is year-fraction
    /// from [`base_date`](Self::base_date).
    ///
    /// Evaluates and admits both endpoint discount factors before checking the
    /// equal-coordinate identity, returning `DF(to_t) / DF(from_t)`.
    ///
    /// # Arguments
    ///
    /// * `from_t` - Finite start year fraction from [`base_date`](Self::base_date).
    /// * `to_t` - Finite end year fraction from [`base_date`](Self::base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if either time is non-finite, either discount factor
    /// cannot be admitted, or their quotient overflows or underflows to zero.
    #[inline]
    fn df_between_times(&self, from_t: f64, to_t: f64) -> crate::Result<f64> {
        if !from_t.is_finite() || !to_t.is_finite() {
            return Err(crate::Error::Validation(format!(
                "df_between_times requires finite times, got from_t={from_t}, to_t={to_t}"
            )));
        }

        let df_from = self.df(from_t)?;
        if !df_from.is_finite() || df_from <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor at 'from_t' ({from_t}): {df_from}"
            )));
        }

        let df_to = self.df(to_t)?;
        if !df_to.is_finite() || df_to <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor at 'to_t' ({to_t}): {df_to}"
            )));
        }

        if exact_f64_eq(from_t, to_t) {
            return Ok(1.0);
        }

        let relative_df = df_to / df_from;
        if !relative_df.is_finite() || relative_df <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid relative discount factor from {from_t} to {to_t}: {relative_df}"
            )));
        }
        Ok(relative_df)
    }

    /// Analytic instantaneous forward rate `-D'(t) / D(t)` at time `t`.
    ///
    /// Uses the derivative of the curve's actual evaluator with one-sided
    /// limits at knots and adjustment breakpoints (inward right at the source
    /// origin, left interval at interior breakpoints, inward left at the last
    /// pillar).
    ///
    /// # Arguments
    ///
    /// * `t` - Finite curve time coordinate in years.
    ///
    /// # Errors
    ///
    /// Returns an error if `t` is non-finite, outside the curve's domain or
    /// tail policy, or the derivative or factor quotient is invalid.
    fn instantaneous_forward(&self, t: f64) -> crate::Result<f64>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CurveId;

    struct FlatCurve {
        id: CurveId,
        df_const: f64,
    }

    impl FlatCurve {
        fn new(id: &'static str, df_const: f64) -> Self {
            Self {
                id: CurveId::new(id),
                df_const,
            }
        }
    }

    impl Discounting for FlatCurve {
        fn id(&self) -> &crate::types::CurveId {
            &self.id
        }

        fn base_date(&self) -> Date {
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date")
        }

        fn df(&self, t: f64) -> crate::Result<f64> {
            if !t.is_finite() {
                return Err(crate::Error::Validation("non-finite t".into()));
            }
            if !self.df_const.is_finite() || self.df_const <= 0.0 {
                return Err(crate::InputError::NonPositiveValue.into());
            }
            Ok(self.df_const)
        }

        fn instantaneous_forward(&self, t: f64) -> crate::Result<f64> {
            let _ = self.df(t)?;
            Ok(0.0)
        }
    }

    #[test]
    fn discounting_trait_works() {
        let c = FlatCurve::new("TEST", 0.9);
        let df = c.df(1.0).expect("valid df");
        assert_eq!(df, 0.9);
    }

    #[test]
    fn discounting_df_between_dates_constant_curve_is_one() {
        let c = FlatCurve::new("TEST", 0.9);
        let as_of = c.base_date();
        let to = as_of + time::Duration::days(365);
        let df = c
            .df_between_dates(as_of, to)
            .expect("constant curve should produce valid DFs");
        assert_eq!(df, 1.0);
    }

    #[test]
    fn relative_discounting_rejects_overflow_and_underflow() {
        struct ExtremeCurve {
            id: CurveId,
            reverse: bool,
        }
        impl Discounting for ExtremeCurve {
            fn id(&self) -> &CurveId {
                &self.id
            }
            fn base_date(&self) -> Date {
                Date::from_calendar_date(2025, time::Month::January, 1).expect("date")
            }
            fn df(&self, t: f64) -> crate::Result<f64> {
                if (t > 0.0) != self.reverse {
                    Ok(1e308)
                } else {
                    Ok(1e-308)
                }
            }
            fn instantaneous_forward(&self, _t: f64) -> crate::Result<f64> {
                Ok(0.0)
            }
        }
        for reverse in [false, true] {
            let curve = ExtremeCurve {
                id: CurveId::new("EXTREME"),
                reverse,
            };
            let base = curve.base_date();
            assert!(
                curve
                    .df_between_dates(base, base + time::Duration::days(365))
                    .is_err()
            );
        }
    }

    #[test]
    fn discounting_df_between_times_validates_denominator() {
        let c = FlatCurve::new("TEST", 0.0);
        assert!(c.df_between_times(0.0, 1.0).is_err());
        assert!(c.df_between_times(0.0, 0.0).is_err());
    }

    struct QuadraticLogDfCurve {
        id: CurveId,
        a: f64,
        b: f64,
    }

    impl Discounting for QuadraticLogDfCurve {
        fn id(&self) -> &crate::types::CurveId {
            &self.id
        }

        fn base_date(&self) -> Date {
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date")
        }

        fn df(&self, t: f64) -> crate::Result<f64> {
            Ok((-(self.a * t + 0.5 * self.b * t * t)).exp())
        }

        fn instantaneous_forward(&self, t: f64) -> crate::Result<f64> {
            Ok(self.a + self.b * t)
        }
    }

    #[test]
    fn instantaneous_forward_matches_analytic_value() {
        let curve = QuadraticLogDfCurve {
            id: CurveId::new("QUADRATIC"),
            a: 0.02,
            b: 0.01,
        };

        let t = 2.0;
        let expected = curve.a + curve.b * t;
        let actual = curve
            .instantaneous_forward(t)
            .expect("instantaneous forward should be computable");

        assert!((actual - expected).abs() < 1e-12);
    }

    #[test]
    fn df_between_times_and_instantaneous_forward_reject_non_finite_times() {
        let curve = FlatCurve::new("TEST", 0.95);
        assert!(
            curve
                .df_between_times(f64::INFINITY, f64::INFINITY)
                .is_err()
        );
        assert!(curve.df_between_times(f64::NAN, 1.0).is_err());
        assert!(curve.df_between_times(0.0, f64::NAN).is_err());

        assert!(curve.instantaneous_forward(f64::NAN).is_err());
        assert!(curve.instantaneous_forward(f64::INFINITY).is_err());
    }
}
