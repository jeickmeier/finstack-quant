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

// Minimal traits for polymorphism only

/// Minimal trait for discount curve polymorphism.
///
/// Only implement this where you need to accept different discount curve types.
/// Most code should use concrete curve types directly for better discoverability.
///
/// # Required Methods
///
/// - [`base_date`](Self::base_date) - Returns the curve's valuation date
/// - [`df`](Self::df) - Returns discount factor at time `t`
///
/// # Provided Methods
///
/// - [`day_count`](Self::day_count) - Day count convention (default: Act365F)
/// - [`df_between_dates`](Self::df_between_dates) - Relative discount factor between dates
/// - [`df_between_times`](Self::df_between_times) - Relative discount factor between times
/// - [`instantaneous_forward`](Self::instantaneous_forward) - Instantaneous forward rate
///
/// # Implementation Guide
///
/// Implementors must provide:
/// 1. A base (valuation) date via [`base_date`](Self::base_date)
/// 2. A discount factor function via [`df`](Self::df) that maps year fractions to discount factors
///
/// The default [`day_count`](Self::day_count) returns `Act365F`. Override if your curve uses a
/// different convention.
///
/// # Examples
///
/// ## Using a Discount Curve
///
/// ```rust
/// use finstack_quant_core::market_data::traits::Discounting;
/// use finstack_quant_core::types::CurveId;
/// use finstack_quant_core::dates::Date;
/// use time::macros::date;
///
/// struct FlatCurve {
///     id: CurveId,
///     df_const: f64,
/// }
///
/// impl FlatCurve {
///     fn new(id: &str, df_const: f64) -> Self {
///         Self { id: CurveId::from(id), df_const }
///     }
/// }
///
/// impl Discounting for FlatCurve {
///     fn id(&self) -> &CurveId { &self.id }
///     fn base_date(&self) -> Date {
///         date!(2025 - 01 - 01)
///     }
///     fn df(&self, _t: f64) -> f64 { self.df_const }
/// }
///
/// let curve = FlatCurve::new("USD", 0.97);
/// assert!(curve.df(1.0) < 1.0);
/// ```
///
/// # See Also
///
/// - [`crate::market_data::term_structures::ForwardCurve`] - Concrete forward rate curve
/// - [`Survival`] - Trait for hazard/survival curves
pub trait Discounting: Send + Sync {
    /// Unique identifier of the curve.
    fn id(&self) -> &crate::types::CurveId;

    /// Base (valuation) date of the curve.
    fn base_date(&self) -> Date;
    /// Discount factor at time `t` (year fraction from the base date).
    fn df(&self, t: f64) -> f64;

    /// Day count convention used by the curve for time-to-maturity calculations.
    ///
    /// This is the day count that should be used when converting dates to year
    /// fractions for looking up discount factors. Defaults to `Act365F` which
    /// is the most common convention for discount curves.
    ///
    /// **Important**: For consistent pricing, code that discounts cashflows should
    /// use the curve's day count (via this method) rather than the instrument's
    /// accrual day count.
    fn day_count(&self) -> DayCount {
        DayCount::Act365F
    }

    /// Discount factor from `from` to `to` using the curve's day-count.
    ///
    /// Canonical helper for the common "relative DF" pattern:
    /// `DF(from→to) = DF(0→to) / DF(0→from)`.
    ///
    /// Returns `Ok(1.0)` when `from == to`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Day-count year fraction calculation fails
    /// - Either discount factor or their quotient is non-finite or non-positive
    #[inline]
    fn df_between_dates(&self, from: Date, to: Date) -> crate::Result<f64> {
        if from == to {
            return Ok(1.0);
        }

        let day_count = self.day_count();
        let base = self.base_date();

        let t_from = day_count.signed_year_fraction(base, from, DayCountContext::default())?;
        let df_from = self.df(t_from);
        if !df_from.is_finite() || df_from <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor on 'from' date ({from}): {df_from}"
            )));
        }

        let t_to = day_count.signed_year_fraction(base, to, DayCountContext::default())?;
        let df_to = self.df(t_to);
        if !df_to.is_finite() || df_to <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor on 'to' date ({to}): {df_to}"
            )));
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
    /// from the curve base date.
    ///
    /// Canonical helper for `DF(from_t→to_t) = DF(0→to_t) / DF(0→from_t)`.
    ///
    /// Returns `Ok(1.0)` when `from_t == to_t`.
    ///
    /// # Errors
    ///
    /// Returns an error if either discount factor is non-finite or non-positive.
    #[inline]
    fn df_between_times(&self, from_t: f64, to_t: f64) -> crate::Result<f64> {
        // Exact comparison: degenerate-case guard for identical time arguments.
        #[allow(clippy::float_cmp)]
        if from_t == to_t {
            return Ok(1.0);
        }

        let df_from = self.df(from_t);
        if !df_from.is_finite() || df_from <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor at 'from_t' ({from_t}): {df_from}"
            )));
        }

        let df_to = self.df(to_t);
        if !df_to.is_finite() || df_to <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor at 'to_t' ({to_t}): {df_to}"
            )));
        }

        let relative_df = df_to / df_from;
        if !relative_df.is_finite() || relative_df <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid relative discount factor from {from_t} to {to_t}: {relative_df}"
            )));
        }
        Ok(relative_df)
    }

    /// Instantaneous forward rate at time `t` (year fraction from base date).
    ///
    /// Approximates the derivative of `-ln P(0,t)` using a centered finite
    /// difference around `t`, capped to a small local neighborhood:
    /// `f ≈ -ln(DF(start→end)) / (end - start)`.
    fn instantaneous_forward(&self, t: f64) -> crate::Result<f64> {
        let eps = (t.abs() * 1e-4).clamp(1e-6, 1e-4);
        let start = (t - eps).max(0.0);
        let end = t + eps;
        let df_ratio = self.df_between_times(start, end)?;
        Ok(-df_ratio.ln() / (end - start))
    }
}
/// Minimal trait for survival/hazard curve polymorphism where needed.
///
/// Most code should call methods directly on `HazardCurve`. This trait enables
/// polymorphic code that needs to accept different survival curve implementations.
///
/// # Required Methods
///
/// - [`sp`](Self::sp) - Returns the survival probability at time `t`
///
/// # Mathematical Background
///
/// The survival probability `S(t)` represents the probability of no default
/// occurring before time `t`. It relates to the hazard rate `λ(t)` via:
///
/// ```text
/// S(t) = exp(-∫₀ᵗ λ(s) ds)
/// ```
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::traits::Survival;
/// use finstack_quant_core::types::CurveId;
///
/// struct FlatHazard {
///     id: CurveId,
///     hazard_rate: f64, // Constant hazard rate
/// }
///
/// impl Survival for FlatHazard {
///     fn id(&self) -> &CurveId { &self.id }
///     fn sp(&self, t: f64) -> f64 {
///         (-self.hazard_rate * t).exp()
///     }
/// }
///
/// let curve = FlatHazard { id: CurveId::from("XYZ-HAZARD"), hazard_rate: 0.02 };
/// let sp_1y = curve.sp(1.0);
/// assert!(sp_1y < 1.0 && sp_1y > 0.0); // Survival prob decreases over time
/// ```
pub trait Survival: Send + Sync {
    /// Unique identifier of the curve.
    fn id(&self) -> &crate::types::CurveId;

    /// Survival probability up to time `t`.
    ///
    /// # Arguments
    ///
    /// * `t` - Time in years from the curve's base date
    ///
    /// # Returns
    ///
    /// The probability of survival (no default) from time 0 to time `t`,
    /// a value in the range `(0, 1]`.
    fn sp(&self, t: f64) -> f64;

    /// Optional curve base date for date-based conditional survival.
    fn base_date(&self) -> Option<Date> {
        None
    }

    /// Day-count convention used by the curve when a base date is available.
    fn day_count(&self) -> DayCount {
        DayCount::Act365F
    }
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
        fn df(&self, _t: f64) -> f64 {
            self.df_const
        }
    }

    #[test]
    fn discounting_trait_works() {
        let c = FlatCurve::new("TEST", 0.9);
        let df = c.df(1.0);
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
            fn df(&self, t: f64) -> f64 {
                if (t > 0.0) != self.reverse {
                    1e308
                } else {
                    1e-308
                }
            }
        }
        for reverse in [false, true] {
            let curve = ExtremeCurve {
                id: CurveId::new("EXTREME"),
                reverse,
            };
            let base = curve.base_date();
            assert!(curve
                .df_between_dates(base, base + time::Duration::days(365))
                .is_err());
        }
    }

    #[test]
    fn discounting_df_between_times_validates_denominator() {
        let c = FlatCurve::new("TEST", 0.0);
        assert!(c.df_between_times(0.0, 1.0).is_err());
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

        fn df(&self, t: f64) -> f64 {
            (-(self.a * t + 0.5 * self.b * t * t)).exp()
        }
    }
    #[test]
    fn instantaneous_forward_uses_centered_difference() {
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

        assert!(
            (actual - expected).abs() < 1e-8,
            "centered finite difference should recover the analytical forward: expected {}, got {}",
            expected,
            actual
        );
    }
}
