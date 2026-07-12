//! Schedule generation from the builder state.
//!
//! Provides the canonical `CashFlowSchedule` type and helpers for sorting and
//! deriving schedule metadata. Downstream pricing/risk code consumes this shape.

use crate::builder::Notional;
use crate::primitives::{is_cash_settlement_kind, CFKind, CashFlow};
use finstack_quant_core::cashflow::CashFlowAccrual;
use finstack_quant_core::cashflow::Discountable;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Period, PeriodId};
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
pub fn kind_rank(kind: CFKind) -> u8 {
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
pub fn sort_flows(flows: &mut [CashFlow]) {
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
        .map(|schedule| schedule.spec.calendar_id.clone())
        .chain(
            floating
                .iter()
                .map(|schedule| schedule.spec.rate_spec.calendar_id.clone()),
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
        schedule.spec.dc
    } else if let Some(schedule) = floating.first() {
        schedule.spec.rate_spec.dc
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
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct CashFlowSchedule {
    /// Ordered cashflows (coupons, principal payments, fees)
    pub flows: Vec<CashFlow>,
    /// Notional schedule (constant or amortizing)
    pub notional: Notional,
    /// Day count convention for interest calculations
    pub day_count: DayCount,
    /// Additional metadata (calendars, facility limits)
    pub meta: CashFlowMeta,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CashFlowScheduleWire {
    flows: Vec<CashFlow>,
    notional: Notional,
    day_count: DayCount,
    meta: CashFlowMetaWire,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CashFlowMetaWire {
    #[serde(default)]
    representation: CashflowRepresentation,
    calendar_ids: Vec<String>,
    facility_limit: Option<Money>,
    #[serde(default)]
    issue_date: Option<Date>,
    #[serde(default)]
    maturity_date: Option<Date>,
    #[serde(default)]
    accrual_periods: Vec<Option<(Date, Date)>>,
    #[serde(default)]
    accrual_day_counts: Vec<Option<DayCount>>,
}

impl TryFrom<CashFlowScheduleWire> for CashFlowSchedule {
    type Error = String;

    fn try_from(wire: CashFlowScheduleWire) -> Result<Self, Self::Error> {
        let CashFlowScheduleWire {
            mut flows,
            notional,
            day_count,
            meta,
        } = wire;
        let flow_count = flows.len();
        for (name, len) in [
            ("accrual_periods", meta.accrual_periods.len()),
            ("accrual_day_counts", meta.accrual_day_counts.len()),
        ] {
            if len != 0 && len != flow_count {
                return Err(format!(
                    "cashflow schedule metadata '{name}' has {len} entries for {flow_count} flows"
                ));
            }
        }

        if meta.accrual_periods.is_empty() && !meta.accrual_day_counts.is_empty() {
            return Err(
                "cashflow schedule has legacy accrual_day_counts without accrual_periods"
                    .to_string(),
            );
        }

        if !meta.accrual_periods.is_empty() {
            for (idx, flow) in flows.iter_mut().enumerate() {
                let period = meta.accrual_periods[idx];
                let legacy_day_count = meta
                    .accrual_day_counts
                    .get(idx)
                    .copied()
                    .flatten()
                    .unwrap_or(day_count);
                match period {
                    Some((start, end)) => {
                        let legacy = CashFlowAccrual {
                            start,
                            end,
                            day_count: legacy_day_count,
                            projected_index_rate: None,
                        };
                        if let Some(current) = flow.accrual {
                            if current.start != start
                                || current.end != end
                                || current.day_count != legacy_day_count
                            {
                                return Err(format!(
                                    "cashflow {idx} has conflicting canonical and legacy accrual metadata"
                                ));
                            }
                        } else {
                            flow.accrual = Some(legacy);
                        }
                    }
                    None => {
                        if meta
                            .accrual_day_counts
                            .get(idx)
                            .copied()
                            .flatten()
                            .is_some()
                        {
                            return Err(format!(
                                "cashflow {idx} has a legacy accrual day count without a period"
                            ));
                        }
                    }
                }
            }
        }

        Ok(Self {
            flows,
            notional,
            day_count,
            meta: CashFlowMeta {
                representation: meta.representation,
                calendar_ids: meta.calendar_ids,
                facility_limit: meta.facility_limit,
                issue_date: meta.issue_date,
                maturity_date: meta.maturity_date,
            },
        })
    }
}

impl<'de> serde::Deserialize<'de> for CashFlowSchedule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = CashFlowScheduleWire::deserialize(deserializer)?;
        Self::try_from(wire).map_err(serde::de::Error::custom)
    }
}

impl Discountable for CashFlowSchedule {
    type PVOutput = finstack_quant_core::Result<Money>;

    fn npv(&self, disc: &dyn Discounting, base: Date) -> finstack_quant_core::Result<Money> {
        // Compute NPV directly without allocating an intermediate Vec.
        // Two-pass approach: first find currency and check non-empty,
        // then compute the discounted sum.

        let mut ccy = None;
        let mut has_future_cash = false;

        // First pass: determine currency from eligible future cashflows.
        // Historical and non-cash state rows must not determine the result's
        // currency or cause an otherwise empty future schedule to fail.
        for cf in &self.flows {
            if cf.date <= base || !is_cash_settlement_kind(cf.kind) {
                continue;
            }
            has_future_cash = true;
            ccy = Some(cf.amount.currency());
            break;
        }

        if !has_future_cash {
            return Money::try_new(0.0, self.notional.initial.currency());
        }
        let Some(ccy) = ccy else {
            return Err(finstack_quant_core::error::InputError::TooFewPoints.into());
        };
        let day_count = disc.day_count();
        let curve_base = disc.base_date();
        let ctx = finstack_quant_core::dates::DayCountContext::default();
        let t_base = day_count.signed_year_fraction(curve_base, base, ctx)?;
        let df_base = disc.df(t_base);

        if !df_base.is_finite() || df_base <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "npv: discount factor at the valuation date ({base}) is invalid: {df_base}"
            )));
        }

        // Second pass: accumulate discounted amounts. `Money` is `Decimal`-backed,
        // so summing per-flow `Money` values pays a `Decimal` multiply + add each
        // iteration; since `df` is already `f64`, accumulate the discounted
        // amounts with the same Neumaier-compensated `f64` policy used by the
        // aggregation module and materialize `Money` once at the end.
        let inv_df_base = 1.0 / df_base;
        let mut acc = finstack_quant_core::math::summation::NeumaierAccumulator::default();
        for cf in &self.flows {
            if cf.date <= base || !is_cash_settlement_kind(cf.kind) {
                continue;
            }
            if cf.amount.currency() != ccy {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected: ccy,
                    actual: cf.amount.currency(),
                });
            }
            let t = day_count.signed_year_fraction(curve_base, cf.date, ctx)?;
            let df = disc.df(t) * inv_df_base;
            acc.add(cf.amount.amount() * df);
        }

        Money::try_new(acc.total(), ccy)
    }
}

impl CashFlowSchedule {
    /// Internal raw constructor for already-classified flows.
    pub(crate) fn from_parts(
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
    /// use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec};
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
    ///     freq: Tenor::semi_annual(),
    ///     dc: DayCount::Act365F,
    ///     bdc: BusinessDayConvention::Following,
    ///     calendar_id: "weekends_only".to_string(),
    ///     end_of_month: false,
    ///     payment_lag_days: 0,
    ///     stub: StubKind::None,
    /// };
    ///
    /// let schedule = CashFlowSchedule::builder()
    ///     .principal(notional, issue, maturity)
    ///     .fixed_cf(spec)
    ///     .build_with_curves(None)?;
    /// # let _ = schedule;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn builder() -> super::CashFlowBuilder {
        super::CashFlowBuilder::default()
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
    /// 4. Attach the given representation tag
    #[must_use]
    pub fn normalize_public(self, as_of: Date, representation: CashflowRepresentation) -> Self {
        let mut normalized = self.filter_future(as_of).omit_pure_pik();
        sort_schedule_with_metadata(&mut normalized);
        normalized.meta.representation = representation;
        normalized
    }

    /// Outstanding principal path tracking Amortization and PIK flows only.
    ///
    /// This method provides a simplified balance view suitable for coupon calculations
    /// where the accrual base tracks principal reductions (Amortization) and PIK
    /// capitalizations, but **excludes** ad-hoc notional draws/repays.
    ///
    /// Returns one entry per cashflow, tracking the outstanding balance after
    /// each flow is processed. Useful for debugging and detailed analysis.
    ///
    /// # When to Use Each Method
    ///
    /// - **`outstanding_path_per_flow()`**: Use for coupon accrual calculations on fixed
    ///   amortization schedules (bonds, term loans with scheduled amortization).
    /// - **[`Self::outstanding_by_date()`]**: Use for full balance tracking including
    ///   notional events (revolving credit facilities, delayed draws, prepayments).
    ///
    /// Note: Amortization amounts in the schedule are stored as POSITIVE values
    /// (the builder internally manages the reduction of outstanding balance).
    /// PIK amounts are positive and increase outstanding.
    ///
    /// # Negative Balances
    ///
    /// Replayed balances are permitted to go negative (e.g. amortization
    /// exceeding the tracked outstanding); a `tracing::warn!` is emitted when
    /// the balance drops below a small negative tolerance, but no error is
    /// raised. The builder validates over-repayment at construction time.
    ///
    /// # Errors
    ///
    /// Returns error if there is a currency mismatch between flows and the
    /// notional.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_cashflows::builder::schedule::{CashFlowMeta, CashFlowSchedule};
    /// use finstack_quant_core::cashflow::{CashFlow, CFKind};
    /// use finstack_quant_cashflows::builder::Notional;
    /// use time::Month;
    ///
    /// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    /// let notional = Notional { initial: Money::new(100.0, Currency::USD), amort: Default::default() };
    /// let flows = vec![
    ///   CashFlow::new(base, None, Money::new(10.0, Currency::USD), CFKind::Amortization, 0.0, None),
    ///   CashFlow::new(base, None, Money::new(5.0, Currency::USD), CFKind::PIK, 0.0, None),
    /// ];
    /// let s = CashFlowSchedule { flows, notional, day_count: finstack_quant_core::dates::DayCount::Act365F, meta: CashFlowMeta::default() };
    /// let path = s.outstanding_path_per_flow().expect("valid schedule");
    /// assert_eq!(path.len(), 2);
    /// assert_eq!(path[0].1.amount(), 90.0);  // 100 - 10 = 90
    /// assert_eq!(path[1].1.amount(), 95.0);  // 90 + 5 = 95
    /// ```
    pub fn outstanding_path_per_flow(&self) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        let mut out = Vec::with_capacity(self.flows.len());
        let mut outstanding = self.notional.initial;
        for cf in &self.flows {
            // `outstanding_path_per_flow` historically ignored notional draws/repays and
            // only tracked Amortization and PIK. Preserve that behavior by
            // passing `include_notional = false`.
            apply_flow_to_outstanding(&mut outstanding, cf, false, false)?;
            out.push((cf.date, outstanding));
        }
        Ok(out)
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
    /// let schedule = CashFlowSchedule {
    ///     flows,
    ///     notional: Notional::par(1_000_000.0, Currency::USD),
    ///     day_count: DayCount::Act365F,
    ///     meta: CashFlowMeta::default(),
    /// };
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
        let (principal_time_sum, principal_total) = self
            .flows
            .iter()
            .filter(|cf| {
                matches!(
                    cf.kind,
                    CFKind::Amortization | CFKind::Notional | CFKind::PrePayment
                ) && cf.date > as_of
                    && cf.amount.amount() > 0.0
            })
            .try_fold((0.0_f64, 0.0_f64), |(pts, pt), cf| {
                let t =
                    DayCount::Act365F.year_fraction(as_of, cf.date, DayCountContext::default())?;
                let a = cf.amount.amount();
                Ok::<_, finstack_quant_core::Error>((pts + a * t, pt + a))
            })?;

        if principal_total > 0.0 {
            Ok(principal_time_sum / principal_total)
        } else {
            Ok(0.0)
        }
    }

    /// Full outstanding path including Amortization, PIK, and Notional draws/repays.
    ///
    /// Returns one entry per unique date after applying all balance-affecting flows
    /// on that date. This is the **canonical method** for tracking outstanding balance
    /// in instruments with dynamic draws/repays (RCFs, delayed-draw term loans).
    ///
    /// # When to Use Each Method
    ///
    /// - **[`Self::outstanding_path_per_flow()`]**: Simplified view for scheduled amortization
    ///   (excludes Notional draws/repays).
    /// - **`outstanding_by_date()`**: Full balance tracking including all notional events.
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
        let mut result: Vec<(Date, Money)> = Vec::with_capacity(self.flows.len());
        if self.flows.is_empty() {
            return Ok(result);
        }
        let issue = self.meta.issue_date.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "outstanding_by_date: schedule.meta.issue_date is required to identify the initial funding flow"
                    .into(),
            )
        })?;

        let mut outstanding = self.notional.initial;

        // Identify and skip the initial funding flow (negative notional equal to initial).
        // This flow is already accounted for in `notional.initial`, and may not be the
        // earliest flow if there are pre-issue principal events.
        let mut initial_funding_skipped = false;
        let initial_amount = self.notional.initial.amount();

        for same_day_flows in self.flows.chunk_by(|left, right| left.date == right.date) {
            let d = same_day_flows[0].date;
            // Process all flows on this date in their deterministic order.
            for cf in same_day_flows {
                let is_initial_funding =
                    is_initial_funding_flow(cf, issue, initial_amount, initial_funding_skipped);
                if is_initial_funding {
                    initial_funding_skipped = true;
                }

                // `outstanding_by_date` is the canonical balance tracker, including
                // subsequent notional draws/repays as well as Amortization and PIK.
                apply_flow_to_outstanding(&mut outstanding, cf, is_initial_funding, true)?;
            }
            result.push((d, outstanding));
        }

        Ok(result)
    }
}

fn retain_schedule_flows(schedule: &mut CashFlowSchedule, mut keep: impl FnMut(&CashFlow) -> bool) {
    schedule.flows.retain(|flow| keep(flow));
}

/// Sort a schedule using the canonical self-contained flow order.
pub(crate) fn sort_schedule_with_metadata(schedule: &mut CashFlowSchedule) {
    sort_flows(&mut schedule.flows);
}

fn merge_matching<T: Copy + PartialEq>(current: &mut Option<T>, value: T, fallback: T) {
    *current = Some(match *current {
        None => value,
        Some(existing) if existing == value => existing,
        Some(_) => fallback,
    });
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
/// Concatenates flows from every input schedule, deduplicates the union of
/// their `meta.calendar_ids`, and reduces remaining metadata fields with the
/// rules listed below. The combined flow list is then re-sorted via
/// [`sort_flows`] so the resulting schedule is in canonical order.
///
/// # Arguments
///
/// * `schedules` - Iterable of [`CashFlowSchedule`] values to combine.
/// * `notional` - Notional stamped on the merged schedule. The caller is
///   responsible for choosing a notional that makes sense for the composite
///   (this function does not aggregate input notionals).
/// * `day_count` - Day count convention attached to the merged schedule.
///
/// # Returns
///
/// A single [`CashFlowSchedule`] containing every input flow, sorted, with
/// merged metadata.
///
/// # Metadata merge rules
///
/// - `representation`: collapses to the common value if every input agrees,
///   otherwise falls back to [`CashflowRepresentation::default()`]
///   (`Contractual`).
/// - `calendar_ids`: union of all inputs, sorted and deduplicated.
/// - `facility_limit`: kept only if every input agrees; mismatches → `None`.
/// - `issue_date`: kept only if every input agrees; mismatches → `None`.
///
/// Empty input yields an empty schedule with default metadata.
pub fn merge_cashflow_schedules<I>(
    schedules: I,
    notional: Notional,
    day_count: DayCount,
) -> finstack_quant_core::Result<CashFlowSchedule>
where
    I: IntoIterator<Item = CashFlowSchedule>,
{
    let expected_ccy = notional.initial.currency();
    let mut flows = Vec::new();
    let mut calendar_ids = Vec::new();
    let mut facility_limit: Option<Option<Money>> = None;
    let mut issue_date: Option<Option<Date>> = None;
    let mut maturity_date: Option<Option<Date>> = None;
    let mut representation: Option<CashflowRepresentation> = None;

    for schedule in schedules {
        // Reject any flow whose currency does not match the merged-notional
        // currency. Silent stamping of a wrong notional currency on
        // mismatched flows would corrupt downstream PV/aggregation.
        for cf in &schedule.flows {
            if cf.amount.currency() != expected_ccy {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected: expected_ccy,
                    actual: cf.amount.currency(),
                });
            }
        }
        merge_matching(
            &mut representation,
            schedule.meta.representation,
            CashflowRepresentation::default(),
        );
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

    sort_flows(&mut flows);

    Ok(CashFlowSchedule::from_parts(
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
    ))
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
/// 1. **Emission time** — the builder pipeline mutates a live balance as it
///    generates flows (e.g. [`crate::builder::emit_default_on`] subtracts the
///    defaulted amount when it emits a `DefaultedNotional` flow).
/// 2. **Reconstruction time** — this function, driven by
///    [`CashFlowSchedule::outstanding_by_date`] /
///    [`CashFlowSchedule::outstanding_path_per_flow`], rebuilds the balance
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
        )
        .expect("matched currencies should merge");

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
    fn legacy_accrual_sidecars_deserialize_into_flows_only() {
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
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
            serde_json::to_value(vec![Some((start, end))]).expect("periods"),
        );
        meta.insert(
            "accrual_day_counts".to_string(),
            serde_json::to_value(vec![Some(DayCount::Act360)]).expect("day counts"),
        );

        let decoded: CashFlowSchedule = serde_json::from_value(legacy).expect("legacy schedule");
        assert_eq!(
            decoded.flows[0].accrual,
            Some(CashFlowAccrual {
                start,
                end,
                day_count: DayCount::Act360,
                projected_index_rate: None,
            })
        );
        let canonical = serde_json::to_value(decoded).expect("canonical schedule");
        assert!(canonical["meta"].get("accrual_periods").is_none());
        assert!(canonical["meta"].get("accrual_day_counts").is_none());
    }

    #[test]
    fn legacy_accrual_sidecars_reject_misaligned_lengths() {
        let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let schedule = CashFlowSchedule::from_parts(
            vec![flow(date, 12.5, CFKind::Fixed)],
            Notional::par(100.0, Currency::USD),
            DayCount::Act365F,
            CashFlowMeta::default(),
        );
        let mut legacy = serde_json::to_value(&schedule).expect("serialize schedule");
        legacy["meta"]["accrual_periods"] =
            serde_json::to_value(vec![Some((date, date + time::Duration::days(1))), None])
                .expect("periods");

        let error = serde_json::from_value::<CashFlowSchedule>(legacy)
            .expect_err("misaligned sidecars must fail");
        assert!(error.to_string().contains("2 entries for 1 flows"));
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
}
