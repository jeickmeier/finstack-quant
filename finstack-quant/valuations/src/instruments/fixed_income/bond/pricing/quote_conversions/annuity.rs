use crate::instruments::common_impl::pricing::time::{rate_between_on_dates, rate_period_on_dates};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};

/// Convert payment frequency to approximate periods per year.
///
/// **Important:** This function is for **frequency conversion only**, NOT day count conventions.
///
/// # Purpose
///
/// This helper determines how many payment periods occur in a year based on the
/// payment frequency. For example, semi-annual payments occur 2 times per year,
/// monthly payments occur 12 times per year.
///
/// # Day Count Conventions
///
/// Actual day count calculations (Actual/360, Actual/365, Actual/Actual, 30/360, etc.)
/// are handled separately via the `DayCount` enum and `year_fraction()` methods in
/// finstack-quant-core. Those methods properly account for:
/// - Leap years (Actual/Actual)
/// - Different day count bases (360 vs 365)
/// - Month length variations (30/360)
///
/// # Arguments
///
/// * `frequency` - Payment frequency (e.g., `Tenor::semi_annual()`)
///
/// # Returns
///
/// Number of periods per year as `f64`.
///
/// # Errors
///
/// Returns `Err` when:
/// - Tenor is zero (invalid)
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::periods_per_year;
/// use finstack_quant_core::dates::Tenor;
///
/// assert_eq!(periods_per_year(Tenor::semi_annual())?, 2.0);
/// assert_eq!(periods_per_year(Tenor::quarterly())?, 4.0);
/// assert_eq!(periods_per_year(Tenor::annual())?, 1.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Note on Daily Tenor
///
/// For daily frequencies, this uses 365 as an approximation of annual periods.
/// This is appropriate for frequency calculations but should NOT be confused with
/// the Actual/365 day count convention used in accrual and discount factor calculations.
#[inline]
pub fn periods_per_year(
    frequency: finstack_quant_core::dates::Tenor,
) -> finstack_quant_core::Result<f64> {
    match frequency.unit() {
        finstack_quant_core::dates::TenorUnit::Months => {
            if frequency.count() == 0 {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
            Ok(12.0 / (frequency.count() as f64))
        }
        finstack_quant_core::dates::TenorUnit::Days => {
            if frequency.count() == 0 {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
            // Use 365 as approximate annual basis for frequency calculations
            // Note: This is NOT a day count convention - actual day count is handled
            // via the DayCount enum (Actual/360, Actual/365, Actual/Actual, etc.)
            Ok(365.0 / (frequency.count() as f64))
        }
        finstack_quant_core::dates::TenorUnit::Years => {
            if frequency.count() == 0 {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
            Ok(1.0 / (frequency.count() as f64))
        }
        finstack_quant_core::dates::TenorUnit::Weeks => {
            if frequency.count() == 0 {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
            Ok(52.0 / (frequency.count() as f64))
        }
    }
}

/// Fixed-leg annuity for a bond-style schedule using discount-curve discount factors.
///
/// This computes the standard swap-style annuity:
/// ```text
/// Annuity = Σ (α_i · P(as_of, T_i))
/// ```
/// where `α_i` is the year fraction between consecutive schedule dates under `day_count`,
/// and `P(as_of, T_i)` is the discount factor from `as_of` to date `T_i`.
///
/// The `schedule` is expected to start at the valuation date (`as_of`) and
/// contain strictly increasing dates.
///
/// # Arguments
///
/// * `disc` - Discount curve supplying date-based fixed-leg discount factors.
/// * `day_count` - Fixed-leg accrual day-count convention.
/// * `frequency` - Optional coupon frequency required by conventions such as
///   ACT/ACT (ICMA); `None` is valid for conventions without it.
/// * `schedule` - Ordered coupon boundary/payment dates; adjacent pairs form
///   accrual periods and the first date anchors the leg. For ICMA, the first
///   coupon anchors one quasi-coupon grid for the entire leg, preserving EOM
///   rolls and both front and back stubs.
///
/// # Returns
///
/// The fixed-leg annuity value.
///
/// # Errors
///
/// Returns an error if any year_fraction calculation fails (e.g., invalid dates).
pub fn fixed_leg_annuity(
    disc: &DiscountCurve,
    day_count: finstack_quant_core::dates::DayCount,
    frequency: Option<finstack_quant_core::dates::Tenor>,
    schedule: &[Date],
) -> finstack_quant_core::Result<f64> {
    if schedule.len() < 2 {
        return Ok(0.0);
    }

    let dc_ctx = DayCountContext {
        frequency,
        coupon_period: frequency.and_then(|frequency| {
            super::icma_reference_period(
                day_count,
                frequency,
                schedule.iter().copied(),
                schedule[0],
            )
        }),
        ..DayCountContext::default()
    };
    let mut ann = 0.0;
    let mut prev = schedule[0];
    for &d in &schedule[1..] {
        let alpha = day_count.year_fraction(prev, d, dc_ctx)?;
        let p = disc.df_on_date_curve(d)?;
        ann += alpha * p;
        prev = d;
    }
    Ok(ann)
}

/// Par swap rate from discount-curve discount ratios and a fixed-leg annuity.
///
/// Uses the standard discount-ratio formula:
/// ```text
/// par_rate = (P(as_of, T₀) - P(as_of, Tₙ)) / Annuity
/// ```
/// where the denominator is the fixed-leg annuity computed with `day_count`.
///
/// Returns both the par rate and the annuity so callers can reuse the latter
/// in asset-swap formulas and related analytics.
///
/// # Arguments
///
/// * `disc` - Discount curve supplying date-based fixed-leg discount factors.
/// * `day_count` - Fixed-leg accrual day-count convention.
/// * `frequency` - Optional coupon frequency required by conventions such as
///   ACT/ACT (ICMA); `None` is valid for conventions without it.
/// * `schedule` - Ordered coupon boundary/payment dates; the first and last
///   dates define the discount-ratio numerator.
///
/// # Returns
///
/// Tuple of `(par_rate, annuity)` where:
/// - `par_rate` is the par swap rate (decimal, e.g., 0.05 for 5%)
/// - `annuity` is the fixed-leg annuity value
///
/// # Errors
///
/// Returns an error if the annuity calculation fails (invalid dates/day-count).
pub fn par_rate_and_annuity_from_discount(
    disc: &DiscountCurve,
    day_count: finstack_quant_core::dates::DayCount,
    frequency: Option<finstack_quant_core::dates::Tenor>,
    schedule: &[Date],
) -> finstack_quant_core::Result<(f64, f64)> {
    if schedule.len() < 2 {
        return Ok((0.0, 0.0));
    }

    let ann = fixed_leg_annuity(disc, day_count, frequency, schedule)?;
    // Use epsilon check to avoid division by near-zero values that could amplify numerical noise
    if ann.abs() < 1e-12 {
        return Ok((0.0, 0.0));
    }

    let p0 = disc.df_on_date_curve(schedule[0])?;
    // `schedule.len() >= 2` by the guard above, so `schedule[0]` and `schedule[last]` are safe.
    let pn_date = schedule[schedule.len() - 1];
    let pn = disc.df_on_date_curve(pn_date)?;
    let num = p0 - pn;
    Ok((num / ann, ann))
}

/// Forward-projected par rate and fixed-leg annuity for an asset-swap schedule.
///
/// # Arguments
///
/// * `disc` - Discount curve supplying fixed-leg and projected floating-coupon
///   present-value discount factors.
/// * `fwd` - Forward curve supplying date-based floating reference rates.
/// * `fixed_day_count` - Fixed-leg accrual day-count convention.
/// * `fixed_frequency` - Optional fixed coupon frequency required by
///   ACT/ACT-style accrual calculations.
/// * `schedule` - Ordered swap coupon boundary/payment dates shared by both
///   legs.
/// * `float_spread_bp` - Contractual floating-leg spread in basis points,
///   added to each forward rate.
pub fn par_rate_and_annuity_from_forward(
    disc: &DiscountCurve,
    fwd: &ForwardCurve,
    fixed_day_count: finstack_quant_core::dates::DayCount,
    fixed_frequency: Option<finstack_quant_core::dates::Tenor>,
    schedule: &[Date],
    float_spread_bp: f64,
) -> finstack_quant_core::Result<(f64, f64)> {
    let ann = fixed_leg_annuity(disc, fixed_day_count, fixed_frequency, schedule)?;
    if ann.abs() < 1e-12 {
        return Ok((0.0, 0.0));
    }

    let f_day_count = fwd.day_count();
    let spread = float_spread_bp * 1e-4;
    let mut pv_float = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut prev = schedule[0];
    for &d in &schedule[1..] {
        let yf = f_day_count.year_fraction(prev, d, DayCountContext::default())?;
        let rate = asset_swap_projection_rate(fwd, prev, d)? + spread;
        let df = disc.df_on_date_curve(d)?;
        pv_float.add(rate * yf * df);
        prev = d;
    }

    Ok((pv_float.total() / ann, ann))
}

/// Asset-swap forward leg PV and fixed/floating annuities per unit notional.
///
/// # Arguments
///
/// * `disc` - Discount curve supplying present-value factors for both legs.
/// * `fwd` - Forward curve supplying date-based floating reference rates.
/// * `fixed_day_count` - Fixed-leg accrual day-count convention.
/// * `fixed_frequency` - Optional fixed coupon frequency required by
///   ACT/ACT-style accrual calculations.
/// * `schedule` - Ordered swap coupon boundary/payment dates shared by both
///   legs.
/// * `float_spread_bp` - Contractual floating-leg spread in basis points,
///   added to every projected forward.
pub fn asset_swap_forward_components(
    disc: &DiscountCurve,
    fwd: &ForwardCurve,
    fixed_day_count: finstack_quant_core::dates::DayCount,
    fixed_frequency: Option<finstack_quant_core::dates::Tenor>,
    schedule: &[Date],
    float_spread_bp: f64,
) -> finstack_quant_core::Result<(f64, f64, f64)> {
    let fixed_ann = fixed_leg_annuity(disc, fixed_day_count, fixed_frequency, schedule)?;
    if schedule.len() < 2 {
        return Ok((0.0, fixed_ann, 0.0));
    }

    let f_day_count = fwd.day_count();
    let spread = float_spread_bp * 1e-4;
    let mut float_pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut float_ann = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut prev = schedule[0];
    for &d in &schedule[1..] {
        let yf = f_day_count.year_fraction(prev, d, DayCountContext::default())?;
        let df = disc.df_on_date_curve(d)?;
        float_pv.add((asset_swap_projection_rate(fwd, prev, d)? + spread) * yf * df);
        float_ann.add(yf * df);
        prev = d;
    }

    Ok((float_pv.total(), fixed_ann, float_ann.total()))
}

/// Project an asset-swap floating coupon from the curve's index convention.
///
/// Overnight indices represent observation rates that are averaged over the
/// coupon window. Term indices instead use the discount-factor-implied simple
/// forward for the whole accrual period.
pub(crate) fn asset_swap_projection_rate(
    fwd: &ForwardCurve,
    start: Date,
    end: Date,
) -> finstack_quant_core::Result<f64> {
    const MAX_OVERNIGHT_TENOR_YEARS: f64 = 1.0 / 52.0;

    if fwd.tenor() <= MAX_OVERNIGHT_TENOR_YEARS {
        rate_period_on_dates(fwd, start, end)
    } else {
        rate_between_on_dates(fwd, start, end)
    }
}
