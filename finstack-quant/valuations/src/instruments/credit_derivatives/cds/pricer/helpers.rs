use crate::constants::{credit, numerical};
use finstack_quant_core::dates::{Date, DateExt, DayCount, HolidayCalendar};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::{Error, Result};
use time::Duration;

/// Tolerance for recovery-rate consistency between the CDS instrument and the
/// hazard curve. Quoted recoveries are typically ≤4 decimals (40 bp resolution),
/// so anything beyond 1e-6 is an actual configuration mismatch.
const RECOVERY_CONSISTENCY_TOL: f64 = 1e-6;

/// Ensure the CDS protection-leg recovery rate matches the recovery used to
/// bootstrap the hazard curve.
///
/// The ISDA Standard Model requires the same R in both legs: λ was bootstrapped
/// under `R_bootstrap`, and the protection-leg multiplier (1 − R_pricing) must
/// agree. If they diverge by more than [`RECOVERY_CONSISTENCY_TOL`], the
/// protection-leg PV is silently mis-scaled. This is a configuration bug, not
/// a legitimate use case, so we surface it as a hard error at the pricer entry.
#[inline]
pub(super) fn validate_recovery_consistency(cds_recovery: f64, surv: &HazardCurve) -> Result<()> {
    let curve_recovery = surv.recovery_rate();
    if (cds_recovery - curve_recovery).abs() > RECOVERY_CONSISTENCY_TOL {
        return Err(Error::Validation(format!(
            "CDS recovery rate ({}) differs from the hazard curve's bootstrap \
             recovery ({}) by more than {:.0e}. The ISDA Standard Model requires \
             the same R in both legs; pricing with mismatched recoveries silently \
             mis-scales the protection leg. Re-bootstrap the curve with the \
             trade's recovery, or use the curve's recovery on the trade.",
            cds_recovery, curve_recovery, RECOVERY_CONSISTENCY_TOL
        )));
    }
    Ok(())
}

// ----- Time-axis helpers -----
//
// These helpers ensure we use the correct day-count conventions:
// - For discounting: use the discount curve's day-count convention
// - For survival: use the hazard curve's day-count convention
// - For accrual: use the instrument's premium leg day-count convention

/// Compute time from hazard curve's base date using its day-count convention.
#[inline]
pub(super) fn haz_t(surv: &HazardCurve, date: Date) -> Result<f64> {
    surv.day_count().year_fraction(
        surv.base_date(),
        date,
        finstack_quant_core::dates::DayCountContext::default(),
    )
}

/// Inverse mapping from hazard-curve time (years) to a calendar date.
///
/// Walks calendar days from `surv.base_date()` until the hazard-curve
/// day-count year-fraction matches `t` to within one calendar day. This is
/// exact for `Act/360`/`Act/365F` (the existing fast path) and *correct* for
/// `30/360`, `30E/360`, `Bus/252`, `ActAct*` — where the previous fixed
/// `days_per_year` inverse drifted by tens of days at multi-year horizons.
///
/// Why it matters: the returned date is then used by `df_asof_to(disc, ...)`
/// on the *discount* curve. When `surv.day_count() != disc.day_count()`, an
/// off-by-N-days inverse mis-attributes the discount lookup, shifting CDS
/// protection-leg PV by tens of dollars per million on cross-currency or
/// mixed-convention setups. See C2 in the calibration code review.
///
/// Convergence: starts from a `365.25`-days-per-year estimate and refines
/// using forward differences on the supplied day-count. Almost always
/// terminates in 1-2 iterations; capped at 30 to bound worst-case cost on
/// non-monotone day-counts (none of the supported conventions exhibit this).
#[inline]
pub(crate) fn date_from_hazard_time(surv: &HazardCurve, t: f64) -> Date {
    let t = t.max(0.0);
    if t == 0.0 {
        return surv.base_date();
    }
    let dc = surv.day_count();
    let base = surv.base_date();

    // Fast paths: Act/360 and Act/365F have exact closed-form inverses.
    match dc {
        DayCount::Act360 => return base + Duration::days((t * 360.0).round() as i64),
        DayCount::Act365F => return base + Duration::days((t * 365.0).round() as i64),
        _ => {}
    }

    // Universal path: start from a generous 365.25 d/y estimate, then
    // refine using forward differences. For 30/360 this typically
    // converges in 1 step; for Bus/252 (which depends on the local holiday
    // calendar via its DayCount impl) it converges in <5.
    let mut date = base + Duration::days((t * 365.25).round() as i64);
    let ctx = finstack_quant_core::dates::DayCountContext::default();
    for _ in 0..30 {
        let Ok(yf) = dc.year_fraction(base, date, ctx) else {
            return date;
        };
        let err_days = ((t - yf) * 365.25).round() as i64;
        if err_days == 0 {
            break;
        }
        date += Duration::days(err_days);
    }
    date
}

/// Resolve settlement date for a default occurring on `default_date`.
///
/// With a holiday calendar the `settlement_delay` business days are applied
/// exactly via [`DateExt::add_business_days`]. Without a calendar the delay
/// is approximated as calendar days (`settlement_delay · 365 /
/// business_days_per_year`); the raw calendar-day jump can land on a
/// weekend, so the result is then rolled forward to the next weekday
/// (`BusinessDayConvention::Following`, weekends only). A settlement date is
/// by definition a business day — the previous implementation could return
/// a Saturday or Sunday.
#[inline]
pub(super) fn settlement_date(
    default_date: Date,
    settlement_delay: u16,
    calendar: Option<&dyn HolidayCalendar>,
    business_days_per_year: f64,
) -> Result<Date> {
    if settlement_delay == 0 {
        return Ok(default_date);
    }

    if let Some(cal) = calendar {
        return default_date.add_business_days(settlement_delay as i32, cal);
    }

    // Fallback: approximate business days into calendar days, then roll the
    // result forward off any weekend so settlement always lands on a weekday.
    let delay_days = ((settlement_delay as f64) * credit::CALENDAR_DAYS_PER_YEAR
        / business_days_per_year)
        .round() as i64;
    let mut settle = default_date + Duration::days(delay_days);
    while settle.is_weekend() {
        settle += Duration::days(1);
    }
    Ok(settle)
}

/// Bloomberg DOCS 2057273 §3 protection-leg integration spec: "the
/// timeline from T to TM is discretized into segments that are
/// sufficiently small to justify constant forward discounting rates and
/// constant hazard rate on each segment (and in no case longer than any
/// accrual period of the premium leg)."
///
/// Default: `25` sub-steps per year (matching FinancePy's
/// `GLOB_NUM_STEPS_PER_YEAR`), giving ~14-day resolution. This is finer
/// than any coupon period (~91 days) and finer than typical
/// discount-curve knot spacings, so within each segment both `r` and `λ`
/// are effectively constant under any reasonable interpolation. Curve
/// knots remain as boundaries so piecewise-constant hazard is honoured.
///
/// Configurable via `CDSPricerConfig::protection_leg_substeps_per_year`
/// — see that field's docs for performance/precision tradeoffs.
pub(crate) const PROTECTION_LEG_SUB_STEPS_PER_YEAR_DEFAULT: f64 = 25.0;

pub(super) fn isda_standard_model_boundaries(
    t_start: f64,
    t_end: f64,
    surv: &HazardCurve,
    disc: &DiscountCurve,
    sub_steps_per_year: f64,
) -> Vec<f64> {
    let mut boundaries = Vec::with_capacity(surv.len() + disc.knots().len() + 2);
    boundaries.push(t_start);
    boundaries.push(t_end);
    boundaries.extend(
        surv.knot_points()
            .map(|(t, _)| t)
            .filter(|&t| t > t_start && t < t_end),
    );
    boundaries.extend(
        disc.knots()
            .iter()
            .copied()
            .filter(|&t| t > t_start && t < t_end),
    );
    // Sub-step subdivision per DOCS 2057273 §3.
    let density = if sub_steps_per_year.is_finite() && sub_steps_per_year > 0.0 {
        sub_steps_per_year
    } else {
        PROTECTION_LEG_SUB_STEPS_PER_YEAR_DEFAULT
    };
    let dt = 1.0 / density;
    let n_steps = ((t_end - t_start) * density).ceil() as usize;
    if n_steps > 0 {
        for i in 1..n_steps {
            let t = t_start + (i as f64) * dt;
            if t > t_start && t < t_end {
                boundaries.push(t);
            }
        }
    }
    // Times come from finite year-fractions on the curve day-counts; NaN here
    // would indicate a corrupt curve and produce silently-wrong PV. Fail fast.
    #[allow(clippy::expect_used)]
    // NaN here implies corrupt curve data; loud failure beats silent drift
    {
        boundaries.sort_by(|a, b| {
            a.partial_cmp(b)
                .expect("hazard/discount knot times must be finite for ISDA boundary integration")
        });
    }
    boundaries.dedup_by(|a, b| (*a - *b).abs() <= numerical::ZERO_TOLERANCE);
    boundaries
}

/// Compute discount factor from as_of to date using curve's time axis.
/// This returns df(date) / df(as_of) = exp(-r*(t_date - t_asof))
#[inline]
pub(super) fn df_asof_to(disc: &DiscountCurve, as_of: Date, date: Date) -> Result<f64> {
    disc.df_between_dates(as_of, date)
}

/// Compute conditional survival probability: S(date | survived to as_of).
/// Returns S(t_date) / S(t_asof) where times are computed using hazard curve's day-count.
///
/// Uses `credit::SURVIVAL_PROBABILITY_FLOOR` to prevent division by near-zero
/// values that could produce inf/NaN results.
#[inline]
pub(super) fn sp_cond_to(surv: &HazardCurve, as_of: Date, date: Date) -> Result<f64> {
    let t_asof = haz_t(surv, as_of)?;
    let t_date = haz_t(surv, date)?;
    let sp_asof = surv.sp(t_asof);
    let sp_date = surv.sp(t_date);
    // Conditional survival: S(date) / S(as_of)
    // Use floor constant to prevent division by near-zero producing inf/NaN
    if sp_asof > credit::SURVIVAL_PROBABILITY_FLOOR {
        Ok(sp_date / sp_asof)
    } else {
        Ok(0.0) // Already defaulted (or effectively defaulted) by as_of
    }
}
