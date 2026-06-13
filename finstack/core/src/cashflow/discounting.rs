//! Present value calculations using market discount curves.
//!
//! This module provides functions for discounting cashflows using market-derived
//! discount curves rather than constant rates. This is the standard approach for
//! pricing fixed income securities and derivatives.
//!
//! # Approach
//!
//! Unlike IRR/XIRR analysis (see [`xirr`](super::xirr)),
//! this module uses term structures of discount factors from market data:
//! [`DiscountCurve`](crate::market_data::term_structures::DiscountCurve) and
//! the [`Discounting`](crate::market_data::traits::Discounting) trait are the
//! canonical curve-side contracts for these present-value operations.
//! ```text
//! PV = Σ CF_i * DF(t_i)
//!
//! where DF(t) is the discount factor from the market curve
//! ```
//!
//! # Valuation-Date Cutoff (IMPORTANT)
//!
//! [`npv`], [`npv_with_ctx`], and the [`Discountable`] trait follow
//! **market-standard pricing semantics**: cashflows dated **on or before** the
//! valuation date are excluded (only strictly-future flows are discounted).
//! A flow that has already paid is not part of the instrument's present value.
//! This default changed per the 2026-06-09 core quant review — previously,
//! past flows were silently future-valued using curve extrapolation.
//!
//! To include flows on or before the valuation date (e.g. for project/
//! investment NPV where the time-0 outlay belongs in the result), opt in via
//! [`NpvOptions::include_past_flows`] and [`npv_with_options`]. The scalar
//! helper [`npv_amounts`] retains the investment-NPV convention (all flows
//! included, signed year fractions) since its default base date is the
//! earliest flow.
//!
//! # Use Cases
//!
//! - **Bond pricing**: Government and corporate bonds
//! - **Swap valuation**: Interest rate swaps using OIS/LIBOR curves
//! - **Derivative pricing**: Future cashflows under risk-neutral measure
//! - **Portfolio valuation**: Mark-to-market of fixed income positions
//!
//! # Examples
//!
//! ```rust
//! use finstack_core::cashflow::npv;
//! use finstack_core::market_data::term_structures::DiscountCurve;
//! use finstack_core::dates::{Date, DayCount};
//! use finstack_core::money::Money;
//! use finstack_core::currency::Currency;
//! use time::Month;
//!
//! // Build a flat discount curve
//! let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let curve = DiscountCurve::builder("USD-OIS")
//!     .base_date(base_date)
//!     .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.78)])
//!     .build()?;
//!
//! // Cashflows to discount
//! let cf1 = (
//!     Date::from_calendar_date(2026, Month::January, 1).expect("Valid date"),
//!     Money::new(100.0, Currency::USD)
//! );
//! let flows = vec![cf1];
//!
//! // Use curve's day count (recommended for par-rate consistency)
//! let pv = npv(&curve, base_date, None, &flows)?;
//! assert!(pv.amount() < 100.0); // Discounted value < face value
//!
//! // Or override with explicit day count when needed
//! let pv_explicit = npv(&curve, base_date, Some(DayCount::Act360), &flows)?;
//! # Ok::<(), finstack_core::Error>(())
//! ```
//!
//! # References
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Pearson. Chapters 4-7 (Interest Rates and Curve Construction).
//! - Andersen, L., & Piterbarg, V. (2010). *Interest Rate Modeling* (3 vols).
//!   Atlantic Financial Press. Volume 1, Chapter 3.

use crate::dates::{Date, DayCount, DayCountContext};
use crate::market_data::traits::Discounting;
use crate::math::NeumaierAccumulator;
use crate::money::Money;

/// Objects that can be present-valued against a `Discount` curve.
///
/// Provides a unified interface for NPV calculations across different
/// cashflow representations and instrument types. Implemented for any
/// type that implements `AsRef<[(Date, Money)]>` (including `&[(..)]`
/// and `Vec<(..)>`).
///
/// # Required Methods
///
/// Implementors must provide:
/// - [`npv`](Self::npv): Compute present value against a discount curve
///
/// # Provided Implementations
///
/// This trait is automatically implemented for any type `T` where
/// `T: AsRef<[(Date, Money)]>`, including:
/// - `&[(Date, Money)]`
/// - `Vec<(Date, Money)>`
///
/// # Examples
///
/// ```rust
/// use finstack_core::cashflow::Discountable;
/// use finstack_core::market_data::term_structures::DiscountCurve;
/// use finstack_core::market_data::traits::Discounting;
/// use finstack_core::dates::{Date, DayCount};
/// use finstack_core::money::Money;
/// use finstack_core::currency::Currency;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = DiscountCurve::builder("USD-OIS")
///     .base_date(base)
///     .knots([(0.0, 1.0), (1.0, 0.95)])
///     .build()?;
///
/// let flows = vec![(
///     Date::from_calendar_date(2026, Month::January, 1).expect("Valid date"),
///     Money::new(100.0, Currency::USD),
/// )];
///
/// // Use the trait method
/// let pv = flows.npv(&curve, base, None)?;
/// # Ok::<(), finstack_core::Error>(())
/// ```
pub trait Discountable {
    /// Output type for the NPV calculation.
    type PVOutput;

    /// Compute present value using the given discount curve.
    ///
    /// Follows market-standard pricing semantics: flows dated on or before
    /// `base` are excluded. See the module docs ("Valuation-Date Cutoff")
    /// and [`npv_with_options`] for the opt-in include-past behavior.
    ///
    /// # Arguments
    ///
    /// * `disc` - Discount curve implementing the `Discounting` trait
    /// * `base` - Valuation date
    /// * `dc` - Day count convention: `None` uses the curve's day count (recommended),
    ///   `Some(dc)` overrides with an explicit day count
    ///
    /// # Returns
    ///
    /// Present value of all strictly-future cashflows discounted to the base date.
    ///
    /// # Errors
    ///
    /// The default implementation returns `Err` when:
    /// - [`InputError::TooFewPoints`](crate::error::InputError::TooFewPoints): Empty cashflow list
    /// - Day count calculation fails (e.g., missing calendar for Bus/252)
    fn npv(&self, disc: &dyn Discounting, base: Date, dc: Option<DayCount>) -> Self::PVOutput;
}

/// Compute NPV of dated `Money` flows using a discount curve with static dispatch.
///
/// By default, uses the curve's own day count convention for year fraction calculations.
/// This ensures consistency between NPV and metric calculations (e.g., par rate).
///
/// # Valuation-Date Cutoff
///
/// Flows dated **on or before** `base` are excluded (market-standard pricing
/// semantics; default changed per the 2026-06-09 core quant review). If every
/// flow is on or before `base`, the result is zero in the flows' currency.
/// Use [`npv_with_options`] with [`NpvOptions::include_past_flows`] for the
/// legacy include-everything behavior.
///
/// # Arguments
///
/// * `disc` - Discount curve implementing the `Discounting` trait
/// * `base` - Valuation date
/// * `dc` - Day count convention: `None` uses the curve's day count (recommended),
///   `Some(dc)` overrides with an explicit day count
/// * `flows` - Dated cashflows to discount
///
/// # Returns
///
/// Present value as a [`Money`] amount in the same currency as the input flows.
///
/// # Errors
///
/// Returns `Err` when:
/// - [`InputError::TooFewPoints`](crate::error::InputError::TooFewPoints): The `flows`
///   slice is empty
/// - Day count year fraction calculation fails (e.g., [`InputError::MissingCalendarForBus252`](crate::error::InputError::MissingCalendarForBus252)
///   when using Bus/252 without a calendar context)
/// - [`Error::CurrencyMismatch`](crate::Error::CurrencyMismatch): Cashflows have
///   mixed currencies (all flows must share the same currency)
///
/// # Day Count Selection
///
/// - **`None` (recommended)**: Uses the curve's internal day count. This ensures
///   consistency between NPV calculations and metrics like par rate, so pricing
///   at par rate yields zero PV.
/// - **`Some(dc)`**: Overrides with an explicit day count. Use when you need a
///   specific convention that differs from the curve's (e.g., instrument-specific accrual).
///
/// # Example
///
/// ```rust
/// use finstack_core::cashflow::npv;
/// use finstack_core::market_data::term_structures::DiscountCurve;
/// use finstack_core::dates::{Date, DayCount};
/// use finstack_core::money::Money;
/// use finstack_core::currency::Currency;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = DiscountCurve::builder("USD-OIS")
///     .base_date(base)
///     .day_count(DayCount::Act360)
///     .knots([(0.0, 1.0), (1.0, 0.95)])
///     .build()?;
///
/// let flows = vec![(
///     Date::from_calendar_date(2026, Month::January, 1).expect("Valid date"),
///     Money::new(100.0, Currency::USD),
/// )];
///
/// // Use curve's day count (recommended)
/// let pv = npv(&curve, base, None, &flows)?;
///
/// // Override with explicit day count
/// let pv_act365 = npv(&curve, base, Some(DayCount::Act365F), &flows)?;
/// # Ok::<(), finstack_core::Error>(())
/// ```
pub fn npv<D: Discounting + ?Sized>(
    disc: &D,
    base: Date,
    dc: Option<DayCount>,
    flows: &[(Date, Money)],
) -> crate::Result<Money> {
    npv_with_ctx(disc, base, dc, DayCountContext::default(), flows)
}

/// Options controlling NPV flow selection (Tier-2 builder style).
///
/// The default excludes flows dated on or before the valuation date
/// (market-standard pricing semantics). See the module docs
/// ("Valuation-Date Cutoff").
///
/// # Examples
///
/// ```rust
/// use finstack_core::cashflow::NpvOptions;
///
/// // Default: strictly-future flows only.
/// let pricing = NpvOptions::default();
///
/// // Investment/project NPV: keep the time-0 outlay and any past flows.
/// let investment = NpvOptions::default().include_past_flows(true);
/// # let _ = (pricing, investment);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NpvOptions {
    include_past_flows: bool,
}

impl NpvOptions {
    /// Create options with the market-standard default (exclude flows on or
    /// before the valuation date).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Include flows dated on or before the valuation date.
    ///
    /// Past flows are then valued with the curve's discount factor at their
    /// (negative or zero) year fraction, i.e. future-valued to the valuation
    /// date — the pre-2026-06-09-review legacy behavior, appropriate for
    /// project/investment NPV that must contain the time-0 outlay.
    #[must_use]
    pub fn include_past_flows(mut self, include: bool) -> Self {
        self.include_past_flows = include;
        self
    }
}

/// Compute NPV of dated `Money` cashflows using an explicit day-count context.
///
/// Flows dated on or before `base` are excluded (see the module docs,
/// "Valuation-Date Cutoff"). Use [`npv_with_options`] to opt in to the
/// legacy include-everything behavior.
///
/// # Errors
///
/// Same error conditions as [`npv`].
pub fn npv_with_ctx<D: Discounting + ?Sized>(
    disc: &D,
    base: Date,
    dc: Option<DayCount>,
    ctx: DayCountContext<'_>,
    flows: &[(Date, Money)],
) -> crate::Result<Money> {
    npv_with_options(disc, base, dc, ctx, NpvOptions::default(), flows)
}

/// Compute NPV of dated `Money` cashflows with explicit [`NpvOptions`].
///
/// This is the most general entry point: it accepts a day-count context and
/// options controlling whether flows on or before the valuation date are
/// included (see [`NpvOptions::include_past_flows`]).
///
/// # Errors
///
/// Returns `Err` when:
/// - [`InputError::TooFewPoints`](crate::error::InputError::TooFewPoints): The `flows`
///   slice is empty
/// - Day count year fraction calculation fails
/// - [`Error::CurrencyMismatch`](crate::Error::CurrencyMismatch): Mixed currencies
/// - The discount factor at the valuation date is non-finite or non-positive
pub fn npv_with_options<D: Discounting + ?Sized>(
    disc: &D,
    base: Date,
    dc: Option<DayCount>,
    ctx: DayCountContext<'_>,
    options: NpvOptions,
    flows: &[(Date, Money)],
) -> crate::Result<Money> {
    if flows.is_empty() {
        return Err(crate::error::InputError::TooFewPoints.into());
    }
    let day_count = dc.unwrap_or_else(|| disc.day_count());
    let ccy = flows[0].1.currency();

    // Validate all cashflows have the same currency
    for (_, amt) in flows.iter().skip(1) {
        if amt.currency() != ccy {
            return Err(crate::Error::CurrencyMismatch {
                expected: ccy,
                actual: amt.currency(),
            });
        }
    }

    // Discount each flow to the valuation date `base`, which need not coincide
    // with the curve's own base date. `Discounting::df` expects an abscissa
    // measured from the *curve* base date, so all year fractions are taken
    // from `disc.base_date()`; the flow is then discounted by the relative
    // factor DF(curve_base→d) / DF(curve_base→base). When `base` equals the
    // curve base the denominator is `df(0) == 1`, so this reduces exactly to
    // the plain `df(t)` lookup.
    //
    // Per-flow discounting: Money × f64 discount factor produces a Money
    // value rounded to Money's Decimal scale. Accumulation of rounded
    // per-flow values is exact at that scale. For bit-exact precision,
    // callers should pre-discount amounts in Decimal and sum via
    // npv_prediscounted_money().
    let curve_base = disc.base_date();
    let t_base = day_count.signed_year_fraction(curve_base, base, ctx)?;
    let df_base = disc.df(t_base);
    if !df_base.is_finite() || df_base <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "npv: discount factor at the valuation date ({base}) is invalid: {df_base}"
        )));
    }

    let mut total = Money::new(0.0, ccy);
    for (d, amt) in flows {
        // Market-standard pricing semantics (2026-06-09 core quant review):
        // flows on or before the valuation date have already paid and are
        // not part of present value. `include_past_flows` opts back in to
        // the legacy include-everything behavior.
        if !options.include_past_flows && *d <= base {
            continue;
        }
        let t = day_count.signed_year_fraction(curve_base, *d, ctx)?;
        let df = disc.df(t) / df_base;
        let disc_amt = *amt * df;
        total = total.checked_add(disc_amt)?;
    }
    Ok(total)
}

/// Sum pre-discounted `Money` cashflows for bit-exact accumulation.
///
/// Callers that need maximum precision should discount each flow
/// using `Decimal` arithmetic and then pass the results here. This
/// avoids the `f64` rounding that occurs in [`npv_with_ctx`] when
/// multiplying `Money` by `f64` discount factors.
///
/// # Errors
///
/// - [`InputError::TooFewPoints`](crate::error::InputError::TooFewPoints): Empty flow slice
/// - [`Error::CurrencyMismatch`](crate::Error::CurrencyMismatch): Mixed currencies
pub fn sum_prediscounted_money(flows: &[Money]) -> crate::Result<Money> {
    if flows.is_empty() {
        return Err(crate::error::InputError::TooFewPoints.into());
    }
    let ccy = flows[0].currency();
    for amt in flows.iter().skip(1) {
        if amt.currency() != ccy {
            return Err(crate::Error::CurrencyMismatch {
                expected: ccy,
                actual: amt.currency(),
            });
        }
    }
    let mut total = Money::new(0.0, ccy);
    for amt in flows {
        total = total.checked_add(*amt)?;
    }
    Ok(total)
}

/// Sum pre-discounted dated `Money` cashflows.
///
/// Dates are ignored because amounts are already discounted. Prefer
/// [`sum_prediscounted_money`] for new code when dates carry no meaning.
pub fn npv_prediscounted_money(flows: &[(Date, Money)]) -> crate::Result<Money> {
    let amounts = flows.iter().map(|(_, amt)| *amt).collect::<Vec<_>>();
    sum_prediscounted_money(&amounts)
}

/// Compute NPV of dated scalar cashflows using a flat annual discount rate.
///
/// This is a convenience helper for performance analytics and bindings that work in
/// scalar amounts (e.g. `[(date, f64)]`) rather than typed [`Money`] cashflows.
///
/// The discounting convention for this helper is:
/// - `discount_rate` is an annually-compounded rate expressed as a decimal (0.05 = 5%)
/// - Internally this is converted to continuous compounding via `ln(1 + r)` for stability.
///
/// Defaults (when the optional arguments are `None`):
/// - `base_date`: first cashflow date
/// - `day_count`: [`DayCount::Act365F`]
///
/// # Flow Convention
///
/// Unlike [`npv`], this helper follows the **investment-NPV convention**: all
/// flows are included, with signed year fractions relative to the base date.
/// The time-0 outlay (a flow on the base date) is part of the result, which
/// is what project/return analytics expect.
///
/// # Errors
/// - [`InputError::TooFewPoints`](crate::error::InputError::TooFewPoints) when `cash_flows` is empty
/// - Day count year-fraction calculation failures
pub fn npv_amounts(
    cash_flows: &[(Date, f64)],
    discount_rate: f64,
    base_date: Option<Date>,
    day_count: Option<DayCount>,
) -> crate::Result<f64> {
    npv_amounts_with_ctx(
        cash_flows,
        discount_rate,
        base_date,
        day_count,
        crate::dates::DayCountContext::default(),
    )
}

/// Compute scalar NPV with an explicit day-count context.
pub fn npv_amounts_with_ctx(
    cash_flows: &[(Date, f64)],
    discount_rate: f64,
    base_date: Option<Date>,
    day_count: Option<DayCount>,
    ctx: crate::dates::DayCountContext<'_>,
) -> crate::Result<f64> {
    if cash_flows.is_empty() {
        return Err(crate::Error::from(crate::error::InputError::TooFewPoints));
    }

    let base = base_date.unwrap_or_else(|| {
        cash_flows
            .iter()
            .map(|(date, _)| *date)
            .min()
            .unwrap_or(cash_flows[0].0)
    });
    let dc = day_count.unwrap_or(DayCount::Act365F);

    // Convert annually compounded rate to continuously compounded rate:
    // FlatCurve expects continuously compounded rates: r_cont = ln(1 + r_annual)
    if !discount_rate.is_finite() || (1.0 + discount_rate) <= 0.0 {
        return Err(crate::Error::from(crate::error::InputError::Invalid));
    }
    let continuous_rate = (1.0 + discount_rate).ln();

    // Use Neumaier compensated summation for numerical stability with many cashflows
    let mut acc = NeumaierAccumulator::new();
    for (date, amount) in cash_flows {
        let t = dc.signed_year_fraction(base, *date, ctx)?;
        acc.add(amount * (-continuous_rate * t).exp());
    }

    Ok(acc.total())
}

#[cfg(test)]
mod hardening_tests {
    use super::*;
    use crate::currency::Currency;
    use crate::dates::calendar::TARGET2;
    use crate::dates::create_date;
    use crate::market_data::term_structures::FlatCurve;
    use time::Month;

    #[test]
    fn npv_amounts_uses_earliest_cashflow_as_default_base_date() {
        let base = create_date(2024, Month::January, 1).expect("Valid test date");
        let later = create_date(2025, Month::January, 1).expect("Valid test date");
        let rate = 0.05;

        let sorted = vec![(base, -100000.0), (later, 110000.0)];
        let unsorted = vec![(later, 110000.0), (base, -100000.0)];

        let pv_sorted = npv_amounts(&sorted, rate, None, Some(DayCount::Act365F))
            .expect("sorted npv should succeed");
        let pv_unsorted = npv_amounts(&unsorted, rate, None, Some(DayCount::Act365F))
            .expect("unsorted npv should succeed");

        assert!((pv_sorted - pv_unsorted).abs() < 1e-10);
    }

    #[test]
    fn npv_with_bus252_context_counts_business_days() {
        let base = create_date(2025, Month::January, 6).expect("Valid test date"); // Monday
        let pay = create_date(2025, Month::January, 13).expect("Valid test date"); // Next Monday
        let curve = FlatCurve::new(0.10, base, DayCount::Bus252, "BRL-FLAT");
        let flows = vec![(pay, Money::new(100.0, Currency::USD))];
        let ctx = DayCountContext {
            calendar: Some(&TARGET2),
            frequency: None,
            bus_basis: None,
            coupon_period: None,
        };

        let pv = npv_with_ctx(&curve, base, Some(DayCount::Bus252), ctx, &flows)
            .expect("Bus/252 NPV should succeed");
        let expected = 100.0 * (-0.10_f64 * (5.0 / 252.0)).exp();
        assert!(
            (pv.amount() - expected).abs() < 1e-10,
            "{} vs {}",
            pv.amount(),
            expected
        );
    }

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
        let flows = vec![(flow_date, Money::new(1_000_000.0, Currency::USD))];

        // Valuation at the curve base: PV = CF · DF(0→2y) = CF · 0.88.
        let pv_at_curve_base = npv(&curve, curve_base, None, &flows).expect("npv");
        assert!((pv_at_curve_base.amount() - 880_000.0).abs() < 1.0);

        // Valuation one year forward must use the relative DF
        // DF(1y→2y) = df(2)/df(1) = 0.88/0.95, not df(year_fraction(val,flow)).
        let pv_forward = npv(&curve, val_date, None, &flows).expect("npv");
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

/// Compute NPV of dated `Money` flows using a discount curve.
///
/// Discounts each cashflow to the base date using the provided curve.
/// All flows must be in the same currency for the calculation to succeed.
impl<T> Discountable for T
where
    T: AsRef<[(Date, Money)]>,
{
    type PVOutput = crate::Result<Money>;

    fn npv(
        &self,
        disc: &dyn Discounting,
        base: Date,
        dc: Option<DayCount>,
    ) -> crate::Result<Money> {
        npv(disc, base, dc, self.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::Currency;
    use crate::dates::create_date;
    use crate::market_data::term_structures::FlatCurve;
    use crate::market_data::traits::TermStructure;
    use crate::types::CurveId;
    use time::Month;

    /// Test helper: creates a flat curve with DF=1.0 for all times (0% rate).
    struct ZeroRateCurve {
        id: CurveId,
    }

    impl TermStructure for ZeroRateCurve {
        fn id(&self) -> &CurveId {
            &self.id
        }
    }

    impl Discounting for ZeroRateCurve {
        fn base_date(&self) -> Date {
            Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date")
        }
        fn df(&self, _t: f64) -> f64 {
            1.0
        }
    }

    #[test]
    fn tuples_discountable_paths_through() {
        let curve = ZeroRateCurve {
            id: CurveId::new("USD-OIS"),
        };
        let base = curve.base_date();
        // Flows must be strictly after the valuation date to be included
        // (2026-06-09 core quant review: npv excludes flows on/before base).
        let pay = base + time::Duration::days(1);
        let flows = vec![
            (pay, Money::new(10.0, crate::currency::Currency::USD)),
            (pay, Money::new(5.0, crate::currency::Currency::USD)),
        ];
        // Use None to use curve's default day count
        let pv = flows
            .npv(&curve, base, None)
            .expect("NPV calculation should succeed in test");
        assert!((pv.amount() - 15.0).abs() < 1e-12);
    }

    #[test]
    fn tuples_discountable_with_explicit_dc() {
        let curve = ZeroRateCurve {
            id: CurveId::new("USD-OIS"),
        };
        let base = curve.base_date();
        let pay = base + time::Duration::days(1);
        let flows = vec![
            (pay, Money::new(10.0, crate::currency::Currency::USD)),
            (pay, Money::new(5.0, crate::currency::Currency::USD)),
        ];
        // Use explicit day count
        let pv = flows
            .npv(&curve, base, Some(DayCount::Act365F))
            .expect("NPV calculation should succeed in test");
        assert!((pv.amount() - 15.0).abs() < 1e-12);
    }

    #[test]
    fn npv_errors_on_empty_flows() {
        let curve = ZeroRateCurve {
            id: CurveId::new("USD-OIS"),
        };
        let base = curve.base_date();
        let flows: Vec<(Date, Money)> = vec![];
        let err = npv(&curve, base, None, &flows).expect_err("Should fail with empty flows");
        let _ = format!("{}", err);
    }

    #[test]
    fn test_npv_simple_with_flat_curve() {
        let base = create_date(2024, Month::January, 1).expect("Valid test date");
        let flows = vec![
            (base, Money::new(-100000.0, Currency::USD)),
            (
                create_date(2025, Month::January, 1).expect("Valid test date"),
                Money::new(110000.0, Currency::USD),
            ),
        ];
        let rate: f64 = 0.05;
        let dc = DayCount::Act365F;

        // Create FlatCurve with continuous rate
        let continuous_rate = (1.0 + rate).ln();
        let curve = FlatCurve::new(continuous_rate, base, dc, "NPV-TEST");

        // 2026-06-09 core quant review + user decision: the default npv now
        // excludes flows on or before the valuation date, so the time-0
        // outlay (-100000 at base) is NOT part of the pricing PV.
        let pv =
            npv(&curve, base, Some(dc), &flows).expect("NPV calculation should succeed in test");
        // Approximately: 110000/(1.05) ≈ 104761.90 (initial outlay excluded)
        assert!(pv.amount() > 104700.0 && pv.amount() < 104800.0);

        // Investment-NPV semantics (legacy default) via explicit opt-in.
        let pv_investment = npv_with_options(
            &curve,
            base,
            Some(dc),
            DayCountContext::default(),
            NpvOptions::default().include_past_flows(true),
            &flows,
        )
        .expect("NPV with include_past_flows should succeed");
        // Approximately: -100000 + 110000/(1.05) ≈ 4761.90
        assert!(pv_investment.amount() > 4700.0 && pv_investment.amount() < 4800.0);
    }

    #[test]
    fn test_npv_amounts_matches_money_npv() {
        let base = create_date(2024, Month::January, 1).expect("Valid test date");
        let dates = [
            base,
            create_date(2025, Month::January, 1).expect("Valid test date"),
        ];
        let amounts = [-100000.0, 110000.0];

        let amount_flows = vec![(dates[0], amounts[0]), (dates[1], amounts[1])];
        let money_flows = vec![
            (dates[0], Money::new(amounts[0], Currency::USD)),
            (dates[1], Money::new(amounts[1], Currency::USD)),
        ];

        let rate: f64 = 0.05;
        let dc = DayCount::Act365F;

        // Scalar NPV via npv_amounts (investment convention: includes the
        // base-date flow). Compare against npv_with_options with
        // include_past_flows, since the default npv now excludes flows on
        // or before the valuation date (2026-06-09 core quant review).
        let pv_amounts =
            npv_amounts(&amount_flows, rate, None, None).expect("npv_amounts should succeed");

        // Money NPV via npv_with_options with FlatCurve
        let continuous_rate = (1.0 + rate).ln();
        let curve = FlatCurve::new(continuous_rate, base, dc, "TEST");
        let pv_money = npv_with_options(
            &curve,
            base,
            Some(dc),
            DayCountContext::default(),
            NpvOptions::default().include_past_flows(true),
            &money_flows,
        )
        .expect("npv should succeed")
        .amount();

        assert!(
            (pv_amounts - pv_money).abs() < 1e-10,
            "npv_amounts should match npv: {} vs {}",
            pv_amounts,
            pv_money
        );
    }

    #[test]
    fn test_npv_zero_discount() {
        let base = create_date(2024, Month::January, 1).expect("Valid test date");
        let flows = vec![
            (base, Money::new(-100.0, Currency::USD)),
            (
                create_date(2025, Month::January, 1).expect("Valid test date"),
                Money::new(100.0, Currency::USD),
            ),
        ];
        let dc = DayCount::Act365F;

        // Create FlatCurve with 0% rate (continuous rate = ln(1) = 0)
        let curve = FlatCurve::new(0.0, base, dc, "ZERO-RATE");

        // Default pricing semantics exclude the base-date flow
        // (2026-06-09 core quant review), so only the +100 remains.
        let pv =
            npv(&curve, base, Some(dc), &flows).expect("NPV calculation should succeed in test");
        assert_eq!(pv.amount(), 100.0);

        // With include_past_flows the legacy result (0.0) is recovered.
        let pv_all = npv_with_options(
            &curve,
            base,
            Some(dc),
            DayCountContext::default(),
            NpvOptions::default().include_past_flows(true),
            &flows,
        )
        .expect("NPV with include_past_flows should succeed");
        assert_eq!(pv_all.amount(), 0.0);
    }

    /// Default pricing semantics exclude flows on or before the valuation
    /// date; `include_past_flows` restores the legacy include-everything
    /// behavior (2026-06-09 core quant review + user decision).
    #[test]
    fn test_npv_excludes_past_flows_by_default() {
        let base = create_date(2025, Month::January, 1).expect("Valid test date");
        let past = create_date(2024, Month::July, 1).expect("Valid test date");
        let future = create_date(2025, Month::July, 1).expect("Valid test date");
        let flows = vec![
            (past, Money::new(-50.0, Currency::USD)), // past relative to base
            (base, Money::new(-25.0, Currency::USD)), // on the valuation date
            (future, Money::new(55.0, Currency::USD)), // future relative to base
        ];
        let rate: f64 = 0.05;
        let dc = DayCount::Act365F;

        let continuous_rate = (1.0 + rate).ln();
        let curve = FlatCurve::new(continuous_rate, base, dc, "TEST");

        // Default: only the strictly-future +55 flow is priced.
        let pv =
            npv(&curve, base, Some(dc), &flows).expect("NPV calculation should succeed in test");
        let only_future = npv(&curve, base, Some(dc), &flows[2..])
            .expect("future-only NPV should succeed in test");
        assert_eq!(pv.amount(), only_future.amount());
        assert!(pv.amount() > 0.0 && pv.amount() < 55.0);

        // Opt-in: past and on-date flows are included (future-valued at the
        // curve's signed year fraction), reproducing the legacy behavior.
        let pv_all = npv_with_options(
            &curve,
            base,
            Some(dc),
            DayCountContext::default(),
            NpvOptions::default().include_past_flows(true),
            &flows,
        )
        .expect("NPV with include_past_flows should succeed");
        assert!(pv_all.amount() < pv.amount());
    }

    /// If every flow is on or before the valuation date, the default
    /// pricing NPV is zero in the flows' currency (nothing left to price).
    #[test]
    fn test_npv_all_past_flows_is_zero() {
        let base = create_date(2025, Month::January, 1).expect("Valid test date");
        let flows = vec![
            (
                create_date(2024, Month::July, 1).expect("Valid test date"),
                Money::new(100.0, Currency::USD),
            ),
            (base, Money::new(50.0, Currency::USD)),
        ];
        let dc = DayCount::Act365F;
        let curve = FlatCurve::new((1.05_f64).ln(), base, dc, "TEST");

        let pv = npv(&curve, base, Some(dc), &flows).expect("NPV should succeed");
        assert_eq!(pv.amount(), 0.0);
        assert_eq!(pv.currency(), Currency::USD);
    }

    #[test]
    fn test_npv_errors_on_empty_flows_with_flat_curve() {
        let base = create_date(2025, Month::January, 1).expect("Valid date");
        let flows: Vec<(Date, Money)> = vec![];
        let dc = DayCount::Act365F;

        let continuous_rate = (1.05_f64).ln();
        let curve = FlatCurve::new(continuous_rate, base, dc, "TEST");

        let err = npv(&curve, base, Some(dc), &flows).expect_err("Should fail with empty flows");
        let _ = format!("{}", err);
    }

    #[test]
    fn npv_precision_many_cashflows() {
        // Regression test for Neumaier compensated summation precision.
        // A 30Y quarterly swap has 120 cashflows where naive summation can
        // accumulate floating-point errors of ~1e-10 to 1e-9 of total PV.
        // With Neumaier summation, we should maintain much higher precision.
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
                (date, Money::new(100.0, Currency::USD))
            })
            .collect();

        let pv = npv(&curve, base, None, &flows).expect("NPV should succeed");

        // With Neumaier summation, we expect precision better than 1e-10
        assert!(
            (pv.amount() - 12000.0).abs() < 1e-10,
            "NPV precision lost with {} cashflows: expected 12000.0, got {} (error: {:.2e})",
            flows.len(),
            pv.amount(),
            (pv.amount() - 12000.0).abs()
        );
    }

    #[test]
    fn npv_prediscounted_money_exact_summation() {
        let base = create_date(2025, Month::January, 1).expect("Valid test date");
        let flows: Vec<(Date, Money)> = (1..=120)
            .map(|i| {
                let date = base + time::Duration::days(i as i64 * 91);
                (date, Money::new(100.0, Currency::USD))
            })
            .collect();

        let pv = npv_prediscounted_money(&flows).expect("summation should succeed");
        assert!(
            (pv.amount() - 12000.0).abs() < 1e-12,
            "expected exact 12000.0, got {} (error: {:.2e})",
            pv.amount(),
            (pv.amount() - 12000.0).abs()
        );
    }

    #[test]
    fn sum_prediscounted_money_accepts_money_without_dates() {
        let flows: Vec<Money> = (1..=120)
            .map(|_| Money::new(100.0, Currency::USD))
            .collect();

        let pv = sum_prediscounted_money(&flows).expect("summation should succeed");
        assert_eq!(pv.currency(), Currency::USD);
        assert!((pv.amount() - 12000.0).abs() < 1e-12);
    }

    #[test]
    fn npv_prediscounted_money_empty_errors() {
        let flows: Vec<(Date, Money)> = vec![];
        assert!(
            npv_prediscounted_money(&flows).is_err(),
            "empty flows should error"
        );
    }

    #[test]
    fn npv_prediscounted_money_currency_mismatch_errors() {
        let base = create_date(2025, Month::January, 1).expect("Valid test date");
        let flows = vec![
            (base, Money::new(100.0, Currency::USD)),
            (base, Money::new(100.0, Currency::EUR)),
        ];
        assert!(
            npv_prediscounted_money(&flows).is_err(),
            "mixed currencies should error"
        );
    }

    #[test]
    fn npv_amounts_precision_many_cashflows() {
        // Same precision test for npv_amounts (scalar version)
        let base = create_date(2025, Month::January, 1).expect("Valid test date");

        // Create 120 cashflows with 0% discount rate (DF=1.0 at all times)
        let flows: Vec<(Date, f64)> = (1..=120)
            .map(|i| {
                let date = base + time::Duration::days(i as i64 * 91);
                (date, 100.0)
            })
            .collect();

        let pv = npv_amounts(&flows, 0.0, Some(base), None).expect("npv_amounts should succeed");

        // With Neumaier summation, we expect precision better than 1e-10
        assert!(
            (pv - 12000.0).abs() < 1e-10,
            "npv_amounts precision lost with {} cashflows: expected 12000.0, got {} (error: {:.2e})",
            flows.len(),
            pv,
            (pv - 12000.0).abs()
        );
    }
}
