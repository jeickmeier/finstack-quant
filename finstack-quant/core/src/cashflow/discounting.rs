//! Present value of dated cash using a discounting source (record
//! CORE-CONVENTIONS-016).
//!
//! Two explicit paths share one selection contract:
//!
//! - [`npv`] for homogeneous [`Money`] cash, with exact Decimal products and
//!   sums (record 012).
//! - [`npv_amounts`] for caller-unit `f64` cash, with checked binary64
//!   products and ordered compensated summation.
//!
//! ```text
//! PV = Σ C_i · D(as_of, t_i)
//! ```
//!
//! # Selection
//!
//! Both take the valuation date `as_of` and a required `include_today`:
//! payments dated before `as_of` are excluded, payments on `as_of` are
//! included at factor 1 exactly when `include_today` is true, and later
//! payments use the source's relative factor
//! [`Discounting::df_between_dates`]`(as_of, date)`. Whether same-day cash is
//! still owed is the product owner's decision; there is no default. Rows are
//! kept in input order and are not sorted, merged or netted.
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::cashflow::npv;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::Date;
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use finstack_quant_core::money::Money;
//! use time::Month;
//!
//! let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let curve = DiscountCurve::builder("USD-OIS")
//!     .base_date(base_date)
//!     .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.78)])
//!     .build()?;
//! let flows = vec![(
//!     Date::from_calendar_date(2026, Month::January, 1).expect("Valid date"),
//!     Money::from((100_i64, Currency::USD)),
//! )];
//! let pv = npv(&curve, base_date, &flows, false)?;
//! assert!(pv.amount() < 100.0);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # References
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Pearson. Chapters 4-7 (Interest Rates and Curve Construction). `docs/REFERENCES.md#hull-options-futures`
//! - Andersen, L., & Piterbarg, V. (2010). *Interest Rate Modeling* (3 vols).
//!   Atlantic Financial Press. Volume 1, Chapter 3. `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`

use crate::dates::Date;
#[cfg(test)]
use crate::dates::DayCount;
use crate::market_data::traits::Discounting;
use crate::math::summation::NeumaierAccumulator;
use crate::money::Money;

/// Discount factor for a flat, continuously compounded rate over `years`.
///
/// ```text
/// DF(t) = exp(-r * t)
/// ```
///
/// The rate is a continuously compounded annual decimal and `years` is an
/// already-computed year fraction on the caller's time basis; no date,
/// day-count or compounding conversion is applied.
///
/// # Arguments
///
/// * `rate` - Finite continuously compounded annual rate as a decimal
///   (0.05 = 5 %). May be negative, including below -1.
/// * `years` - Finite, non-negative year fraction to the payoff horizon.
///
/// # Returns
///
/// The factor: exactly 1 when either argument is zero, above 1 for a
/// negative rate. Positive subnormal factors are returned as they are.
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) when `rate` is not
/// finite, `years` is not finite or is negative, the exponent `-rate * years`
/// is not finite, or the factor overflows or underflows to zero.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::cashflow::flat_discount_factor;
///
/// assert_eq!(flat_discount_factor(0.0, 5.0)?, 1.0);
/// assert!(flat_discount_factor(0.05, 1.0)? < 1.0);
/// assert!(flat_discount_factor(-0.01, 1.0)? > 1.0);
/// assert!(flat_discount_factor(0.05, -1.0).is_err());
/// // exp(-800) underflows to zero: an error, not a zero factor.
/// assert!(flat_discount_factor(8.0, 100.0).is_err());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn flat_discount_factor(rate: f64, years: f64) -> crate::Result<f64> {
    if !rate.is_finite() {
        return Err(crate::Error::Validation(format!(
            "discount rate must be finite, got {rate}"
        )));
    }
    if !years.is_finite() || years < 0.0 {
        return Err(crate::Error::Validation(format!(
            "discount horizon must be finite and non-negative, got {years}"
        )));
    }
    if rate == 0.0 || years == 0.0 {
        return Ok(1.0);
    }
    let exponent = -rate * years;
    if !exponent.is_finite() {
        return Err(crate::Error::Validation(format!(
            "discount exponent is not finite for rate={rate}, years={years}"
        )));
    }
    let factor = exponent.exp();
    if !factor.is_finite() {
        return Err(crate::Error::Validation(format!(
            "discount factor overflowed for rate={rate}, years={years}"
        )));
    }
    if factor == 0.0 {
        return Err(crate::Error::Validation(format!(
            "discount factor underflowed to zero for rate={rate}, years={years}"
        )));
    }
    Ok(factor)
}

/// Present value of dated `Money` cash on `disc`.
///
/// Every amount must share the first row's currency (checked on every row,
/// including excluded ones). Payments before `as_of` are excluded; payments on
/// `as_of` are added at their exact amount when `include_today` is true; later
/// nonzero payments are multiplied exactly by the admitted Decimal of
/// [`Discounting::df_between_dates`]`(as_of, date)` (record 012 #4/#7) and
/// added in input order with exact-or-error addition. When no nonzero cash is
/// selected the result is zero in the flows' currency and the source is not
/// queried.
///
/// # Arguments
///
/// * `disc` - Discounting source; its own date admission, clock and
///   interpolation define the relative factors.
/// * `as_of` - Valuation date that payments are discounted to.
/// * `flows` - Payment dates and signed settlement cash, in one currency.
/// * `include_today` - Whether a payment dated `as_of` is still owed and
///   counts at factor 1. Chosen by the product owner; there is no default.
///
/// # Returns
///
/// Present value in the flows' currency.
///
/// # Errors
///
/// Returns `InputError::TooFewPoints` for empty `flows` (no currency is
/// known), `Error::CurrencyMismatch` for mixed currencies, any source error
/// from `df_between_dates`, `Error::Validation` for a non-finite or
/// non-positive factor, and Money admission, product or addition errors
/// (record 012).
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::cashflow::npv;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::{Date, DayCount};
/// use finstack_quant_core::market_data::term_structures::DiscountCurve;
/// use finstack_quant_core::money::Money;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = DiscountCurve::builder("USD-OIS")
///     .base_date(base)
///     .day_count(DayCount::Act360)
///     .knots([(0.0, 1.0), (1.0, 0.95)])
///     .build()?;
/// let today = (base, Money::from((7_i64, Currency::USD)));
/// let later = (
///     Date::from_calendar_date(2026, Month::January, 1).expect("Valid date"),
///     Money::from((100_i64, Currency::USD)),
/// );
/// let excluding = npv(&curve, base, &[today, later], false)?;
/// let including = npv(&curve, base, &[today, later], true)?;
/// assert_eq!(including.checked_sub(excluding)?, Money::from((7_i64, Currency::USD)));
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn npv<D: Discounting + ?Sized>(
    disc: &D,
    as_of: Date,
    flows: &[(Date, Money)],
    include_today: bool,
) -> crate::Result<Money> {
    let Some(&(_, first)) = flows.first() else {
        return Err(crate::InputError::TooFewPoints.into());
    };
    let ccy = first.currency();
    for (_, amount) in flows {
        if amount.currency() != ccy {
            return Err(crate::Error::CurrencyMismatch {
                expected: ccy,
                actual: amount.currency(),
            });
        }
    }
    let mut total = Money::zero(ccy);
    for &(date, amount) in flows {
        let Some(factor) = selected_factor(
            disc,
            as_of,
            date,
            include_today,
            amount.amount_decimal().is_zero(),
        )?
        else {
            continue;
        };
        let present = match factor {
            Factor::Today => amount,
            Factor::Future(df) => amount.checked_mul_f64(df)?,
        };
        total = total.checked_add(present)?;
    }
    Ok(total)
}

/// Present value of dated scalar cash on `disc`.
///
/// Every amount must be finite (checked on every row, including excluded
/// ones). Selection is as for [`npv`]. Each later nonzero amount is multiplied
/// by its positive relative factor in binary64; a non-finite product, or a
/// zero product of a nonzero amount, is an error. Products and same-day
/// amounts are accumulated in input order with the shared checked Neumaier
/// sum, and a zero result is unsigned.
///
/// # Arguments
///
/// * `disc` - Discounting source; its own date admission, clock and
///   interpolation define the relative factors.
/// * `as_of` - Valuation date that payments are discounted to.
/// * `flows` - Payment dates and signed amounts in the caller's units; empty
///   input returns `0.0`.
/// * `include_today` - Whether a payment dated `as_of` is still owed and
///   counts at factor 1. Chosen by the product owner; there is no default.
///
/// # Returns
///
/// Present value in the caller's amount units.
///
/// # Errors
///
/// Returns `Error::Validation` for a non-finite amount, factor, product or
/// running total, a non-positive factor or a nonzero amount whose product
/// rounds to zero, and any source error from `df_between_dates`.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::cashflow::npv_amounts;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::market_data::term_structures::DiscountCurve;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = DiscountCurve::builder("USD-OIS")
///     .base_date(base)
///     .knots([(0.0, 1.0), (1.0, 0.95)])
///     .build()?;
/// assert_eq!(npv_amounts(&curve, base, &[], false)?, 0.0);
/// assert_eq!(npv_amounts(&curve, base, &[(base, 5.0)], true)?, 5.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn npv_amounts<D: Discounting + ?Sized>(
    disc: &D,
    as_of: Date,
    flows: &[(Date, f64)],
    include_today: bool,
) -> crate::Result<f64> {
    if let Some((date, amount)) = flows.iter().find(|(_, amount)| !amount.is_finite()) {
        return Err(crate::Error::Validation(format!(
            "cashflow amount on {date} must be finite, got {amount}"
        )));
    }
    let mut total = NeumaierAccumulator::new();
    for &(date, amount) in flows {
        let Some(factor) = selected_factor(disc, as_of, date, include_today, amount == 0.0)? else {
            continue;
        };
        let present = match factor {
            Factor::Today => amount,
            Factor::Future(df) => {
                let product = amount * df;
                if !product.is_finite() {
                    return Err(crate::Error::Validation(format!(
                        "discounted cashflow on {date} is not finite"
                    )));
                }
                if product == 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "discounted cashflow on {date} underflowed to zero"
                    )));
                }
                product
            }
        };
        total.checked_add(present)?;
    }
    let result = total.checked_total()?;
    Ok(if result == 0.0 { 0.0 } else { result })
}

/// The factor a selected row is valued at.
enum Factor {
    /// Same-day cash at factor 1, without a source query.
    Today,
    /// Validated relative factor `D(as_of, date)`.
    Future(f64),
}

/// Select a row and resolve its factor (record 016 #2-#5): past and zero rows
/// and excluded same-day rows are skipped without querying `disc`.
fn selected_factor<D: Discounting + ?Sized>(
    disc: &D,
    as_of: Date,
    date: Date,
    include_today: bool,
    is_zero: bool,
) -> crate::Result<Option<Factor>> {
    if date < as_of || is_zero {
        return Ok(None);
    }
    if date == as_of {
        return Ok(include_today.then_some(Factor::Today));
    }
    let df = disc.df_between_dates(as_of, date)?;
    if !df.is_finite() || df <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "npv: discount factor for cashflow date {date} is invalid: {df}"
        )));
    }
    Ok(Some(Factor::Future(df)))
}
#[cfg(test)]
fn flat_curve(
    id: &str,
    base: Date,
    continuous_rate: f64,
    day_count: DayCount,
) -> crate::market_data::term_structures::DiscountCurve {
    use crate::market_data::term_structures::{DiscountCurve, ValidationMode};
    use crate::math::interp::{ExtrapolationPolicy, InterpStyle};

    DiscountCurve::builder(id)
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, (-continuous_rate).exp())])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::FlatForward)
        .validation(ValidationMode::Raw {
            allow_non_monotonic: continuous_rate < 0.0,
            forward_floor: None,
        })
        .day_count(day_count)
        .build()
        .expect("valid flat curve")
}

#[cfg(test)]
mod hardening_tests {
    use super::*;
    use crate::currency::Currency;
    use crate::dates::create_date;
    use time::Month;

    /// `npv` must discount to the supplied valuation date even when it differs
    /// from the curve's own base date — using the relative discount factor
    /// `DF(curve_base→d) / DF(curve_base→base)` rather than the curve-base-
    /// anchored `df(year_fraction(base, d))`.
    ///
    /// A non-flat curve is required: a flat curve is translation-invariant and
    /// would hide the time-origin error.
    #[test]
    fn npv_discounts_to_valuation_date_when_base_differs_from_curve_base() {
        use crate::market_data::term_structures::DiscountCurve;

        let curve_base = create_date(2025, Month::January, 1).expect("date");
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(curve_base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.88)])
            .build()
            .expect("curve");

        let val_date = create_date(2026, Month::January, 1).expect("date"); // 1y forward
        let flow_date = create_date(2027, Month::January, 1).expect("date"); // 2y forward
        let flows = vec![(flow_date, Money::from((1_000_000_i64, Currency::USD)))];

        // Valuation at the curve base: PV = CF · DF(0→2y) = CF · 0.88.
        let pv_at_curve_base = npv(&curve, curve_base, &flows, false).expect("npv");
        assert!((pv_at_curve_base.amount() - 880_000.0).abs() < 1.0);

        // Valuation one year forward must use the relative DF
        // DF(1y→2y) = df(2)/df(1) = 0.88/0.95, not df(year_fraction(val,flow)).
        let pv_forward = npv(&curve, val_date, &flows, false).expect("npv");
        let expected_forward = 1_000_000.0 * (0.88 / 0.95);
        assert!(
            (pv_forward.amount() - expected_forward).abs() < 1.0,
            "npv with base != curve base must use the relative DF: got {}, expected {}",
            pv_forward.amount(),
            expected_forward
        );
        // The pre-fix engine returned CF·df(1y) = 950_000; guard the regression.
        assert!(
            (pv_forward.amount() - 950_000.0).abs() > 1_000.0,
            "npv must not reuse the curve-base-anchored df lookup"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::Currency;
    use crate::dates::create_date;
    use crate::types::CurveId;
    use time::Month;

    // flat_discount_factor

    #[test]
    fn flat_discount_factor_matches_the_closed_form() {
        for (rate, years) in [(0.05_f64, 1.0_f64), (0.02, 7.5), (-0.01, 3.0), (0.10, 0.25)] {
            let expected = (-rate * years).exp();
            assert_eq!(
                flat_discount_factor(rate, years).expect("valid inputs"),
                expected,
                "rate={rate}, years={years}"
            );
        }
    }

    #[test]
    fn flat_discount_factor_is_one_at_the_identity_points() {
        assert_eq!(flat_discount_factor(0.0, 5.0).expect("zero rate"), 1.0);
        assert_eq!(flat_discount_factor(0.05, 0.0).expect("zero horizon"), 1.0);
    }

    #[test]
    fn flat_discount_factor_accretes_under_a_negative_rate() {
        // Negative rates are a real market state, not an input error.
        assert!(flat_discount_factor(-0.005, 2.0).expect("negative rate") > 1.0);
    }

    #[test]
    fn flat_discount_factor_composes_across_horizons() {
        // DF(t1 + t2) == DF(t1) * DF(t2) for a flat rate.
        let (rate, t1, t2) = (0.03, 1.5, 2.5);
        let whole = flat_discount_factor(rate, t1 + t2).expect("valid");
        let split = flat_discount_factor(rate, t1).expect("valid")
            * flat_discount_factor(rate, t2).expect("valid");
        assert!((whole - split).abs() < 1e-15, "{whole} vs {split}");
    }

    #[test]
    fn flat_discount_factor_rejects_bad_inputs() {
        // A non-finite rate must be reported here, not surface later as a
        // non-finite price with no indication of which input caused it.
        assert!(flat_discount_factor(f64::NAN, 1.0).is_err());
        assert!(flat_discount_factor(f64::INFINITY, 1.0).is_err());
        assert!(flat_discount_factor(0.05, f64::NAN).is_err());
        // Negative time would silently accrete rather than discount.
        assert!(flat_discount_factor(0.05, -1.0).is_err());
    }

    #[test]
    fn flat_discount_factor_rejects_overflow_to_infinity() {
        // A large negative rate over a long horizon overflows; that is an
        // error rather than an infinite discount factor.
        assert!(flat_discount_factor(-1000.0, 1e6).is_err());
    }

    /// Test helper: creates a flat curve with DF=1.0 for all times (0% rate).
    struct ZeroRateCurve {
        id: CurveId,
    }

    impl Discounting for ZeroRateCurve {
        fn id(&self) -> &CurveId {
            &self.id
        }

        fn base_date(&self) -> Date {
            Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date")
        }
        fn df(&self, _t: f64) -> f64 {
            1.0
        }
    }

    struct InvalidBaseDfCurve {
        id: CurveId,
    }

    struct InvalidFlowDfCurve {
        id: CurveId,
    }

    impl Discounting for InvalidFlowDfCurve {
        fn id(&self) -> &CurveId {
            &self.id
        }

        fn base_date(&self) -> Date {
            Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date")
        }
        fn df(&self, t: f64) -> f64 {
            if t.abs() < f64::EPSILON {
                1.0
            } else {
                f64::NAN
            }
        }
    }

    impl Discounting for InvalidBaseDfCurve {
        fn id(&self) -> &CurveId {
            &self.id
        }

        fn base_date(&self) -> Date {
            Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date")
        }
        fn df(&self, t: f64) -> f64 {
            if t.abs() < f64::EPSILON { 0.0 } else { 1.0 }
        }
    }

    #[test]
    fn tuples_npv_sums_future_flows() {
        let curve = ZeroRateCurve {
            id: CurveId::new("USD-OIS"),
        };
        let base = curve.base_date();
        let pay = base + time::Duration::days(1);
        let flows = vec![
            (pay, Money::from((10_i64, crate::currency::Currency::USD))),
            (pay, Money::from((5_i64, crate::currency::Currency::USD))),
        ];
        let pv = npv(&curve, base, &flows, false).expect("NPV calculation should succeed in test");
        assert!((pv.amount() - 15.0).abs() < 1e-12);
    }

    #[test]
    fn npv_rejects_invalid_valuation_date_discount_factor() {
        let curve = InvalidBaseDfCurve {
            id: CurveId::new("BAD-DF"),
        };
        let base = curve.base_date();
        let flows = vec![(
            base + time::Duration::days(1),
            Money::from((10_i64, Currency::USD)),
        )];

        let err = npv(&curve, base, &flows, false).expect_err("df_base <= 0 should be rejected");

        assert!(
            err.to_string()
                .contains("Invalid discount factor on 'from' date"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn npv_rejects_invalid_cashflow_discount_factor() {
        let curve = InvalidFlowDfCurve {
            id: CurveId::new("BAD-FLOW-DF"),
        };
        let base = curve.base_date();
        let flows = vec![(
            base + time::Duration::days(1),
            Money::from((10_i64, Currency::USD)),
        )];

        let err = npv(&curve, base, &flows, false)
            .expect_err("non-finite cashflow discount factor should be rejected");

        assert!(
            err.to_string()
                .contains("Invalid discount factor on 'to' date"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn bus252_discount_curve_requires_its_own_calendar() {
        let base = create_date(2025, Month::January, 6).expect("Valid test date");
        let err = crate::market_data::term_structures::DiscountCurve::builder("BRL-FLAT")
            .base_date(base)
            .day_count(DayCount::Bus252)
            .knots([(0.0, 1.0), (1.0, 0.9)])
            .build()
            .expect_err("Bus/252 time axis needs a calendar");
        assert!(err.to_string().contains("requires a holiday calendar"));
    }

    #[test]
    fn npv_counts_business_days_on_the_curve_calendar() {
        use crate::market_data::term_structures::{DiscountCurve, ValidationMode};
        use crate::math::interp::{ExtrapolationPolicy, InterpStyle};

        let base = create_date(2025, Month::January, 6).expect("Valid test date");
        let pay = create_date(2025, Month::January, 13).expect("Valid test date");
        let curve = DiscountCurve::builder("BRL-BUS252")
            .base_date(base)
            .day_count(DayCount::Bus252)
            .calendar_id("target2")
            .knots([(0.0, 1.0), (1.0, (-0.10_f64).exp())])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .validation(ValidationMode::Raw {
                allow_non_monotonic: false,
                forward_floor: None,
            })
            .build()
            .expect("calendar-backed Bus/252 curve");
        let flows = vec![(pay, Money::from((100_i64, Currency::USD)))];

        let pv = npv(&curve, base, &flows, false).expect("calendar-backed NPV");
        let independently_counted = 100.0 * (-0.10_f64 * 5.0 / 252.0).exp();
        assert!((pv.amount() - independently_counted).abs() < 1e-12);
        assert_eq!(curve.calendar_id(), Some("target2"));
    }

    #[test]
    fn test_npv_simple_with_flat_curve() {
        let base = create_date(2024, Month::January, 1).expect("Valid test date");
        let flows = vec![
            (base, Money::from((-100000_i64, Currency::USD))),
            (
                create_date(2025, Month::January, 1).expect("Valid test date"),
                Money::from((110000_i64, Currency::USD)),
            ),
        ];
        let rate: f64 = 0.05;
        let day_count = DayCount::Act365F;

        // Create a flat curve with continuous rate
        let continuous_rate = (1.0 + rate).ln();
        let curve = flat_curve("NPV-TEST", base, continuous_rate, day_count);

        // The default npv excludes flows on or before the valuation date, so
        // the time-0 outlay (-100000 at base) is NOT part of the pricing PV.
        let pv = npv(&curve, base, &flows, false).expect("NPV calculation should succeed in test");
        // Approximately: 110000/(1.05) ≈ 104761.90 (initial outlay excluded)
        assert!(pv.amount() > 104700.0 && pv.amount() < 104800.0);
    }
    #[test]
    fn test_npv_zero_discount() {
        let base = create_date(2024, Month::January, 1).expect("Valid test date");
        let flows = vec![
            (base, Money::from((-100_i64, Currency::USD))),
            (
                create_date(2025, Month::January, 1).expect("Valid test date"),
                Money::from((100_i64, Currency::USD)),
            ),
        ];
        let day_count = DayCount::Act365F;

        // Create a flat curve with 0% rate (continuous rate = ln(1) = 0)
        let curve = flat_curve("ZERO-RATE", base, 0.0, day_count);

        // Default pricing semantics exclude the base-date flow, so only the
        // +100 remains.
        let pv = npv(&curve, base, &flows, false).expect("NPV calculation should succeed in test");
        assert_eq!(pv.amount(), 100.0);
    }

    /// Pricing excludes flows on or before the valuation date.
    #[test]
    fn test_npv_excludes_past_flows_by_default() {
        let base = create_date(2025, Month::January, 1).expect("Valid test date");
        let past = create_date(2024, Month::July, 1).expect("Valid test date");
        let future = create_date(2025, Month::July, 1).expect("Valid test date");
        let flows = vec![
            (past, Money::from((-50_i64, Currency::USD))), // past relative to base
            (base, Money::from((-25_i64, Currency::USD))), // on the valuation date
            (future, Money::from((55_i64, Currency::USD))), // future relative to base
        ];
        let rate: f64 = 0.05;
        let day_count = DayCount::Act365F;

        let continuous_rate = (1.0 + rate).ln();
        let curve = flat_curve("TEST", base, continuous_rate, day_count);

        // Default: only the strictly-future +55 flow is priced.
        let pv = npv(&curve, base, &flows, false).expect("NPV calculation should succeed in test");
        let only_future =
            npv(&curve, base, &flows[2..], false).expect("future-only NPV should succeed in test");
        assert_eq!(pv.amount(), only_future.amount());
        assert!(pv.amount() > 0.0 && pv.amount() < 55.0);
    }

    /// If every flow is on or before the valuation date, the default
    /// pricing NPV is zero in the flows' currency (nothing left to price).
    #[test]
    fn test_npv_all_past_flows_is_zero() {
        let base = create_date(2025, Month::January, 1).expect("Valid test date");
        let flows = vec![
            (
                create_date(2024, Month::July, 1).expect("Valid test date"),
                Money::from((100_i64, Currency::USD)),
            ),
            (base, Money::from((50_i64, Currency::USD))),
        ];
        let day_count = DayCount::Act365F;
        let curve = flat_curve("TEST", base, (1.05_f64).ln(), day_count);

        let pv = npv(&curve, base, &flows, false).expect("NPV should succeed");
        assert_eq!(pv.amount(), 0.0);
        assert_eq!(pv.currency(), Currency::USD);
    }

    #[test]
    fn test_npv_errors_on_empty_flows_with_flat_curve() {
        let base = create_date(2025, Month::January, 1).expect("Valid date");
        let flows: Vec<(Date, Money)> = vec![];
        let day_count = DayCount::Act365F;

        let continuous_rate = (1.05_f64).ln();
        let curve = flat_curve("TEST", base, continuous_rate, day_count);

        let err = npv(&curve, base, &flows, false).expect_err("Should fail with empty flows");
        let _ = format!("{}", err);
    }

    #[test]
    fn npv_precision_many_cashflows() {
        // 120 identical `Money` amounts at a unit discount factor. `npv` adds
        // them in Decimal, so the total is exact.
        let curve = ZeroRateCurve {
            id: CurveId::new("PRECISION-TEST"),
        };
        let base = curve.base_date();

        // Create 120 cashflows (30Y quarterly), each 100.0 USD
        // With DF=1.0 (flat curve), the sum should be exactly 12000.0
        let flows: Vec<(Date, Money)> = (1..=120)
            .map(|i| {
                // ~91 days per quarter
                let date = base + time::Duration::days(i as i64 * 91);
                (date, Money::from((100_i64, Currency::USD)))
            })
            .collect();

        let pv = npv(&curve, base, &flows, false).expect("NPV should succeed");

        // Decimal addition of these unit amounts is exact.
        assert!(
            (pv.amount() - 12000.0).abs() < 1e-10,
            "NPV precision lost with {} cashflows: expected 12000.0, got {} (error: {:.2e})",
            flows.len(),
            pv.amount(),
            (pv.amount() - 12000.0).abs()
        );
    }
}

#[cfg(test)]
mod record_016_tests {
    use super::*;
    use crate::currency::Currency;
    use crate::market_data::term_structures::DiscountCurve;
    use crate::types::CurveId;
    use time::Month;

    /// A source whose every factor is invalid: any query errors.
    struct UnusableCurve {
        id: CurveId,
        base: Date,
    }

    impl Discounting for UnusableCurve {
        fn id(&self) -> &CurveId {
            &self.id
        }
        fn base_date(&self) -> Date {
            self.base
        }
        fn df(&self, _t: f64) -> f64 {
            f64::NAN
        }
    }

    fn day(y: i32, m: Month, d: u8) -> Date {
        crate::dates::create_date(y, m, d).expect("date")
    }

    fn usd(amount: i64) -> Money {
        Money::from((amount, Currency::USD))
    }

    #[test]
    fn same_day_cash_is_a_caller_choice() {
        let as_of = day(2025, Month::January, 2);
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(day(2025, Month::January, 1))
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("curve");
        let flows = [(as_of, usd(7)), (day(2026, Month::January, 2), usd(100))];
        let excluded = npv(&curve, as_of, &flows, false).expect("exclude today");
        let included = npv(&curve, as_of, &flows, true).expect("include today");
        assert_eq!(included.checked_sub(excluded).expect("same ccy"), usd(7));
        let scalar: Vec<_> = flows.iter().map(|(d, m)| (*d, m.amount())).collect();
        let s_excluded = npv_amounts(&curve, as_of, &scalar, false).expect("scalar");
        let s_included = npv_amounts(&curve, as_of, &scalar, true).expect("scalar");
        assert!((s_included - s_excluded - 7.0).abs() < 1e-12);
        // The future factor is the relative D(as_of, t), not D(base, t).
        let relative = curve
            .df_between_dates(as_of, flows[1].0)
            .expect("relative df");
        assert!((s_excluded - 100.0 * relative).abs() < 1e-9);
    }

    #[test]
    fn nothing_selected_returns_zero_without_querying_the_source() {
        let as_of = day(2025, Month::January, 1);
        let curve = UnusableCurve {
            id: CurveId::new("UNUSABLE"),
            base: as_of,
        };
        let flows = [
            (day(2024, Month::June, 1), usd(5)),
            (as_of, usd(3)),
            (day(2026, Month::January, 1), usd(0)),
        ];
        assert_eq!(
            npv(&curve, as_of, &flows, false).expect("nothing selected"),
            Money::zero(Currency::USD)
        );
        assert_eq!(
            npv(&curve, as_of, &flows, true).expect("today only"),
            usd(3)
        );
        let scalar = [
            (day(2024, Month::June, 1), 5.0),
            (day(2026, Month::January, 1), 0.0),
        ];
        assert_eq!(
            npv_amounts(&curve, as_of, &scalar, false).expect("scalar"),
            0.0
        );
        assert_eq!(
            npv_amounts(&curve, as_of, &[], true).expect("empty scalar"),
            0.0
        );
        // A selected nonzero future flow still needs its factor.
        assert!(
            npv(
                &curve,
                as_of,
                &[(day(2026, Month::January, 1), usd(1))],
                false
            )
            .is_err()
        );
    }

    #[test]
    fn every_row_is_validated_before_selection() {
        let as_of = day(2025, Month::January, 1);
        let curve = UnusableCurve {
            id: CurveId::new("UNUSABLE"),
            base: as_of,
        };
        // Empty Money supplies no currency.
        assert!(npv(&curve, as_of, &[], false).is_err());
        // A past row's currency still has to match.
        let mixed = [
            (
                day(2024, Month::June, 1),
                Money::from((5_i64, Currency::EUR)),
            ),
            (day(2024, Month::July, 1), usd(5)),
        ];
        assert!(matches!(
            npv(&curve, as_of, &mixed, false),
            Err(crate::Error::CurrencyMismatch { .. })
        ));
        // A discarded past scalar row must still be finite.
        assert!(
            npv_amounts(
                &curve,
                as_of,
                &[(day(2024, Month::June, 1), f64::NAN)],
                false
            )
            .is_err()
        );
    }

    #[test]
    fn scalar_products_that_underflow_are_errors() {
        let as_of = day(2025, Month::January, 1);
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, 0.5)])
            .build()
            .expect("curve");
        let tiny = [(day(2026, Month::January, 1), 5e-324)];
        assert!(npv_amounts(&curve, as_of, &tiny, false).is_err());
    }

    #[test]
    fn flat_factor_rejects_underflow_and_keeps_subnormals() {
        assert!(flat_discount_factor(8.0, 100.0).is_err());
        let subnormal = flat_discount_factor(1.0, 740.0).expect("positive subnormal factor");
        assert!(subnormal > 0.0 && subnormal < f64::MIN_POSITIVE);
        assert!(flat_discount_factor(-1.0, 800.0).is_err(), "overflow");
        assert_eq!(
            flat_discount_factor(f64::MAX, 0.0).expect("zero horizon"),
            1.0
        );
    }
}
