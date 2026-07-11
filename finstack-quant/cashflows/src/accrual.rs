//! Schedule-driven accrued interest.
//!
//! [`accrued_interest_amount`] reads a [`crate::builder::CashFlowSchedule`]
//! only: coupon shape, PIK splits, amortization, draws/repays, and ex-coupon
//! rules are inferred from `CFKind`-tagged flows and the schedule outstanding
//! path, not from instrument specs.

use crate::builder::schedule::CashFlowSchedule;
use crate::primitives::CFKind;
use finstack_quant_core::dates::calendar::calendar_by_id;
use finstack_quant_core::dates::HolidayCalendar;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::money::Money;

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
    // Note: The jump heuristic assumes at most 5 business days per calendar week.
    // For calendars where a week could contain more business days than `remaining`
    // (e.g., a hypothetical 6-business-day week), a full-week jump would overshoot;
    // the guard below reverts such jumps and falls back to step-wise iteration so
    // the result stays exact for any calendar.
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

        // Guard: revert the jump and fall through to step-by-step iteration when
        // - the week contains no business days (pathological calendar with 7+
        //   consecutive holidays), or
        // - the week contains more business days than `remaining` (non-standard
        //   calendars with >5 business days per week), which would overshoot.
        if week_business_days == 0 || week_business_days > remaining {
            date += time::Duration::days(-jump_days);
            break;
        }

        // Deduct the business days we actually traversed
        remaining = remaining.saturating_sub(week_business_days);
    }

    // Handle remaining days with step-by-step iteration (at most 4 business days
    // for standard Mon-Fri calendars; more when a week-jump guard triggered above).
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
    /// and should not be cited as ICMA-style. It is intended for
    /// instruments that genuinely compound within a coupon period
    /// (e.g. some leveraged loans).
    ///
    /// **Ex-coupon window convention:** inside an ex-coupon window the
    /// accrued interest is the negative rebate of the *remaining* stub,
    /// compounded on the same basis:
    ///
    /// `Accrued = −N × expm1((1 − f) × ln1p(r))`
    ///
    /// where `f = elapsed / period`. ICMA Rule 251.1 (and the UK DMO
    /// gilt convention) prescribe a *linear* ex-coupon rebate
    /// (`−C × (1 − f)`); use [`AccrualMethod::Linear`] for those markets.
    Compounded,
}

/// Maximum accepted `days_before_coupon` for an [`ExCouponRule`].
///
/// Real-world ex-coupon periods are a handful of days (e.g. 7 business days
/// for UK gilts); anything beyond a full year (366 days) is treated as a
/// configuration error rather than silently producing an ex-date before the
/// coupon period even starts.
const MAX_EX_COUPON_DAYS: u32 = 366;

/// Ex-coupon convention applied to coupon flows.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExCouponRule {
    /// Number of days before coupon date that go ex.
    ///
    /// Values greater than 366 are rejected by [`ExCouponRule::ex_date`].
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
    /// Returns an error when:
    ///
    /// - `days_before_coupon` exceeds 366 (a configuration error — see
    ///   `MAX_EX_COUPON_DAYS`)
    /// - `calendar_id` is set but cannot be resolved
    pub fn ex_date(&self, payment_date: Date) -> finstack_quant_core::Result<Date> {
        if self.days_before_coupon > MAX_EX_COUPON_DAYS {
            return Err(finstack_quant_core::Error::Validation(format!(
                "ExCouponRule: days_before_coupon {} exceeds the maximum of {} days",
                self.days_before_coupon, MAX_EX_COUPON_DAYS
            )));
        }
        let days = i32::try_from(self.days_before_coupon).map_err(|_| {
            finstack_quant_core::Error::Validation(format!(
                "ExCouponRule: days_before_coupon {} does not fit in i32",
                self.days_before_coupon
            ))
        })?;
        if let Some(cal_id) = &self.calendar_id {
            let cal = calendar_by_id(cal_id).ok_or_else(|| {
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                    id: cal_id.clone(),
                })
            })?;
            Ok(advance_business_days(cal, payment_date, -days))
        } else {
            Ok(payment_date - time::Duration::days(i64::from(days)))
        }
    }
}

/// Generic configuration for schedule-driven interest accrual.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AccrualConfig {
    /// Accrual method (Linear or Compounded).
    pub method: AccrualMethod,
    /// Optional ex-coupon rule applied to coupon dates.
    pub ex_coupon: Option<ExCouponRule>,
    /// Whether to include PIK interest in the accrued amount.
    pub include_pik: bool,
    /// Coupon frequency — required for ACT/ACT ISMA day count.
    ///
    /// When `None` and the schedule uses ACT/ACT ISMA, year-fraction
    /// calculations return
    /// [`InputError::MissingFrequencyForActActIsma`](finstack_quant_core::InputError::MissingFrequencyForActActIsma);
    /// there is no fallback to ISDA semantics.
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
/// use finstack_quant_cashflows::builder::CashFlowSchedule;
/// use finstack_quant_cashflows::{accrued_interest_amount, AccrualConfig, AccrualMethod};
/// use finstack_quant_core::dates::Date;
///
/// fn accrued_as_of(
///     schedule: &CashFlowSchedule,
///     as_of: Date,
/// ) -> finstack_quant_core::Result<f64> {
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
) -> finstack_quant_core::Result<f64> {
    schedule.validate()?;
    let expected_currency = schedule.notional.initial.currency();
    for flow in &schedule.flows {
        if is_coupon_kind(flow.kind, cfg.include_pik) && flow.amount.currency() != expected_currency
        {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: expected_currency,
                actual: flow.amount.currency(),
            });
        }
    }
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
    accrual_start: Option<Date>,
    accrual_end: Option<Date>,
    /// Day-count convention attached to the underlying coupon flow(s).
    /// `None` means metadata was absent or same-date flows used conflicting
    /// conventions, so the schedule representative convention is used.
    accrual_day_count: Option<DayCount>,
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
) -> finstack_quant_core::Result<Date> {
    // The issue date is required unconditionally. Falling back to the earliest
    // flow date is unsafe: a pre-issue flow (e.g. an upfront fee) would
    // silently become the accrual start and lengthen the first coupon period.
    schedule.meta.issue_date.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "accrual: schedule.meta.issue_date is unset; the start of the first coupon \
             period (first coupon date {}) cannot be inferred from flow dates because \
             pre-issue flows would silently shift the accrual start. Set meta.issue_date \
             on the CashFlowSchedule.",
            first_bucket.date
        ))
    })
}

/// Build coupon buckets grouped by date from the schedule.
fn build_coupon_periods(
    schedule: &CashFlowSchedule,
    cfg: &AccrualConfig,
) -> finstack_quant_core::Result<Vec<CouponPeriod>> {
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
        let true_period = schedule.meta.accrual_periods.get(i).copied().flatten();
        let flow_day_count = schedule.meta.accrual_day_counts.get(i).copied().flatten();

        let cf_af = if cf.accrual_factor > 0.0 {
            if !cf.accrual_factor.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "accrual: non-finite accrual_factor {} for coupon flow dated {}",
                    cf.accrual_factor, cf.date
                )));
            }
            Some(cf.accrual_factor)
        } else {
            if !cf.accrual_factor.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "accrual: non-finite accrual_factor {} for coupon flow dated {}",
                    cf.accrual_factor, cf.date
                )));
            }
            None
        };

        if let Some(last) = buckets.last_mut() {
            if last.date == cf.date {
                if cf.kind == CFKind::PIK {
                    last.pik_amount += cf.amount.amount();
                } else {
                    last.cash_amount += cf.amount.amount();
                    if last.accrual_start.is_none() {
                        last.accrual_start = true_period.map(|(start, _)| start);
                    }
                    if last.accrual_end.is_none() {
                        last.accrual_end = true_period.map(|(_, end)| end);
                    }
                    if let Some(flow_dc) = flow_day_count {
                        match last.accrual_day_count {
                            None => last.accrual_day_count = Some(flow_dc),
                            Some(existing) if existing != flow_dc => {
                                last.accrual_day_count = None;
                            }
                            _ => {}
                        }
                    }
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
                accrual_start: true_period.map(|(start, _)| start),
                accrual_end: true_period.map(|(_, end)| end),
                accrual_day_count: flow_day_count,
                cash_amount: 0.0,
                pik_amount: cf.amount.amount(),
                accrual_factor: None,
                rate: None,
            }
        } else {
            CouponBucket {
                date: cf.date,
                accrual_start: true_period.map(|(start, _)| start),
                accrual_end: true_period.map(|(_, end)| end),
                accrual_day_count: flow_day_count,
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

    // Derive the start of the first coupon period: `meta.issue_date` is
    // required. Inferring it from flow dates or via the legacy inverse
    // day-count approximation is intentionally not supported.
    let first_bucket = &buckets[0];
    let horizon_start = derive_horizon_start(schedule, first_bucket)?;

    let mut prev = horizon_start;

    let mut periods = Vec::with_capacity(buckets.len());
    for bucket in buckets {
        let start = bucket.accrual_start.unwrap_or(prev);
        let end = bucket.accrual_end.unwrap_or(bucket.date);
        if start < end {
            periods.push(CouponPeriod {
                start,
                end,
                dc: bucket.accrual_day_count.unwrap_or(schedule.day_count),
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
) -> finstack_quant_core::Result<Vec<PeriodInputs>> {
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

        if !coupon_total.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "accrual: non-finite coupon total {coupon_total} for period ending {}",
                p.end
            )));
        }

        if coupon_total == 0.0 {
            // No coupon in this period; skip.
            continue;
        }

        // Prefer accrual_factor from builder when present; otherwise derive via day count.
        let total_yf = match p.bucket.accrual_factor {
            Some(af) if af.is_finite() && af > 0.0 => af,
            Some(af) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "accrual: non-finite or non-positive accrual_factor {af} for period ending {}",
                    p.end
                )));
            }
            _ => {
                let ctx = DayCountContext {
                    frequency,
                    // ACT/ACT ICMA: pass the actual coupon period so irregular
                    // (stub) periods use core's reference-period subdivision
                    // instead of re-anchoring a quasi-coupon grid from `p.start`.
                    // Other conventions ignore this field.
                    coupon_period: Some((p.start, p.end)),
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
/// # Year-Fraction Basis Consistency
///
/// `total_yf` may come from the builder's `accrual_factor`, which is computed
/// over the *true accrual period*, while periods here are reconstructed from
/// *payment dates*. With a payment lag or BDC-shifted period ends the two
/// bases diverge and the raw day-count `elapsed` can exceed `total_yf` before
/// the payment date (accrued exceeding the full coupon). To keep both numbers
/// on the same basis, when `elapsed` exceeds `total_yf` it is rescaled by
/// `total_yf × dc_elapsed / dc_total` (where `dc_total` is the day-count
/// fraction over the same payment-date boundaries) and then clamped to
/// `[0, total_yf]`, so accrued interest never exceeds the full coupon.
///
/// # Ex-Coupon Handling
///
/// If an ex-coupon rule is configured and the `as_of` date falls within the
/// ex-coupon window (between ex-date and payment date), the elapsed year
/// fraction is returned as `elapsed - period`, clamped to `≤ 0` (negative).
/// Market standard (e.g. UK gilts): the buyer does not receive the imminent
/// coupon, so the seller compensates the buyer for the remaining stub via
/// **negative accrued interest**.
fn find_active_period_and_elapsed<'a>(
    periods: &'a [PeriodInputs],
    as_of: Date,
    dc: DayCount,
    cfg: &AccrualConfig,
) -> finstack_quant_core::Result<Option<(&'a PeriodInputs, f64)>> {
    for inputs in periods {
        if inputs.start <= as_of && as_of < inputs.end {
            let dc_ctx = DayCountContext {
                frequency: cfg.frequency,
                // ACT/ACT ICMA: anchor on the actual coupon period (see
                // `build_period_inputs`). Other conventions ignore this field.
                coupon_period: Some((inputs.start, inputs.end)),
                ..Default::default()
            };
            let dc_elapsed = dc.year_fraction(inputs.start, as_of, dc_ctx)?.max(0.0);

            // Rescale onto the `total_yf` basis under a single day-count
            // context so stub reference-period choices cancel instead of
            // mixing builder and elapsed bases.
            let dc_total = dc.year_fraction(inputs.start, inputs.end, dc_ctx)?;
            let elapsed = if dc_total.is_finite() && dc_total > 0.0 {
                inputs.total_yf * dc_elapsed / dc_total
            } else {
                dc_elapsed
            };
            let elapsed = elapsed.clamp(0.0, inputs.total_yf);

            // Apply ex-coupon convention if present: inside the ex-window the
            // accrual flips to a negative stub from `as_of` to the period end.
            // The clamp to `≤ 0` guarantees the stub can never flip positive
            // even under residual basis mismatch.
            if let Some(ref ex) = cfg.ex_coupon {
                let ex_date = ex.ex_date(inputs.end)?;
                if ex_date <= inputs.start {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "ex-coupon date {ex_date} must fall after active period start {}",
                        inputs.start
                    )));
                }
                if as_of >= ex_date && as_of < inputs.end {
                    return Ok(Some((inputs, (elapsed - inputs.total_yf).min(0.0))));
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
/// # Ex-Coupon Window (negative `elapsed_yf`)
///
/// Inside an ex-coupon window the caller passes `elapsed_yf = elapsed − period`
/// (negative). The compounded stub rebate is computed explicitly as
///
/// `Accrued = −Notional × expm1((1 − f) × ln1p(period_rate))`
///
/// where `f = elapsed/period`, i.e. the negative of the compounded value of
/// the *remaining* stub. ICMA Rule 251.1 (and the UK DMO gilt convention)
/// prescribe a linear rebate; this compounded variant is for instruments
/// that genuinely compound within a period.
///
/// Note: ICMA Rule 251.1 prescribes *linear* interpolation for accrued interest.
/// This function's compounded variant is used for instruments that genuinely
/// compound within a period (e.g. some leveraged loans); it is not the ICMA method.
fn accrue_in_period(
    inputs: &PeriodInputs,
    elapsed_yf: f64,
    method: &AccrualMethod,
) -> finstack_quant_core::Result<f64> {
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

            if fraction < 0.0 {
                // Ex-coupon window: `elapsed_yf = elapsed − period`, so
                // `−fraction = 1 − f` where `f = elapsed/period`. The accrued
                // is the negative rebate of the remaining stub, compounded:
                // `−N × [(1 + r)^(1−f) − 1]` (see function docs).
                let stub_growth = (-fraction * period_rate.ln_1p()).exp_m1();
                return Ok(-(notional * stub_growth));
            }

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
    use finstack_quant_core::cashflow::CashFlow;
    use finstack_quant_core::currency::Currency;
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
    // Issue date requirement tests
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
    fn test_pre_coupon_flow_does_not_substitute_for_issue_date() {
        // A flow dated before the first coupon (e.g. a pre-issue fee or a
        // funding leg) must NOT silently become the accrual start;
        // meta.issue_date is required unconditionally.
        let mut schedule = make_test_schedule(
            &[(make_date(2025, 7, 1), 0.5), (make_date(2026, 1, 1), 0.5)],
            DayCount::Thirty360,
        );

        schedule.flows.insert(
            0,
            CashFlow {
                date: make_date(2025, 1, 15),
                amount: Money::new(-1_000_000.0, Currency::USD),
                kind: CFKind::Notional,
                accrual_factor: 0.0,
                rate: None,
                reset_date: None,
            },
        );

        let cfg = AccrualConfig::default();
        let err = build_coupon_periods(&schedule, &cfg)
            .expect_err("issue_date must be required even when a pre-coupon flow exists");
        assert!(err.to_string().contains("issue_date"));

        // With the explicit issue date set, the first period starts there.
        schedule.meta.issue_date = Some(make_date(2025, 1, 15));
        let periods = build_coupon_periods(&schedule, &cfg).expect("periods");
        assert!(!periods.is_empty());
        assert_eq!(
            periods[0].start,
            make_date(2025, 1, 15),
            "Should use explicit meta.issue_date"
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

    // =========================================================================
    // Ex-coupon window tests (negative accrued interest, UK gilt convention)
    // =========================================================================

    /// Semiannual 30/360 schedule: issue 2025-01-01, coupons 2025-07-01 and
    /// 2026-01-01, $25k per coupon, builder accrual factor 0.5.
    fn ex_coupon_test_schedule() -> CashFlowSchedule {
        let mut schedule = make_test_schedule(
            &[(make_date(2025, 7, 1), 0.5), (make_date(2026, 1, 1), 0.5)],
            DayCount::Thirty360,
        );
        schedule.meta.issue_date = Some(make_date(2025, 1, 1));
        schedule
    }

    fn ex_coupon_cfg(days: u32) -> AccrualConfig {
        AccrualConfig {
            ex_coupon: Some(ExCouponRule {
                days_before_coupon: days,
                calendar_id: None,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn ex_coupon_negative_ai_linear_golden() {
        // 7 calendar days before the 2025-07-01 coupon → ex-date 2025-06-24.
        // On the ex-date (inclusive boundary) the bond trades ex-coupon:
        // AI = −C × (total − elapsed) / total = −25_000 × 7/180 (30/360 days).
        let schedule = ex_coupon_test_schedule();
        let cfg = ex_coupon_cfg(7);

        let accrued =
            accrued_interest_amount(&schedule, make_date(2025, 6, 24), &cfg).expect("accrued");
        let expected = -25_000.0 * 7.0 / 180.0;
        assert!(
            (accrued - expected).abs() < 1e-9,
            "ex-coupon AI: expected {expected}, got {accrued}"
        );
    }

    #[test]
    fn ex_coupon_boundary_day_before_ex_date_accrues_positively() {
        // 2025-06-23 is one day before the ex-date: normal positive accrual.
        // elapsed = 172/360 (30/360), AI = 25_000 × 172/180.
        let schedule = ex_coupon_test_schedule();
        let cfg = ex_coupon_cfg(7);

        let accrued =
            accrued_interest_amount(&schedule, make_date(2025, 6, 23), &cfg).expect("accrued");
        let expected = 25_000.0 * 172.0 / 180.0;
        assert!(
            (accrued - expected).abs() < 1e-9,
            "cum-coupon AI: expected {expected}, got {accrued}"
        );
    }

    #[test]
    fn ex_coupon_boundary_day_before_period_end() {
        // 2025-06-30 = period end − 1 day, deep in the ex window:
        // AI = −25_000 × 1/180 (one 30/360 day of remaining stub).
        let schedule = ex_coupon_test_schedule();
        let cfg = ex_coupon_cfg(7);

        let accrued =
            accrued_interest_amount(&schedule, make_date(2025, 6, 30), &cfg).expect("accrued");
        let expected = -25_000.0 / 180.0;
        assert!(
            (accrued - expected).abs() < 1e-9,
            "ex-window stub AI: expected {expected}, got {accrued}"
        );
    }

    // =========================================================================
    // Year-fraction basis consistency (payment lag / BDC-shifted period ends)
    // =========================================================================

    /// Period reconstructed from payment dates (end = 2025-07-05, e.g. a
    /// payment lag) while the builder accrual factor 0.5 covers the true
    /// accrual period [2025-01-01, 2025-07-01).
    fn lagged_period_inputs() -> PeriodInputs {
        PeriodInputs {
            start: make_date(2025, 1, 1),
            end: make_date(2025, 7, 5),
            notional_start: 1_000_000.0,
            coupon_total: 25_000.0,
            total_yf: 0.5,
        }
    }

    #[test]
    fn elapsed_rescaled_and_clamped_when_exceeding_builder_accrual_factor() {
        let inputs = lagged_period_inputs();
        let periods = [inputs.clone()];
        let cfg = AccrualConfig::default();

        // 2025-07-03: 30/360 elapsed = 182/360 > total_yf = 0.5.
        // dc_total = 184/360, so elapsed is rescaled to 0.5 × 182/184.
        let (active, elapsed) = find_active_period_and_elapsed(
            &periods,
            make_date(2025, 7, 3),
            DayCount::Thirty360,
            &cfg,
        )
        .expect("ok")
        .expect("active period");

        let expected = 0.5 * (182.0 / 360.0) / (184.0 / 360.0);
        assert!(
            (elapsed - expected).abs() < 1e-12,
            "rescaled elapsed: expected {expected}, got {elapsed}"
        );
        assert!(
            elapsed <= inputs.total_yf,
            "elapsed must not exceed total_yf"
        );

        let accrued = accrue_in_period(active, elapsed, &AccrualMethod::Linear).expect("accrued");
        assert!(
            accrued <= inputs.coupon_total,
            "accrued {accrued} must not exceed full coupon {}",
            inputs.coupon_total
        );
    }

    #[test]
    fn elapsed_rescaled_even_when_raw_day_count_is_below_builder_accrual_factor() {
        let inputs = PeriodInputs {
            start: make_date(2025, 1, 1),
            end: make_date(2025, 7, 1),
            notional_start: 1_000_000.0,
            coupon_total: 37_500.0,
            total_yf: 0.75,
        };
        let periods = [inputs.clone()];
        let cfg = AccrualConfig::default();

        let (_active, elapsed) = find_active_period_and_elapsed(
            &periods,
            make_date(2025, 4, 1),
            DayCount::Act365F,
            &cfg,
        )
        .expect("ok")
        .expect("active period");
        let expected = inputs.total_yf * 90.0 / 181.0;

        assert!(
            (elapsed - expected).abs() < 1e-12,
            "elapsed should be scaled from raw day count to builder accrual factor, got {elapsed}"
        );
    }

    #[test]
    fn ex_window_stub_never_positive_under_payment_lag() {
        let inputs = lagged_period_inputs();
        let periods = [inputs];
        let cfg = ex_coupon_cfg(7); // ex-date 2025-06-28

        let (active, elapsed_yf) = find_active_period_and_elapsed(
            &periods,
            make_date(2025, 7, 3),
            DayCount::Thirty360,
            &cfg,
        )
        .expect("ok")
        .expect("active period");

        assert!(
            elapsed_yf <= 0.0,
            "ex-window elapsed_yf must be ≤ 0, got {elapsed_yf}"
        );
        let accrued =
            accrue_in_period(active, elapsed_yf, &AccrualMethod::Linear).expect("accrued");
        assert!(accrued <= 0.0, "ex-window AI must be ≤ 0, got {accrued}");
    }

    // =========================================================================
    // Compounded accrual method
    // =========================================================================

    fn compounded_inputs() -> PeriodInputs {
        PeriodInputs {
            start: make_date(2025, 1, 1),
            end: make_date(2025, 7, 1),
            notional_start: 1_000_000.0,
            coupon_total: 25_000.0,
            total_yf: 0.5,
        }
    }

    #[test]
    fn compounded_accrual_positive_golden() {
        // Halfway through the period: f = 0.25/0.5 = 0.5, r = 0.025.
        // AI = N × ((1 + r)^f − 1) = 1e6 × (1.025^0.5 − 1).
        let inputs = compounded_inputs();
        let accrued = accrue_in_period(&inputs, 0.25, &AccrualMethod::Compounded).expect("accrued");
        let expected = 1_000_000.0 * (1.025f64.powf(0.5) - 1.0);
        assert!(
            (accrued - expected).abs() < 1e-6,
            "compounded AI: expected {expected}, got {accrued}"
        );
    }

    #[test]
    fn compounded_accrual_near_zero_rate_falls_back_to_linear() {
        // r = 1e-7 / 1e6 = 1e-13 < 1e-12 threshold → linear fallback.
        let inputs = PeriodInputs {
            coupon_total: 1e-7,
            ..compounded_inputs()
        };
        let accrued = accrue_in_period(&inputs, 0.25, &AccrualMethod::Compounded).expect("accrued");
        let expected = 1e-7 * (0.25 / 0.5);
        assert!(
            (accrued - expected).abs() < 1e-15,
            "near-zero-rate fallback: expected {expected}, got {accrued}"
        );
    }

    #[test]
    fn compounded_accrual_ex_window_negative_golden() {
        // elapsed_yf = elapsed − total = 0.4 − 0.5 = −0.1, so 1 − f = 0.2.
        // AI = −N × ((1 + r)^(1−f) − 1) = −1e6 × (1.025^0.2 − 1).
        let inputs = compounded_inputs();
        let accrued = accrue_in_period(&inputs, -0.1, &AccrualMethod::Compounded).expect("accrued");
        let expected = -1_000_000.0 * (1.025f64.powf(0.2) - 1.0);
        assert!(
            (accrued - expected).abs() < 1e-6,
            "compounded ex-window AI: expected {expected}, got {accrued}"
        );
        assert!(accrued < 0.0);
    }

    // =========================================================================
    // Validation tests
    // =========================================================================

    #[test]
    fn act_act_isma_without_frequency_errors() {
        let mut schedule = make_test_schedule(
            &[(make_date(2025, 7, 1), 0.5), (make_date(2026, 1, 1), 0.5)],
            DayCount::ActActIsma,
        );
        schedule.meta.issue_date = Some(make_date(2025, 1, 1));

        // No frequency configured → core errors (no ISDA fallback).
        let err =
            accrued_interest_amount(&schedule, make_date(2025, 4, 1), &AccrualConfig::default())
                .expect_err("ICMA without frequency must error");
        assert!(err.to_string().to_lowercase().contains("frequency"));

        // With the frequency set, the calculation succeeds.
        let cfg = AccrualConfig {
            frequency: Some(Tenor::semi_annual()),
            ..Default::default()
        };
        let accrued = accrued_interest_amount(&schedule, make_date(2025, 4, 1), &cfg)
            .expect("ICMA with frequency succeeds");
        assert!(accrued > 0.0);
    }

    #[test]
    fn nan_coupon_total_is_rejected() {
        let schedule = make_test_schedule(&[(make_date(2025, 7, 1), 0.5)], DayCount::Thirty360);
        let periods = [CouponPeriod {
            start: make_date(2025, 1, 1),
            end: make_date(2025, 7, 1),
            dc: DayCount::Thirty360,
            bucket: CouponBucket {
                date: make_date(2025, 7, 1),
                accrual_start: None,
                accrual_end: None,
                accrual_day_count: None,
                cash_amount: f64::NAN,
                pik_amount: 0.0,
                accrual_factor: Some(0.5),
                rate: None,
            },
        }];

        let err = build_period_inputs(&schedule, &periods, &[], None)
            .expect_err("NaN coupon total must be rejected");
        assert!(err.to_string().contains("non-finite"));
    }

    #[test]
    fn infinite_builder_accrual_factor_is_rejected() {
        let mut schedule = make_test_schedule(
            &[(make_date(2025, 7, 1), f64::INFINITY)],
            DayCount::Thirty360,
        );
        schedule.meta.issue_date = Some(make_date(2025, 1, 1));

        let err = build_coupon_periods(&schedule, &AccrualConfig::default())
            .expect_err("infinite accrual_factor must be rejected");
        assert!(err.to_string().contains("non-finite"));
    }

    #[test]
    fn ex_coupon_window_spanning_full_period_is_rejected() {
        let inputs = PeriodInputs {
            start: make_date(2025, 1, 1),
            end: make_date(2025, 2, 1),
            notional_start: 1_000_000.0,
            coupon_total: 4_166.67,
            total_yf: 1.0 / 12.0,
        };
        let periods = [inputs];
        let cfg = ex_coupon_cfg(45);

        let err = find_active_period_and_elapsed(
            &periods,
            make_date(2025, 1, 1),
            DayCount::Act365F,
            &cfg,
        )
        .expect_err("ex-coupon window before period start must be rejected");
        assert!(err.to_string().contains("ex-coupon"));
    }

    #[test]
    fn accrual_config_rejects_unknown_json_fields() {
        let err = serde_json::from_str::<AccrualConfig>(
            r#"{"method":"Linear","include_pik":true,"frequency":null,"strict_issue_date":true}"#,
        )
        .expect_err("unknown top-level accrual field must be rejected");
        assert!(err.to_string().contains("strict_issue_date"));

        let nested = serde_json::from_str::<AccrualConfig>(
            r#"{"method":"Linear","include_pik":true,"frequency":null,"ex_coupon":{"days_before_coupon":7,"bogus":1}}"#,
        )
        .expect_err("unknown nested ex-coupon field must be rejected");
        assert!(nested.to_string().contains("bogus"));
    }

    #[test]
    fn ex_coupon_rule_rejects_days_above_366() {
        let rule = ExCouponRule {
            days_before_coupon: 367,
            calendar_id: None,
        };
        let err = rule
            .ex_date(make_date(2025, 7, 1))
            .expect_err("days_before_coupon > 366 must be rejected");
        assert!(err.to_string().contains("366"));

        // 366 itself is accepted (one leap year).
        let rule_ok = ExCouponRule {
            days_before_coupon: 366,
            calendar_id: None,
        };
        assert!(rule_ok.ex_date(make_date(2025, 7, 1)).is_ok());
    }
}
