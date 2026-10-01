use crate::instruments::common_impl::pricing::time::{
    curve_time, rate_between_on_dates, rate_period_on_dates,
};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
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
///   ACT/ACT (ICMA) and ACT/365L; `None` is valid for conventions without it.
/// * `schedule` - Ordered coupon boundary/payment dates; adjacent pairs form
///   accrual periods and the first date anchors the leg. For ICMA, the first
///   coupon anchors one quasi-coupon grid for the entire leg, preserving EOM
///   rolls and both front and back stubs.
///   For ACT/365L, every adjacent pair must be a full contractual coupon;
///   rate or balance subintervals cannot replace its original boundaries.
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
        let context = if day_count == finstack_quant_core::dates::DayCount::Act365L {
            DayCountContext {
                coupon_period: Some((prev, d)),
                ..dc_ctx
            }
        } else {
            dc_ctx
        };
        let alpha = day_count.year_fraction(prev, d, context)?;
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
///   ACT/ACT (ICMA) and ACT/365L; `None` is valid for conventions without it.
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

/// Asset-swap forward leg PV and fixed/floating annuities per unit notional.
///
/// # Arguments
///
/// * `disc` - Discount curve supplying present-value factors for both legs.
/// * `fwd` - Forward curve supplying date-based floating reference rates.
///   This curve-convention helper also uses its day count for floating coupon
///   accrual; callers with an explicit floating convention must use that basis
///   when calculating the floating leg.
/// * `fixed_day_count` - Fixed-leg accrual day-count convention.
/// * `fixed_frequency` - Optional fixed coupon frequency required by
///   ACT/ACT-style and ACT/365L accrual calculations.
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

    let (float_pv, float_ann) =
        floating_leg_pv_and_annuity(disc, fwd, fwd.day_count(), schedule, float_spread_bp, None)?;
    Ok((float_pv, fixed_ann, float_ann))
}

/// PV and annuity of an asset-swap floating leg per unit notional.
///
/// Each period `[prev, d]` pays `(forward + spread) · α · P(d)` with `α` under
/// `day_count`; the annuity is `Σ α · P(d)`. Both sums use compensated
/// (Neumaier) summation.
///
/// # Arguments
///
/// * `disc` - Discount curve supplying `P(d)` at each period end.
/// * `fwd` - Forward curve projecting unstarted coupons (see
///   [`asset_swap_projection_rate`]).
/// * `day_count` - Floating-leg accrual day count.
/// * `schedule` - Ordered period boundary dates; fewer than two dates give
///   `(0, 0)`.
/// * `float_spread_bp` - Spread over the index in basis points.
/// * `seasoned` - `Some((market, as_of))` fixes periods that started before
///   `as_of` from the forward curve's historical fixing series in `market`;
///   `None` projects every period.
///
/// # Errors
///
/// Returns an error when a year fraction, discount factor or projection
/// fails, or when a started period has no fixing on its reset date.
pub(crate) fn floating_leg_pv_and_annuity(
    disc: &DiscountCurve,
    fwd: &ForwardCurve,
    day_count: finstack_quant_core::dates::DayCount,
    schedule: &[Date],
    float_spread_bp: f64,
    seasoned: Option<(&MarketContext, Date)>,
) -> finstack_quant_core::Result<(f64, f64)> {
    if schedule.len() < 2 {
        return Ok((0.0, 0.0));
    }
    let spread = float_spread_bp * 1e-4;
    let mut float_pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut float_ann = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut prev = schedule[0];
    for &d in &schedule[1..] {
        let yf = day_count.year_fraction(prev, d, DayCountContext::default())?;
        let df = disc.df_on_date_curve(d)?;
        let forward = match seasoned {
            Some((market, as_of)) if prev < as_of => {
                let fixing_id =
                    finstack_quant_core::market_data::fixings::fixing_series_id(fwd.id().as_str());
                let fixings = market.get_series(&fixing_id).map_err(|_| {
                    finstack_quant_core::Error::Validation(format!(
                        "Seasoned asset swap requires historical fixing series '{}' for reset date {}; \
                         started term coupons must use observed fixings, not projection",
                        fixing_id, prev
                    ))
                })?;
                fixings.value_on_exact(prev)?
            }
            _ => asset_swap_projection_rate(fwd, prev, d, yf)?,
        };
        float_pv.add((forward + spread) * yf * df);
        float_ann.add(yf * df);
        prev = d;
    }
    Ok((float_pv.total(), float_ann.total()))
}

/// Project an asset-swap floating coupon from the curve's index convention.
///
/// Overnight indices represent observation rates that are averaged over the
/// coupon window. Term indices instead use the discount-factor-implied simple
/// forward for the whole accrual period, annualized using the floating leg
/// accrual fraction rather than the curve's time coordinate.
fn asset_swap_projection_rate(
    fwd: &ForwardCurve,
    start: Date,
    end: Date,
    accrual_year_fraction: f64,
) -> finstack_quant_core::Result<f64> {
    const MAX_OVERNIGHT_TENOR_YEARS: f64 = 1.0 / 52.0;

    if fwd.tenor() <= MAX_OVERNIGHT_TENOR_YEARS {
        if end <= start
            || start < fwd.base_date()
            || !accrual_year_fraction.is_finite()
            || accrual_year_fraction <= 0.0
        {
            return Err(finstack_quant_core::Error::Validation(
                "asset-swap overnight projection requires a future positive accrual period".into(),
            ));
        }
        let curve_accrual = curve_time(fwd, end)? - curve_time(fwd, start)?;
        let rate = rate_period_on_dates(fwd, start, end)? * curve_accrual / accrual_year_fraction;
        if !rate.is_finite() || curve_accrual <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "asset-swap overnight projection produced an invalid rate or curve accrual".into(),
            ));
        }
        Ok(rate)
    } else {
        rate_between_on_dates(fwd, start, end, accrual_year_fraction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{DayCount, Tenor};
    use time::macros::date;

    #[test]
    fn floating_asset_swap_uses_contractual_accrual_with_a_different_curve_clock() {
        let start = date!(2025 - 04 - 02);
        let end = date!(2025 - 07 - 02);
        let disc = DiscountCurve::builder("USD-DISC")
            .base_date(start)
            .knots([(0.0, 1.0), (1.0, 1.0)])
            .build()
            .expect("zero discounting");
        for tenor in [0.25, 1.0 / 365.0] {
            let fwd = ForwardCurve::builder("USD-INDEX", tenor)
                .base_date(start)
                .day_count(DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
                .knots([(0.0, 0.04), (1.0, 0.07)])
                .build()
                .expect("forward curve");
            let spread_bp = 125.0;
            let accrual = (end - start).whole_days() as f64 / 360.0;
            let growth = if tenor > 1.0 / 52.0 {
                fwd.df_on_date_curve(start).expect("start DF")
                    / fwd.df_on_date_curve(end).expect("end DF")
                    - 1.0
            } else {
                // Exact integral of the linear raw curve on its ACT/365F clock.
                let curve_time = (end - start).whole_days() as f64 / 365.0;
                (0.04 + 0.5 * (0.07 - 0.04) * curve_time) * curve_time
            };
            let (pv, annuity) = floating_leg_pv_and_annuity(
                &disc,
                &fwd,
                DayCount::Act360,
                &[start, end],
                spread_bp,
                None,
            )
            .expect("floating asset swap");
            assert!((annuity - accrual).abs() < 1e-14);
            assert!((pv - growth - spread_bp * 1e-4 * accrual).abs() < 1e-14);
        }
    }

    #[test]
    fn act365l_annuity_preserves_annual_coupon_denominators_across_slices() {
        let schedule = [
            date!(2023 - 03 - 01),
            date!(2024 - 03 - 01),
            date!(2025 - 03 - 01),
        ];
        let disc = DiscountCurve::builder("ACT365L-ANNUITY")
            .base_date(schedule[0])
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (366.0 / 365.0, 0.97), (731.0 / 365.0, 0.93)])
            .build()
            .expect("discount curve");
        let annuity = fixed_leg_annuity(&disc, DayCount::Act365L, Some(Tenor::annual()), &schedule)
            .expect("annual ACT/365L annuity");
        assert!((annuity - 1.90).abs() < 1e-14);

        // A balance or rate split changes cashflow weights, never the full
        // coupon's denominator. These two slices reconstruct each unit coupon.
        let mut segmented_annuity = 0.0;
        for (start, end, split, denominator, df) in [
            (schedule[0], schedule[1], date!(2023 - 06 - 01), 366.0, 0.97),
            (schedule[1], schedule[2], date!(2024 - 06 - 01), 365.0, 0.93),
        ] {
            let context = DayCountContext {
                frequency: Some(Tenor::annual()),
                coupon_period: Some((start, end)),
                ..DayCountContext::default()
            };
            let first = DayCount::Act365L
                .year_fraction(start, split, context)
                .expect("first slice");
            let second = DayCount::Act365L
                .year_fraction(split, end, context)
                .expect("second slice");
            assert!((first - (split - start).whole_days() as f64 / denominator).abs() < 1e-14);
            assert!((second - (end - split).whole_days() as f64 / denominator).abs() < 1e-14);
            segmented_annuity += (first + second) * df;
        }
        assert!((annuity - segmented_annuity).abs() < 1e-14);
        assert!(fixed_leg_annuity(&disc, DayCount::Act365L, None, &schedule).is_err());
    }

    #[test]
    fn act365l_annuity_uses_actual_stub_and_nonannual_boundaries() {
        for (start, end, frequency, denominator) in [
            (
                date!(2024 - 03 - 01),
                date!(2025 - 01 - 01),
                Tenor::annual(),
                365.0,
            ),
            (
                date!(2023 - 12 - 01),
                date!(2025 - 01 - 01),
                Tenor::annual(),
                366.0,
            ),
            (
                date!(2024 - 03 - 01),
                date!(2024 - 06 - 01),
                Tenor::quarterly(),
                366.0,
            ),
        ] {
            let disc = DiscountCurve::builder("ACT365L-STUB-ANNUITY")
                .base_date(start)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (3.0, 1.0)])
                .build()
                .expect("flat curve");
            let annuity =
                fixed_leg_annuity(&disc, DayCount::Act365L, Some(frequency), &[start, end])
                    .expect("ACT/365L annuity");
            let expected = (end - start).whole_days() as f64 / denominator;
            assert!((annuity - expected).abs() < 1e-14, "{start} -> {end}");
        }
    }
}
