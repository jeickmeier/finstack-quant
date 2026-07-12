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

// -----------------------------------------------------------------------------
// Minimal traits for polymorphism only
// -----------------------------------------------------------------------------

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
/// - [`forward_rate_between_times`](Self::forward_rate_between_times) - Forward rate from times
/// - [`forward_rate_between_dates`](Self::forward_rate_between_dates) - Forward rate from dates
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
/// use finstack_quant_core::market_data::traits::{Discounting, TermStructure};
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
/// impl TermStructure for FlatCurve {
///     fn id(&self) -> &CurveId { &self.id }
/// }
///
/// impl Discounting for FlatCurve {
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
/// - [`Forward`] - Trait for forward rate curves
/// - [`Survival`] - Trait for hazard/survival curves
pub trait Discounting: TermStructure + Send + Sync {
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
    /// - Either discount factor is non-finite or non-positive
    #[inline]
    fn df_between_dates(&self, from: Date, to: Date) -> crate::Result<f64> {
        if from == to {
            return Ok(1.0);
        }

        let dc = self.day_count();
        let base = self.base_date();

        let t_from = dc.signed_year_fraction(base, from, DayCountContext::default())?;
        let df_from = self.df(t_from);
        if !df_from.is_finite() || df_from <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor on 'from' date ({from}): {df_from}"
            )));
        }

        let t_to = dc.signed_year_fraction(base, to, DayCountContext::default())?;
        let df_to = self.df(t_to);
        if !df_to.is_finite() || df_to <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor on 'to' date ({to}): {df_to}"
            )));
        }

        Ok(df_to / df_from)
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

        Ok(df_to / df_from)
    }

    /// Forward rate between times `from_t` and `to_t` (year fractions from base date).
    ///
    /// Uses the standard log-discount ratio:
    /// `f = -ln(DF(from→to)) / (to_t - from_t)`.
    fn forward_rate_between_times(&self, from_t: f64, to_t: f64) -> crate::Result<f64> {
        if to_t <= from_t {
            return Err(crate::Error::Validation(format!(
                "forward_rate_between_times requires to_t > from_t (from_t={from_t}, to_t={to_t})"
            )));
        }

        let df_ratio = self.df_between_times(from_t, to_t)?;
        Ok(-df_ratio.ln() / (to_t - from_t))
    }

    /// Forward rate between dates using the curve's day-count convention.
    fn forward_rate_between_dates(&self, from: Date, to: Date) -> crate::Result<f64> {
        if to <= from {
            return Err(crate::Error::Validation(format!(
                "forward_rate_between_dates requires to > from (from={from}, to={to})"
            )));
        }

        let dc = self.day_count();
        let base = self.base_date();
        let from_t = dc.year_fraction(base, from, DayCountContext::default())?;
        let to_t = dc.year_fraction(base, to, DayCountContext::default())?;
        self.forward_rate_between_times(from_t, to_t)
    }

    /// Instantaneous forward rate at time `t` (year fraction from base date).
    ///
    /// Approximates the derivative of `-ln P(0,t)` using a centered finite
    /// difference around `t`, capped to a small local neighborhood.
    fn instantaneous_forward(&self, t: f64) -> crate::Result<f64> {
        let eps = (t.abs() * 1e-4).clamp(1e-6, 1e-4);
        let start = (t - eps).max(0.0);
        let end = t + eps;
        self.forward_rate_between_times(start, end)
    }
}

/// Minimal trait for forward curve polymorphism where needed.
///
/// Most code should call methods directly on `ForwardCurve`. This trait enables
/// polymorphic code that needs to accept different forward curve implementations.
///
/// # Required Methods
///
/// - [`rate`](Self::rate) - Returns the fixed-tenor index rate resetting at time `t`
///
/// # Provided Methods
///
/// - [`rate_period`](Self::rate_period) - Simpson-rule integral average over `[t1, t2]`
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::traits::{Forward, TermStructure};
/// use finstack_quant_core::types::CurveId;
///
/// struct FlatForward {
///     id: CurveId,
///     rate: f64,
/// }
///
/// impl TermStructure for FlatForward {
///     fn id(&self) -> &CurveId { &self.id }
/// }
///
/// impl Forward for FlatForward {
///     fn rate(&self, _t: f64) -> f64 { self.rate }
/// }
///
/// let curve = FlatForward { id: CurveId::from("USD-3M"), rate: 0.05 };
/// assert_eq!(curve.rate(1.0), 0.05);
/// assert!((curve.rate_period(0.5, 1.0) - 0.05).abs() < 1e-14); // Flat curve: period average ≈ rate
/// ```
pub trait Forward: TermStructure + Send + Sync {
    /// Fixed-tenor index rate resetting at time `t`.
    ///
    /// # Arguments
    ///
    /// * `t` - Time in years from the curve's base date
    ///
    /// # Returns
    ///
    /// The index rate fixed at time `t`.
    fn rate(&self, t: f64) -> f64;

    // Concrete curve types can override this with analytical integration for
    // piecewise-flat/linear curves to avoid numerical quadrature.
    /// Simpson-rule integral average rate over `[t1, t2]`.
    ///
    /// This is appropriate for averaging overnight observation sub-windows.
    /// Implementors that expose projection discount factors should provide a
    /// separate discount-factor-implied period-forward API for arbitrary term
    /// projection intervals.
    ///
    /// Uses 8-interval composite Simpson's rule for accurate integration
    /// of the forward rate curve over the period, matching the concrete
    /// `ForwardCurve::rate_period()` implementation.
    ///
    /// # Arguments
    ///
    /// * `t1` - Start time in years
    /// * `t2` - End time in years
    ///
    /// # Returns
    ///
    /// The integral average forward rate over `[t1, t2]`.
    ///
    /// Returns [`f64::NAN`] if `t2 < t1` or inputs are non-finite. For
    /// `t2` equal to `t1` within ~1e-12 years, returns [`Self::rate`] at `t1`
    /// (degenerate interval / instantaneous forward).
    #[inline]
    fn rate_period(&self, t1: f64, t2: f64) -> f64 {
        if !(t1.is_finite() && t2.is_finite()) {
            return f64::NAN;
        }
        let period = t2 - t1;
        if period < 0.0 {
            return f64::NAN;
        }
        if period <= 1e-12 {
            return self.rate(t1);
        }
        // Adaptive-interval composite Simpson's rule:
        //   ∫f(x)dx ≈ (h/3)[f(x0) + 4f(x1) + 2f(x2) + ... + f(xn)]
        //
        // Use more intervals for longer periods to maintain accuracy for
        // long-dated forward averages while keeping performance for short periods.
        let n = if period > 20.0 {
            32_usize
        } else if period > 5.0 {
            16_usize
        } else {
            8_usize
        };
        let h = (t2 - t1) / n as f64;
        let mut sum = self.rate(t1) + self.rate(t2);
        for i in 1..n {
            let t = t1 + i as f64 * h;
            let coeff = if i % 2 == 0 { 2.0 } else { 4.0 };
            sum += coeff * self.rate(t);
        }
        sum * h / (3.0 * (t2 - t1))
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
/// use finstack_quant_core::market_data::traits::{Survival, TermStructure};
/// use finstack_quant_core::types::CurveId;
///
/// struct FlatHazard {
///     id: CurveId,
///     hazard_rate: f64, // Constant hazard rate
/// }
///
/// impl TermStructure for FlatHazard {
///     fn id(&self) -> &CurveId { &self.id }
/// }
///
/// impl Survival for FlatHazard {
///     fn sp(&self, t: f64) -> f64 {
///         (-self.hazard_rate * t).exp()
///     }
/// }
///
/// let curve = FlatHazard { id: CurveId::from("XYZ-HAZARD"), hazard_rate: 0.02 };
/// let sp_1y = curve.sp(1.0);
/// assert!(sp_1y < 1.0 && sp_1y > 0.0); // Survival prob decreases over time
/// ```
pub trait Survival: TermStructure + Send + Sync {
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
}

/// Minimal trait for term structure polymorphism where needed.
///
/// # Examples
/// ```rust
/// use finstack_quant_core::market_data::traits::TermStructure;
/// use finstack_quant_core::types::CurveId;
///
/// struct DummyCurve { id: CurveId }
///
/// impl TermStructure for DummyCurve {
///     fn id(&self) -> &CurveId { &self.id }
/// }
///
/// let curve = DummyCurve { id: CurveId::from("DUMMY") };
/// assert_eq!(curve.id().as_str(), "DUMMY");
/// ```
pub trait TermStructure: Send + Sync {
    /// Unique identifier of the term structure.
    fn id(&self) -> &crate::types::CurveId;
}

/// Trait for types that supply implied volatility given market coordinates.
///
/// Provides a unified interface for looking up implied vol from both 2D surfaces
/// (where tenor is ignored) and 3D cubes (where all coordinates are used).
///
/// # Required Methods
///
/// - [`vol`](Self::vol) — Look up implied vol at (expiry, tenor, strike)
/// - [`vol_clamped`](Self::vol_clamped) — Same as vol but clamps out-of-bounds coordinates
/// - [`vol_id`](Self::vol_id) — The identifier of this vol source
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::traits::VolProvider;
/// use finstack_quant_core::market_data::surfaces::VolSurface;
///
/// let surface = VolSurface::builder("EQ-VOL")
///     .expiries(&[1.0, 2.0])
///     .strikes(&[90.0, 100.0])
///     .row(&[0.2, 0.2])
///     .row(&[0.2, 0.2])
///     .build()
///     .expect("VolSurface builder should succeed");
///
/// let provider: &dyn VolProvider = &surface;
/// let vol = provider.vol(1.5, 0.0, 95.0).unwrap();
/// assert!(vol > 0.0);
/// ```
pub trait VolProvider: Send + Sync {
    /// Look up implied vol at (expiry, tenor, strike).
    /// For 2D surfaces: tenor is ignored.
    fn vol(&self, expiry: f64, tenor: f64, strike: f64) -> crate::Result<f64>;

    /// Same as vol() but clamps out-of-bounds coordinates.
    fn vol_clamped(&self, expiry: f64, tenor: f64, strike: f64) -> f64;

    /// The identifier of this vol source.
    fn vol_id(&self) -> &crate::types::CurveId;
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------
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

    impl TermStructure for FlatCurve {
        fn id(&self) -> &crate::types::CurveId {
            &self.id
        }
    }

    impl Discounting for FlatCurve {
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
    fn discounting_df_between_times_validates_denominator() {
        let c = FlatCurve::new("TEST", 0.0);
        assert!(c.df_between_times(0.0, 1.0).is_err());
    }

    struct QuadraticLogDfCurve {
        id: CurveId,
        a: f64,
        b: f64,
    }

    impl TermStructure for QuadraticLogDfCurve {
        fn id(&self) -> &crate::types::CurveId {
            &self.id
        }
    }

    impl Discounting for QuadraticLogDfCurve {
        fn base_date(&self) -> Date {
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date")
        }

        fn df(&self, t: f64) -> f64 {
            (-(self.a * t + 0.5 * self.b * t * t)).exp()
        }
    }

    struct FlatForwardCurve {
        id: CurveId,
        rate: f64,
    }

    impl TermStructure for FlatForwardCurve {
        fn id(&self) -> &CurveId {
            &self.id
        }
    }

    impl Forward for FlatForwardCurve {
        fn rate(&self, _t: f64) -> f64 {
            self.rate
        }
    }

    #[test]
    fn forward_trait_rate_period_rejects_reversed_times() {
        let c = FlatForwardCurve {
            id: CurveId::new("FLAT-FWD"),
            rate: 0.05,
        };
        assert!(Forward::rate_period(&c, 1.0, 0.5).is_nan());
    }

    #[test]
    fn forward_trait_rate_period_degenerate_matches_instantaneous_rate() {
        let c = FlatForwardCurve {
            id: CurveId::new("FLAT-FWD"),
            rate: 0.05,
        };
        assert!((Forward::rate_period(&c, 1.0, 1.0) - 0.05).abs() < 1e-15);
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
