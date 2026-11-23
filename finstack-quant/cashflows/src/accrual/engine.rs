//! Accrual engine implementation.

use crate::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::{CFKind, CashFlowAccrual};
use finstack_quant_core::dates::{
    Date, DateExt, DayCount, DayCountContext, Tenor, calendar_by_id_strict,
};
use finstack_quant_core::money::Money;

/// Generic accrual method usable across instruments.
///
/// This mirrors the semantics of bond accrual methods but is defined at the
/// cashflow layer so it can be reused by any instrument that exposes a
/// `CashFlowSchedule`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
    /// The period rate must be finite and greater than `-1`; valid negative
    /// rates in `(-1, 0)` remain supported.
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

/// Ex-coupon rule applied to coupon flows.
///
/// Settlement on or after the ex-coupon date and strictly before the coupon
/// payment date trades ex-coupon, as in the UK gilt convention.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExCouponRule {
    /// Number of days from the coupon record date to payment.
    ///
    /// Values greater than 366 are rejected by [`ExCouponRule::ex_date`].
    pub days_before_coupon: u32,
    /// Optional calendar ID for business day calculation.
    ///
    /// - `Some(id)`: Subtract N business days from payment date.
    /// - `None`: Subtract N calendar days from payment date.
    pub calendar_id: Option<finstack_quant_core::types::CalendarId>,
}

impl ExCouponRule {
    /// Ex-coupon date for a coupon paid on `payment_date`.
    ///
    /// Settlement on or after this date and strictly before payment trades
    /// ex-coupon, so the seller keeps the coupon and accrued interest is the
    /// rebate of the remaining accrual period.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - `days_before_coupon` exceeds 366 (a configuration error — see
    ///   `MAX_EX_COUPON_DAYS`)
    /// - `calendar_id` is set but cannot be resolved
    /// - the calculated record date is outside the supported date range
    ///
    /// # Arguments
    ///
    /// * `payment_date` - Coupon payment date from which the record-date
    ///   offset is counted backward (`days_before_coupon` business or
    ///   calendar days, depending on whether `calendar_id` is set).
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
            let cal = calendar_by_id_strict(cal_id)?;
            Ok(payment_date.add_business_days(-days, cal)?)
        } else {
            payment_date
                .checked_sub(time::Duration::days(i64::from(days)))
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "coupon record date falls outside the supported date range".into(),
                    )
                })
        }
    }

    /// Whether settlement forfeits the imminent coupon.
    ///
    /// # Arguments
    ///
    /// * `payment_date` - Coupon payment date used to calculate its record date.
    /// * `settlement_date` - Date on which ownership transfers; trades ex-coupon
    ///   when on or after the ex-coupon date and strictly before payment.
    ///   Trade dates must first be converted to settlement.
    ///
    /// # Errors
    ///
    /// Propagates invalid offset, unsupported date-range, and unknown-calendar
    /// errors from [`Self::ex_date`].
    pub fn is_ex_coupon(
        &self,
        payment_date: Date,
        settlement_date: Date,
    ) -> finstack_quant_core::Result<bool> {
        let ex_date = self.ex_date(payment_date)?;
        Ok(ex_date <= settlement_date && settlement_date < payment_date)
    }
}

/// Generic configuration for schedule-driven interest accrual.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
            include_pik: false,
            frequency: None,
        }
    }
}

/// Compute accrued interest as a scalar amount from a cashflow schedule.
///
/// Returns [`Money`] in the homogeneous currency of the selected coupon flows
/// (falling back to `schedule.notional.initial.currency()` when no selected
/// coupon flows exist).
///
/// # Arguments
///
/// * `schedule` - Canonical cashflow schedule containing coupon, PIK, and
///   notional flows.
/// * `as_of` - Accrual snapshot date. Interest earns through accrual end and
///   remains accrued until payment (exclusive), subject to ex-coupon entitlement.
/// * `cfg` - Accrual method and ex-coupon configuration.
///
/// # Returns
///
/// Accrued interest as [`Money`] in the schedule's coupon currency. Returns zero
/// when the schedule has no coupon periods or the `as_of` date is outside all
/// accrual-start-to-payment windows. When `as_of` falls inside an active
/// ex-coupon window the accrued interest is **negative** (the seller keeps
/// the coupon; the buyer is compensated for the remaining stub).
///
/// # Errors
///
/// Returns an error if:
///
/// - the schedule's outstanding-balance path cannot be constructed
/// - selected coupon flows mix currencies or a PIK flow's principal delta
///   disagrees in currency with the flow amount
/// - a required day-count calculation fails
/// - an ex-coupon calendar ID is configured but cannot be resolved
/// - compounded accrual has a non-finite period rate or a rate at or below `-1`
/// - the accrued-interest formula or accumulated result is non-finite
///
/// # Examples
///
/// ```
/// use finstack_quant_cashflows::builder::CashFlowSchedule;
/// use finstack_quant_cashflows::accrual::{accrued_interest_amount, AccrualConfig, AccrualMethod};
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::money::Money;
///
/// fn accrued_as_of(
///     schedule: &CashFlowSchedule,
///     as_of: Date,
/// ) -> finstack_quant_core::Result<Money> {
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
) -> finstack_quant_core::Result<Money> {
    AccrualIndex::build(schedule, cfg)?.accrued_at(as_of)
}

/// Precomputed accrual state for repeated [`Self::accrued_at`] queries.
///
/// Builds coupon periods and the outstanding path once for a
/// `(schedule, config)` pair. Prefer this over [`accrued_interest_amount`] when
/// accruing the same schedule on many dates.
#[derive(Debug, Clone)]
pub struct AccrualIndex {
    /// Per-period accrual inputs; empty when the schedule has no coupon periods.
    pub(super) period_inputs: Vec<PeriodInputs>,
    /// Monotone suffix minima of whole-coupon starts, in period input order.
    pub(super) suffix_min_coupon_start: Vec<Date>,
    /// Currency of the indexed coupon flows (or schedule notional when empty).
    pub(super) currency: finstack_quant_core::currency::Currency,
    /// Configuration captured at build time; every query reuses it.
    pub(super) cfg: AccrualConfig,
}

impl AccrualIndex {
    /// Build reusable accrual state for `schedule` under `cfg`.
    ///
    /// # Arguments
    ///
    /// * `schedule` - Canonical cashflow schedule containing coupon, PIK, and
    ///   notional flows.
    /// * `cfg` - Accrual method and ex-coupon configuration bound into the
    ///   index. Build a separate index to accrue under a different config.
    ///
    /// # Errors
    ///
    /// Returns an error if the schedule fails validation, mixes currencies
    /// across selected coupon flows, carries a non-finite accrual factor, or if
    /// the outstanding-balance path or a day-count calculation fails.
    pub fn build(
        schedule: &CashFlowSchedule,
        cfg: &AccrualConfig,
    ) -> finstack_quant_core::Result<Self> {
        schedule.validate()?;
        let mut selected_currency = None;
        for flow in &schedule.flows {
            if is_coupon_kind(flow.get_kind(), cfg.include_pik) {
                let flow_currency = flow.get_amount().currency();
                if flow.get_kind() == CFKind::Pik
                    && let Some(delta) = flow.get_principal_delta()
                    && delta.currency() != flow_currency
                {
                    return Err(finstack_quant_core::Error::CurrencyMismatch {
                        expected: flow_currency,
                        actual: delta.currency(),
                    });
                }
                match selected_currency {
                    Some(expected) if flow_currency != expected => {
                        return Err(finstack_quant_core::Error::CurrencyMismatch {
                            expected,
                            actual: flow_currency,
                        });
                    }
                    None => selected_currency = Some(flow_currency),
                    _ => {}
                }
            }
        }
        let currency = selected_currency.unwrap_or_else(|| schedule.notional.initial.currency());

        let periods = build_coupon_periods(schedule, cfg)?;
        let period_inputs = if periods.is_empty() {
            Vec::new()
        } else {
            let outstanding_path = schedule.outstanding_by_date()?;
            build_period_inputs(schedule, &periods, &outstanding_path)?
        };

        let mut suffix_min_coupon_start: Vec<Date> = Vec::with_capacity(period_inputs.len());
        for inputs in period_inputs.iter().rev() {
            let minimum = suffix_min_coupon_start
                .last()
                .copied()
                .map_or(inputs.coupon_start, |minimum| {
                    minimum.min(inputs.coupon_start)
                });
            suffix_min_coupon_start.push(minimum);
        }
        suffix_min_coupon_start.reverse();

        Ok(Self {
            period_inputs,
            suffix_min_coupon_start,
            currency,
            cfg: cfg.clone(),
        })
    }

    /// Currency of the accrued interest returned by [`Self::accrued_at`].
    #[must_use]
    pub const fn get_currency(&self) -> finstack_quant_core::currency::Currency {
        self.currency
    }

    /// Accrued interest as of `as_of` using the prebuilt periods.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Accrual snapshot date. Interest earns through accrual end and
    ///   remains accrued until payment (exclusive), subject to ex-coupon entitlement.
    ///
    /// # Returns
    ///
    /// Accrued interest as [`Money`] in [`Self::get_currency`]. Negative inside an
    /// active ex-coupon window (the seller keeps the coupon; the buyer is
    /// compensated for the remaining stub).
    ///
    /// # Errors
    ///
    /// Returns an error if a day-count calculation fails, a configured
    /// ex-coupon calendar ID cannot be resolved, a compounded period rate is
    /// non-finite or at or below `-1`, or the accrued result is non-finite.
    pub fn accrued_at(&self, as_of: Date) -> finstack_quant_core::Result<Money> {
        if self.period_inputs.is_empty() {
            return Ok(Money::zero(self.currency));
        }

        // Payment dates retain schedule order. Suffix minima bound future
        // whole coupons without assuming that individual segment starts are sorted.
        let first = self
            .period_inputs
            .partition_point(|inputs| inputs.payment_date <= as_of);
        let upper = self
            .suffix_min_coupon_start
            .partition_point(|&start| start <= as_of);
        if first >= upper {
            return Ok(Money::zero(self.currency));
        }

        let active =
            find_active_periods_and_elapsed(&self.period_inputs[first..upper], as_of, &self.cfg)?;
        let mut accrued = finstack_quant_core::math::summation::NeumaierAccumulator::default();
        for (inputs, elapsed_yf) in active {
            accrued.add(accrue_in_period(inputs, elapsed_yf, &self.cfg.method)?)?;
        }
        let total = accrued.get_total()?;
        if !total.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "accrual: non-finite total accrued interest {total}"
            )));
        }
        Money::from_model_value(total, self.currency)
    }
}

/// Aggregated coupon information for a single payment date.
#[derive(Debug, Clone)]
pub(super) struct CouponBucket {
    pub(super) multi_period: Option<CashFlowAccrual>,
    pub(super) date: Date,
    pub(super) accrual_start: Option<Date>,
    pub(super) accrual_end: Option<Date>,
    /// Day-count convention attached to the underlying coupon flow(s).
    /// `None` means metadata was absent or same-date flows used conflicting
    /// conventions, so the schedule representative convention is used.
    pub(super) accrual_day_count: Option<DayCount>,
    pub(super) calendar_id: Option<String>,
    pub(super) coupon_period: Option<(Date, Date)>,
    /// Termination status from the accrual context; `None` is unspecified.
    pub(super) end_is_termination_date: Option<bool>,
    pub(super) cash_amount: f64,
    pub(super) pik_amount: f64,
    /// Accrual year fraction stated by the first row of the bucket. It is
    /// always the producer's literal factor; zero is a zero-length accrual,
    /// never a request to derive one (record CORE-CONVENTIONS-015 #9).
    pub(super) accrual_factor: f64,
}

/// A single coupon period derived from the schedule.
#[derive(Debug, Clone)]
pub(super) struct CouponPeriod {
    pub(super) start: Date,
    pub(super) end: Date,
    pub(super) day_count: DayCount,
    pub(super) bucket: CouponBucket,
}

/// Inputs required to apply the accrual formula for a single period.
#[derive(Debug, Clone)]
pub(super) struct PeriodInputs {
    pub(super) multi_period: Option<CashFlowAccrual>,
    pub(super) payment_date: Date,
    /// Earliest actual segment sharing payment date, convention, and reference period.
    pub(super) coupon_start: Date,
    /// Latest actual segment sharing payment date, convention, and reference period.
    pub(super) coupon_end: Date,
    pub(super) calendar_id: Option<String>,
    pub(super) coupon_period: Option<(Date, Date)>,
    pub(super) end_is_termination_date: Option<bool>,
    pub(super) start: Date,
    pub(super) end: Date,
    pub(super) day_count: DayCount,
    pub(super) notional_start: f64,
    pub(super) coupon_total: f64,
    pub(super) total_yf: f64,
}

/// Day-count context of a coupon from its accrual metadata.
fn coupon_day_count_context(
    calendar_id: Option<&str>,
    frequency: Option<Tenor>,
    coupon_period: Option<(Date, Date)>,
    end_is_termination_date: Option<bool>,
) -> finstack_quant_core::Result<DayCountContext<'static>> {
    Ok(DayCountContext {
        calendar: calendar_id.map(calendar_by_id_strict).transpose()?,
        frequency,
        coupon_period,
        end_is_termination_date,
        ..Default::default()
    })
}

/// Check if a cashflow kind is a coupon that should be included in accrual.
fn is_coupon_kind(kind: CFKind, include_pik: bool) -> bool {
    kind.is_coupon() || (include_pik && kind == CFKind::Pik)
}

fn derive_horizon_start(
    schedule: &CashFlowSchedule,
    first_bucket: &CouponBucket,
) -> finstack_quant_core::Result<Date> {
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

/// Build coupon buckets grouped by payment date and full accrual identity.
pub(super) fn build_coupon_periods(
    schedule: &CashFlowSchedule,
    cfg: &AccrualConfig,
) -> finstack_quant_core::Result<Vec<CouponPeriod>> {
    let mut buckets: Vec<CouponBucket> = Vec::new();
    for cf in schedule
        .flows
        .iter()
        .filter(|cf| is_coupon_kind(cf.get_kind(), cfg.include_pik))
    {
        // A PIK row's coupon value is its explicit capitalization (principal
        // delta), not its cash amount (record CORE-CONVENTIONS-015 #1, #4).
        let (cash_value, pik_value) = if cf.get_kind() == CFKind::Pik {
            let delta = cf.get_principal_delta().ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "PIK row on {} has no principal_delta; its capitalized amount must be explicit",
                    cf.get_date()
                ))
            })?;
            (0.0, delta.amount())
        } else {
            (cf.get_amount().amount(), 0.0)
        };
        let accrual = cf.get_accrual().as_ref();
        let multi_period = accrual.filter(|a| !a.get_nominal_dates().is_empty());
        let true_period = accrual.map(|accrual| (accrual.get_start(), accrual.get_end()));
        let calendar_id =
            accrual.and_then(|accrual| accrual.get_context().get_calendar_id().map(str::to_owned));
        let coupon_period = accrual.and_then(|accrual| accrual.get_context().get_coupon_period());
        let end_is_termination_date =
            accrual.and_then(|accrual| accrual.get_context().get_end_is_termination_date());
        let flow_day_count = accrual.map(|accrual| accrual.get_day_count());

        // `AccrualIndex::build` validated the schedule: factors are finite
        // and non-negative. A zero factor is a literal zero-length accrual,
        // never "missing, derive it" (record CORE-CONVENTIONS-015 #9).
        let cf_af = cf.get_accrual_factor();

        let matching_bucket = buckets
            .iter_mut()
            .rev()
            .take_while(|bucket| bucket.date == cf.get_date())
            .find(|bucket| {
                bucket.accrual_start == true_period.map(|(start, _)| start)
                    && bucket.accrual_end == true_period.map(|(_, end)| end)
                    && bucket.accrual_day_count == flow_day_count
                    && bucket.calendar_id == calendar_id
                    && bucket.multi_period.as_ref() == multi_period
                    && bucket.coupon_period == coupon_period
                    && bucket.end_is_termination_date == end_is_termination_date
            });
        if let Some(bucket) = matching_bucket {
            bucket.pik_amount += pik_value;
            bucket.cash_amount += cash_value;
            continue;
        }

        buckets.push(CouponBucket {
            multi_period: multi_period.cloned(),
            date: cf.get_date(),
            accrual_start: true_period.map(|(start, _)| start),
            accrual_end: true_period.map(|(_, end)| end),
            accrual_day_count: flow_day_count,
            calendar_id,
            coupon_period,
            end_is_termination_date,
            cash_amount: cash_value,
            pik_amount: pik_value,
            accrual_factor: cf_af,
        });
    }

    if buckets.is_empty() {
        return Ok(Vec::new());
    }

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
                day_count: bucket.accrual_day_count.unwrap_or(schedule.day_count),
                bucket,
            });
        }
        // Slices of one coupon share a payment date and need not be in
        // accrual order, so the next unannotated coupon starts at the latest
        // accrual end seen, not at the last row's.
        prev = prev.max(end);
    }

    Ok(periods)
}

/// Build period inputs (including notional at start-of-period) from coupon periods
/// and the outstanding path.
///
/// # Notional Lookup
///
/// For each period, outstanding is the latest balance on or before the period start.
pub(super) fn build_period_inputs(
    schedule: &CashFlowSchedule,
    periods: &[CouponPeriod],
    outstanding_path: &[(Date, Money)],
) -> finstack_quant_core::Result<Vec<PeriodInputs>> {
    let mut result = Vec::with_capacity(periods.len());
    let mut coupon_bounds = finstack_quant_core::FxHashMap::default();
    for period in periods {
        coupon_bounds
            .entry((
                period.bucket.date,
                period.day_count,
                period.bucket.calendar_id.as_deref(),
                period.bucket.coupon_period,
            ))
            .and_modify(|(start, end): &mut (Date, Date)| {
                *start = (*start).min(period.start);
                *end = (*end).max(period.end);
            })
            .or_insert((period.start, period.end));
    }

    for p in periods {
        let notional_start = crate::builder::outstanding_on(
            outstanding_path,
            p.start,
            schedule.notional.initial.amount(),
        );

        let coupon_total = p.bucket.cash_amount + p.bucket.pik_amount;

        if !coupon_total.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "accrual: non-finite coupon total {coupon_total} for period ending {}",
                p.end
            )));
        }

        if coupon_total == 0.0 {
            continue;
        }

        let coupon_period = p.bucket.coupon_period.or(Some((p.start, p.end)));
        let total_yf = p.bucket.accrual_factor;

        if total_yf <= 0.0 {
            continue;
        }

        let (coupon_start, coupon_end) = coupon_bounds
            .get(&(
                p.bucket.date,
                p.day_count,
                p.bucket.calendar_id.as_deref(),
                p.bucket.coupon_period,
            ))
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::Internal("coupon payment has no accrual bounds".into())
            })?;

        result.push(PeriodInputs {
            multi_period: p.bucket.multi_period.clone(),
            payment_date: p.bucket.date,
            coupon_start,
            coupon_end,
            calendar_id: p.bucket.calendar_id.clone(),
            coupon_period,
            end_is_termination_date: p.bucket.end_is_termination_date,
            start: p.start,
            end: p.end,
            day_count: p.day_count,
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
/// Annotated flows retain contractual accrual bounds independently of payment.
/// Unannotated rows derive periods from payment dates. Elapsed and total day
/// counts use the same context and are rescaled to the stored coupon fraction.
/// Interest stops earning at accrual end, but remains accrued until payment.
///
/// # Ex-Coupon Handling
///
/// If an ex-coupon rule is configured and the `as_of` date falls within the
/// ex-coupon window (between ex-date and payment date), the elapsed year
/// fraction is returned as `elapsed - period`, clamped to `≤ 0` (negative).
/// The record-date window is validated against the whole coupon sharing a
/// payment date, day count, calendar, and reference period, including zero-amount
/// segments. Future principal segments contribute their
/// entire forfeited amount even before their individual accrual starts.
/// Market standard (e.g. UK gilts): the buyer does not receive the imminent
/// coupon, so the seller compensates the buyer for the remaining stub via
/// **negative accrued interest**.
pub(super) fn find_active_periods_and_elapsed<'a>(
    periods: &'a [PeriodInputs],
    as_of: Date,
    cfg: &AccrualConfig,
) -> finstack_quant_core::Result<Vec<(&'a PeriodInputs, f64)>> {
    let mut active = Vec::new();
    for inputs in periods {
        if inputs.coupon_start <= as_of && as_of < inputs.payment_date {
            let is_ex_coupon = if let Some(ref ex) = cfg.ex_coupon {
                let record_date = ex.ex_date(inputs.payment_date)?;
                if record_date <= inputs.coupon_start {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "ex-coupon date {record_date} must fall after coupon start {}",
                        inputs.coupon_start
                    )));
                }
                record_date <= as_of
            } else {
                false
            };
            if as_of < inputs.start && !is_ex_coupon {
                continue;
            }
            let day_count_context = coupon_day_count_context(
                inputs.calendar_id.as_deref(),
                cfg.frequency,
                inputs.coupon_period,
                inputs.end_is_termination_date,
            )?;
            let elapsed_end = as_of.clamp(inputs.start, inputs.end);
            let full_period = if inputs.day_count == DayCount::Act365L {
                if let Some((cp_start, cp_end)) = inputs.coupon_period {
                    (
                        inputs.coupon_start.min(cp_start),
                        inputs.coupon_end.max(cp_end),
                    )
                } else {
                    (inputs.coupon_start, inputs.coupon_end)
                }
            } else {
                (inputs.coupon_start, inputs.coupon_end)
            };
            let fraction = |end: Date| {
                if let Some(accrual) = &inputs.multi_period {
                    if end == inputs.start {
                        return Ok(0.0);
                    }
                    return accrual.subperiod(inputs.start, end)?.year_fraction();
                }
                crate::builder::periods::contractual_accrual(
                    inputs.day_count,
                    full_period,
                    inputs.start,
                    end,
                    day_count_context,
                )
            };
            let dc_elapsed = fraction(elapsed_end)?.max(0.0);
            let dc_total = fraction(inputs.end)?;
            let elapsed = if dc_total.is_finite() && dc_total > 0.0 {
                inputs.total_yf * dc_elapsed / dc_total
            } else {
                dc_elapsed
            };
            let elapsed = elapsed.clamp(0.0, inputs.total_yf);

            if is_ex_coupon {
                active.push((inputs, (elapsed - inputs.total_yf).min(0.0)));
                continue;
            }

            active.push((inputs, elapsed));
        }
    }

    Ok(active)
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
pub(super) fn accrue_in_period(
    inputs: &PeriodInputs,
    elapsed_yf: f64,
    method: &AccrualMethod,
) -> finstack_quant_core::Result<f64> {
    if inputs.total_yf <= 0.0 || elapsed_yf < -inputs.total_yf {
        return Ok(0.0);
    }

    let accrued = match method {
        AccrualMethod::Linear => inputs.coupon_total * (elapsed_yf / inputs.total_yf),
        AccrualMethod::Compounded => {
            let notional = inputs.notional_start;
            if notional <= 0.0 {
                return Ok(0.0);
            }

            let period_rate = inputs.coupon_total / notional;
            if !period_rate.is_finite() || period_rate <= -1.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "accrual: compounded period rate must be finite and greater than -1; got \
                     {period_rate} for period ending {}",
                    inputs.end
                )));
            }
            if period_rate.abs() < 1e-12 {
                inputs.coupon_total * (elapsed_yf / inputs.total_yf)
            } else {
                let fraction = elapsed_yf / inputs.total_yf;
                if fraction < 0.0 {
                    let stub_growth = (-fraction * period_rate.ln_1p()).exp_m1();
                    -(notional * stub_growth)
                } else {
                    let compound_growth = (fraction * period_rate.ln_1p()).exp_m1();
                    notional * compound_growth
                }
            }
        }
    };
    if !accrued.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "accrual: non-finite accrued interest {accrued} for period ending {}",
            inputs.end
        )));
    }
    Ok(accrued)
}
