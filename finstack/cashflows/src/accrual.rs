//! Schedule-driven accrued interest.
//!
//! [`accrued_interest_amount`] reads a [`crate::builder::CashFlowSchedule`]
//! only: coupon shape, PIK splits, amortization, draws/repays, and ex-coupon
//! rules are inferred from `CFKind`-tagged flows and the schedule outstanding
//! path, not from instrument specs.

use crate::builder::schedule::CashFlowSchedule;
use crate::primitives::CFKind;
use finstack_core::dates::calendar::calendar_by_id;
use finstack_core::dates::HolidayCalendar;
use finstack_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_core::money::Money;

/// Helper to advance a date by N business days.
///
/// # Performance
///
/// For large shifts (>5 business days), this function uses week-jumping
/// optimization: it advances by full weeks (7 calendar days ≈ 5 business days)
/// to reduce calendar lookups from O(N) to approximately O(N/5) + O(5).
/// This significantly improves performance for long-dated ex-coupon or
/// settlement calculations (e.g., 1 year shift becomes ~52 jumps instead of ~260 lookups).
fn advance_business_days<C: HolidayCalendar + ?Sized>(cal: &C, mut date: Date, days: i32) -> Date {
    if days == 0 {
        return date;
    }

    let forward = days > 0;
    let mut remaining = days.unsigned_abs();

    // Week-jumping optimization for large shifts.
    // A standard week has 5 business days (Mon-Fri), so 7 calendar days ≈ 5 business days.
    // We jump by full weeks to minimize calendar lookups.
    //
    // Note: This assumes a standard Monday-Friday business week. Markets with different
    // conventions (e.g., Sunday-Thursday in Middle East) will see suboptimal performance
    // but still produce correct results since we count actual business days after jumping.
    const BUSINESS_DAYS_PER_WEEK: u32 = 5;
    const CALENDAR_DAYS_PER_WEEK: i64 = 7;

    while remaining >= BUSINESS_DAYS_PER_WEEK {
        // Jump one week in the appropriate direction
        let jump_days = if forward {
            CALENDAR_DAYS_PER_WEEK
        } else {
            -CALENDAR_DAYS_PER_WEEK
        };
        date += time::Duration::days(jump_days);

        // Count actual business days in the week we jumped.
        // This handles weeks with holidays correctly.
        let mut week_business_days = 0u32;
        // For forward: check the 7 days we just traversed (from day after old position to new position)
        // For backward: check from new position to day before old position
        let check_start = if forward {
            date + time::Duration::days(-CALENDAR_DAYS_PER_WEEK + 1)
        } else {
            date
        };

        for i in 0..CALENDAR_DAYS_PER_WEEK {
            let check_date = check_start + time::Duration::days(i);
            if cal.is_business_day(check_date) {
                week_business_days += 1;
            }
        }

        // Guard: if no business days in the week (pathological calendar with 7+ consecutive
        // holidays), revert the jump and fall through to step-by-step iteration.
        if week_business_days == 0 {
            date += time::Duration::days(-jump_days);
            break;
        }

        // Deduct the business days we actually traversed
        remaining = remaining.saturating_sub(week_business_days);
    }

    // Handle remaining days with step-by-step iteration (at most 4 business days)
    let step = if forward { 1i64 } else { -1i64 };
    while remaining > 0 {
        date += time::Duration::days(step);
        if cal.is_business_day(date) {
            remaining -= 1;
        }
    }

    date
}

/// Generic accrual method usable across instruments.
///
/// This mirrors the semantics of bond accrual methods but is defined at the
/// cashflow layer so it can be reused by any instrument that exposes a
/// `CashFlowSchedule`.
#[derive(
    Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum AccrualMethod {
    /// Linear accrual (simple interest interpolation).
    ///
    /// `Accrued = Coupon × (elapsed / period)`
    #[default]
    Linear,

    /// Compounded accrual.
    ///
    /// `Accrued = N × expm1(f × ln1p(r))`
    ///
    /// which is the numerically stable form of
    /// `N × [(1 + r)^f − 1]`, where `r = coupon_amount / notional`
    /// and `f = elapsed / period` (time fraction within the current
    /// coupon period).
    ///
    /// **Note:** ICMA Rule 251.1 prescribes *linear* accrual for bond
    /// AI calculations. This variant uses true exponential compounding
    /// and should not be cited as ICMA-style.
    Compounded,
}

/// Ex-coupon convention applied to coupon flows.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExCouponRule {
    /// Number of days before coupon date that go ex.
    pub days_before_coupon: u32,
    /// Optional calendar ID for business day calculation.
    ///
    /// - `Some(id)`: Subtract N business days from payment date.
    /// - `None`: Subtract N calendar days from payment date.
    pub calendar_id: Option<String>,
}

impl ExCouponRule {
    /// Ex-coupon date for a coupon paid on `payment_date`.
    ///
    /// From this date (inclusive) until the payment date (exclusive), the bond
    /// trades ex-coupon: the seller keeps the coupon and accrued interest is
    /// negative.
    ///
    /// # Errors
    ///
    /// Returns an error when `calendar_id` is set but cannot be resolved.
    pub fn ex_date(&self, payment_date: Date) -> finstack_core::Result<Date> {
        if let Some(cal_id) = &self.calendar_id {
            let cal = calendar_by_id(cal_id).ok_or_else(|| {
                finstack_core::Error::Input(finstack_core::InputError::NotFound {
                    id: cal_id.clone(),
                })
            })?;
            Ok(advance_business_days(
                cal,
                payment_date,
                -(self.days_before_coupon as i32),
            ))
        } else {
            Ok(payment_date - time::Duration::days(self.days_before_coupon as i64))
        }
    }
}

/// Generic configuration for schedule-driven interest accrual.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AccrualConfig {
    /// Accrual method (Linear or Compounded).
    pub method: AccrualMethod,
    /// Optional ex-coupon rule applied to coupon dates.
    pub ex_coupon: Option<ExCouponRule>,
    /// Whether to include PIK interest in the accrued amount.
    pub include_pik: bool,
    /// Coupon frequency — required for ACT/ACT ISMA day count.
    ///
    /// When `None` and the schedule uses ACT/ACT ISMA, the year fraction
    /// falls back to ACT/ACT ISDA semantics, which gives incorrect accrued
    /// interest for most government bonds.
    pub frequency: Option<Tenor>,
}

impl Default for AccrualConfig {
    fn default() -> Self {
        Self {
            method: AccrualMethod::Linear,
            ex_coupon: None,
            include_pik: true,
            frequency: None,
        }
    }
}

/// Compute accrued interest as a scalar amount from a cashflow schedule.
///
/// The returned `f64` is expressed in the same currency space as the schedule's
/// coupon and notional amounts. Callers that need the currency can recover it
/// from `schedule.notional.initial.currency()` or by inspecting the underlying
/// schedule flows.
///
/// # Arguments
///
/// * `schedule` - Canonical cashflow schedule containing coupon, PIK, and
///   notional flows.
/// * `as_of` - Accrual cut-off date. Dates outside all coupon periods return
///   zero accrued interest.
/// * `cfg` - Accrual method and ex-coupon configuration.
///
/// # Returns
///
/// Scalar accrued interest amount in the schedule's currency space. Returns
/// `0.0` when the schedule has no coupon periods or the `as_of` date is
/// outside all coupon periods. When the `as_of` date falls inside an active
/// ex-coupon window the accrued interest is **negative** (the seller keeps
/// the coupon; the buyer is compensated for the remaining stub).
///
/// # Errors
///
/// Returns an error if:
///
/// - the schedule's outstanding-balance path cannot be constructed
/// - a required day-count calculation fails
/// - an ex-coupon calendar ID is configured but cannot be resolved
///
/// # Examples
///
/// ```ignore
/// use finstack_cashflows::builder::CashFlowSchedule;
/// use finstack_cashflows::{accrued_interest_amount, AccrualConfig, AccrualMethod};
/// use finstack_core::dates::Date;
///
/// fn accrued_as_of(
///     schedule: &CashFlowSchedule,
///     as_of: Date,
/// ) -> finstack_core::Result<f64> {
///     accrued_interest_amount(
///         schedule,
///         as_of,
///         &AccrualConfig {
///             method: AccrualMethod::Linear,
///             ..Default::default()
///         },
///     )
/// }
/// ```
pub fn accrued_interest_amount(
    schedule: &CashFlowSchedule,
    as_of: Date,
    cfg: &AccrualConfig,
) -> finstack_core::Result<f64> {
    let periods = build_coupon_periods(schedule, cfg)?;
    if periods.is_empty() {
        return Ok(0.0);
    }

    // Build outstanding path including notional draws/repays and PIK.
    let outstanding_path = schedule.outstanding_by_date()?;
    let period_inputs = build_period_inputs(schedule, &periods, &outstanding_path, cfg.frequency)?;

    // Locate active period and compute accrued in that period.
    if let Some((inputs, elapsed_yf)) =
        find_active_period_and_elapsed(&period_inputs, as_of, schedule.day_count, cfg)?
    {
        accrue_in_period(inputs, elapsed_yf, &cfg.method)
    } else {
        Ok(0.0)
    }
}

/// Aggregated coupon information for a single payment date.
#[derive(Debug, Clone)]
struct CouponBucket {
    date: Date,
    cash_amount: f64,
    pik_amount: f64,
    /// Accrual year fraction as reported by the builder.
    ///
    /// None means no builder-provided accrual factor; downstream falls back
    /// to a day-count-based year fraction. Some(af) means the builder
    /// explicitly set af. We use Option rather than 0.0 as a sentinel so a
    /// legitimate zero-length period is distinguishable from unset.
    accrual_factor: Option<f64>,
    rate: Option<f64>,
}

/// A single coupon period derived from the schedule.
#[derive(Debug, Clone)]
struct CouponPeriod {
    start: Date,
    end: Date,
    dc: DayCount,
    bucket: CouponBucket,
}

/// Inputs required to apply the accrual formula for a single period.
#[derive(Debug, Clone)]
struct PeriodInputs {
    start: Date,
    end: Date,
    notional_start: f64,
    coupon_total: f64,
    total_yf: f64,
}

/// Check if a cashflow kind is a coupon that should be included in accrual.
fn is_coupon_kind(kind: CFKind, include_pik: bool) -> bool {
    kind.is_interest_like() || (include_pik && kind == CFKind::PIK)
}

fn derive_horizon_start(
    schedule: &CashFlowSchedule,
    first_bucket: &CouponBucket,
) -> finstack_core::Result<Date> {
    if let Some(issue) = schedule.meta.issue_date {
        return Ok(issue);
    }

    if let Some(min_date) = schedule.dates().into_iter().min() {
        if min_date < first_bucket.date {
            return Ok(min_date);
        }
    }

    Err(finstack_core::Error::Validation(format!(
        "accrual: schedule.meta.issue_date is unset and no flow precedes the first coupon \
         date {}; set meta.issue_date on the CashFlowSchedule.",
        first_bucket.date
    )))
}

/// Build coupon buckets grouped by date from the schedule.
fn build_coupon_periods(
    schedule: &CashFlowSchedule,
    cfg: &AccrualConfig,
) -> finstack_core::Result<Vec<CouponPeriod>> {
    // Same-date coupon merging depends on date-ordered input.
    let mut coupon_idx: Vec<usize> = schedule
        .flows
        .iter()
        .enumerate()
        .filter(|(_, cf)| is_coupon_kind(cf.kind, cfg.include_pik))
        .map(|(i, _)| i)
        .collect();
    if !coupon_idx
        .windows(2)
        .all(|w| schedule.flows[w[0]].date <= schedule.flows[w[1]].date)
    {
        coupon_idx.sort_by_key(|&i| schedule.flows[i].date);
    }
    debug_assert!(
        coupon_idx
            .windows(2)
            .all(|w| schedule.flows[w[0]].date <= schedule.flows[w[1]].date),
        "coupon flows must preserve schedule date order"
    );

    let mut buckets: Vec<CouponBucket> = Vec::new();

    // Cash and PIK coupon flows are grouped by payment date.
    for &i in &coupon_idx {
        let cf = &schedule.flows[i];

        let cf_af = (cf.accrual_factor > 0.0).then_some(cf.accrual_factor);

        if let Some(last) = buckets.last_mut() {
            if last.date == cf.date {
                if cf.kind == CFKind::PIK {
                    last.pik_amount += cf.amount.amount();
                } else {
                    last.cash_amount += cf.amount.amount();
                    if last.accrual_factor.is_none() {
                        last.accrual_factor = cf_af;
                    }
                    if last.rate.is_none() {
                        last.rate = cf.rate;
                    }
                }
                continue;
            }
        }

        buckets.push(if cf.kind == CFKind::PIK {
            CouponBucket {
                date: cf.date,
                cash_amount: 0.0,
                pik_amount: cf.amount.amount(),
                accrual_factor: None,
                rate: None,
            }
        } else {
            CouponBucket {
                date: cf.date,
                cash_amount: cf.amount.amount(),
                pik_amount: 0.0,
                accrual_factor: cf_af,
                rate: cf.rate,
            }
        });
    }

    if buckets.is_empty() {
        return Ok(Vec::new());
    }

    let dc = schedule.day_count;

    // Derive the start of the first coupon period (issue date).
    //
    // Strategy (in priority order):
    // 1. If `meta.issue_date` is set, use it directly (most accurate).
    // 2. If schedule.dates().min() differs from the first coupon date, use that
    //    (this handles cases where issue date flow exists in the schedule).
    // 3. Otherwise, fail with an explicit issue-date error. The legacy inverse
    //    day-count approximation is intentionally no longer used.
    let first_bucket = &buckets[0];
    let horizon_start = derive_horizon_start(schedule, first_bucket)?;

    let mut prev = horizon_start;

    let mut periods = Vec::with_capacity(buckets.len());
    for bucket in buckets {
        let start = prev;
        let end = bucket.date;
        if start < end {
            periods.push(CouponPeriod {
                start,
                end,
                dc,
                bucket,
            });
            prev = end;
        } else {
            // Skip degenerate periods (e.g., duplicated dates).
            prev = end;
        }
    }

    Ok(periods)
}

/// Build period inputs (including notional at start-of-period) from coupon periods
/// and the outstanding path.
///
/// # Notional Lookup
///
/// For each period, we find the outstanding balance at the period start date.
/// This is the correct base for compounded accrual calculations since it
/// represents the principal on which interest accrues during the period.
fn build_period_inputs(
    schedule: &CashFlowSchedule,
    periods: &[CouponPeriod],
    outstanding_path: &[(Date, Money)],
    frequency: Option<Tenor>,
) -> finstack_core::Result<Vec<PeriodInputs>> {
    let mut result = Vec::with_capacity(periods.len());

    for p in periods {
        // Find the outstanding at period start: the latest entry on or before p.start.
        // outstanding_path is sorted by date (guaranteed by CashFlowSchedule construction),
        // so partition_point gives us O(log n) binary search instead of O(n) linear scan.
        //
        //   partition_point(|d| d <= p.start)  →  first index where d > p.start
        //   idx - 1                            →  last index where d <= p.start
        let idx = outstanding_path.partition_point(|(d, _)| *d <= p.start);
        let notional_start = if idx > 0 {
            outstanding_path[idx - 1].1.amount()
        } else {
            schedule.notional.initial.amount()
        };

        let coupon_total = p.bucket.cash_amount + p.bucket.pik_amount;

        if coupon_total == 0.0 {
            // No coupon in this period; skip.
            continue;
        }

        // Prefer accrual_factor from builder when present; otherwise derive via day count.
        let total_yf = match p.bucket.accrual_factor {
            Some(af) if af > 0.0 => af,
            _ => {
                let ctx = DayCountContext {
                    frequency,
                    ..Default::default()
                };
                p.dc.year_fraction(p.start, p.end, ctx)?
            }
        };

        if total_yf <= 0.0 {
            continue;
        }

        result.push(PeriodInputs {
            start: p.start,
            end: p.end,
            notional_start,
            coupon_total,
            total_yf,
        });
    }

    Ok(result)
}

/// Locate the active period for `as_of` and compute elapsed year fraction.
///
/// # Ex-Coupon Handling
///
/// If an ex-coupon rule is configured and the `as_of` date falls within the
/// ex-coupon window (between ex-date and payment date), the elapsed year
/// fraction is returned as `elapsed - period` (negative). Market standard
/// (e.g. UK gilts): the buyer does not receive the imminent coupon, so the
/// seller compensates the buyer for the remaining stub via **negative
/// accrued interest**.
fn find_active_period_and_elapsed<'a>(
    periods: &'a [PeriodInputs],
    as_of: Date,
    dc: DayCount,
    cfg: &AccrualConfig,
) -> finstack_core::Result<Option<(&'a PeriodInputs, f64)>> {
    for inputs in periods {
        if inputs.start <= as_of && as_of < inputs.end {
            let dc_ctx = DayCountContext {
                frequency: cfg.frequency,
                ..Default::default()
            };
            let elapsed = dc.year_fraction(inputs.start, as_of, dc_ctx)?.max(0.0);

            // Apply ex-coupon convention if present: inside the ex-window the
            // accrual flips to a negative stub from `as_of` to the period end.
            if let Some(ref ex) = cfg.ex_coupon {
                let ex_date = ex.ex_date(inputs.end)?;
                if as_of >= ex_date && as_of < inputs.end {
                    return Ok(Some((inputs, elapsed - inputs.total_yf)));
                }
            }

            return Ok(Some((inputs, elapsed)));
        }
    }

    Ok(None)
}

/// Apply the chosen accrual method to a single period.
///
/// # Compounded Accrual
///
/// Uses the numerically stable formula: `(1+r)^f - 1 = expm1(f * ln1p(r))`
///
/// This approach:
/// - Avoids precision loss for small `r` via `ln1p` (log(1+r) accurate near 0)
/// - Avoids precision loss for small results via `expm1` (exp(x)-1 accurate near 0)
/// - Works correctly across all fraction values without threshold switching
///
/// The compounded accrual formula computes:
///
/// `Accrued = Notional × [(1 + period_rate)^(elapsed/period) - 1]`
///
/// where `period_rate = coupon_amount / notional` is the yield per coupon period.
///
/// Note: ICMA Rule 251.1 prescribes *linear* interpolation for accrued interest.
/// This function's compounded variant is used for instruments that genuinely
/// compound within a period (e.g. some leveraged loans); it is not the ICMA method.
fn accrue_in_period(
    inputs: &PeriodInputs,
    elapsed_yf: f64,
    method: &AccrualMethod,
) -> finstack_core::Result<f64> {
    if inputs.total_yf <= 0.0 || elapsed_yf < -inputs.total_yf {
        return Ok(0.0);
    }

    match method {
        AccrualMethod::Linear => Ok(inputs.coupon_total * (elapsed_yf / inputs.total_yf)),
        AccrualMethod::Compounded => {
            let notional = inputs.notional_start;
            if notional <= 0.0 {
                return Ok(0.0);
            }

            let period_rate = inputs.coupon_total / notional;
            if period_rate.abs() < 1e-12 {
                // Zero-coupon or near-zero rate: fall back to linear.
                return Ok(inputs.coupon_total * (elapsed_yf / inputs.total_yf));
            }

            let fraction = elapsed_yf / inputs.total_yf;

            // Numerically stable computation: (1+r)^f - 1 = expm1(f * ln1p(r))
            // This avoids precision loss for both small rates and small fractions.
            let compound_growth = (fraction * period_rate.ln_1p()).exp_m1();

            Ok(notional * compound_growth)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{CashFlowSchedule, Notional};
    use finstack_core::cashflow::CashFlow;
    use finstack_core::currency::Currency;
    use time::Month;

    fn make_date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
    }

    /// Create a minimal schedule with coupon flows for testing issue date derivation.
    fn make_test_schedule(
        coupon_dates: &[(Date, f64)], // (date, accrual_factor)
        day_count: DayCount,
    ) -> CashFlowSchedule {
        let flows: Vec<CashFlow> = coupon_dates
            .iter()
            .map(|(date, af)| CashFlow {
                date: *date,
                amount: Money::new(25000.0, Currency::USD), // $25k coupon
                kind: CFKind::Fixed,
                accrual_factor: *af,
                rate: Some(0.05),
                reset_date: None,
            })
            .collect();

        CashFlowSchedule {
            flows,
            notional: Notional::par(1_000_000.0, Currency::USD),
            day_count,
            meta: Default::default(),
        }
    }

    // =========================================================================
    // Issue date derivation tests
    //
    // Note: The inverse day count approximation may produce dates that differ
    // by 1-2 days from the true issue date. This is acceptable for establishing
    // coupon period boundaries for accrual calculations, as the error is small
    // relative to the coupon period length.
    // =========================================================================

    #[test]
    fn test_missing_issue_date_errors() {
        let schedule = make_test_schedule(
            &[(make_date(2025, 7, 1), 0.5), (make_date(2026, 1, 1), 0.5)],
            DayCount::Thirty360,
        );

        let cfg = AccrualConfig::default();
        let err = build_coupon_periods(&schedule, &cfg).expect_err("missing issue date errors");

        assert!(err.to_string().contains("issue_date"));
    }

    #[test]
    fn test_issue_date_from_schedule_when_present() {
        // When schedule includes a flow before the first coupon (e.g., notional),
        // that date should be used instead of derivation
        let mut schedule = make_test_schedule(
            &[(make_date(2025, 7, 1), 0.5), (make_date(2026, 1, 1), 0.5)],
            DayCount::Thirty360,
        );

        // Add a notional flow on issue date
        schedule.flows.insert(
            0,
            CashFlow {
                date: make_date(2025, 1, 15), // Issue date (different from derived Jan 1)
                amount: Money::new(-1_000_000.0, Currency::USD),
                kind: CFKind::Notional,
                accrual_factor: 0.0,
                rate: None,
                reset_date: None,
            },
        );

        let cfg = AccrualConfig::default();
        let periods = build_coupon_periods(&schedule, &cfg).expect("periods");

        assert!(!periods.is_empty());
        let first_period = &periods[0];

        // Should use the explicit issue date from the schedule, not derived
        assert_eq!(
            first_period.start,
            make_date(2025, 1, 15),
            "Should use explicit issue date from schedule"
        );
    }

    #[test]
    fn build_coupon_periods_sorts_only_when_schedule_coupons_are_unsorted() {
        let mut schedule = make_test_schedule(
            &[(make_date(2026, 1, 1), 0.5), (make_date(2025, 7, 1), 0.5)],
            DayCount::Thirty360,
        );
        schedule.meta.issue_date = Some(make_date(2025, 1, 1));

        let periods = build_coupon_periods(&schedule, &AccrualConfig::default()).expect("periods");

        assert_eq!(periods.len(), 2);
        assert_eq!(periods[0].end, make_date(2025, 7, 1));
        assert_eq!(periods[1].end, make_date(2026, 1, 1));
    }

    #[test]
    fn test_accrued_interest_uses_explicit_issue_date() {
        // Integration test: accrued interest requires explicit issue metadata
        // when outstanding balances are computed from the schedule.
        let mut schedule = make_test_schedule(
            &[
                (make_date(2025, 7, 1), 0.5), // First coupon July 1
                (make_date(2026, 1, 1), 0.5), // Second coupon
            ],
            DayCount::Thirty360,
        );
        schedule.meta.issue_date = Some(make_date(2025, 1, 1));

        // Calculate accrued at April 1 (halfway through first period)
        let as_of = make_date(2025, 4, 1);
        let accrued = accrued_interest_amount(&schedule, as_of, &AccrualConfig::default()).unwrap();

        // With explicit issue date Jan 1 and first coupon July 1:
        // - Period length: 180 days (30/360)
        // - Elapsed: 90 days (Jan 1 to Apr 1)
        // - Fraction: 90/180 = 0.5
        // - Accrued: $25,000 × 0.5 = $12,500
        assert!(
            accrued > 12_000.0 && accrued < 13_000.0,
            "Accrued should be approximately $12,500, got {}",
            accrued
        );
    }
}
