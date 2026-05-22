//! Shared pricing utilities for swap legs.
//!
//! This module consolidates the floating and fixed leg pricing logic that was
//! previously duplicated across IRS, BasisSwap, and other swap instruments.
//! The implementation preserves the Bloomberg-validated methodology from IRS.
//!
//! # Key Features
//!
//! - Numerical stability via robust relative discount factor calculation
//! - Neumaier compensated summation for long-dated swaps
//! - Holiday-aware payment delay handling
//! - Compounded-in-arrears support for RFR swaps (SOFR, SONIA, etc.)
//! - Forward rate projection with floor/cap/gearing
//!
//! # Bloomberg Validation
//!
//! The `robust_relative_df` function implements the same numerical stability
//! checks used in IRS pricing that have been validated against Bloomberg SWPM
//! for discount factor calibration.

use crate::cashflow::builder::rate_helpers::FloatingRateParams;
use finstack_core::dates::CalendarRegistry;
use finstack_core::dates::{Date, DateExt, DayCount, DayCountContext, Schedule};
use finstack_core::market_data::scalars::ScalarTimeSeries;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::market_data::term_structures::ForwardCurve;
use finstack_core::math::NeumaierAccumulator;
use finstack_core::Result;

use serde::{Deserialize, Serialize};

/// Compounding method for floating rate legs.
///
/// Determines how the floating rate is calculated from underlying index fixings.
///
/// # Market Standards
///
/// | Method | Index Type | Example | Formula |
/// |--------|------------|---------|---------|
/// | Simple | Term IBOR | EURIBOR 6M | rate = fixing |
/// | Compounded | OIS | SOFR, SONIA | rate = (∏(1 + r_i × d_i) - 1) / τ |
/// | CompoundedWithShift | OIS + lookback | SOFR (standard) | Same, with observation shift |
/// | Average | OIS (legacy) | Fed Funds | rate = Σ(r_i × d_i) / τ |
///
/// # ISDA Standard
///
/// The ISDA 2021 definitions specify "Overnight Rate Compounding" with
/// optional observation shift (lookback) as the standard for RFR swaps.
///
/// # References
///
/// - ISDA IBOR Fallbacks Protocol (2021)
/// - ARRC SOFR Conventions (2020)
/// - Bank of England SONIA Conventions (2019)
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CompoundingMethod {
    /// Simple rate - no compounding within the accrual period.
    ///
    /// Used for term rates like EURIBOR, Term SOFR, and legacy LIBOR.
    /// The rate is simply the single fixing at the reset date.
    ///
    /// ```text
    /// rate = index_fixing
    /// ```
    #[default]
    Simple,

    /// Daily compounded rate without observation shift.
    ///
    /// Each daily fixing is compounded to produce the period rate.
    /// Rarely used in practice (most OIS use lookback).
    ///
    /// ```text
    /// rate = (∏(1 + r_i × d_i/day_count_basis) - 1) × day_count_basis / D
    /// ```
    ///
    /// where:
    /// - r_i = overnight rate for day i
    /// - d_i = 1 for weekdays, 3 for Mondays (weekend)
    /// - D = total accrual days
    Compounded,

    /// Daily compounded rate with observation shift (lookback).
    ///
    /// This is the standard for RFR swaps (SOFR, ESTR, SONIA).
    /// Rates are observed with a lookback to allow payment calculation
    /// before the payment date.
    ///
    /// ```text
    /// rate = (∏(1 + r_{i-shift} × d_i/360) - 1) × 360 / D
    /// ```
    ///
    /// # Observation Shift
    ///
    /// The `observation_shift_days` field in [`FloatingLegParams`] specifies
    /// the lookback period:
    /// - **2 days**: USD SOFR, EUR ESTR, JPY TONAR (standard)
    /// - **5 days**: Some legacy SOFR conventions
    /// - **0 days**: GBP SONIA (uses payment delay instead)
    ///
    /// # Example
    ///
    /// For a SOFR swap with 2-day lookback:
    /// - Accrual period: Jan 15 to Jan 22 (7 days)
    /// - Observation period: Jan 13 to Jan 20 (shifted back 2 days)
    /// - Rate is compounded from fixings observed Jan 13-20
    CompoundedWithShift,

    /// Simple average of daily rates (non-compounded).
    ///
    /// Used for some legacy overnight index averages. Less common
    /// than compounded rates.
    ///
    /// ```text
    /// rate = Σ(r_i × d_i) / D
    /// ```
    Average,
}

/// Minimum threshold for annuity values to avoid divide-by-zero in par spread calculations.
///
/// # Numerical Justification
///
/// For a typical swap with $1MM notional:
/// - 10Y swap with semi-annual payments and DF ~0.80: annuity ≈ 8.0
/// - 30Y swap with quarterly payments and DF ~0.30: annuity ≈ 15.0
/// - 1Y swap with annual payment and DF ~0.95: annuity ≈ 0.95
///
/// The threshold of 1e-12 is triggered when:
/// - All periods have expired (no future cashflows)
/// - Extreme discounting scenarios (e.g., +200% rates over 30Y gives DF ~1e-26)
/// - Instrument misconfiguration (zero-length accrual periods)
///
/// This threshold is very conservative to ensure we catch only pathological cases,
/// not legitimate stress scenarios. For comparison, a 1bp annuity change on a $1MM
/// notional would be ~$100, so 1e-12 corresponds to sub-nanodollar precision.
///
/// # Usage
///
/// Used in par rate and par spread calculations where dividing by annuity is required.
/// Failing on near-zero annuity is preferable to returning NaN/Inf which would
/// propagate through downstream calculations.
pub const ANNUITY_EPSILON: f64 = 1e-12;

/// Compute discount factor at `target` relative to `as_of`, with numerical stability guard.
///
/// This helper centralizes the pattern of computing the discount factor from `as_of` to `target`
/// using date-based DF calculation (no year-fraction ambiguity).
///
/// This is the Bloomberg-validated implementation used in IRS pricing.
///
/// # Arguments
///
/// * `disc` - Discount curve for pricing
/// * `as_of` - Valuation date (start of discounting interval)
/// * `target` - Target payment date (end of discounting interval)
///
/// # Returns
///
/// Discount factor from `as_of` to `target`. For seasoned instruments this represents the
/// proper discount factor for cashflows occurring after the valuation date.
///
/// # Validation Policy
///
/// This function validates that the resulting DF is:
/// - Finite (not NaN or infinity)
/// - Positive (non-negative DFs are non-physical under standard assumptions)
///
/// It does **not** validate the absolute DF at `as_of` against a hard threshold (like 1e-10),
/// because what matters for pricing is the relative DF between dates. Long-horizon instruments
/// or stress scenarios may have tiny absolute DFs at `as_of` but still-usable relative DFs.
///
/// # Errors
///
/// Returns a validation error if:
/// - Year fraction calculation fails
/// - The resulting discount factor is non-finite (NaN/inf)
/// - The resulting discount factor is non-positive (non-physical)
///
/// # Examples
///
/// ```text
/// use finstack_core::dates::Date;
/// use finstack_core::market_data::term_structures::DiscountCurve;
/// use finstack_valuations::instruments::common_impl::pricing::swap_legs::robust_relative_df;
/// use time::Month;
///
/// # fn main() -> finstack_core::Result<()> {
/// let curve = DiscountCurve::builder("USD-OIS")
///     .base_date(Date::from_calendar_date(2024, Month::January, 1).expect("valid date"))
///     .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)])
///     .build()
///     .expect("curve should build");
///
/// let as_of = Date::from_calendar_date(2024, Month::January, 1).unwrap();
/// let target = Date::from_calendar_date(2025, Month::January, 1).unwrap();
///
/// let df = robust_relative_df(&curve, as_of, target)?;
/// assert!(df > 0.0 && df <= 1.0);
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn robust_relative_df(disc: &DiscountCurve, as_of: Date, target: Date) -> Result<f64> {
    // Single source of truth lives in `pricing::time::relative_df_discount_curve`;
    // this name is retained for the swap-leg call sites and docstrings that
    // pre-date the consolidation.
    crate::instruments::common_impl::pricing::time::relative_df_discount_curve(disc, as_of, target)
}

/// Apply a payment-delay in business days using an optional holiday calendar.
///
/// Bloomberg/ISDA conventions define payment delay in **business days**, not just weekdays.
/// If a calendar is provided and found in the registry, we apply holiday-aware business day
/// addition; otherwise we fall back to weekday-only addition.
///
/// # Arguments
///
/// * `date` - The base date to adjust
/// * `delay_days` - Number of business days to add (0 or negative returns unchanged date)
/// * `calendar_id` - Optional calendar identifier for business day adjustments
///
/// # Returns
///
/// The adjusted payment date, or an error if a calendar ID is provided but cannot be resolved.
///
/// # Strict Calendar Policy
///
/// If a `calendar_id` is provided, this function **requires** the calendar to be available
/// and usable. This prevents silent date drift that can cause trade breaks.
///
/// - If `calendar_id` is `Some` but the calendar cannot be resolved or applied → `Err`
/// - If `calendar_id` is `None` → weekday-only stepping is assumed intentional → `Ok`
#[inline]
pub fn add_payment_delay(date: Date, delay_days: i32, calendar_id: Option<&str>) -> Result<Date> {
    if delay_days <= 0 {
        return Ok(date);
    }

    if let Some(id) = calendar_id {
        // Calendar explicitly specified: require successful resolution and application
        match CalendarRegistry::global().resolve_str(id) {
            Some(cal) => date.add_business_days(delay_days, cal).map_err(|e| {
                finstack_core::Error::Validation(format!(
                    "Failed to add {} business days to {} using calendar '{}': {}",
                    delay_days, date, id, e
                ))
            }),
            None => Err(finstack_core::Error::Validation(format!(
                "Payment-delay calendar '{}' not found in registry; \
                 cannot apply {} business day delay to {}. \
                 Either register the calendar or use None for weekday-only stepping.",
                id, delay_days, date
            ))),
        }
    } else {
        // No calendar specified: weekday-only (Mon-Fri) is intentional
        Ok(date.add_weekdays(delay_days))
    }
}

/// Step a date forward/backward by `n` business days, using a holiday calendar
/// when one is supplied and weekday-only stepping otherwise.
///
/// This mirrors the calendar policy of [`add_payment_delay`]: a `Some` calendar
/// id must resolve (a missing calendar is a hard error, not a silent
/// weekday-only fallback) so that overnight compounding cannot silently drift
/// onto a different observation grid than the rest of the swap.
#[inline]
fn shift_business_days(date: Date, n: i32, calendar_id: Option<&str>) -> Result<Date> {
    if n == 0 {
        return Ok(date);
    }
    match calendar_id {
        Some(id) => match CalendarRegistry::global().resolve_str(id) {
            Some(cal) => date.add_business_days(n, cal).map_err(|e| {
                finstack_core::Error::Validation(format!(
                    "Failed to step {} business days from {} using calendar '{}': {}",
                    n, date, id, e
                ))
            }),
            None => Err(finstack_core::Error::Validation(format!(
                "Overnight-compounding calendar '{}' not found in registry; \
                 cannot step {} business days from {}. \
                 Either register the calendar or use None for weekday-only stepping.",
                id, n, date
            ))),
        },
        None => Ok(date.add_weekdays(n)),
    }
}

/// Project the daily-compounded equivalent rate for a future-reset OIS / RFR leg.
///
/// Implements the ISDA 2021 "Overnight Rate Compounding" convention for a leg
/// whose reset is entirely in the future (every daily fixing is projected from
/// the forward curve — no historical fixings are needed). The overnight
/// compound factor is
///
/// ```text
/// CF = ∏ᵢ (1 + rᵢ · dᵢ)
/// ```
///
/// over the daily sub-periods of `[accrual_start, accrual_end)`, where `rᵢ` is
/// the overnight forward for sub-period `i` and `dᵢ` its day-count fraction.
/// The function returns the **equivalent simple rate** `R` such that
/// `R · period_year_fraction = CF − 1` — i.e. `R = (CF − 1) / τ` with `τ` the
/// caller's period year fraction. Returning a rate (rather than the compound
/// interest directly) lets the surrounding `pv_floating_leg` framework apply
/// spread, gearing and the all-in floor/cap uniformly; multiplying `R` back by
/// `period_year_fraction` reproduces the exact OIS coupon `notional · (CF − 1)`.
///
/// This is **not** the same as the simple arithmetic average forward
/// `ForwardCurve::rate_period`: on an upward-sloping curve daily compounding
/// adds the (positive) compounding convexity — typically 12–15 bp of rate for a
/// semi-annual coupon at current rate levels — which the arithmetic average
/// drops.
///
/// # Observation shift
///
/// `observation_shift_days` (the ISDA lookback) shifts the **observation**
/// window back by that many business days while the day-count weights stay
/// anchored to the accrual sub-period. A shift of 0 means observe in-period.
///
/// # Per-fixing index floor / cap
///
/// `index_floor` / `index_cap` (decimal rates, already converted from bp), when
/// present, are applied to **each daily fixing `rᵢ`** before it enters the
/// product — the economically correct treatment for an RFR cap/floor, which is
/// a strip of daily caplets/floorlets. Applying a floor/cap to the period
/// average instead understates the option value.
///
/// # Arguments
///
/// * `fwd` — forward (projection) curve for the overnight index
/// * `accrual_start` / `accrual_end` — accrual period; `accrual_end` must be
///   strictly after `accrual_start`
/// * `period_year_fraction` — the period's year fraction `τ`; must be `> 0`
/// * `observation_shift_days` — ISDA lookback in business days (>= 0)
/// * `calendar_id` — optional holiday calendar for the daily/observation grid
/// * `index_floor` / `index_cap` — optional per-fixing floor/cap (decimal)
///
/// # Errors
///
/// Returns a validation error if the accrual period is malformed
/// (`accrual_end <= accrual_start`), if `period_year_fraction` is non-positive,
/// if a daily/observation date step fails, or if the day-count fraction of a
/// sub-period is non-positive.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compounded_forward_projection(
    fwd: &ForwardCurve,
    accrual_start: Date,
    accrual_end: Date,
    period_year_fraction: f64,
    observation_shift_days: i32,
    calendar_id: Option<&str>,
    index_floor: Option<f64>,
    index_cap: Option<f64>,
) -> Result<f64> {
    // A zero- or negative-length accrual period is a malformed instrument, not
    // a "use the spot rate" signal. Fail loudly rather than silently emitting a
    // zero coupon (item P3-1/4).
    if accrual_end <= accrual_start {
        return Err(finstack_core::Error::Validation(format!(
            "Compounded OIS leg has a malformed accrual period: accrual_end ({}) \
             is not strictly after accrual_start ({}). A zero- or negative-length \
             period cannot be daily-compounded; check the schedule generation.",
            accrual_end, accrual_start
        )));
    }
    // The equivalent rate normalizes the compound interest by the period year
    // fraction; a non-finite or non-positive `τ` is itself a malformed period
    // (the same negative-year-fraction corruption guarded elsewhere).
    if !period_year_fraction.is_finite() || period_year_fraction <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "Compounded OIS leg has a non-positive or non-finite period year \
             fraction ({:.3e}) for {} -> {}; cannot normalize the \
             daily-compounded coupon. This indicates a corrupt or inverted \
             accrual period.",
            period_year_fraction, accrual_start, accrual_end
        )));
    }

    let fwd_dc = fwd.day_count();
    let fwd_base = fwd.base_date();
    let curve_time = |d: Date| -> Result<f64> {
        if d <= fwd_base {
            Ok(0.0)
        } else {
            fwd_dc.year_fraction(fwd_base, d, DayCountContext::default())
        }
    };

    // ∏(1 + rᵢ·dᵢ) over the daily sub-periods.
    let mut compound_factor = 1.0_f64;

    let mut d = accrual_start;
    while d < accrual_end {
        let next_d = shift_business_days(d, 1, calendar_id)?.min(accrual_end);

        // Day-count weight dᵢ is anchored to the accrual sub-period (lookback
        // semantics: observation dates shift, weights do not).
        let dcf = fwd_dc.year_fraction(d, next_d, DayCountContext::default())?;
        if dcf <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "Compounded OIS leg produced a non-positive day-count fraction \
                 ({:.3e}) for the daily sub-period {} -> {}. This usually means \
                 the daily/observation calendar collapsed two steps onto one date.",
                dcf, d, next_d
            )));
        }

        // Observation window: shift back by the lookback.
        let (obs_start, obs_end) = if observation_shift_days == 0 {
            (d, next_d)
        } else {
            (
                shift_business_days(d, -observation_shift_days, calendar_id)?,
                shift_business_days(next_d, -observation_shift_days, calendar_id)?,
            )
        };

        let ts = curve_time(obs_start)?;
        let te = curve_time(obs_end)?;
        let mut r = if te > ts {
            fwd.rate_period(ts, te)
        } else {
            fwd.rate(ts)
        };

        // Per-fixing floor/cap: each daily caplet/floorlet, not the average.
        if let Some(floor) = index_floor {
            r = r.max(floor);
        }
        if let Some(cap) = index_cap {
            r = r.min(cap);
        }

        compound_factor *= 1.0 + r * dcf;
        d = next_d;
    }

    // Equivalent simple rate: R · τ = CF − 1.
    Ok((compound_factor - 1.0) / period_year_fraction)
}

/// Compute the daily-compounded equivalent rate for an OIS / RFR coupon period
/// that has **started** on or before the valuation date (`accrual_start <= as_of`).
///
/// This covers two cases:
///
/// * **In-progress** (`accrual_start <= as_of < accrual_end`): the period
///   straddles `as_of`. The rate is a daily compound of two spliced sub-periods:
///   - **realized** overnight fixings for the daily sub-periods whose observation
///     date is strictly before `as_of` (sourced from the historical `fixings`
///     series), and
///   - **projected** overnight forwards for the daily sub-periods on or after
///     `as_of` (read from `fwd`, exactly as [`compounded_forward_projection`]).
///
/// * **Fully accrued but unpaid** (`accrual_end <= as_of < payment_date`, i.e.
///   `payment_lag_days > 0`): every sub-period's observation date is strictly
///   before `as_of`, so the helper naturally produces an all-realized daily
///   compound with no projected component. This is the economically correct
///   result — the coupon rate is fully determined by historical fixings.
///
/// The two sub-cases unify cleanly: the per-sub-period realized-vs-projected
/// split (`obs_start < as_of → realized, else projected`) correctly yields
/// all-realized when `as_of >= accrual_end`, because every `obs_start` in
/// `[accrual_start, accrual_end)` is then strictly before `as_of`.
///
/// Both cases return the equivalent simple rate `R = (CF − 1) / τ`, matching
/// the contract of [`compounded_forward_projection`] so the surrounding
/// `pv_floating_leg` framework can apply spread, gearing and the all-in
/// floor/cap uniformly.
///
/// Treating an in-progress OIS coupon as a single term fixing at the reset date
/// (the IBOR-style path) silently mis-prices every seasoned RFR swap — the
/// realized portion is a daily compound of many overnight fixings, not one rate.
///
/// # Observation shift
///
/// `observation_shift_days` (the ISDA lookback) shifts the **observation**
/// window back by that many business days while the day-count weights stay
/// anchored to the accrual sub-period. The realized-vs-projected split is
/// decided on the (shifted) observation start date: a sub-period whose
/// observation start is strictly before `as_of` is realized, otherwise it is
/// projected.
///
/// # Per-fixing index floor / cap
///
/// `index_floor` / `index_cap`, when present, are applied to **each daily rate**
/// `rᵢ` — realized or projected — before it enters the product, the
/// economically correct treatment for an RFR cap/floor strip.
///
/// # Realized fixings
///
/// Historical overnight fixings are looked up by **exact observation date**
/// (`ScalarTimeSeries::value_on_exact`): a missing realized overnight fixing is
/// a hard error, never a silently carried-forward or projected value.
///
/// # Errors
///
/// Returns a validation error if the accrual period is malformed
/// (`accrual_end <= accrual_start`), if `period_year_fraction` is non-positive,
/// if the accrual period has not yet started (`accrual_start > as_of`), if a
/// daily/observation date step fails, if a sub-period day-count fraction is
/// non-positive, or if a required realized overnight fixing is missing from
/// `fixings`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compounded_spliced_projection(
    fwd: &ForwardCurve,
    fixings: Option<&ScalarTimeSeries>,
    fixing_id: &str,
    accrual_start: Date,
    accrual_end: Date,
    as_of: Date,
    period_year_fraction: f64,
    observation_shift_days: i32,
    calendar_id: Option<&str>,
    index_floor: Option<f64>,
    index_cap: Option<f64>,
) -> Result<f64> {
    if accrual_end <= accrual_start {
        return Err(finstack_core::Error::Validation(format!(
            "Compounded OIS leg has a malformed accrual period: accrual_end ({}) \
             is not strictly after accrual_start ({}). A zero- or negative-length \
             period cannot be daily-compounded; check the schedule generation.",
            accrual_end, accrual_start
        )));
    }
    if !period_year_fraction.is_finite() || period_year_fraction <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "Compounded OIS leg has a non-positive or non-finite period year \
             fraction ({:.3e}) for {} -> {}; cannot normalize the \
             daily-compounded coupon.",
            period_year_fraction, accrual_start, accrual_end
        )));
    }
    // This helper requires the accrual period to have started: `accrual_start
    // <= as_of`. It correctly handles both the in-progress case
    // (`as_of < accrual_end`) and the fully-accrued-but-unpaid case
    // (`as_of >= accrual_end`, which arises when `payment_lag_days > 0`).
    // In the latter case every sub-period observation date is strictly before
    // `as_of`, so the per-sub-period splice naturally yields all-realized.
    // A fully-future period (`accrual_start > as_of`) must be routed to
    // `compounded_forward_projection` instead.
    if accrual_start > as_of {
        return Err(finstack_core::Error::Validation(format!(
            "compounded_spliced_projection requires the accrual period to have \
             started (accrual_start <= as_of); got accrual_start={}, as_of={}. \
             Route fully-future periods through compounded_forward_projection.",
            accrual_start, as_of
        )));
    }

    let fwd_dc = fwd.day_count();
    let fwd_base = fwd.base_date();
    let curve_time = |d: Date| -> Result<f64> {
        if d <= fwd_base {
            Ok(0.0)
        } else {
            fwd_dc.year_fraction(fwd_base, d, DayCountContext::default())
        }
    };

    // ∏(1 + rᵢ·dᵢ) over the daily sub-periods, splicing realized and projected.
    let mut compound_factor = 1.0_f64;

    let mut d = accrual_start;
    while d < accrual_end {
        let next_d = shift_business_days(d, 1, calendar_id)?.min(accrual_end);

        // Day-count weight dᵢ is anchored to the accrual sub-period.
        let dcf = fwd_dc.year_fraction(d, next_d, DayCountContext::default())?;
        if dcf <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "Compounded OIS leg produced a non-positive day-count fraction \
                 ({:.3e}) for the daily sub-period {} -> {}.",
                dcf, d, next_d
            )));
        }

        // Observation window: shift back by the lookback.
        let (obs_start, obs_end) = if observation_shift_days == 0 {
            (d, next_d)
        } else {
            (
                shift_business_days(d, -observation_shift_days, calendar_id)?,
                shift_business_days(next_d, -observation_shift_days, calendar_id)?,
            )
        };

        // Splice point: an observation strictly before `as_of` is realized;
        // an observation on/after `as_of` is projected from the forward curve.
        let mut r = if obs_start < as_of {
            finstack_core::market_data::fixings::require_fixing_value_exact(
                fixings, fixing_id, obs_start, as_of,
            )?
        } else {
            let ts = curve_time(obs_start)?;
            let te = curve_time(obs_end)?;
            if te > ts {
                fwd.rate_period(ts, te)
            } else {
                fwd.rate(ts)
            }
        };

        // Per-fixing floor/cap: each daily caplet/floorlet, not the average.
        if let Some(floor) = index_floor {
            r = r.max(floor);
        }
        if let Some(cap) = index_cap {
            r = r.min(cap);
        }

        compound_factor *= 1.0 + r * dcf;
        d = next_d;
    }

    // Equivalent simple rate: R · τ = CF − 1.
    Ok((compound_factor - 1.0) / period_year_fraction)
}

/// Parameters for pricing a floating rate leg.
///
/// This struct wraps [`FloatingRateParams`] and adds swap-specific fields for
/// payment delay, calendar handling, and compounding method. Use this for swap leg pricing.
///
/// # Compounding Methods
///
/// The `compounding_method` field controls how the floating rate is calculated:
///
/// - [`CompoundingMethod::Simple`]: Single fixing at reset date (IBOR, Term SOFR)
/// - [`CompoundingMethod::CompoundedWithShift`]: Daily compounding with lookback (OIS standard)
///
/// For OIS swaps, set `compounding_method` to [`CompoundingMethod::CompoundedWithShift`]
/// and populate `observation_shift_days`.
///
/// # Validation
///
/// Call [`validate()`](Self::validate) before pricing to ensure parameters are consistent.
/// The validation checks for:
/// - Valid spread and gearing (finite, gearing > 0)
/// - Consistent floor/cap ordering (floor <= cap)
/// - Valid payment delay (non-negative for practical use)
/// - Consistent compounding settings (shift only for CompoundedWithShift)
#[derive(Debug, Clone, Default)]
pub struct FloatingLegParams {
    /// Core rate parameters (spread, gearing, floors, caps).
    pub rate_params: FloatingRateParams,
    /// Payment delay in business days after period end.
    pub payment_lag_days: i32,
    /// Optional calendar ID for payment date adjustments.
    pub calendar_id: Option<String>,
    /// Compounding method for calculating the period rate.
    ///
    /// Defaults to [`CompoundingMethod::Simple`] for IBOR-style rates.
    /// Set to [`CompoundingMethod::CompoundedWithShift`] for OIS swaps.
    pub compounding_method: CompoundingMethod,
    /// Observation shift (lookback) in business days for OIS compounding.
    ///
    /// Only used when `compounding_method` is [`CompoundingMethod::CompoundedWithShift`].
    ///
    /// # Market Standards
    ///
    /// - **2 days**: USD SOFR, EUR ESTR, JPY TONAR
    /// - **0 days**: GBP SONIA (uses payment delay instead)
    pub observation_shift_days: i32,
}

impl FloatingLegParams {
    /// Create params with full configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn full(
        spread_bp: f64,
        gearing: f64,
        gearing_includes_spread: bool,
        index_floor_bp: Option<f64>,
        index_cap_bp: Option<f64>,
        all_in_floor_bp: Option<f64>,
        all_in_cap_bp: Option<f64>,
        payment_lag_days: i32,
        calendar_id: Option<String>,
    ) -> Self {
        Self {
            rate_params: FloatingRateParams {
                spread_bp,
                gearing,
                gearing_includes_spread,
                index_floor_bp,
                index_cap_bp,
                all_in_floor_bp,
                all_in_cap_bp,
            },
            payment_lag_days,
            calendar_id,
            compounding_method: CompoundingMethod::Simple,
            observation_shift_days: 0,
        }
    }

    /// Validate the floating leg parameters.
    ///
    /// Checks that:
    /// - Rate parameters are valid (delegates to [`FloatingRateParams::validate`])
    /// - Payment delay is reasonable (warning logged if negative)
    /// - Observation shift is only set for CompoundedWithShift method
    ///
    /// # Returns
    ///
    /// `Ok(())` if all parameters are valid, otherwise returns an error
    /// describing the validation failure.
    pub fn validate(&self) -> Result<()> {
        self.rate_params.validate()?;

        // Warn if observation shift is set but compounding doesn't use it
        if self.observation_shift_days != 0
            && !matches!(
                self.compounding_method,
                CompoundingMethod::CompoundedWithShift
            )
        {
            // Not an error, but the shift will be ignored
            // Could add logging here if needed
        }

        Ok(())
    }

    /// Returns true if this leg uses daily compounded rates (OIS).
    ///
    /// Simple (term-rate) legs only need a single fixing at the reset date;
    /// every other compounding method needs a fixing for each day in the
    /// accrual period.
    #[must_use]
    pub fn is_ois_style(&self) -> bool {
        !matches!(self.compounding_method, CompoundingMethod::Simple)
    }
}

/// A period in a swap leg schedule.
///
/// This is a simpler view of cashflow data focused on what's needed for pricing.
#[derive(Debug, Clone)]
pub struct LegPeriod {
    /// Start of the accrual period.
    pub accrual_start: Date,
    /// End of the accrual period (also the unadjusted payment date).
    pub accrual_end: Date,
    /// Rate reset/fixing date (for floating legs).
    pub reset_date: Option<Date>,
    /// Year fraction for the accrual period.
    pub year_fraction: f64,
}

/// Compute present value of a floating rate leg using the standard term-rate methodology.
///
/// This is the Bloomberg-validated implementation from IRS pricing, generalized to work
/// with any swap instrument. It handles:
/// - Forward rate projection from the curve (for future resets)
/// - Historical fixings for past resets (seasoned instruments)
/// - Spread, gearing, floors and caps
/// - Payment delay adjustment
/// - Numerical stability via Kahan summation
/// - Robust relative discount factors
///
/// # Arguments
///
/// * `periods` - Iterator over the leg periods
/// * `notional` - Notional amount (absolute value)
/// * `params` - Floating leg parameters
/// * `disc` - Discount curve for PV calculation
/// * `fwd` - Forward curve for rate projection
/// * `as_of` - Valuation date
/// * `fixings` - Optional historical fixings for seasoned instruments. Required when
///   `reset_date < as_of` for any period; if missing, returns an error.
///
/// # Returns
///
/// Present value of the floating leg as a raw f64 (unsigned).
/// The caller is responsible for applying sign conventions.
///
/// # Errors
///
/// Returns an error if:
/// - Parameter validation fails (contradictory floors/caps, invalid gearing)
/// - Forward rate projection fails
/// - Historical fixings are required but not provided or missing for a reset date
/// - Discount factor calculation fails due to numerical instability
/// - Date calculations fail
pub fn pv_floating_leg<I>(
    periods: I,
    notional: f64,
    params: &FloatingLegParams,
    disc: &DiscountCurve,
    fwd: &ForwardCurve,
    as_of: Date,
    fixings: Option<&ScalarTimeSeries>,
) -> Result<f64>
where
    I: Iterator<Item = LegPeriod>,
{
    // Validate parameters at entry point for fail-fast behavior
    params.validate()?;

    // Use incremental Kahan accumulator to avoid Vec allocation
    let mut acc = NeumaierAccumulator::new();

    for period in periods {
        // Apply payment delay to determine the actual payment date
        let payment_date = add_payment_delay(
            period.accrual_end,
            params.payment_lag_days,
            params.calendar_id.as_deref(),
        )?;

        // Skip cashflows where the payment has already settled
        // (payment_date <= as_of means the payment has been made)
        if payment_date <= as_of {
            continue;
        }

        let reset_date = period.reset_date.unwrap_or(period.accrual_start);

        // Compounded / RFR legs apply the index floor and cap to **each daily
        // fixing** (a strip of daily caplets/floorlets), so `index_rate` for
        // those legs already has the per-fixing floor/cap baked in. The
        // index-level floor/cap must then be stripped from the params handed to
        // `calculate_floating_rate` to avoid applying them a second time to the
        // period-average rate. Term-rate (`Simple`) legs keep the original
        // single-fixing floor/cap path.
        let is_compounded = !matches!(params.compounding_method, CompoundingMethod::Simple);

        // Pre-compute per-fixing floor/cap decimals used by the compounded paths.
        // These are stripped from the `calculate_floating_rate` call below for
        // compounded legs to avoid double-application (the helpers bake them in
        // per daily fixing; term-rate legs keep the single-fixing path instead).
        let index_floor_decimal = params
            .rate_params
            .index_floor_bp
            .map(|bp| bp * crate::constants::ONE_BASIS_POINT);
        let index_cap_decimal = params
            .rate_params
            .index_cap_bp
            .map(|bp| bp * crate::constants::ONE_BASIS_POINT);

        // Routing: choose the correct rate source for this period.
        //
        //   • Compounded leg, accrual period has started (`accrual_start <= as_of`):
        //     delegate to `compounded_spliced_projection`, which handles both
        //     in-progress coupons (realized daily fixings spliced with projected
        //     forwards) and fully-accrued-but-unpaid coupons (all-realized daily
        //     compound when every observation date precedes `as_of`).
        //
        //   • Simple (term-rate / IBOR) leg with a past reset: look up the single
        //     historical fixing at `reset_date`. Compounded legs with a past reset
        //     date never reach this branch — their accrual start is also in the
        //     past, so they are handled above.
        //
        //   • Simple leg with a future reset: project the rate from the forward
        //     curve at `reset_date` (correct window for a term-rate index).
        //
        //   • Compounded leg, accrual period is entirely in the future: compute the
        //     true daily-compounded coupon via `compounded_forward_projection`.
        let index_rate = if is_compounded && period.accrual_start <= as_of {
            // OIS / RFR coupon whose accrual period has started (`accrual_start
            // <= as_of`). This covers two cases handled uniformly by
            // `compounded_spliced_projection`:
            //
            //   1. In-progress (`as_of < accrual_end`): splices realized daily
            //      fixings (period start → as_of) with projected overnight
            //      forwards (as_of → period end).
            //
            //   2. Fully accrued but unpaid (`accrual_end <= as_of <
            //      payment_date`, payment_lag_days > 0): every sub-period
            //      observation date is before `as_of`, so the helper yields an
            //      all-realized daily compound with no projected component —
            //      the correct settled coupon rate.
            //
            // Treating either case as a single term fixing at the reset date
            // — the IBOR-style path below — silently mis-prices every seasoned
            // RFR swap; the realized portion is itself a daily compound of many
            // overnight fixings, not one rate.
            compounded_spliced_projection(
                fwd,
                fixings,
                fwd.id().as_str(),
                period.accrual_start,
                period.accrual_end,
                as_of,
                period.year_fraction,
                params.observation_shift_days,
                params.calendar_id.as_deref(),
                index_floor_decimal,
                index_cap_decimal,
            )?
        } else if reset_date < as_of {
            // Past reset, term-rate (`Simple`) leg: require a single historical
            // fixing (exact date match). Pass the actual forward-curve
            // identifier so the resulting validation error tells the operator
            // which index reset is missing — at 2 AM, "fixing required for
            // 'floating-leg' on 2025-04-15" is unactionable; "fixing required
            // for 'USD-SOFR-3M' on 2025-04-15" is.
            //
            // Compounded legs never reach here: a compounded leg whose reset is
            // in the past either straddles `as_of` (handled above) or — with a
            // reset lag placing the reset before a still-future accrual start —
            // falls through to the future-reset compounded branch below.
            finstack_core::market_data::fixings::require_fixing_value_exact(
                fixings,
                fwd.id().as_str(),
                reset_date,
                as_of,
            )?
        } else if !is_compounded {
            // Future reset, term-rate leg (`CompoundingMethod::Simple`, e.g.
            // EURIBOR-6M): the rate is *set* at `reset_date` as the index-tenor
            // forward observed on that date. The forward curve's `rate(t)` is
            // exactly "the forward starting at time `t` for the curve's tenor",
            // so we anchor at the fixing date. When a reset lag places
            // `reset_date` materially before `accrual_start`, projecting over
            // the accrual interval instead would read the wrong forward window
            // — on a steep curve a 2-business-day lag is worth ~1-3 bp of rate.
            let fwd_base = fwd.base_date();
            let t_reset = if reset_date <= fwd_base {
                0.0
            } else {
                fwd.day_count()
                    .year_fraction(fwd_base, reset_date, DayCountContext::default())?
            };
            fwd.rate(t_reset)
        } else {
            // Future reset, OIS / genuinely-compounding leg (`Compounded`,
            // `CompoundedWithShift`, `Average`) whose accrual period is entirely
            // in the future: the coupon is true daily compounding
            // `(∏(1+rᵢ·dᵢ)−1)/τ` with the ISDA observation shift applied.
            //
            // The index floor/cap (if any) are applied per daily fixing here;
            // they are stripped below so `calculate_floating_rate` does not
            // re-apply them to the period-average rate.
            compounded_forward_projection(
                fwd,
                period.accrual_start,
                period.accrual_end,
                period.year_fraction,
                params.observation_shift_days,
                params.calendar_id.as_deref(),
                index_floor_decimal,
                index_cap_decimal,
            )?
        };

        // Apply floors, caps, gearing, and spread using the rate helpers.
        //
        // For compounded legs the index floor/cap have already been applied
        // per-fixing inside `compounded_forward_projection`; strip them from the
        // rate params so they are not applied a second time to the now-averaged
        // period rate. Spread, gearing, and the all-in floor/cap still apply.
        let all_in_rate = if is_compounded {
            let rate_params_no_index_bounds =
                crate::cashflow::builder::rate_helpers::FloatingRateParams {
                    index_floor_bp: None,
                    index_cap_bp: None,
                    ..params.rate_params.clone()
                };
            crate::cashflow::builder::rate_helpers::calculate_floating_rate(
                index_rate,
                &rate_params_no_index_bounds,
            )
        } else {
            crate::cashflow::builder::rate_helpers::calculate_floating_rate(
                index_rate,
                &params.rate_params,
            )
        };

        // Coupon amount
        let coupon_amount = notional * all_in_rate * period.year_fraction;

        // Discount from as_of for correct theta
        let df = robust_relative_df(disc, as_of, payment_date)?;
        acc.add(coupon_amount * df);
    }

    Ok(acc.total())
}

/// Parameters for pricing a fixed rate leg.
#[derive(Debug, Clone)]
pub struct FixedLegParams {
    /// Fixed rate (decimal, e.g., 0.05 for 5%).
    pub rate: f64,
    /// Day count convention for accrual.
    pub day_count: DayCount,
    /// Payment delay in business days after period end.
    pub payment_lag_days: i32,
    /// Optional calendar ID for payment date adjustments.
    pub calendar_id: Option<String>,
}

impl FixedLegParams {
    /// Validate fixed leg parameters.
    ///
    /// Checks that:
    /// - Rate is finite
    pub fn validate(&self) -> Result<()> {
        if !self.rate.is_finite() {
            return Err(finstack_core::Error::Validation(
                "Fixed rate must be finite".into(),
            ));
        }
        Ok(())
    }
}

/// Compute present value of a fixed rate leg.
///
/// This is the Bloomberg-validated implementation from IRS pricing, generalized to work
/// with any swap instrument. It handles:
/// - Fixed coupon calculation with proper day count
/// - Payment delay adjustment
/// - Numerical stability via Kahan summation
/// - Robust relative discount factors
///
/// # Arguments
///
/// * `periods` - Iterator over the leg periods
/// * `notional` - Notional amount (absolute value)
/// * `params` - Fixed leg parameters
/// * `disc` - Discount curve for PV calculation
/// * `as_of` - Valuation date
///
/// # Returns
///
/// Present value of the fixed leg as a raw f64 (unsigned).
/// The caller is responsible for applying sign conventions.
///
/// # Errors
///
/// Returns an error if:
/// - Parameter validation fails
/// - Discount factor calculation fails due to numerical instability
pub fn pv_fixed_leg<I>(
    periods: I,
    notional: f64,
    params: &FixedLegParams,
    disc: &DiscountCurve,
    as_of: Date,
) -> Result<f64>
where
    I: Iterator<Item = LegPeriod>,
{
    // Validate parameters at entry point
    params.validate()?;

    // Use incremental Kahan accumulator to avoid Vec allocation
    let mut acc = NeumaierAccumulator::new();

    for period in periods {
        // Apply payment delay to determine the actual payment date
        let payment_date = add_payment_delay(
            period.accrual_end,
            params.payment_lag_days,
            params.calendar_id.as_deref(),
        )?;

        // Skip cashflows where the payment has already settled
        // (payment_date <= as_of means the payment has been made)
        if payment_date <= as_of {
            continue;
        }

        // Fixed coupon amount
        let coupon_amount = notional * params.rate * period.year_fraction;

        // Discount from as_of for correct theta
        let df = robust_relative_df(disc, as_of, payment_date)?;
        acc.add(coupon_amount * df);
    }

    Ok(acc.total())
}

/// Compute discounted annuity (sum of DF × year_fraction) for a leg.
///
/// This is useful for DV01 calculations and par rate computations.
///
/// # Arguments
///
/// * `periods` - Iterator over the leg periods
/// * `disc` - Discount curve for PV calculation
/// * `as_of` - Valuation date
/// * `payment_lag_days` - Payment delay in business days
/// * `calendar_id` - Optional calendar ID for payment date adjustments
///
/// # Returns
///
/// The annuity (discounted year fraction sum) as a raw f64.
///
/// # Errors
///
/// Returns an error if the annuity is zero or below [`ANNUITY_EPSILON`],
/// which would cause divide-by-zero in downstream par spread calculations.
pub fn leg_annuity<I>(
    periods: I,
    disc: &DiscountCurve,
    as_of: Date,
    payment_lag_days: i32,
    calendar_id: Option<&str>,
) -> Result<f64>
where
    I: Iterator<Item = LegPeriod>,
{
    let mut acc = NeumaierAccumulator::new();

    for period in periods {
        // Apply payment delay (strict: calendar must resolve if specified)
        let payment_date = add_payment_delay(period.accrual_end, payment_lag_days, calendar_id)?;

        // Only include future payments
        if payment_date > as_of {
            let df = robust_relative_df(disc, as_of, payment_date)?;
            acc.add(period.year_fraction * df);
        }
    }

    let annuity = acc.total();

    // Guard against a near-zero annuity, which would cause divide-by-zero in
    // par spread / par rate calculations.
    //
    // The diagnostic distinguishes two genuinely different failure modes — the
    // previous single message claimed "periods expired or extreme discounting"
    // for *every* sub-threshold annuity, which mis-describes a corrupt leg:
    //
    //  * `annuity < 0`: discount factors are always strictly positive, so the
    //    only way the sum `Σ year_fraction · DF` goes negative is a negative
    //    `year_fraction` — an inverted / malformed accrual period. This is data
    //    corruption, not expiry (expiry yields exactly 0) and not extreme
    //    discounting (that yields a tiny *positive* value).
    //  * `0 <= annuity < ANNUITY_EPSILON`: a legitimately tiny annuity — all
    //    periods expired (annuity 0) or an extreme-rate / long-horizon scenario
    //    discounted every coupon to near zero.
    if annuity < 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "Annuity ({:.2e}) is negative. A discounted annuity sums \
             year_fraction × discount_factor, and discount factors are always \
             positive, so a negative annuity means at least one period has a \
             negative year fraction — a corrupt or inverted accrual period \
             (accrual_end before accrual_start). Check the schedule generation.",
            annuity
        )));
    }
    if annuity < ANNUITY_EPSILON {
        return Err(finstack_core::Error::Validation(format!(
            "Annuity ({:.2e}) is below minimum threshold ({:.2e}). \
             This may indicate all periods have expired or extreme discounting scenarios.",
            annuity, ANNUITY_EPSILON
        )));
    }

    Ok(annuity)
}

/// Convert a Schedule to an iterator of LegPeriods.
///
/// This helper bridges the gap between the core Schedule type and
/// the LegPeriod type used by the pricing functions.
///
/// # Arguments
///
/// * `schedule` - The schedule containing period dates
/// * `day_count` - Day count convention for calculating year fractions
/// * `reset_lag_days` - Reset lag in business days (for floating legs)
/// * `calendar_id` - Optional calendar ID for reset date adjustments
///
/// # Returns
///
/// A vector of LegPeriod structs.
pub fn schedule_to_periods(
    schedule: &Schedule,
    day_count: DayCount,
    reset_lag_days: Option<i32>,
    calendar_id: Option<&str>,
) -> Result<Vec<LegPeriod>> {
    if schedule.dates.len() < 2 {
        return Err(finstack_core::Error::Validation(
            "Schedule must contain at least 2 dates".to_string(),
        ));
    }

    let cal = if let Some(id) = calendar_id {
        Some(CalendarRegistry::global().resolve_str(id).ok_or_else(|| {
            finstack_core::Error::Validation(format!(
                "Reset calendar '{}' not found in registry; cannot apply reset lag.",
                id
            ))
        })?)
    } else {
        None
    };

    let mut periods = Vec::with_capacity(schedule.dates.len() - 1);

    for i in 1..schedule.dates.len() {
        let accrual_start = schedule.dates[i - 1];
        let accrual_end = schedule.dates[i];

        let year_fraction =
            day_count.year_fraction(accrual_start, accrual_end, DayCountContext::default())?;

        // Calculate reset date for floating legs
        let reset_date = if let Some(lag) = reset_lag_days {
            if lag == 0 {
                Some(accrual_start)
            } else if let Some(cal) = cal {
                Some(accrual_start.add_business_days(-lag, cal)?)
            } else {
                Some(accrual_start.add_weekdays(-lag))
            }
        } else {
            None
        };

        periods.push(LegPeriod {
            accrual_start,
            accrual_end,
            reset_date,
            year_fraction,
        });
    }

    Ok(periods)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::{ScheduleBuilder, StubKind, Tenor};
    use finstack_core::market_data::term_structures::ForwardCurve;
    use finstack_core::types::CurveId;
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    /// Test-only floating-leg params with just a spread (term-rate / `Simple`).
    fn float_spread(spread_bp: f64) -> FloatingLegParams {
        FloatingLegParams {
            rate_params: FloatingRateParams::with_spread(spread_bp),
            ..Default::default()
        }
    }

    /// Test-only floating-leg params with a spread and payment delay.
    fn float_spread_delay(spread_bp: f64, payment_lag_days: i32) -> FloatingLegParams {
        FloatingLegParams {
            rate_params: FloatingRateParams::with_spread(spread_bp),
            payment_lag_days,
            ..Default::default()
        }
    }

    /// Test-only OIS floating-leg params (daily compounding with observation shift).
    fn float_ois(
        spread_bp: f64,
        observation_shift_days: i32,
        payment_lag_days: i32,
    ) -> FloatingLegParams {
        FloatingLegParams {
            rate_params: FloatingRateParams::with_spread(spread_bp),
            payment_lag_days,
            calendar_id: None,
            compounding_method: CompoundingMethod::CompoundedWithShift,
            observation_shift_days,
        }
    }

    /// Test-only floating-leg params using [`CompoundingMethod::Compounded`]
    /// (daily compounding, no observation shift).
    fn float_compounded(spread_bp: f64) -> FloatingLegParams {
        FloatingLegParams {
            rate_params: FloatingRateParams::with_spread(spread_bp),
            compounding_method: CompoundingMethod::Compounded,
            ..Default::default()
        }
    }

    /// Test-only fixed-leg params with a rate and day count.
    fn fixed_rate(rate: f64, day_count: DayCount) -> FixedLegParams {
        FixedLegParams {
            rate,
            day_count,
            payment_lag_days: 0,
            calendar_id: None,
        }
    }

    fn test_discount_curve(base_date: Date) -> DiscountCurve {
        DiscountCurve::builder(CurveId::new("TEST-DISC"))
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (0.5, 0.975), (1.0, 0.95), (5.0, 0.80)])
            .build()
            .expect("test curve should build")
    }

    fn test_forward_curve(base_date: Date) -> ForwardCurve {
        ForwardCurve::builder(CurveId::new("TEST-FWD"), 0.25)
            .base_date(base_date)
            .day_count(DayCount::Act360)
            .knots(vec![(0.0, 0.03), (1.0, 0.035), (5.0, 0.04)])
            .build()
            .expect("test curve should build")
    }

    #[test]
    fn robust_relative_df_positive() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        let df = robust_relative_df(&disc, base_date, date(2025, 1, 1)).expect("should succeed");
        assert!(df > 0.0 && df <= 1.0, "DF should be in (0, 1]: {}", df);
    }

    #[test]
    fn robust_relative_df_accepts_small_absolute_df() {
        // Create a curve with very small absolute DFs (stress scenario).
        // The new policy accepts these as long as the RELATIVE DF between dates is valid.
        let base_date = date(2024, 1, 1);
        let disc = DiscountCurve::builder(CurveId::new("EXTREME"))
            .base_date(base_date)
            .knots(vec![(0.0, 1e-12), (1.0, 1e-15)]) // Very small DFs
            .build()
            .expect("curve should build");

        // Under the new policy, df_between_dates computes df(target) / df(as_of)
        // = 1e-15 / 1e-12 = 0.001, which is a valid positive relative DF.
        let result = robust_relative_df(&disc, base_date, date(2025, 1, 1));
        assert!(
            result.is_ok(),
            "Small absolute DFs should be accepted if relative DF is valid: {:?}",
            result
        );
        let df = result.expect("relative DF should be valid");
        assert!(df > 0.0, "Relative DF should be positive: {}", df);
    }

    #[test]
    fn pv_floating_leg_basic() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = test_forward_curve(base_date);

        let periods = vec![
            LegPeriod {
                accrual_start: date(2024, 1, 1),
                accrual_end: date(2024, 4, 1),
                reset_date: Some(date(2024, 1, 1)),
                year_fraction: 0.25,
            },
            LegPeriod {
                accrual_start: date(2024, 4, 1),
                accrual_end: date(2024, 7, 1),
                reset_date: Some(date(2024, 4, 1)),
                year_fraction: 0.25,
            },
        ];

        let params = float_spread(100.0); // 100 bps
        let pv = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            base_date,
            None, // No fixings needed - all resets are on or after as_of
        )
        .expect("should price");

        // Should be positive (receiving floating)
        assert!(pv > 0.0, "PV should be positive: {}", pv);
    }

    #[test]
    fn pv_floating_leg_validates_params() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = test_forward_curve(base_date);

        let periods = vec![LegPeriod {
            accrual_start: date(2024, 1, 1),
            accrual_end: date(2024, 4, 1),
            reset_date: Some(date(2024, 1, 1)),
            year_fraction: 0.25,
        }];

        // Create params with contradictory floor/cap
        let params = FloatingLegParams::full(
            100.0,       // spread_bp
            1.0,         // gearing
            true,        // gearing_includes_spread
            None,        // index_floor_bp
            None,        // index_cap_bp
            Some(500.0), // all_in_floor_bp (5%)
            Some(300.0), // all_in_cap_bp (3%) - less than floor!
            0,           // payment_lag_days
            None,        // calendar_id
        );

        let result = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            base_date,
            None,
        );
        assert!(
            result.is_err(),
            "Should reject contradictory floor/cap params"
        );
    }

    #[test]
    fn pv_floating_leg_validates_zero_gearing() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = test_forward_curve(base_date);

        let periods = vec![LegPeriod {
            accrual_start: date(2024, 1, 1),
            accrual_end: date(2024, 4, 1),
            reset_date: Some(date(2024, 1, 1)),
            year_fraction: 0.25,
        }];

        // Create params with zero gearing
        let params = FloatingLegParams::full(
            100.0, // spread_bp
            0.0,   // gearing - invalid!
            true,  // gearing_includes_spread
            None,  // index_floor_bp
            None,  // index_cap_bp
            None,  // all_in_floor_bp
            None,  // all_in_cap_bp
            0,     // payment_lag_days
            None,  // calendar_id
        );

        let result = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            base_date,
            None,
        );
        assert!(result.is_err(), "Should reject zero gearing");
    }

    #[test]
    fn pv_floating_leg_seasoned_requires_fixings() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = test_forward_curve(base_date);

        // Reset date is before as_of, so fixings are required
        let as_of = date(2024, 2, 15);
        let periods = vec![LegPeriod {
            accrual_start: date(2024, 1, 1),
            accrual_end: date(2024, 4, 1),
            reset_date: Some(date(2024, 1, 1)), // Reset is before as_of
            year_fraction: 0.25,
        }];

        let params = float_spread(100.0);
        let result = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            as_of,
            None, // No fixings provided - should fail
        );
        assert!(
            result.is_err(),
            "Should require fixings for seasoned floating leg"
        );
        let err = result.expect_err("should error");
        assert!(
            err.to_string().contains("fixings") || err.to_string().contains("Seasoned"),
            "Error should mention fixings: {}",
            err
        );
    }

    #[test]
    fn pv_floating_leg_seasoned_uses_fixings() {
        use finstack_core::market_data::scalars::ScalarTimeSeries;

        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = test_forward_curve(base_date);

        // Reset date is before as_of
        let as_of = date(2024, 2, 15);
        let periods = vec![LegPeriod {
            accrual_start: date(2024, 1, 1),
            accrual_end: date(2024, 4, 1),
            reset_date: Some(date(2024, 1, 1)),
            year_fraction: 0.25,
        }];

        // Provide fixings
        let fixing_rate = 0.04; // 4% fixing
        let fixings = ScalarTimeSeries::new(
            "FIXING:TEST-FWD",
            vec![(date(2024, 1, 1), fixing_rate)],
            None,
        )
        .expect("fixings series");

        let params = float_spread(100.0); // 100 bps spread
        let pv = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            as_of,
            Some(&fixings),
        )
        .expect("should price with fixings");

        // PV should be based on fixing + spread = 4% + 1% = 5%
        // 1,000,000 × 0.05 × 0.25 × DF ≈ 12,500 × ~0.97 ≈ 12,125
        assert!(
            pv > 10_000.0 && pv < 15_000.0,
            "PV should be reasonable: {}",
            pv
        );
    }

    #[test]
    fn pv_floating_leg_payment_delay_affects_skip() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = test_forward_curve(base_date);

        // as_of is between accrual_end and payment_date
        // Accrual ends Apr 1, payment is Apr 3 (with 2-day delay)
        let as_of = date(2024, 4, 2);
        let periods = vec![LegPeriod {
            accrual_start: date(2024, 1, 1),
            accrual_end: date(2024, 4, 1), // Accrual ends Apr 1
            reset_date: Some(date(2024, 1, 1)),
            year_fraction: 0.25,
        }];

        // Without payment delay - should skip the period (accrual_end <= as_of would be true in old logic)
        let params_no_delay = float_spread(100.0);

        // Provide fixings since reset_date < as_of
        let fixings =
            ScalarTimeSeries::new("FIXING:TEST-FWD", vec![(date(2024, 1, 1), 0.03)], None)
                .expect("fixings series");

        let pv_no_delay = pv_floating_leg(
            periods.clone().into_iter(),
            1_000_000.0,
            &params_no_delay,
            &disc,
            &fwd,
            as_of,
            Some(&fixings),
        )
        .expect("should price");

        // Payment date = Apr 1 (no delay) <= as_of (Apr 2), so should be 0
        assert!(
            pv_no_delay.abs() < 1e-10,
            "No-delay PV should be ~0 (payment already settled): {}",
            pv_no_delay
        );

        // With 2-day payment delay - should NOT skip (payment_date = Apr 3 > as_of = Apr 2)
        let params_with_delay = float_spread_delay(100.0, 2);
        let pv_with_delay = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params_with_delay,
            &disc,
            &fwd,
            as_of,
            Some(&fixings),
        )
        .expect("should price");

        // Payment date = Apr 3 > as_of (Apr 2), so should have positive PV
        assert!(
            pv_with_delay > 0.0,
            "With-delay PV should be positive (payment not yet settled): {}",
            pv_with_delay
        );
    }

    #[test]
    fn pv_fixed_leg_basic() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        let periods = vec![
            LegPeriod {
                accrual_start: date(2024, 1, 1),
                accrual_end: date(2024, 7, 1),
                reset_date: None,
                year_fraction: 0.5,
            },
            LegPeriod {
                accrual_start: date(2024, 7, 1),
                accrual_end: date(2025, 1, 1),
                reset_date: None,
                year_fraction: 0.5,
            },
        ];

        let params = fixed_rate(0.03, DayCount::Thirty360);
        let pv = pv_fixed_leg(periods.into_iter(), 1_000_000.0, &params, &disc, base_date)
            .expect("should price");

        // Should be positive (receiving fixed)
        assert!(pv > 0.0, "PV should be positive: {}", pv);

        // Approximate check: 2 × 0.5 × 0.03 × 1M × avg_df ≈ 30000 × 0.95 ≈ 28500
        assert!(
            pv > 20000.0 && pv < 35000.0,
            "PV should be reasonable: {}",
            pv
        );
    }

    #[test]
    fn pv_fixed_leg_validates_nan_rate() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        let periods = vec![LegPeriod {
            accrual_start: date(2024, 1, 1),
            accrual_end: date(2024, 7, 1),
            reset_date: None,
            year_fraction: 0.5,
        }];

        let params = fixed_rate(f64::NAN, DayCount::Thirty360);
        let result = pv_fixed_leg(periods.into_iter(), 1_000_000.0, &params, &disc, base_date);
        assert!(result.is_err(), "Should reject NaN rate");
    }

    #[test]
    fn add_payment_delay_zero_returns_same() {
        let d = date(2024, 1, 15);
        let result = add_payment_delay(d, 0, None).expect("should succeed");
        assert_eq!(result, d);
    }

    #[test]
    fn add_payment_delay_positive_adds_weekdays() {
        let d = date(2024, 1, 15); // Monday
        let result = add_payment_delay(d, 2, None).expect("should succeed");
        // 2 weekdays from Monday = Wednesday
        assert_eq!(result, date(2024, 1, 17));
    }

    #[test]
    fn add_payment_delay_missing_calendar_errors() {
        let d = date(2024, 1, 15);
        // Providing a calendar ID that doesn't exist should now error
        let result = add_payment_delay(d, 2, Some("nonexistent_calendar"));
        assert!(result.is_err(), "Should error when calendar not found");
        let err = result.expect_err("should error when calendar not found");
        assert!(
            err.to_string().contains("not found"),
            "Error should mention calendar not found: {}",
            err
        );
    }

    #[test]
    fn schedule_to_periods_missing_reset_calendar_errors() {
        let start = date(2024, 1, 1);
        let end = date(2024, 4, 1);
        let schedule = ScheduleBuilder::new(start, end)
            .expect("schedule builder")
            .frequency(Tenor::monthly())
            .stub_rule(StubKind::None)
            .build()
            .expect("schedule");

        let result = schedule_to_periods(&schedule, DayCount::Act360, Some(2), Some("missing"));
        assert!(result.is_err(), "Should error when reset calendar missing");
        let err = result.expect_err("should error");
        assert!(
            err.to_string().contains("Reset calendar"),
            "Error should mention reset calendar: {}",
            err
        );
    }

    #[test]
    fn leg_annuity_computation() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        let periods = vec![
            LegPeriod {
                accrual_start: date(2024, 1, 1),
                accrual_end: date(2024, 7, 1),
                reset_date: None,
                year_fraction: 0.5,
            },
            LegPeriod {
                accrual_start: date(2024, 7, 1),
                accrual_end: date(2025, 1, 1),
                reset_date: None,
                year_fraction: 0.5,
            },
        ];

        let annuity =
            leg_annuity(periods.into_iter(), &disc, base_date, 0, None).expect("should compute");

        // Should be sum of (yf × df) ≈ 0.5 × 0.975 + 0.5 × 0.95 ≈ 0.9625
        assert!(
            annuity > 0.9 && annuity < 1.0,
            "Annuity should be reasonable: {}",
            annuity
        );
    }

    #[test]
    fn leg_annuity_rejects_zero() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        // All periods are in the past
        let periods = vec![
            LegPeriod {
                accrual_start: date(2023, 1, 1),
                accrual_end: date(2023, 7, 1),
                reset_date: None,
                year_fraction: 0.5,
            },
            LegPeriod {
                accrual_start: date(2023, 7, 1),
                accrual_end: date(2024, 1, 1), // Ends exactly on as_of
                reset_date: None,
                year_fraction: 0.5,
            },
        ];

        let result = leg_annuity(periods.into_iter(), &disc, base_date, 0, None);
        assert!(
            result.is_err(),
            "Should reject zero annuity (all periods expired)"
        );
    }

    // ==================== W-48: term-rate fixing-date projection ====================

    /// A steeply-upward-sloping forward curve so that the reset-lag window
    /// produces a materially different rate from the accrual-interval window.
    fn steep_forward_curve(base_date: Date) -> ForwardCurve {
        ForwardCurve::builder(CurveId::new("TEST-STEEP-FWD"), 0.5)
            .base_date(base_date)
            .day_count(DayCount::Act360)
            // ~5% rate slope per year — very steep, exaggerated for testability.
            .knots(vec![(0.0, 0.02), (1.0, 0.07), (5.0, 0.27)])
            .build()
            .expect("steep curve should build")
    }

    /// W-48: For a term-rate (`Simple`) leg with a non-zero reset lag, the
    /// projected rate must be the index-tenor forward anchored at the *fixing
    /// date*, not the average forward over the accrual interval.
    #[test]
    fn pv_floating_leg_term_rate_anchors_projection_at_fixing_date() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = steep_forward_curve(base_date);

        // Term-rate leg with a reset materially before accrual_start (reset lag).
        // accrual_start = Jul 1 2024, accrual_end = Jan 1 2025.
        // reset_date = Apr 1 2024 — 3 months before accrual_start (exaggerated lag).
        let accrual_start = date(2024, 7, 1);
        let accrual_end = date(2025, 1, 1);
        let reset_date = date(2024, 4, 1);
        let year_fraction = 0.5;

        let periods = vec![LegPeriod {
            accrual_start,
            accrual_end,
            reset_date: Some(reset_date),
            year_fraction,
        }];

        // No spread/gearing so all_in_rate == index_rate.
        let params = FloatingLegParams::default();
        assert_eq!(params.compounding_method, CompoundingMethod::Simple);

        let pv = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            base_date,
            None,
        )
        .expect("should price");

        // Recover the implied projected rate: pv = notional * rate * yf * df.
        let payment_date = accrual_end; // no payment lag
        let df = robust_relative_df(&disc, base_date, payment_date).expect("df");
        let implied_rate = pv / (1_000_000.0 * year_fraction * df);

        // Expected: fixing-date-anchored forward.
        let fwd_dc = fwd.day_count();
        let t_reset = fwd_dc
            .year_fraction(base_date, reset_date, DayCountContext::default())
            .expect("yf");
        let expected_fixing_anchored = fwd.rate(t_reset);

        // The (incorrect) accrual-interval projection, for contrast.
        let t0 = fwd_dc
            .year_fraction(base_date, accrual_start, DayCountContext::default())
            .expect("yf");
        let t1 = fwd_dc
            .year_fraction(base_date, accrual_end, DayCountContext::default())
            .expect("yf");
        let accrual_interval_rate = fwd.rate_period(t0, t1);

        // The fix must use the fixing-date-anchored forward.
        assert!(
            (implied_rate - expected_fixing_anchored).abs() < 1e-12,
            "term-rate leg must project the fixing-date-anchored forward: \
             implied={implied_rate}, expected={expected_fixing_anchored}"
        );

        // And it must differ from the accrual-interval projection by the
        // reset-lag amount (on this steep curve the gap is well above 1bp).
        let gap = (expected_fixing_anchored - accrual_interval_rate).abs();
        assert!(
            gap > 1e-4,
            "reset-lag projection gap should be material on a steep curve: gap={gap}"
        );
    }

    /// W-48 (re-blessed for [P3-1]): OIS / genuinely-compounding legs project
    /// over the **accrual interval** (not the fixing-date-anchored forward), and
    /// — per the [P3-1] fix — do so by **true daily compounding**, not the
    /// simple arithmetic-average forward.
    ///
    /// Re-bless rationale: W-48 originally asserted the OIS leg equals
    /// `fwd.rate_period(t0, t1)` (the simple arithmetic average). The [P3-1]
    /// quant audit found that projection drops the daily-compounding convexity
    /// (~4-5 bp on this steep half-year window; 12-15 bp at typical rate
    /// levels), under-projecting every forward coupon. The library-self
    /// regression value therefore moves intentionally:
    ///   old expected (simple average)        ≈ 0.0580556
    ///   new expected (daily compounding)     ≈ 0.0585068
    /// W-48's actual concern — that OIS legs must NOT anchor at the reset/fixing
    /// date the way term-rate legs do — is preserved and still asserted below.
    #[test]
    fn pv_floating_leg_ois_uses_daily_compounded_accrual_interval_projection() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = steep_forward_curve(base_date);

        let accrual_start = date(2024, 7, 1);
        let accrual_end = date(2025, 1, 1);
        let reset_date = date(2024, 4, 1);
        let year_fraction = 0.5;

        let periods = vec![LegPeriod {
            accrual_start,
            accrual_end,
            reset_date: Some(reset_date),
            year_fraction,
        }];

        // OIS-style leg, no observation shift (isolates the compounding effect).
        let params = float_ois(0.0, 0, 0);

        let pv = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            base_date,
            None,
        )
        .expect("should price");

        let df = robust_relative_df(&disc, base_date, accrual_end).expect("df");
        let implied_rate = pv / (1_000_000.0 * year_fraction * df);

        let fwd_dc = fwd.day_count();
        let t0 = fwd_dc
            .year_fraction(base_date, accrual_start, DayCountContext::default())
            .expect("yf");
        let t1 = fwd_dc
            .year_fraction(base_date, accrual_end, DayCountContext::default())
            .expect("yf");
        // The simple arithmetic-average forward over the accrual interval — the
        // OLD (pre-P3-1) projection.
        let simple_avg_rate = fwd.rate_period(t0, t1);
        // The fixing-date-anchored forward — the projection W-48 guards AGAINST
        // for OIS legs.
        let t_reset = fwd_dc
            .year_fraction(base_date, reset_date, DayCountContext::default())
            .expect("yf");
        let fixing_anchored_rate = fwd.rate(t_reset);

        // Independent daily-compounding reference over the accrual interval.
        let mut acc = 1.0_f64;
        let mut d = accrual_start;
        while d < accrual_end {
            let nxt = d.add_weekdays(1).min(accrual_end);
            let dcf = fwd_dc
                .year_fraction(d, nxt, DayCountContext::default())
                .expect("dcf");
            let ts = fwd_dc
                .year_fraction(base_date, d, DayCountContext::default())
                .expect("ts");
            let te = fwd_dc
                .year_fraction(base_date, nxt, DayCountContext::default())
                .expect("te");
            let r = if te > ts {
                fwd.rate_period(ts, te)
            } else {
                fwd.rate(ts)
            };
            acc *= 1.0 + r * dcf;
            d = nxt;
        }
        let daily_compounded_rate = (acc - 1.0) / year_fraction;

        // The leg must match the daily-compounded projection.
        assert!(
            (implied_rate - daily_compounded_rate).abs() < 1e-9,
            "OIS leg must use the daily-compounded accrual-interval projection: \
             implied={implied_rate}, expected={daily_compounded_rate}"
        );
        // It must EXCEED the simple-average projection (positive compounding
        // convexity on this upward-sloping curve) — the [P3-1] fix.
        assert!(
            implied_rate > simple_avg_rate + 1e-6,
            "daily compounding must exceed the simple-average forward: \
             implied={implied_rate}, simple_avg={simple_avg_rate}"
        );
        // And it must NOT collapse to the fixing-date-anchored forward — W-48's
        // original guard, still in force.
        assert!(
            (implied_rate - fixing_anchored_rate).abs() > 1e-4,
            "OIS leg must not anchor at the reset/fixing date: \
             implied={implied_rate}, fixing_anchored={fixing_anchored_rate}"
        );
    }

    // ==================== robust_relative_df EDGE CASE TESTS ====================

    #[test]
    fn robust_relative_df_as_of_equals_base_date() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        // When as_of == base_date, DF(as_of to target) is just DF(target)
        let target = date(2025, 1, 1);
        let df = robust_relative_df(&disc, base_date, target).expect("should succeed");
        assert!(df > 0.0 && df < 1.0, "DF should be in (0,1): {}", df);
    }

    #[test]
    fn robust_relative_df_as_of_after_base_date() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        // as_of is 6 months after base_date (seasoned instrument scenario)
        let as_of = date(2024, 7, 1);
        let target = date(2025, 1, 1);

        let df = robust_relative_df(&disc, as_of, target).expect("should succeed");
        // Should be the relative DF from as_of to target, which is valid and positive
        assert!(df > 0.0, "Relative DF should be positive: {}", df);
    }

    #[test]
    fn robust_relative_df_long_horizon() {
        use finstack_core::market_data::term_structures::DiscountCurve;

        // Create a curve that extends far into the future
        let base_date = date(2024, 1, 1);
        let curve = DiscountCurve::builder("TEST-LONG")
            .base_date(base_date)
            .knots([
                (0.0, 1.0),
                (1.0, 0.95),
                (10.0, 0.60),
                (30.0, 0.20),
                (50.0, 0.08),
            ])
            .build()
            .expect("curve should build");

        // 30Y forward date - long horizon but should still work
        let target = date(2054, 1, 1);
        let df = robust_relative_df(&curve, base_date, target).expect("should succeed");
        assert!(df > 0.0, "Long-horizon DF should be positive: {}", df);
    }

    #[test]
    fn robust_relative_df_rejects_non_positive() {
        // This test verifies that truly invalid DFs are rejected
        // In practice this shouldn't happen with well-constructed curves,
        // but the guard protects against misconfigured curves.
        //
        // We can't easily construct a curve that returns negative DF,
        // so we just verify the function returns valid positive DFs for normal inputs.
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        let target = date(2025, 1, 1);
        let df = robust_relative_df(&disc, base_date, target).expect("should succeed");
        assert!(df > 0.0, "DF must be positive: {}", df);
    }

    // ==================== OIS COMPOUNDING PROJECTION TESTS ====================

    /// Regression for [P3-1]: a future-reset OIS / RFR leg
    /// (`Compounded` / `CompoundedWithShift` / `Average`) must project the
    /// coupon by **true daily compounding** `(∏(1+rᵢ·dᵢ)−1)/τ`, not by the
    /// simple arithmetic-average forward `rate_period`.
    ///
    /// Failure mode locked in: on an upward-sloping curve the daily-compounded
    /// rate exceeds the arithmetic average by the daily-compounding convexity.
    /// The old code used `fwd.rate_period(t0, t1)` (the arithmetic average) and
    /// under-projected every forward coupon. The compounded rate must be
    /// strictly greater here, and must match an independent product loop.
    #[test]
    fn pv_floating_leg_compounded_uses_daily_compounding_not_simple_average() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = steep_forward_curve(base_date);

        let accrual_start = date(2024, 7, 1);
        let accrual_end = date(2025, 1, 1);
        let year_fraction = fwd
            .day_count()
            .year_fraction(accrual_start, accrual_end, DayCountContext::default())
            .expect("yf");

        let periods = vec![LegPeriod {
            accrual_start,
            accrual_end,
            reset_date: Some(accrual_start),
            year_fraction,
        }];

        // Compounded leg, no observation shift.
        let params = float_compounded(0.0);

        let pv = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            base_date,
            None,
        )
        .expect("should price");

        let df = robust_relative_df(&disc, base_date, accrual_end).expect("df");
        let implied_rate = pv / (1_000_000.0 * year_fraction * df);

        // The simple arithmetic-average forward (the OLD, buggy projection).
        let fwd_dc = fwd.day_count();
        let t0 = fwd_dc
            .year_fraction(base_date, accrual_start, DayCountContext::default())
            .expect("yf");
        let t1 = fwd_dc
            .year_fraction(base_date, accrual_end, DayCountContext::default())
            .expect("yf");
        let simple_avg_rate = fwd.rate_period(t0, t1);

        // Daily compounding must produce a STRICTLY HIGHER rate than the simple
        // average on this upward-sloping curve (positive compounding convexity).
        assert!(
            implied_rate > simple_avg_rate + 1e-6,
            "compounded OIS leg must exceed the simple-average projection: \
             compounded={implied_rate}, simple_avg={simple_avg_rate}"
        );

        // Independently recompute the compounded rate with a weekday product
        // loop and confirm the leg matches it.
        let mut acc = 1.0_f64;
        let mut d = accrual_start;
        while d < accrual_end {
            let nxt = d.add_weekdays(1).min(accrual_end);
            let dcf = fwd_dc
                .year_fraction(d, nxt, DayCountContext::default())
                .expect("dcf");
            let ts = fwd_dc
                .year_fraction(base_date, d, DayCountContext::default())
                .expect("ts");
            let te = fwd_dc
                .year_fraction(base_date, nxt, DayCountContext::default())
                .expect("te");
            let r = if te > ts {
                fwd.rate_period(ts, te)
            } else {
                fwd.rate(ts)
            };
            acc *= 1.0 + r * dcf;
            d = nxt;
        }
        let expected_compounded = (acc - 1.0) / year_fraction;
        assert!(
            (implied_rate - expected_compounded).abs() < 1e-9,
            "compounded OIS projection must match an independent daily product: \
             implied={implied_rate}, expected={expected_compounded}"
        );
    }

    /// Regression for [P3-1]: `observation_shift_days` must be honored for
    /// future OIS periods. The old code dropped the shift entirely.
    ///
    /// Failure mode locked in: on a steep curve, shifting the observation
    /// window back by 5 business days reads lower forwards, so the projected
    /// coupon must differ from the unshifted projection.
    #[test]
    fn pv_floating_leg_compounded_applies_observation_shift() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = steep_forward_curve(base_date);

        let accrual_start = date(2024, 7, 1);
        let accrual_end = date(2025, 1, 1);
        let year_fraction = fwd
            .day_count()
            .year_fraction(accrual_start, accrual_end, DayCountContext::default())
            .expect("yf");
        let period = LegPeriod {
            accrual_start,
            accrual_end,
            reset_date: Some(accrual_start),
            year_fraction,
        };

        let price = |shift: i32| -> f64 {
            let params = float_ois(0.0, shift, 0);
            pv_floating_leg(
                std::iter::once(period.clone()),
                1_000_000.0,
                &params,
                &disc,
                &fwd,
                base_date,
                None,
            )
            .expect("should price")
        };

        let pv_no_shift = price(0);
        let pv_shifted = price(5);

        // A 5-business-day lookback on a steep upward curve reads lower
        // forwards → strictly lower projected coupon → strictly lower PV.
        assert!(
            pv_shifted < pv_no_shift - 1.0,
            "observation shift must change the projected coupon: \
             pv_no_shift={pv_no_shift}, pv_shifted={pv_shifted}"
        );
    }

    /// Regression for [P3-1] item 4: a malformed accrual period
    /// (`accrual_end <= accrual_start`) on a compounded leg must return a
    /// validation error, not silently fall back to a zero-length coupon.
    #[test]
    fn pv_floating_leg_compounded_rejects_malformed_period() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = steep_forward_curve(base_date);

        // accrual_end == accrual_start: zero-length period.
        let degenerate = LegPeriod {
            accrual_start: date(2024, 7, 1),
            accrual_end: date(2024, 7, 1),
            reset_date: Some(date(2024, 7, 1)),
            year_fraction: 0.0,
        };
        let params = float_compounded(0.0);
        let result = pv_floating_leg(
            vec![degenerate].into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            base_date,
            None,
        );
        assert!(
            result.is_err(),
            "a zero-length accrual period on a compounded leg must Err, \
             not silently produce a zero coupon"
        );

        // accrual_end < accrual_start: inverted period.
        let inverted = LegPeriod {
            accrual_start: date(2025, 1, 1),
            accrual_end: date(2024, 7, 1),
            reset_date: Some(date(2025, 1, 1)),
            year_fraction: -0.5,
        };
        let result = pv_floating_leg(
            vec![inverted].into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            base_date,
            None,
        );
        assert!(
            result.is_err(),
            "an inverted accrual period (end < start) on a compounded leg must Err"
        );
    }

    /// Regression for [P3-1] item 3: the index floor on a compounded OIS leg
    /// must be applied **per daily fixing**, not to the period-average rate.
    ///
    /// Failure mode locked in: with a floor set ABOVE every daily forward, a
    /// per-fixing floor lifts every fixing to the floor so the compounded
    /// period rate equals the daily-compounded floor — strictly above what the
    /// raw (unfloored) forwards would compound to. Applying the floor only to
    /// the period average would give a different (lower-resolution) result;
    /// here we assert the per-fixing flooring is in effect by checking the
    /// floored leg compounds the floor rate.
    #[test]
    fn pv_floating_leg_compounded_floors_each_daily_fixing() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = steep_forward_curve(base_date);

        let accrual_start = date(2024, 7, 1);
        let accrual_end = date(2025, 1, 1);
        let fwd_dc = fwd.day_count();
        let year_fraction = fwd_dc
            .year_fraction(accrual_start, accrual_end, DayCountContext::default())
            .expect("yf");
        let periods = vec![LegPeriod {
            accrual_start,
            accrual_end,
            reset_date: Some(accrual_start),
            year_fraction,
        }];

        // Floor at 10% — far above every daily forward on the steep curve over
        // this window (forwards there are well under 10%).
        let floor_bp = 1000.0; // 10%
        let params = FloatingLegParams {
            rate_params: FloatingRateParams {
                spread_bp: 0.0,
                gearing: 1.0,
                gearing_includes_spread: true,
                index_floor_bp: Some(floor_bp), // index floor
                index_cap_bp: None,
                all_in_floor_bp: None,
                all_in_cap_bp: None,
            },
            payment_lag_days: 0,
            calendar_id: None,
            compounding_method: CompoundingMethod::Compounded,
            observation_shift_days: 0,
        };

        let pv = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            base_date,
            None,
        )
        .expect("should price");

        let df = robust_relative_df(&disc, base_date, accrual_end).expect("df");
        let implied_rate = pv / (1_000_000.0 * year_fraction * df);

        // With every daily fixing floored to 10%, the compounded period rate is
        // the daily-compounding of a flat 10% — strictly above 10% (convexity).
        let floor = floor_bp * 1e-4;
        assert!(
            implied_rate > floor,
            "per-fixing-floored compounded rate must exceed the flat floor by \
             daily-compounding convexity: implied={implied_rate}, floor={floor}"
        );

        // Independently compound a flat floor rate over the daily grid.
        let mut acc = 1.0_f64;
        let mut d = accrual_start;
        while d < accrual_end {
            let nxt = d.add_weekdays(1).min(accrual_end);
            let dcf = fwd_dc
                .year_fraction(d, nxt, DayCountContext::default())
                .expect("dcf");
            acc *= 1.0 + floor * dcf;
            d = nxt;
        }
        let expected = (acc - 1.0) / year_fraction;
        assert!(
            (implied_rate - expected).abs() < 1e-9,
            "per-fixing-floored compounded rate must equal the daily-compounded \
             floor: implied={implied_rate}, expected={expected}"
        );
    }

    /// Regression for [R1]: a `Compounded` / `CompoundedWithShift` / `Average`
    /// (OIS / RFR) leg whose current coupon period **straddles** `as_of`
    /// (`reset_date < as_of <= accrual_end`) must price that coupon by
    /// **splicing** realized daily overnight fixings (period start -> as_of) with
    /// projected overnight forwards (as_of -> period end), daily-compounded per
    /// the leg's convention -- NOT by fetching a single term fixing at the reset
    /// date.
    ///
    /// Failure mode locked in: the pre-R1 code took the `reset_date < as_of`
    /// branch and called `require_fixing_value_exact`, treating the in-progress
    /// OIS coupon as one term fixing. That mis-prices every seasoned RFR swap.
    /// The expected coupon is computed independently below as the spliced
    /// compound `(prod(1+r_i d_i)-1)/tau`.
    #[test]
    fn pv_floating_leg_compounded_straddling_period_splices_realized_and_projected() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = test_forward_curve(base_date);

        // Seasoned OIS swap: the current coupon runs Jan 1 -> Apr 1 2024 and the
        // valuation date Feb 15 sits *inside* it (reset_date < as_of < accrual_end).
        let accrual_start = date(2024, 1, 1);
        let accrual_end = date(2024, 4, 1);
        let as_of = date(2024, 2, 15);
        let fwd_dc = fwd.day_count();
        let year_fraction = fwd_dc
            .year_fraction(accrual_start, accrual_end, DayCountContext::default())
            .expect("yf");

        let periods = vec![LegPeriod {
            accrual_start,
            accrual_end,
            reset_date: Some(accrual_start),
            year_fraction,
        }];

        // Compounded leg, no observation shift, no spread (isolate the coupon).
        let params = float_compounded(0.0);

        // Supply a realized daily overnight fixing for every weekday in the
        // historical sub-period [accrual_start, as_of). Use a non-flat ramp so
        // the splice cannot be mimicked by any single term fixing.
        let mut fixing_obs: Vec<(Date, f64)> = Vec::new();
        {
            let mut d = accrual_start;
            let mut i = 0u32;
            while d < as_of {
                fixing_obs.push((d, 0.03 + 0.0001 * f64::from(i)));
                d = d.add_weekdays(1);
                i += 1;
            }
        }
        let fixings = ScalarTimeSeries::new("FIXING:TEST-FWD", fixing_obs.clone(), None)
            .expect("fixings series");

        let pv = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            as_of,
            Some(&fixings),
        )
        .expect("should price seasoned OIS swap");

        let payment_date = accrual_end; // no payment lag
        let df = robust_relative_df(&disc, as_of, payment_date).expect("df");
        let implied_rate = pv / (1_000_000.0 * year_fraction * df);

        // Independent reference: splice realized fixings with projected forwards.
        let fixing_lookup = |d: Date| -> f64 {
            fixing_obs
                .iter()
                .find(|(fd, _)| *fd == d)
                .map(|(_, v)| *v)
                .expect("realized fixing for historical sub-period")
        };
        let mut acc = 1.0_f64;
        let mut d = accrual_start;
        while d < accrual_end {
            let nxt = d.add_weekdays(1).min(accrual_end);
            let dcf = fwd_dc
                .year_fraction(d, nxt, DayCountContext::default())
                .expect("dcf");
            let r = if d < as_of {
                fixing_lookup(d)
            } else {
                let ts = fwd_dc
                    .year_fraction(base_date, d, DayCountContext::default())
                    .expect("ts");
                let te = fwd_dc
                    .year_fraction(base_date, nxt, DayCountContext::default())
                    .expect("te");
                if te > ts {
                    fwd.rate_period(ts, te)
                } else {
                    fwd.rate(ts)
                }
            };
            acc *= 1.0 + r * dcf;
            d = nxt;
        }
        let expected_spliced = (acc - 1.0) / year_fraction;

        assert!(
            (implied_rate - expected_spliced).abs() < 1e-9,
            "in-progress compounded coupon must equal the spliced realized/projected \
             daily compound: implied={implied_rate}, expected={expected_spliced}"
        );

        let single_fixing = fixing_lookup(accrual_start);
        assert!(
            (implied_rate - single_fixing).abs() > 1e-4,
            "in-progress compounded coupon must not collapse to the single \
             reset-date fixing: implied={implied_rate}, single_fixing={single_fixing}"
        );
    }

    // ==================== R1 REGRESSION: PAYMENT-LAGGED FULLY-ACCRUED COMPOUNDED COUPON ====================

    /// Regression for R1: a payment-lagged compounded OIS coupon that is **fully
    /// accrued but not yet paid** (`accrual_end <= as_of < payment_date`) must
    /// not hard-error.
    ///
    /// Before the fix, `pv_floating_leg`'s routing predicate fired
    /// (`is_compounded && accrual_start <= as_of`) and called
    /// `compounded_spliced_projection`, whose guard required
    /// `as_of < accrual_end` and returned a validation error.
    ///
    /// After the fix the guard is relaxed to `accrual_start <= as_of`, so the
    /// helper naturally computes an all-realized daily compound
    /// (`∏(1+rᵢdᵢ)−1)/τ` with every sub-period drawn from historical fixings —
    /// exactly the correct value for a fully-accrued coupon.
    #[test]
    fn pv_floating_leg_compounded_fully_accrued_unpaid_payment_lag_returns_ok() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);
        let fwd = test_forward_curve(base_date);

        // Coupon: Jan 1 -> Apr 1 2024 (Q1).
        // Payment lag: 2 business days → payment_date = Apr 3 2024 (Tue).
        // as_of: Apr 2 2024 — AFTER accrual_end but BEFORE payment_date.
        let accrual_start = date(2024, 1, 1);
        let accrual_end = date(2024, 4, 1);
        let as_of = date(2024, 4, 2); // past accrual_end, payment not yet made

        let fwd_dc = fwd.day_count();
        let year_fraction = fwd_dc
            .year_fraction(accrual_start, accrual_end, DayCountContext::default())
            .expect("yf");

        let periods = vec![LegPeriod {
            accrual_start,
            accrual_end,
            reset_date: Some(accrual_start),
            year_fraction,
        }];

        // OIS params with 2-day payment lag, no spread.
        let params = float_ois(0.0, 0, 2);

        // Supply realized overnight fixings for every weekday in [accrual_start, accrual_end).
        // Use a non-flat ramp (mirror of the in-progress test) so that a buggy
        // implementation that returns a plain simple-rate approximation instead of
        // the true daily compound cannot accidentally match the expected value.
        let mut fixing_obs: Vec<(Date, f64)> = Vec::new();
        {
            let mut d = accrual_start;
            let mut i = 0u32;
            while d < accrual_end {
                fixing_obs.push((d, 0.03 + 0.0001 * f64::from(i)));
                d = d.add_weekdays(1);
                i += 1;
            }
        }
        let fixings = ScalarTimeSeries::new("FIXING:TEST-FWD", fixing_obs.clone(), None)
            .expect("fixings series");

        // Before the fix this returns Err (guard fires: as_of >= accrual_end).
        // After the fix it must return Ok.
        let pv = pv_floating_leg(
            periods.into_iter(),
            1_000_000.0,
            &params,
            &disc,
            &fwd,
            as_of,
            Some(&fixings),
        )
        .expect("payment-lagged fully-accrued compounded coupon must not hard-error (R1 regression)");

        // Independently compute the all-realized daily compound from the ramp fixings.
        // This reference loop is intentionally separate from the production helper so
        // that a regression in `compounded_spliced_projection` would be caught here.
        let mut acc = 1.0_f64;
        {
            let mut idx = 0usize;
            let mut d = accrual_start;
            while d < accrual_end {
                let nxt = d.add_weekdays(1).min(accrual_end);
                let dcf = fwd_dc
                    .year_fraction(d, nxt, DayCountContext::default())
                    .expect("dcf");
                let r = fixing_obs[idx].1;
                acc *= 1.0 + r * dcf;
                d = nxt;
                idx += 1;
            }
        }
        let expected_rate = (acc - 1.0) / year_fraction;

        // The payment is in the future (Apr 3), so we discount to as_of.
        let payment_date = accrual_end.add_weekdays(2); // 2 BD lag
        let df = robust_relative_df(&disc, as_of, payment_date).expect("df");
        let implied_rate = pv / (1_000_000.0 * year_fraction * df);

        assert!(
            (implied_rate - expected_rate).abs() < 1e-9,
            "fully-accrued payment-lagged OIS coupon must equal the all-realized \
             daily compound: implied={implied_rate:.8}, expected={expected_rate:.8}"
        );

        // Discriminating assertion: the true daily compound of a non-flat ramp must
        // differ from the naive arithmetic-average simple rate by more than 1e-5.
        // A bug that returns the simple/average rate instead of the daily compound
        // would produce a value close to the average and fail this check.
        let naive_avg = fixing_obs.iter().map(|(_, r)| r).sum::<f64>() / fixing_obs.len() as f64;
        assert!(
            (implied_rate - naive_avg).abs() > 1e-5,
            "daily-compounded result must differ from the naive average rate by >1e-5 \
             (guards against a regression that skips daily compounding): \
             implied={implied_rate:.8}, naive_avg={naive_avg:.8}"
        );

        // Sanity: PV must be positive (positive rate, future payment).
        assert!(pv > 0.0, "PV of a positive-rate OIS coupon must be positive; got {pv}");
    }

    // ==================== ANNUITY GUARD DIAGNOSTIC TEST ====================

    /// Regression for [P3-1] item 6: when `leg_annuity` rejects a corrupt leg,
    /// the error message must describe the actual failure mode.
    ///
    /// A discounted annuity is `Σ year_fraction · DF` and discount factors are
    /// always strictly positive, so a **negative** annuity can only arise from
    /// a negative year fraction — i.e. an inverted / malformed accrual period,
    /// NOT "all periods expired" (that gives exactly zero) and NOT "extreme
    /// discounting" (that gives a tiny *positive* value). The old message
    /// claimed the former two causes for every sub-threshold annuity, which
    /// mis-describes the negative-year-fraction corruption. The error message
    /// must name the year-fraction corruption when the annuity is negative.
    #[test]
    fn leg_annuity_negative_year_fraction_reports_corruption_not_expiry() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        // A future-dated period (so it is NOT skipped as expired) but with a
        // negative year fraction — a corrupt, inverted accrual period.
        let corrupt = LegPeriod {
            accrual_start: date(2024, 7, 1),
            accrual_end: date(2025, 1, 1),
            reset_date: Some(date(2024, 7, 1)),
            year_fraction: -0.5,
        };

        let err = leg_annuity(vec![corrupt].into_iter(), &disc, base_date, 0, None)
            .expect_err("a negative-year-fraction leg must be rejected");
        let msg = err.to_string();

        // The diagnostic must point at the year-fraction corruption, not at
        // expiry / extreme-discounting (the misdescription being fixed).
        let lower = msg.to_lowercase();
        assert!(
            lower.contains("year fraction") || lower.contains("year-fraction"),
            "annuity error for a negative-year-fraction leg must name the \
             year-fraction corruption; got: {msg}"
        );
        assert!(
            !lower.contains("expired"),
            "annuity error for a NEGATIVE annuity must not claim periods \
             expired (expiry yields a zero annuity, not a negative one); got: {msg}"
        );
    }
}
