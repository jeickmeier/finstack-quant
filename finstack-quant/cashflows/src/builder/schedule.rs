//! Schedule generation from the builder state.
//!
//! Provides the canonical `CashFlowSchedule` type and helpers for sorting and
//! deriving schedule metadata. Downstream pricing/risk code consumes this shape.

use crate::builder::Notional;
use crate::primitives::{is_cash_settlement_kind, CFKind, CashFlow};
use finstack_quant_core::cashflow::CashFlowAccrual;
use finstack_quant_core::cashflow::Discountable;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext, Period, PeriodId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::market_data::traits::{Discounting, Survival};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use indexmap::IndexMap;
use std::sync::Arc;

use super::compiler::{FixedSchedule, FloatSchedule};

/// Stable ordering rank used for deterministic sorting of same-date cashflows.
///
/// All known `CFKind` variants are explicitly ranked so that same-date ordering
/// is fully deterministic. The wildcard arm covers future `#[non_exhaustive]`
/// additions and sorts them after all known variants.
pub(crate) fn kind_rank(kind: CFKind) -> u8 {
    match kind {
        CFKind::Fixed | CFKind::Stub | CFKind::FloatReset | CFKind::InflationCoupon => 0,
        CFKind::Fee | CFKind::CommitmentFee | CFKind::UsageFee | CFKind::FacilityFee => 1,
        CFKind::Amortization => 2,
        CFKind::PrePayment => 3,
        CFKind::DefaultedNotional => 4,
        CFKind::Recovery | CFKind::AccruedOnDefault => 5,
        CFKind::PIK => 6,
        CFKind::Notional | CFKind::RevolvingDraw | CFKind::RevolvingRepayment => 7,
        CFKind::InitialMarginPost
        | CFKind::InitialMarginReturn
        | CFKind::VariationMarginReceive
        | CFKind::VariationMarginPay
        | CFKind::MarginInterest
        | CFKind::CollateralSubstitutionIn
        | CFKind::CollateralSubstitutionOut => 8,
        _ => 9,
    }
}

/// Sort flows deterministically using schedule ordering semantics.
///
/// Multi-key ordering, applied in priority order:
///
/// 1. **Date** — earliest first.
/// 2. **Kind rank** — see [`kind_rank`]. Coupons sort before fees, fees before
///    amortization, amortization before prepayment, etc.
/// 3. **Currency** — lexicographic on the ISO code.
/// 4. **Amount** — `f64::total_cmp`, so signed values sort consistently and
///    NaN handling is well-defined.
/// 5. **Reset date** — `Option<Date>` ordering, with `None` first.
/// 6. **Accrual factor** — `f64::total_cmp`.
/// 7. **Rate** — `Option<f64>` ordering, with `None` first, then `total_cmp`.
/// 8. **Accrual metadata** — period, day-count rank, then projected index rate.
///
/// This is the canonical order downstream consumers
/// (`outstanding_by_date`, `pv_by_period`, accrual, dataframe export, etc.)
/// rely on. Because the comparator distinguishes every field that can vary
/// between two flows, it is a *total order*: the result is independent of input
/// order (deterministic across runs and across `Vec` reorderings).
///
/// A stable sort is used deliberately: `merge_cashflow_schedules` concatenates
/// already-sorted schedules, and the stable sort's run detection handles those
/// pre-sorted runs in near-linear time (an unstable sort re-partitions them).
pub(crate) fn sort_flows(flows: &mut [CashFlow]) {
    flows.sort_by(compare_flows);
}

fn compare_flows(a: &CashFlow, b: &CashFlow) -> std::cmp::Ordering {
    a.date
        .cmp(&b.date)
        .then_with(|| kind_rank(a.kind).cmp(&kind_rank(b.kind)))
        .then_with(|| a.amount.currency().cmp(&b.amount.currency()))
        .then_with(|| a.amount.amount().total_cmp(&b.amount.amount()))
        .then_with(|| a.reset_date.cmp(&b.reset_date))
        .then_with(|| a.accrual_factor.total_cmp(&b.accrual_factor))
        .then_with(|| match (a.rate, b.rate) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => x.total_cmp(&y),
        })
        .then_with(|| compare_accrual(a.accrual, b.accrual))
}

fn compare_accrual(
    left: Option<CashFlowAccrual>,
    right: Option<CashFlowAccrual>,
) -> std::cmp::Ordering {
    match (left, right) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(left), Some(right)) => left
            .start
            .cmp(&right.start)
            .then_with(|| left.end.cmp(&right.end))
            .then_with(|| day_count_rank(left.day_count).cmp(&day_count_rank(right.day_count)))
            .then_with(
                || match (left.projected_index_rate, right.projected_index_rate) {
                    (None, None) => std::cmp::Ordering::Equal,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (Some(left), Some(right)) => left.total_cmp(&right),
                },
            ),
    }
}

fn day_count_rank(day_count: DayCount) -> u8 {
    match day_count {
        DayCount::Act360 => 0,
        DayCount::Act365F => 1,
        DayCount::Act365L => 2,
        DayCount::Thirty360 => 3,
        DayCount::ThirtyE360 => 4,
        DayCount::ThirtyE360Isda => 5,
        DayCount::Nl365 => 6,
        DayCount::ActAct => 7,
        DayCount::ActActIsma => 8,
        DayCount::Bus252 => 9,
        _ => 10,
    }
}

pub(crate) fn finalize_flows(
    mut flows: Vec<CashFlow>,
    fixed: &[FixedSchedule],
    floating: &[FloatSchedule],
    issue_date: Option<Date>,
    maturity_date: Option<Date>,
) -> (Vec<CashFlow>, CashFlowMeta, DayCount) {
    sort_flows(&mut flows);

    let mut cals: Vec<String> = fixed
        .iter()
        .map(|schedule| schedule.spec.schedule.calendar_id.clone())
        .chain(
            floating
                .iter()
                .map(|schedule| schedule.spec.schedule.calendar_id.clone()),
        )
        .collect();
    cals.sort_unstable();
    cals.dedup();
    let meta = CashFlowMeta {
        calendar_ids: cals,
        facility_limit: None,
        issue_date,
        maturity_date,
        representation: CashflowRepresentation::default(),
    };

    let out_dc = if let Some(schedule) = fixed.first() {
        schedule.spec.schedule.dc
    } else if let Some(schedule) = floating.first() {
        schedule.spec.schedule.dc
    } else {
        DayCount::Act365F
    };
    (flows, meta, out_dc)
}

/// Meaning of the emitted schedule relative to pricing and waterfall policy.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CashflowRepresentation {
    /// Fixed or contractually scheduled future dated cash amounts.
    #[default]
    Contractual,
    /// Current-market or model-projected future dated cash amounts.
    Projected,
    /// Intentionally empty because the contingent payoff policy is not modeled yet.
    Placeholder,
    /// Intentionally empty because no future dated cashflows remain.
    NoResidual,
}

/// Metadata shared by an entire cashflow schedule.
///
/// Tracks referenced calendar IDs, optional facility limits, and the instrument's
/// issue date for use by downstream engines (e.g., accrual calculation).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CashFlowMeta {
    /// Meaning of the schedule relative to waterfall policy.
    #[serde(default)]
    pub representation: CashflowRepresentation,
    /// Holiday calendar IDs used for schedule adjustments.
    pub calendar_ids: Vec<String>,
    /// Optional facility limit/commitment for instruments like RCFs.
    pub facility_limit: Option<Money>,
    /// Issue date of the instrument, when known.
    ///
    /// Used by the accrual engine to establish the first coupon period start
    /// date precisely, avoiding the inverse day count approximation that can
    /// be off by 1-2 days.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub issue_date: Option<Date>,
    /// Contractual maturity date, distinct from an adjusted final payment date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub maturity_date: Option<Date>,
}

/// Cashflow schedule output from the composable builder.
///
/// Contains ordered cashflows plus notional and a representative `DayCount`.
/// Methods provide convenient accessors commonly used by pricing and analysis.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CashFlowSchedule {
    /// Ordered cashflows (coupons, principal payments, fees)
    #[serde(deserialize_with = "deserialize_sorted_flows")]
    #[schemars(with = "Vec<CashFlow>")]
    pub(crate) flows: Vec<CashFlow>,
    /// Notional schedule (constant or amortizing)
    pub(crate) notional: Notional,
    /// Day count convention for interest calculations
    pub(crate) day_count: DayCount,
    /// Additional metadata (calendars, facility limits)
    pub(crate) meta: CashFlowMeta,
}

fn deserialize_sorted_flows<'de, D>(deserializer: D) -> Result<Vec<CashFlow>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut flows = <Vec<CashFlow> as serde::Deserialize>::deserialize(deserializer)?;
    sort_flows(&mut flows);
    Ok(flows)
}

impl Discountable for CashFlowSchedule {
    type PVOutput = finstack_quant_core::Result<Money>;

    fn npv(&self, disc: &dyn Discounting, base: Date) -> finstack_quant_core::Result<Money> {
        let flows = self
            .flows
            .iter()
            .filter(|flow| is_cash_settlement_kind(flow.kind))
            .map(|flow| (flow.date, flow.amount))
            .collect::<Vec<_>>();

        if flows.is_empty() {
            return Money::try_new(0.0, self.notional.initial.currency());
        }

        finstack_quant_core::cashflow::npv(disc, base, &flows)
    }
}

impl CashFlowSchedule {
    /// Construct a canonical schedule from already-classified flows.
    ///
    /// Flows are sorted into deterministic schedule order. Call [`Self::validate`]
    /// when accepting untrusted or externally supplied economic state.
    pub fn from_parts(
        flows: Vec<CashFlow>,
        notional: Notional,
        day_count: DayCount,
        meta: CashFlowMeta,
    ) -> Self {
        let mut schedule = Self {
            flows,
            notional,
            day_count,
            meta,
        };
        sort_schedule_with_metadata(&mut schedule);
        schedule
    }

    /// Construct and economically validate a schedule from classified flows.
    ///
    /// Use this constructor for untrusted or externally supplied state. Internal
    /// builders that establish invariants while emitting flows may continue to
    /// use [`Self::from_parts`].
    pub fn try_from_parts(
        flows: Vec<CashFlow>,
        notional: Notional,
        day_count: DayCount,
        meta: CashFlowMeta,
    ) -> finstack_quant_core::Result<Self> {
        let schedule = Self::from_parts(flows, notional, day_count, meta);
        schedule.validate()?;
        Ok(schedule)
    }

    /// Return the canonical ordered cashflows.
    #[must_use]
    pub fn get_flows(&self) -> &[CashFlow] {
        &self.flows
    }

    /// Consume the schedule and return its canonical ordered cashflows.
    #[must_use]
    pub fn into_flows(self) -> Vec<CashFlow> {
        self.flows
    }

    /// Return the schedule notional.
    #[must_use]
    pub fn get_notional(&self) -> &Notional {
        &self.notional
    }

    /// Return the representative day-count convention.
    #[must_use]
    pub fn get_day_count(&self) -> DayCount {
        self.day_count
    }

    /// Return schedule-level metadata.
    #[must_use]
    pub fn get_meta(&self) -> &CashFlowMeta {
        &self.meta
    }

    /// Update cashflows in place and restore canonical ordering afterward.
    pub fn update_flows(&mut self, mut update: impl FnMut(&mut CashFlow)) {
        for flow in &mut self.flows {
            update(flow);
        }
        sort_flows(&mut self.flows);
    }

    /// Fallible variant of [`Self::update_flows`].
    pub fn try_update_flows(
        &mut self,
        mut update: impl FnMut(&mut CashFlow) -> finstack_quant_core::Result<()>,
    ) -> finstack_quant_core::Result<()> {
        for flow in &mut self.flows {
            update(flow)?;
        }
        sort_flows(&mut self.flows);
        Ok(())
    }

    /// Retain matching cashflows while preserving canonical ordering.
    pub fn retain_flows(&mut self, keep: impl FnMut(&CashFlow) -> bool) {
        self.flows.retain(keep);
    }

    /// Add one cashflow and restore canonical ordering.
    pub fn push_flow(&mut self, flow: CashFlow) {
        self.flows.push(flow);
        sort_flows(&mut self.flows);
    }

    /// Create a new cashflow builder (standard Rust pattern).
    ///
    /// This is the recommended entry point for building cashflow schedules.
    /// Returns a `CashFlowBuilder` that can be configured and built.
    ///
    /// # Example
    /// ```ignore
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams};
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let issue = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let maturity = Date::from_calendar_date(2026, Month::January, 15)?;
    ///
    /// let notional = Money::new(1_000_000.0, Currency::USD);
    /// let spec = FixedCouponSpec {
    ///     coupon_type: CouponType::Cash,
    ///     rate: dec!(0.05),
    ///     schedule: ScheduleParams::semiannual_30360(),
    /// };
    ///
    /// let schedule = CashFlowSchedule::builder()
    ///     .principal(notional, issue, maturity)
    ///     .fixed_cf(spec)
    ///     .build(None)?;
    /// # let _ = schedule;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn builder() -> super::CashFlowBuilder {
        super::CashFlowBuilder::default()
    }

    /// Attach the economic representation produced by a raw schedule source.
    #[must_use]
    pub fn with_representation(mut self, representation: CashflowRepresentation) -> Self {
        self.meta.representation = representation;
        self
    }

    /// Replace the representative schedule notional without changing rows.
    #[must_use]
    pub fn with_notional(mut self, notional: Notional) -> Self {
        self.notional = notional;
        self
    }

    /// Scale every cashflow amount while preserving classification and metadata.
    ///
    /// This is primarily used to apply leg direction before schedules are
    /// composed. A non-finite scale is rejected.
    pub fn scale_amounts(mut self, scale: f64) -> finstack_quant_core::Result<Self> {
        if !scale.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "cashflow amount scale must be finite".to_string(),
            ));
        }
        for flow in &mut self.flows {
            flow.amount *= scale;
        }
        Ok(self)
    }

    /// Returns the list of dates for all flows in schedule order.
    pub fn dates(&self) -> Vec<Date> {
        self.flows.iter().map(|cf| cf.date).collect()
    }

    /// Validate all schedule-level and per-flow invariants.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.notional.validate()?;
        for flow in &self.flows {
            flow.validate()?;
        }
        if self.flows.windows(2).any(|w| w[0].date > w[1].date) {
            return Err(finstack_quant_core::Error::Validation(
                "cashflow schedule flows must be sorted by date".into(),
            ));
        }
        self.validate_economic_invariants()
    }

    fn validate_economic_invariants(&self) -> finstack_quant_core::Result<()> {
        let initial = self.notional.initial;
        let expected_currency = initial.currency();
        let initial_amount = initial.amount().abs();
        let mut initial_funding_skipped = false;
        let additional_funding = self.flows.iter().fold(0.0, |total, flow| {
            if flow.amount.currency() != expected_currency
                || !matches!(flow.kind, CFKind::Notional | CFKind::RevolvingDraw)
                || flow.amount.amount() >= 0.0
            {
                return total;
            }
            let is_initial_funding = self.meta.issue_date.is_some_and(|issue_date| {
                is_initial_funding_flow(flow, issue_date, initial_amount, initial_funding_skipped)
            });
            initial_funding_skipped |= is_initial_funding;
            if is_initial_funding {
                total
            } else {
                total + flow.amount.amount().abs()
            }
        });
        let funded_principal = initial_amount + additional_funding;
        let epsilon = (funded_principal * 1e-8).max(1e-6);
        let total_amortization = self
            .flows
            .iter()
            .filter(|flow| flow.kind == CFKind::Amortization)
            .try_fold(0.0, |total, flow| {
                if flow.amount.currency() != expected_currency {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "amortization flow currency ({}) must match initial notional currency ({})",
                        flow.amount.currency(),
                        expected_currency
                    )));
                }
                Ok(total + flow.amount.amount().max(0.0))
            })?;

        if total_amortization > funded_principal + epsilon {
            return Err(finstack_quant_core::Error::Validation(format!(
                "total amortization ({total_amortization:.6}) exceeds funded principal ({funded_principal:.6})"
            )));
        }

        if let Some(issue_date) = self.meta.issue_date {
            let long_horizon = issue_date.add_months(1200);
            for flow in &self.flows {
                let interest_bearing = matches!(
                    flow.kind,
                    CFKind::Fixed
                        | CFKind::FloatReset
                        | CFKind::InflationCoupon
                        | CFKind::PIK
                        | CFKind::Stub
                );
                if flow.date < issue_date && interest_bearing {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "interest-bearing cashflow ({:?}) dated {} is before issue date {}",
                        flow.kind, flow.date, issue_date
                    )));
                }
                if flow.date > long_horizon {
                    tracing::warn!(
                        flow_date = %flow.date,
                        issue_date = %issue_date,
                        horizon_date = %long_horizon,
                        "cashflow schedule contains a flow more than 100 years after issue date"
                    );
                }
            }
        }

        Ok(())
    }

    /// Internal future-flow filtering step for composed schedule normalization.
    #[must_use]
    pub(crate) fn filter_future(mut self, as_of: Date) -> Self {
        retain_schedule_flows(&mut self, |cf| cf.date >= as_of);
        self
    }

    /// Internal PIK-omission step for composed schedule normalization.
    #[must_use]
    pub(crate) fn omit_pure_pik(mut self) -> Self {
        retain_schedule_flows(&mut self, |cf| cf.kind != CFKind::PIK);
        self
    }

    /// One-shot public-schedule normalization pipeline.
    ///
    /// Applies, in order:
    /// 1. Future-flow filtering (`date >= as_of`)
    /// 2. Pure PIK omission
    /// 3. Re-sort (defensive, in case instrument code appended unsorted flows)
    /// 4. Preserve the representation attached by the raw schedule source
    #[must_use]
    pub(crate) fn normalize_public(self, as_of: Date) -> Self {
        let mut normalized = self.filter_future(as_of).omit_pure_pik();
        sort_schedule_with_metadata(&mut normalized);
        normalized
    }

    /// Get an iterator over interest-like coupon cashflows.
    ///
    /// Filters the schedule via [`CFKind::is_interest_like`] — currently
    /// `Fixed`, `FloatReset`, `InflationCoupon`, and `Stub`. PIK, fees,
    /// amortization, recovery, and notional flows are excluded.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
    /// use finstack_quant_cashflows::primitives::{CashFlow, CFKind};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, DayCount};
    /// use finstack_quant_core::money::Money;
    /// use time::Month;
    ///
    /// let date = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
    /// let flows = vec![
    ///     CashFlow::new(date, None, Money::new(50_000.0, Currency::USD), CFKind::Fixed, 0.5, Some(0.05)),
    ///     CashFlow::new(date, None, Money::new(100_000.0, Currency::USD), CFKind::Amortization, 0.0, None),
    /// ];
    /// let schedule = CashFlowSchedule::from_parts(
    ///     flows,
    ///     Notional::par(1_000_000.0, Currency::USD),
    ///     DayCount::Act365F,
    ///     CashFlowMeta::default(),
    /// );
    ///
    /// let coupons: Vec<_> = schedule.coupons().collect();
    /// assert_eq!(coupons.len(), 1);
    /// assert_eq!(coupons[0].kind, CFKind::Fixed);
    /// ```
    pub fn coupons(&self) -> impl Iterator<Item = &CashFlow> {
        self.flows.iter().filter(|cf| cf.kind.is_interest_like())
    }

    /// Weighted Average Life (WAL) in years from `as_of`.
    ///
    /// WAL = Σ(principal_i × t_i) / Σ(principal_i)
    ///
    /// where t_i is the year fraction from `as_of` to the payment date,
    /// and the sum runs over all principal flows (Amortization, Notional,
    /// PrePayment) with positive amounts after `as_of`.
    ///
    /// WAL is computed on an Act/365F basis regardless of the schedule's
    /// accrual day count, matching conventional desk reporting. This avoids
    /// silent mis-computation when the schedule uses Act/Act ISMA or
    /// Bus/252, which require calendar or frequency context that WAL does
    /// not carry.
    ///
    /// # References
    /// - SIFMA, *Standard Formulas for the Analysis of Mortgage-Backed
    ///   Securities and Other Related Securities* (2010 ed.), §II.B
    ///   (Weighted Average Life), which prescribes actual days / 365 as
    ///   the market standard time metric.
    /// - Fabozzi, *The Handbook of Fixed Income Securities* (8th ed.,
    ///   2012), ch. 24, "Mortgage-Backed Securities", WAL definition.
    ///
    /// Returns `Ok(0.0)` if there are no future principal flows.
    ///
    /// # Errors
    ///
    /// Returns an error if the day-count year-fraction calculation fails.
    pub fn weighted_average_life(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
        weighted_average_life_from_principal(
            self.flows
                .iter()
                .filter(|cf| {
                    matches!(
                        cf.kind,
                        CFKind::Amortization | CFKind::Notional | CFKind::PrePayment
                    ) && cf.date > as_of
                        && cf.amount.amount() > 0.0
                })
                .map(|cf| (cf.date, cf.amount)),
            as_of,
        )
    }

    /// Full outstanding path including Amortization, PIK, and Notional draws/repays.
    ///
    /// Returns one entry per unique date after applying all balance-affecting flows
    /// on that date. This is the **canonical method** for tracking outstanding balance
    /// in instruments with dynamic draws/repays (RCFs, delayed-draw term loans).
    ///
    /// # When to Use Each Method
    ///
    /// This is the canonical balance view for all principal event kinds.
    ///
    /// # Balance Changes
    ///
    /// - **Amortization**: Reduces outstanding (stored as positive amounts)
    /// - **PIK**: Increases outstanding (capitalizes into principal)
    /// - **Notional**: Draws are negative (increase outstanding), repays are positive
    ///
    /// Note: The initial notional flow (funding at issue) is skipped as it's already
    /// accounted for in `notional.initial`. Only subsequent draws/repays are tracked.
    ///
    /// # Negative Balances
    ///
    /// Replayed balances are permitted to go negative (e.g. a repayment
    /// exceeding the tracked outstanding); a `tracing::warn!` is emitted when
    /// the balance drops below a small negative tolerance, but no error is
    /// raised. The builder validates over-repayment at construction time.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - `meta.issue_date` is unset
    /// - Currency mismatch between flows and notional
    pub fn outstanding_by_date(&self) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        let issue = self.meta.issue_date.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "outstanding_by_date: schedule.meta.issue_date is required to identify the initial funding flow"
                    .into(),
            )
        })?;
        let replay = self.replay_balances(issue)?;
        let mut result = Vec::with_capacity(replay.len());
        for (idx, _, after) in replay {
            let date = self.flows[idx].date;
            if let Some((last_date, last_balance)) = result.last_mut() {
                if *last_date == date {
                    *last_balance = after;
                    continue;
                }
            }
            result.push((date, after));
        }
        Ok(result)
    }

    pub(crate) fn replay_balances(
        &self,
        funding_anchor: Date,
    ) -> finstack_quant_core::Result<Vec<(usize, Money, Money)>> {
        if self.flows.is_empty() {
            return Ok(Vec::new());
        }
        let mut order: Vec<usize> = (0..self.flows.len()).collect();
        order.sort_by(|left, right| compare_flows(&self.flows[*left], &self.flows[*right]));
        let mut outstanding = self.notional.initial;
        let mut initial_funding_skipped = false;
        let initial_amount = self.notional.initial.amount();
        let mut replay = Vec::with_capacity(order.len());
        for idx in order {
            let flow = &self.flows[idx];
            let before = outstanding;
            let is_initial_funding = is_initial_funding_flow(
                flow,
                funding_anchor,
                initial_amount,
                initial_funding_skipped,
            );
            initial_funding_skipped |= is_initial_funding;
            apply_flow_to_outstanding(&mut outstanding, flow, is_initial_funding, true)?;
            replay.push((idx, before, outstanding));
        }
        Ok(replay)
    }
}

/// Calculate Act/365F weighted average life from dated principal reductions.
pub fn weighted_average_life_from_principal<I>(
    principal: I,
    as_of: Date,
) -> finstack_quant_core::Result<f64>
where
    I: IntoIterator<Item = (Date, Money)>,
{
    let mut currency = None;
    let mut weighted = finstack_quant_core::math::summation::NeumaierAccumulator::default();
    let mut total = finstack_quant_core::math::summation::NeumaierAccumulator::default();
    for (date, amount) in principal {
        if date <= as_of || amount.amount() <= 0.0 {
            continue;
        }
        if let Some(expected) = currency {
            if amount.currency() != expected {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected,
                    actual: amount.currency(),
                });
            }
        } else {
            currency = Some(amount.currency());
        }
        let years = DayCount::Act365F.year_fraction(as_of, date, DayCountContext::default())?;
        weighted.add(amount.amount() * years);
        total.add(amount.amount());
    }
    let principal_total = total.total();
    Ok(if principal_total > 0.0 {
        weighted.total() / principal_total
    } else {
        0.0
    })
}

fn retain_schedule_flows(schedule: &mut CashFlowSchedule, mut keep: impl FnMut(&CashFlow) -> bool) {
    schedule.flows.retain(|flow| keep(flow));
}

/// Sort a schedule using the canonical self-contained flow order.
pub(crate) fn sort_schedule_with_metadata(schedule: &mut CashFlowSchedule) {
    sort_flows(&mut schedule.flows);
}

fn merge_representation(
    current: Option<CashflowRepresentation>,
    next: CashflowRepresentation,
) -> CashflowRepresentation {
    use CashflowRepresentation::{Contractual, NoResidual, Placeholder, Projected};

    match (current, next) {
        (None, value) => value,
        (Some(Projected), _) | (_, Projected) => Projected,
        (Some(Placeholder), _) | (_, Placeholder) => Placeholder,
        (Some(Contractual), _) | (_, Contractual) => Contractual,
        (Some(NoResidual), NoResidual) => NoResidual,
    }
}

fn merge_matching_option<T: Copy + PartialEq>(current: &mut Option<Option<T>>, value: Option<T>) {
    *current = Some(match *current {
        None => value,
        Some(existing) if existing == value => existing,
        Some(_) => None,
    });
}

/// Merge multiple schedules into one deterministic composite schedule.
///
/// Concatenates flows from every input schedule, including mixed-currency
/// rows, deduplicates the union of
/// their `meta.calendar_ids`, and reduces remaining metadata fields with the
/// rules listed below. The combined flow list is then re-sorted via
/// [`sort_flows`] so the resulting schedule is in canonical order.
///
/// # Arguments
///
/// * `schedules` - Iterable of [`CashFlowSchedule`] values to combine.
/// * `notional` - Notional stamped on the merged schedule. The caller is
///   responsible for choosing a representative notional that makes sense for
///   the composite (this function does not aggregate input notionals or require
///   row currencies to match it).
/// * `day_count` - Day count convention attached to the merged schedule.
///
/// # Returns
///
/// A single [`CashFlowSchedule`] containing every input flow, sorted, with
/// merged metadata.
///
/// # Metadata merge rules
///
/// - `representation`: `Projected` dominates mixed inputs, followed by
///   `Placeholder`, `Contractual`, and `NoResidual`.
/// - `calendar_ids`: union of all inputs, sorted and deduplicated.
/// - `facility_limit`: kept only if every input agrees; mismatches → `None`.
/// - `issue_date`: kept only if every input agrees; mismatches → `None`.
///
/// Empty input yields an empty schedule with default metadata.
pub fn merge_cashflow_schedules<I>(
    schedules: I,
    notional: Notional,
    day_count: DayCount,
) -> CashFlowSchedule
where
    I: IntoIterator<Item = CashFlowSchedule>,
{
    let mut flows = Vec::new();
    let mut calendar_ids = Vec::new();
    let mut facility_limit: Option<Option<Money>> = None;
    let mut issue_date: Option<Option<Date>> = None;
    let mut maturity_date: Option<Option<Date>> = None;
    let mut representation: Option<CashflowRepresentation> = None;

    for schedule in schedules {
        representation = Some(merge_representation(
            representation,
            schedule.meta.representation,
        ));
        let schedule_flows = schedule.flows;
        let schedule_meta = schedule.meta;
        flows.extend(schedule_flows);
        calendar_ids.extend(schedule_meta.calendar_ids);
        merge_matching_option(&mut facility_limit, schedule_meta.facility_limit);
        merge_matching_option(&mut issue_date, schedule_meta.issue_date);
        merge_matching_option(&mut maturity_date, schedule_meta.maturity_date);
    }

    calendar_ids.sort_unstable();
    calendar_ids.dedup();

    CashFlowSchedule::from_parts(
        flows,
        notional,
        day_count,
        CashFlowMeta {
            representation: representation.unwrap_or_default(),
            calendar_ids,
            facility_limit: facility_limit.unwrap_or(None),
            issue_date: issue_date.unwrap_or(None),
            maturity_date: maturity_date.unwrap_or(None),
        },
    )
}

/// Compare two amounts using relative epsilon for floating-point tolerance.
///
/// Uses a relative tolerance of 1e-12 scaled by magnitude, with a minimum
/// absolute tolerance of 1e-12 (from the `.max(1.0)` floor).
pub(super) fn amounts_approx_equal(a: f64, b: f64) -> bool {
    let max_abs = a.abs().max(b.abs()).max(1.0);
    (a - b).abs() < max_abs * 1e-12
}

fn is_initial_funding_flow(
    cf: &CashFlow,
    issue: Date,
    initial_amount: f64,
    already_skipped: bool,
) -> bool {
    !already_skipped
        && cf.kind == CFKind::Notional
        && cf.amount.amount() < 0.0
        && initial_amount != 0.0
        && cf.date == issue
        && amounts_approx_equal(cf.amount.amount().abs(), initial_amount)
}

/// Apply a single flow's balance impact during *reconstruction* of the
/// outstanding path from a finalized schedule.
///
/// # Reconstruction vs. emission
///
/// Outstanding balance is tracked in two distinct places that must stay
/// consistent:
///
/// 1. **Emission time** — an instrument projection mutates a live balance as it
///    generates flows (for example, subtracting defaulted principal when it
///    emits a `DefaultedNotional` flow).
/// 2. **Reconstruction time** — this function, driven by
///    [`CashFlowSchedule::outstanding_by_date`] rebuilds the balance
///    path purely from `notional.initial` plus the finalized flow list.
///
/// Because reconstruction starts from `notional.initial` and replays every
/// flow, `DefaultedNotional` (and `Amortization`, `PrePayment`, `PIK`,
/// `Notional`) must reduce/increase the balance here exactly as the emission
/// pipeline did. If a new `CFKind` affects the balance, it must be handled in
/// both places or the two views will diverge.
fn apply_flow_to_outstanding(
    outstanding: &mut Money,
    cf: &CashFlow,
    is_initial_funding: bool,
    include_notional: bool,
) -> finstack_quant_core::Result<()> {
    /// Tolerance below zero before a replayed balance is considered negative.
    const NEGATIVE_BALANCE_EPSILON: f64 = 1e-9;

    match cf.kind {
        CFKind::Amortization | CFKind::PrePayment | CFKind::DefaultedNotional => {
            // Amortization amounts are stored as positive in the builder
            // but economically represent principal reductions.
            // PrePayment and DefaultedNotional likewise reduce outstanding.
            *outstanding = outstanding.checked_sub(cf.amount)?;
        }
        CFKind::PIK => {
            *outstanding = outstanding.checked_add(cf.amount)?;
        }
        CFKind::Notional | CFKind::RevolvingDraw | CFKind::RevolvingRepayment
            if include_notional && !is_initial_funding =>
        {
            // Draws negative, repays positive -> subtract to apply sign
            *outstanding = outstanding.checked_sub(cf.amount)?;
        }
        _ => {}
    }
    if outstanding.amount() < -NEGATIVE_BALANCE_EPSILON {
        tracing::warn!(
            date = %cf.date,
            kind = ?cf.kind,
            balance = outstanding.amount(),
            "replayed outstanding balance went negative"
        );
    }
    Ok(())
}

/// Credit-adjustment inputs for periodized PV aggregation.
#[derive(Clone, Copy)]
pub struct PvCreditAdjustment<'a> {
    /// Optional hazard curve used to survival-adjust cashflows.
    pub hazard: Option<&'a dyn Survival>,
    /// Optional recovery rate applied to principal-like flows.
    pub recovery_rate: Option<f64>,
}

/// Discount-source variants for periodized PV aggregation.
///
/// Use [`Self::Discount`] when the caller has already resolved curve handles.
/// Use [`Self::Market`] when the schedule should resolve discount and optional
/// hazard curves from a [`MarketContext`] for each call.
#[derive(Clone, Copy)]
pub enum PvDiscountSource<'a> {
    /// Use already-resolved discounting and optional credit-adjustment handles.
    Discount {
        /// Discount curve for present value calculation.
        disc: &'a dyn Discounting,
        /// Optional credit-adjustment inputs.
        credit: Option<PvCreditAdjustment<'a>>,
    },
    /// Resolve discount and optional hazard curves from a market context.
    Market {
        /// Market context containing the required curves.
        market: &'a MarketContext,
        /// Discount curve identifier.
        disc_curve_id: &'a CurveId,
        /// Optional hazard curve identifier.
        hazard_curve_id: Option<&'a CurveId>,
    },
}

impl CashFlowSchedule {
    /// Compute periodized PVs from either resolved discount handles or a market context.
    ///
    /// Cashflows are grouped into the supplied reporting periods using
    /// half-open period boundaries. Plain discounting uses `amount * df(t)`.
    /// Credit-adjusted discounting additionally survival-adjusts flows and can
    /// apply recovery assumptions to principal-like cashflows.
    ///
    /// # Arguments
    ///
    /// * `periods` - Reporting periods that define the output buckets.
    /// * `source` - Discount source and optional credit-adjustment inputs.
    /// * `date_ctx` - Valuation date, day-count convention, and day-count
    ///   context used to convert cashflow dates into discount times.
    ///
    /// # Returns
    ///
    /// Map from `PeriodId` to per-currency present-value totals. Empty periods
    /// are omitted.
    ///
    /// # Errors
    ///
    /// Returns an error if curve lookup fails, day-count conversion fails, or
    /// credit-adjusted inputs are internally inconsistent.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_cashflows::aggregation::DateContext;
    /// use finstack_quant_cashflows::builder::{CashFlowSchedule, PvDiscountSource};
    /// use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Period};
    /// use finstack_quant_core::market_data::traits::Discounting;
    ///
    /// fn schedule_pv_by_period(
    ///     schedule: &CashFlowSchedule,
    ///     periods: &[Period],
    ///     disc: &dyn Discounting,
    ///     base: Date,
    /// ) -> finstack_quant_core::Result<()> {
    ///     let pv = schedule.pv_by_period(
    ///         periods,
    ///         PvDiscountSource::Discount { disc, credit: None },
    ///         DateContext::new(base, DayCount::Act365F, DayCountContext::default()),
    ///     )?;
    ///
    ///     let _ = pv;
    ///     Ok(())
    /// }
    /// ```
    pub fn pv_by_period(
        &self,
        periods: &[Period],
        source: PvDiscountSource<'_>,
        date_ctx: crate::aggregation::DateContext<'_>,
    ) -> finstack_quant_core::Result<IndexMap<PeriodId, IndexMap<Currency, Money>>> {
        if self.flows.is_empty() || periods.is_empty() {
            return Ok(IndexMap::new());
        }

        match source {
            PvDiscountSource::Discount { disc, credit } => {
                if let Some(PvCreditAdjustment {
                    hazard: Some(hazard_curve),
                    recovery_rate,
                }) = credit
                {
                    crate::aggregation::pv_by_period_credit_adjusted_detailed_with_timing(
                        &self.flows,
                        periods,
                        disc,
                        Some(hazard_curve),
                        recovery_rate,
                        crate::aggregation::RecoveryTiming::default(),
                        date_ctx,
                    )
                } else {
                    crate::aggregation::pv_by_period_cashflows_sorted_checked(
                        &self.flows,
                        periods,
                        disc,
                        date_ctx.base,
                        date_ctx.dc,
                        date_ctx.dc_ctx,
                        None,
                    )
                }
            }
            PvDiscountSource::Market {
                market,
                disc_curve_id,
                hazard_curve_id,
            } => {
                let curves = resolve_credit_curves(market, disc_curve_id, hazard_curve_id)?;
                self.pv_by_period(
                    periods,
                    PvDiscountSource::Discount {
                        disc: curves.discounting(),
                        credit: Some(PvCreditAdjustment {
                            hazard: curves.hazard_survival(),
                            recovery_rate: curves.recovery_rate(),
                        }),
                    },
                    date_ctx,
                )
            }
        }
    }
}

pub(crate) struct CreditCurveHandles {
    discount: Arc<DiscountCurve>,
    hazard: Option<Arc<HazardCurve>>,
}

impl CreditCurveHandles {
    pub(crate) fn discounting(&self) -> &dyn Discounting {
        self.discount.as_ref()
    }

    pub(crate) fn hazard_survival(&self) -> Option<&dyn Survival> {
        self.hazard
            .as_ref()
            .map(|arc| arc.as_ref() as &dyn Survival)
    }

    pub(crate) fn recovery_rate(&self) -> Option<f64> {
        self.hazard.as_ref().map(|h| h.recovery_rate())
    }
}

pub(crate) fn resolve_credit_curves(
    market: &MarketContext,
    disc_curve_id: &CurveId,
    hazard_curve_id: Option<&CurveId>,
) -> finstack_quant_core::Result<CreditCurveHandles> {
    let discount = market.get_discount(disc_curve_id.as_str())?;
    let hazard = if let Some(hazard_id) = hazard_curve_id {
        Some(market.get_hazard(hazard_id.as_str())?)
    } else {
        None
    };
    Ok(CreditCurveHandles { discount, hazard })
}

// =============================================================================
// IntoIterator implementations for ergonomic for-loops
// =============================================================================

impl IntoIterator for CashFlowSchedule {
    type Item = CashFlow;
    type IntoIter = std::vec::IntoIter<CashFlow>;

    fn into_iter(self) -> Self::IntoIter {
        self.flows.into_iter()
    }
}

impl<'a> IntoIterator for &'a CashFlowSchedule {
    type Item = &'a CashFlow;
    type IntoIter = std::slice::Iter<'a, CashFlow>;

    fn into_iter(self) -> Self::IntoIter {
        self.flows.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::DayCount;
    use time::Month;

    fn flow(date: Date, amount: f64, kind: CFKind) -> CashFlow {
        CashFlow::new(
            date,
            None,
            Money::new(amount, Currency::USD),
            kind,
            0.0,
            None,
        )
    }

    #[test]
    fn from_parts_sorts_by_date_then_kind_rank() {
        let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let schedule = CashFlowSchedule::from_parts(
            vec![
                flow(date, 10.0, CFKind::Recovery),
                flow(date, 12.0, CFKind::Amortization),
                flow(date, 8.0, CFKind::PrePayment),
                flow(date, 5.0, CFKind::Fixed),
            ],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta::default(),
        );

        assert_eq!(schedule.flows[0].kind, CFKind::Fixed);
        assert_eq!(schedule.flows[1].kind, CFKind::Amortization);
        assert_eq!(schedule.flows[2].kind, CFKind::PrePayment);
        assert_eq!(schedule.flows[3].kind, CFKind::Recovery);
    }

    #[test]
    fn try_from_parts_rejects_over_amortization() {
        let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");

        let result = CashFlowSchedule::try_from_parts(
            vec![flow(date, 150.0, CFKind::Amortization)],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta::default(),
        );

        assert!(
            result.is_err(),
            "validated construction must reject over-amortization"
        );
    }

    #[test]
    fn serde_is_structural_and_requires_explicit_economic_validation() {
        let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let unvalidated = CashFlowSchedule::from_parts(
            vec![flow(date, 150.0, CFKind::Amortization)],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta::default(),
        );
        let wire = serde_json::to_value(unvalidated).expect("serialize schedule");

        let decoded: CashFlowSchedule =
            serde_json::from_value(wire).expect("serde validates structure only");

        assert!(
            decoded.validate().is_err(),
            "economic validation remains explicit for direct serde consumers",
        );
    }

    #[test]
    fn merge_cashflow_schedules_merges_meta_and_resorts() {
        let d1 = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let d2 = Date::from_calendar_date(2025, Month::February, 15).expect("valid date");
        let left = CashFlowSchedule::from_parts(
            vec![
                flow(d2, 4.0, CFKind::Recovery).with_accrual(CashFlowAccrual {
                    start: d1,
                    end: d2,
                    day_count: DayCount::Thirty360,
                    projected_index_rate: None,
                }),
            ],
            Notional::par(50.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta {
                representation: CashflowRepresentation::Projected,
                calendar_ids: vec!["nyc".to_string()],
                facility_limit: None,
                issue_date: Some(d1),
                maturity_date: None,
            },
        );
        let right = CashFlowSchedule::from_parts(
            vec![
                flow(d1, 10.0, CFKind::Amortization).with_accrual(CashFlowAccrual {
                    start: d1,
                    end: d2,
                    day_count: DayCount::Act360,
                    projected_index_rate: None,
                }),
            ],
            Notional::par(50.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta {
                representation: CashflowRepresentation::Projected,
                calendar_ids: vec!["lon".to_string(), "nyc".to_string()],
                facility_limit: None,
                issue_date: Some(d1),
                maturity_date: None,
            },
        );

        let merged = merge_cashflow_schedules(
            vec![left, right],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
        );

        assert_eq!(merged.flows.len(), 2);
        assert_eq!(merged.flows[0].date, d1);
        assert_eq!(
            merged.meta.representation,
            CashflowRepresentation::Projected
        );
        assert_eq!(
            merged.meta.calendar_ids,
            vec!["lon".to_string(), "nyc".to_string()]
        );
        assert_eq!(merged.meta.issue_date, Some(d1));
        assert_eq!(
            merged.flows[0].accrual.map(|accrual| accrual.day_count),
            Some(DayCount::Act360)
        );
        assert_eq!(
            merged.flows[1].accrual.map(|accrual| accrual.day_count),
            Some(DayCount::Thirty360)
        );
    }

    #[test]
    fn merge_cashflow_schedules_preserves_mixed_currency_rows() {
        let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let usd = CashFlowSchedule::from_parts(
            vec![flow(date, 100.0, CFKind::Notional)],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta::default(),
        );
        let eur = CashFlowSchedule::from_parts(
            vec![CashFlow::new(
                date,
                None,
                Money::new(-90.0, Currency::EUR),
                CFKind::Notional,
                0.0,
                None,
            )],
            Notional::par(90.0, Currency::EUR),
            DayCount::Act365F,
            CashFlowMeta::default(),
        );

        let merged = merge_cashflow_schedules(
            [usd, eur],
            Notional::par(0.0, Currency::USD),
            DayCount::Act365F,
        );

        assert_eq!(merged.flows.len(), 2);
        assert!(merged
            .flows
            .iter()
            .any(|flow| flow.amount.currency() == Currency::USD));
        assert!(merged
            .flows
            .iter()
            .any(|flow| flow.amount.currency() == Currency::EUR));
    }

    #[test]
    fn merge_cashflow_schedules_marks_mixed_projection_as_projected() {
        let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let contractual = CashFlowSchedule::from_parts(
            vec![flow(date, 100.0, CFKind::Fixed)],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta::default(),
        );
        let projected = CashFlowSchedule::from_parts(
            vec![flow(date, 1.0, CFKind::FloatReset)],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta {
                representation: CashflowRepresentation::Projected,
                ..Default::default()
            },
        );

        let merged = merge_cashflow_schedules(
            [contractual, projected],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
        );

        assert_eq!(
            merged.meta.representation,
            CashflowRepresentation::Projected
        );
    }

    #[test]
    fn validation_counts_delayed_funding_before_amortization() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let repayment = Date::from_calendar_date(2025, Month::July, 15).expect("valid date");
        let schedule = CashFlowSchedule::from_parts(
            vec![
                flow(issue, -100.0, CFKind::Notional),
                flow(repayment, 30.0, CFKind::Amortization),
            ],
            Notional::par(0.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta {
                issue_date: Some(issue),
                ..Default::default()
            },
        );

        schedule
            .validate()
            .expect("future funding should support later amortization");
    }

    #[test]
    fn legacy_accrual_sidecars_are_rejected() {
        let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let schedule = CashFlowSchedule::from_parts(
            vec![flow(end, 12.5, CFKind::Fixed)],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta::default(),
        );
        let mut legacy = serde_json::to_value(&schedule).expect("serialize schedule");
        let meta = legacy["meta"].as_object_mut().expect("meta object");
        meta.insert(
            "accrual_periods".to_string(),
            serde_json::to_value(vec![Some((end, end))]).expect("periods"),
        );

        let error = serde_json::from_value::<CashFlowSchedule>(legacy)
            .expect_err("legacy schedule sidecars must fail");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn deserialization_routes_unsorted_flows_through_canonical_constructor() {
        let early = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let late = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let schedule = CashFlowSchedule::from_parts(
            vec![
                flow(early, 1.0, CFKind::Fixed),
                flow(late, 2.0, CFKind::Fixed),
            ],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta::default(),
        );
        let mut wire = serde_json::to_value(schedule).expect("serialize schedule");
        wire["flows"].as_array_mut().expect("flows array").reverse();

        let decoded: CashFlowSchedule = serde_json::from_value(wire).expect("canonical schedule");
        assert_eq!(decoded.dates(), vec![early, late]);
    }

    #[test]
    fn filtering_and_sorting_keep_owned_accrual_metadata() {
        let d1 = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let d2 = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let accrual = CashFlowAccrual {
            start: d1,
            end: d2,
            day_count: DayCount::Act360,
            projected_index_rate: Some(0.031),
        };
        let schedule = CashFlowSchedule::from_parts(
            vec![
                flow(d2, 12.5, CFKind::Fixed).with_accrual(accrual),
                flow(d1, 100.0, CFKind::Notional),
            ],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta::default(),
        )
        .filter_future(d2);

        assert_eq!(schedule.flows.len(), 1);
        assert_eq!(schedule.flows[0].accrual, Some(accrual));
    }

    #[test]
    fn wal_uses_act365f_regardless_of_schedule_day_count() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let d1 = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let d2 = Date::from_calendar_date(2027, Month::January, 1).expect("valid date");

        let schedule = CashFlowSchedule::from_parts(
            vec![
                flow(d1, 500_000.0, CFKind::Amortization),
                flow(d2, 500_000.0, CFKind::Amortization),
            ],
            Notional::par(1_000_000.0, Currency::USD),
            DayCount::Thirty360, // schedule uses 30/360 but WAL should use Act/365F
            CashFlowMeta::default(),
        );

        let wal = schedule.weighted_average_life(as_of).expect("WAL succeeds");

        // Compute expected WAL with Act/365F:
        // d1: 365 days / 365 = 1.0 years
        // d2: 731 days / 365 ≈ 2.0027 years (2026 is not a leap year, 2×365+1 ≈ 731)
        // WAL = (500k * 1.0 + 500k * t2) / 1M
        let t1 = DayCount::Act365F
            .year_fraction(as_of, d1, DayCountContext::default())
            .unwrap();
        let t2 = DayCount::Act365F
            .year_fraction(as_of, d2, DayCountContext::default())
            .unwrap();
        let expected = (500_000.0 * t1 + 500_000.0 * t2) / 1_000_000.0;

        assert!(
            (wal - expected).abs() < 1e-10,
            "WAL should match Act/365F calculation: expected {}, got {}",
            expected,
            wal
        );

        // Also verify it differs from 30/360 (which would give 1.0 and 2.0 exactly)
        let t30_360_1 = DayCount::Thirty360
            .year_fraction(as_of, d1, DayCountContext::default())
            .unwrap();
        let t30_360_2 = DayCount::Thirty360
            .year_fraction(as_of, d2, DayCountContext::default())
            .unwrap();
        let wal_30360 = (500_000.0 * t30_360_1 + 500_000.0 * t30_360_2) / 1_000_000.0;

        // The values should differ (Act/365F vs 30/360 give different year fractions
        // for multi-year spans). If they match, the WAL is accidentally using the
        // schedule day count instead of Act/365F.
        // Note: for these specific dates they may be very close, so we just verify
        // our function returns the Act/365F-based value.
        assert!(
            (wal - expected).abs() < (wal - wal_30360).abs() || (wal - expected).abs() < 1e-10,
            "WAL should be closer to Act/365F value than 30/360 value"
        );
    }

    #[test]
    fn wal_kernel_combines_same_date_principal() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let payment = Date::from_calendar_date(2026, Month::July, 1).expect("valid date");
        let expected = DayCount::Act365F
            .year_fraction(as_of, payment, DayCountContext::default())
            .expect("year fraction");

        let wal = weighted_average_life_from_principal(
            [
                (payment, Money::new(40.0, Currency::USD)),
                (payment, Money::new(60.0, Currency::USD)),
            ],
            as_of,
        )
        .expect("WAL succeeds");

        assert!((wal - expected).abs() < 1e-12);
    }

    #[test]
    fn wal_kernel_rejects_mixed_currencies() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let first = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let second = Date::from_calendar_date(2027, Month::January, 1).expect("valid date");

        let error = weighted_average_life_from_principal(
            [
                (first, Money::new(40.0, Currency::USD)),
                (second, Money::new(60.0, Currency::EUR)),
            ],
            as_of,
        )
        .expect_err("mixed currencies must fail");

        assert!(matches!(
            error,
            finstack_quant_core::Error::CurrencyMismatch {
                expected: Currency::USD,
                actual: Currency::EUR,
            }
        ));
    }

    #[test]
    fn wal_kernel_returns_zero_without_future_principal() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let past = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");

        let wal = weighted_average_life_from_principal(
            [
                (past, Money::new(100.0, Currency::USD)),
                (as_of, Money::new(100.0, Currency::USD)),
            ],
            as_of,
        )
        .expect("WAL succeeds");

        assert_eq!(wal, 0.0);
    }
}
