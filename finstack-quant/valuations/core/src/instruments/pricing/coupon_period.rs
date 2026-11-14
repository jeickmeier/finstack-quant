//! Quasi-coupon period resolution and schedule slicing for ACT/ACT (ICMA) year fractions.

use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext, Tenor, TenorUnit};

const MAX_ICMA_QUASI_STEPS: i32 = 2_400;

/// Infer an ACT/ACT (ICMA) reference coupon period from the cashflow schedule.
///
/// Returns the quasi-coupon period `(first_flow − frequency, first_flow)`
/// surrounding the settlement date, where `first_flow` is the first cashflow
/// date strictly after `as_of`. Callers evaluate multi-period spans on this
/// quasi-coupon grid through [`icma_year_fraction`].
///
/// # Arguments
///
/// * `day_count` - Bond coupon day-count convention; only
///   [`DayCount::ActActIsma`] yields a period.
/// * `frequency` - Contractual coupon frequency defining the quasi-coupon
///   grid; week/day frequencies are unsupported by ICMA and yield `None`.
/// * `dates` - Coupon payment dates in ascending order; the first date
///   strictly after `as_of` anchors the grid.
/// * `as_of` - Settlement/valuation date the reference period must surround.
///
/// # Returns
///
/// `None` for non-ICMA day counts, week/day frequencies, or when no flow falls
/// strictly after `as_of`.
pub fn icma_reference_period(
    day_count: DayCount,
    frequency: Tenor,
    dates: impl IntoIterator<Item = Date>,
    as_of: Date,
) -> Option<(Date, Date)> {
    if day_count != DayCount::ActActIsma {
        return None;
    }
    let months = match frequency.unit() {
        TenorUnit::Months => frequency.count() as i32,
        TenorUnit::Years => (frequency.count() as i32).checked_mul(12)?,
        TenorUnit::Weeks | TenorUnit::Days => return None,
    };
    if months <= 0 {
        return None;
    }
    let next = dates.into_iter().find(|d| *d > as_of)?;
    let mut prev = next.add_months(-months).ok()?;
    // Preserve the end-of-month roll so the quasi-coupon grid matches an EOM schedule.
    if next == next.end_of_month() {
        prev = prev.end_of_month();
    }
    (prev < next).then_some((prev, next))
}

fn step_quasi_grid(
    anchor: Date,
    period_months: i32,
    k: i32,
    preserve_eom: bool,
) -> finstack_quant_core::Result<Date> {
    if k.abs() > MAX_ICMA_QUASI_STEPS {
        return Err(finstack_quant_core::Error::Validation(
            "ACT/ACT ICMA quasi-coupon grid traversal exceeded supported range".into(),
        ));
    }
    let months = k.checked_mul(period_months).ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "ACT/ACT ICMA quasi-coupon month offset overflows".into(),
        )
    })?;
    let shifted = anchor.add_months(months)?;
    Ok(if preserve_eom {
        shifted.end_of_month()
    } else {
        shifted
    })
}

/// Compute `day_count` year fraction between `start` and `end`, slicing
/// multi-period `ActActIsma` spans along the quasi-coupon grid anchored on
/// `reference_period`.
///
/// # Arguments
///
/// * `day_count` - Accrual/yield day-count convention.
/// * `frequency` - Optional contractual coupon frequency required when
///   `day_count` is [`DayCount::ActActIsma`].
/// * `reference_period` - Enclosing nominal coupon period `(period_start, period_end)`
///   anchoring the ICMA quasi-coupon grid (or enclosing period for `Act365L`).
/// * `start` - Inclusive start date of the span being measured.
/// * `end` - Exclusive end date of the span being measured (`end >= start`).
///
/// # Returns
///
/// Non-negative year fraction in years.
///
/// # Errors
///
/// Propagates inverted-date, missing-context, unsupported-frequency, and
/// nominal-grid validation errors from [`DayCount::year_fraction`].
pub fn icma_year_fraction(
    day_count: DayCount,
    frequency: Option<Tenor>,
    reference_period: Option<(Date, Date)>,
    start: Date,
    end: Date,
) -> finstack_quant_core::Result<f64> {
    let base_ctx = DayCountContext {
        frequency,
        coupon_period: reference_period,
        ..DayCountContext::default()
    };
    if day_count != DayCount::ActActIsma {
        return day_count.year_fraction(start, end, base_ctx);
    }
    if start > end {
        return Err(finstack_quant_core::InputError::InvalidDateRange.into());
    }
    day_count.validate_context(base_ctx)?;
    let (ref_start, ref_end) = reference_period
        .ok_or(finstack_quant_core::InputError::MissingCouponPeriodForActActIsma)?;
    if ref_start <= start && end <= ref_end {
        return day_count.year_fraction(start, end, base_ctx);
    }
    if start == end {
        return Ok(0.0);
    }

    let frequency =
        frequency.ok_or(finstack_quant_core::InputError::MissingFrequencyForActActIsma)?;
    let period_months = frequency
        .months()
        .and_then(|m| i32::try_from(m).ok())
        .filter(|&m| m > 0)
        .ok_or_else(
            || finstack_quant_core::InputError::ActActIsmaUnsupportedFrequency {
                frequency: frequency.to_string(),
            },
        )?;

    let preserve_eom = ref_start == ref_start.end_of_month() && ref_end == ref_end.end_of_month();
    let anchor = if ref_start.day() >= ref_end.day() {
        ref_start
    } else {
        ref_end
    };

    let mut k = 0;
    while step_quasi_grid(anchor, period_months, k, preserve_eom)? > start {
        k -= 1;
    }
    while step_quasi_grid(anchor, period_months, k + 1, preserve_eom)? <= start {
        k += 1;
    }

    let mut total = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    loop {
        let p_start = step_quasi_grid(anchor, period_months, k, preserve_eom)?;
        let p_end = step_quasi_grid(anchor, period_months, k + 1, preserve_eom)?;
        let slice_start = start.max(p_start);
        let slice_end = end.min(p_end);
        let slice_ctx = DayCountContext {
            coupon_period: Some((p_start, p_end)),
            ..base_ctx
        };
        total.add(day_count.year_fraction(slice_start, slice_end, slice_ctx)?);
        if end <= p_end {
            return Ok(total.total());
        }
        k += 1;
    }
}
