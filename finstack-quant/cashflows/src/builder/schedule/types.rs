//! [`CashFlowSchedule`], [`CashFlowMeta`], and primary schedule operations.

use super::merge::{apply_flow_to_outstanding, settle_rebased_balance};
use super::ordering::*;
use crate::aggregation::weighted_average_life_from_principal;
use crate::builder::Notional;
use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowParams};
use finstack_quant_core::dates::{Date, DateExt, DayCount};
use finstack_quant_core::money::Money;

/// Meaning of the emitted schedule relative to pricing and waterfall policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
/// Carries the representation tag, the issue and maturity dates, projected
/// fixings for time rolls, and two informational fields (`calendar_ids`,
/// `commitment`) that no Rust engine reads.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CashFlowMeta {
    /// Raw index observations used by projected floating coupons, before spread,
    /// gearing, caps and floors; retained for deterministic time-roll fixings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projected_fixings: Vec<crate::fixings::ProjectedFixing>,
    /// Meaning of the schedule relative to waterfall policy.
    #[serde(default)]
    pub representation: CashflowRepresentation,
    /// Holiday calendar IDs used for schedule adjustments.
    ///
    /// Informational: carried through merge and host views, but no Rust
    /// engine reads it; accrual uses each flow's `accrual.calendar_id`.
    pub calendar_ids: Vec<String>,
    /// Total facility commitment for revolving and delayed-draw facilities;
    /// `None` for instruments without one.
    ///
    /// Informational: carried through merge and host views, but no Rust
    /// engine reads it.
    pub commitment: Option<Money>,
    /// Issue date of the instrument.
    ///
    /// Required by accrual and by `outstanding_by_date` /
    /// `outstanding_at_dates`, which return an error when it is missing rather
    /// than infer the first coupon period start from flow dates. `validate`
    /// and the public lifecycle normalization instead fall back to the first
    /// flow date to locate the issue funding row. Merging schedules keeps it
    /// only when every input carries the same date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub issue_date: Option<Date>,
    /// Contractual maturity date, distinct from an adjusted final payment date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub maturity: Option<Date>,
}

/// Cashflow schedule output from the composable builder.
///
/// Contains ordered cashflows plus notional and a representative `DayCount`.
/// Methods provide convenient accessors commonly used by pricing and analysis.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CashFlowSchedule {
    /// Ordered cashflows (coupons, principal payments, fees)
    #[serde(deserialize_with = "deserialize_sorted_flows")]
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<CashFlow>"))]
    pub(crate) flows: Vec<CashFlow>,
    /// Notional schedule (constant or amortizing)
    pub(crate) notional: Notional,
    /// Day count convention for interest calculations
    pub(crate) day_count: DayCount,
    /// Additional metadata (calendars, facility limits)
    pub(crate) meta: CashFlowMeta,
}

pub(super) fn deserialize_sorted_flows<'de, D>(deserializer: D) -> Result<Vec<CashFlow>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut flows = <Vec<CashFlow> as serde::Deserialize>::deserialize(deserializer)?;
    flows.sort_by(compare_flows);
    Ok(flows)
}

impl CashFlowSchedule {
    /// Construct a canonical schedule from classified flow rows.
    ///
    /// Every [`CashFlow`] is already valid (record CORE-CONVENTIONS-015 #10),
    /// so construction only sorts the rows into canonical schedule order.
    /// Cross-flow economic invariants (principal reconciliation, metadata
    /// dates) are not checked here; call [`Self::validate`] for those.
    ///
    /// # Arguments
    ///
    /// * `flows` - Classified cash, PIK, and notional rows in any order, for
    ///   example rows read from a table or deserialized from a host.
    /// * `notional` - Representative notional stamped on the schedule.
    /// * `day_count` - Day-count convention converting date pairs into year fractions.
    /// * `meta` - Schedule-level calendars, facility limit, issue/maturity
    ///   dates, and representation tag stamped on the result.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
    /// use finstack_quant_core::cashflow::{CFKind, CashFlowParams};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, DayCount};
    /// use finstack_quant_core::money::Money;
    /// use time::Month;
    ///
    /// let date = Date::from_calendar_date(2025, Month::June, 15)?;
    /// let usd = |x| Money::new(x, Currency::USD);
    /// let coupon = CashFlowParams::new(date, None, usd(25.0)?, CFKind::Fixed, 0.25, Some(0.1));
    /// let schedule = CashFlowSchedule::from_flows(
    ///     vec![coupon.clone().build()?],
    ///     Notional::par(1_000.0, Currency::USD)?,
    ///     DayCount::Act360,
    ///     CashFlowMeta::default(),
    /// );
    /// assert_eq!(schedule.get_flows().len(), 1);
    ///
    /// // An invalid row never becomes a `CashFlow`.
    /// assert!(CashFlowParams { accrual_factor: -1.0, ..coupon }.build().is_err());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn from_flows(
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
        schedule.flows.sort_by(compare_flows);
        schedule
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
    ///
    /// Builder-produced schedules take this from the first fixed coupon leg,
    /// else the first floating leg; schedules built with no coupon leg
    /// (principal-only) default to [`DayCount::Act365F`].
    #[must_use]
    pub fn get_day_count(&self) -> DayCount {
        self.day_count
    }

    /// Return schedule-level metadata.
    #[must_use]
    pub fn get_meta(&self) -> &CashFlowMeta {
        &self.meta
    }

    /// Rebuild every cashflow from edited inputs, atomically.
    ///
    /// Each row's [`CashFlowParams`] are passed to `update` in the current
    /// order and rebuilt through validation (record CORE-CONVENTIONS-015
    /// #10); the schedule is replaced and re-sorted only when every row
    /// succeeds, so on error it is unchanged.
    ///
    /// # Arguments
    ///
    /// * `update` - Closure editing one row's inputs in place; returning an
    ///   error stops the update.
    ///
    /// # Errors
    ///
    /// Propagates the first error from `update` or from rebuilding a row.
    pub fn update_flows(
        &mut self,
        mut update: impl FnMut(&mut CashFlowParams) -> finstack_quant_core::Result<()>,
    ) -> finstack_quant_core::Result<()> {
        let flows = self
            .flows
            .iter()
            .map(|flow| {
                let mut params = CashFlowParams::from(flow);
                update(&mut params)?;
                params.build()
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;
        self.flows = flows;
        self.flows.sort_by(compare_flows);
        Ok(())
    }

    /// Retain matching cashflows while preserving canonical ordering.
    pub fn retain_flows(&mut self, keep: impl FnMut(&CashFlow) -> bool) {
        self.flows.retain(keep);
    }

    /// Add one cashflow and restore canonical ordering.
    pub fn push_flow(&mut self, flow: CashFlow) {
        self.flows.push(flow);
        self.flows.sort_by(compare_flows);
    }

    /// Start a cashflow builder.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams};
    /// use rust_decimal_macros::dec;
    /// use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let issue = Date::from_calendar_date(2025, Month::January, 15)?;
    /// let maturity = Date::from_calendar_date(2026, Month::January, 15)?;
    ///
    /// let notional = Money::from((1_000_000_i64, Currency::USD));
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
    pub fn builder() -> super::super::CashFlowBuilder {
        super::super::CashFlowBuilder::default()
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
    /// `scale` multiplies every `Money` amount, including principal, interest,
    /// fees, and recoveries, but leaves the representative notional and all
    /// flow classification/date metadata unchanged. A negative scale reverses
    /// the cashflow direction.
    ///
    /// # Errors
    ///
    /// Returns an error if `scale` is non-finite, cannot be represented as a
    /// decimal, or any scaled amount exceeds the `Money` decimal range.
    /// Because the schedule is consumed, no modified schedule is returned on error.
    ///
    /// # Arguments
    ///
    /// * `scale` - Finite dimensionless multiplier applied to cash amounts and
    ///   explicit principal deltas; negative values reverse their direction.
    pub fn scale_amounts(mut self, scale: f64) -> finstack_quant_core::Result<Self> {
        if !scale.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "cashflow amount scale must be finite".to_string(),
            ));
        }
        for flow in &mut self.flows {
            let mut params = CashFlowParams::from(&*flow);
            params.amount = params.amount.checked_mul_f64(scale)?;
            if let Some(delta) = &mut params.principal_delta {
                *delta = delta.checked_mul_f64(scale)?;
            }
            *flow = params.build()?;
        }
        self.flows.sort_by(compare_flows);
        Ok(self)
    }

    /// Returns the list of dates for all flows in schedule order.
    pub fn dates(&self) -> Vec<Date> {
        self.flows.iter().map(|cf| cf.get_date()).collect()
    }

    /// Validate all schedule-level and per-flow invariants.
    ///
    /// This checks the representative notional, each flow, nondecreasing flow
    /// dates, and the cross-flow economic invariants that reconcile funding,
    /// outstanding balances, currencies, and the recorded schedule metadata.
    /// Use this after low-level mutation or deserializing state that bypassed
    /// construction-time validation.
    ///
    /// # Errors
    ///
    /// Returns the first invalid-notional or invalid-flow error, an error for
    /// out-of-order dates, or an error when a cross-flow economic invariant is
    /// violated.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.validate_structure()?;
        let _ = self.replay_balances()?;
        self.validate_dates()
    }

    fn validate_structure(&self) -> finstack_quant_core::Result<()> {
        self.notional.validate()?;
        if self
            .flows
            .windows(2)
            .any(|w| w[0].get_date() > w[1].get_date())
        {
            return Err(finstack_quant_core::Error::Validation(
                "cashflow schedule flows must be sorted by date".into(),
            ));
        }
        Ok(())
    }

    fn has_multiple_principal_currencies(&self) -> bool {
        let mut principal_currency = None;
        for flow in &self.flows {
            let Some(delta) = flow.get_principal_delta() else {
                continue;
            };
            if delta.amount_decimal().is_zero() {
                continue;
            }
            if let Some(currency) = principal_currency {
                if currency != delta.currency() {
                    return true;
                }
            } else {
                principal_currency = Some(delta.currency());
            }
        }
        false
    }

    /// Validate native settlement rows without imposing a scalar balance on
    /// a composite that has principal movements in multiple currencies.
    /// Single-currency principal paths retain full repayment reconciliation;
    /// a foreign coupon or fee does not exempt that path from validation.
    fn validate_native_cash(&self) -> finstack_quant_core::Result<()> {
        self.validate_structure()?;
        if !self.has_multiple_principal_currencies() {
            let _ = self.replay_balances()?;
        }
        self.validate_dates()
    }

    fn validate_dates(&self) -> finstack_quant_core::Result<()> {
        if let Some(issue_date) = self.meta.issue_date {
            let long_horizon = issue_date.add_months(1200)?;
            for flow in &self.flows {
                let interest_bearing = matches!(
                    flow.get_kind(),
                    CFKind::Fixed
                        | CFKind::Floating
                        | CFKind::InflationCoupon
                        | CFKind::Pik
                        | CFKind::Stub
                );
                if flow.get_date() < issue_date && interest_bearing {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "interest-bearing cashflow ({:?}) dated {} is before issue date {}",
                        flow.get_kind(),
                        flow.get_date(),
                        issue_date
                    )));
                }
                if flow.get_date() > long_horizon {
                    tracing::warn!(
                        flow_date = %flow.get_date(),
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
    pub(crate) fn normalize_public(mut self, as_of: Date) -> finstack_quant_core::Result<Self> {
        if !self.has_multiple_principal_currencies() {
            let initial = self.notional.initial;
            let non_negative_initial = initial.amount_decimal() >= rust_decimal::Decimal::ZERO;
            let mut balance = initial;
            let mut max_scale = initial.amount().abs().max(1.0);
            let order = self
                .balance_replay_order(|flow| flow.get_amount().currency() == initial.currency());
            let mut pending_principal = Vec::new();
            let mut prev_pre_as_of_date: Option<Date> = None;
            for (index, balance_date) in order {
                let flow = &mut self.flows[index];
                let delta = flow.get_principal_delta();
                if balance_date < as_of {
                    if let Some(prev_date) = prev_pre_as_of_date
                        && prev_date != balance_date
                        && non_negative_initial
                    {
                        settle_rebased_balance(
                            &mut balance,
                            initial.currency(),
                            max_scale,
                            prev_date,
                            as_of,
                        )?;
                    }
                    prev_pre_as_of_date = Some(balance_date);
                    if let Some(delta) = delta {
                        balance = balance.checked_add(delta)?;
                        max_scale = max_scale.max(balance.amount().abs());
                        // The cash can still be pending after its economic
                        // movement has entered the opening balance.
                        if flow.get_date() >= as_of {
                            *flow = flow.with_principal(
                                Money::from((0_i64, initial.currency())),
                                balance_date,
                            )?;
                        }
                    }
                } else if flow.get_date() < as_of
                    && let Some(delta) = delta.filter(|delta| delta.amount() != 0.0)
                {
                    // Cash settled before its economic boundary. Keep the
                    // future balance movement without paying that cash again.
                    let date = balance_date;
                    pending_principal.push(
                        CashFlowParams::new(
                            date,
                            None,
                            Money::from((0_i64, initial.currency())),
                            CFKind::Notional,
                            0.0,
                            None,
                        )
                        .with_principal_delta(delta)
                        .with_principal_date(date)
                        .build()?,
                    );
                }
            }
            if non_negative_initial {
                let check_date = prev_pre_as_of_date.unwrap_or(as_of);
                settle_rebased_balance(
                    &mut balance,
                    initial.currency(),
                    max_scale,
                    check_date,
                    as_of,
                )?;
            }
            self.notional.initial = balance;
            self.flows.extend(pending_principal);
        }
        // The remaining rows already resolve the contractual amortization.
        // An inception recipe cannot be reapplied to the rebased opening balance.
        self.notional.amort = super::super::AmortizationSpec::None;
        self.flows.retain(|cf| cf.get_date() >= as_of);
        self.flows.sort_by(compare_flows);
        Ok(self)
    }

    /// Get an iterator over interest-like coupon cashflows.
    ///
    /// Filters the schedule via [`CFKind::is_coupon`] — currently
    /// `Fixed`, `Floating`, `InflationCoupon`, and `Stub`. PIK, fees,
    /// amortization, recovery, and notional flows are excluded.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
    /// use finstack_quant_core::cashflow::{CashFlowParams, CFKind};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, DayCount};
    /// use finstack_quant_core::money::Money;
    /// use time::Month;
    ///
    /// let date = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
    /// let flows = vec![
    ///     CashFlowParams::new(date, None, Money::from((50_000_i64, Currency::USD)), CFKind::Fixed, 0.5, Some(0.05))
    ///         .build()?,
    ///     CashFlowParams::new(date, None, Money::from((100_000_i64, Currency::USD)), CFKind::Amortization, 0.0, None)
    ///         .with_principal_delta(Money::from((-100_000_i64, Currency::USD)))
    ///         .with_principal_date(date)
    ///         .build()?,
    /// ];
    /// let schedule = CashFlowSchedule::from_flows(
    ///     flows,
    ///     Notional::par(1_000_000.0, Currency::USD).expect("valid notional fixture"),
    ///     DayCount::Act365F,
    ///     CashFlowMeta::default(),
    /// );
    ///
    /// let coupons: Vec<_> = schedule.coupons().collect();
    /// assert_eq!(coupons.len(), 1);
    /// assert_eq!(coupons[0].get_kind(), CFKind::Fixed);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn coupons(&self) -> impl Iterator<Item = &CashFlow> {
        self.flows.iter().filter(|cf| cf.get_kind().is_coupon())
    }

    /// Weighted Average Life (WAL) in years from `as_of`.
    ///
    /// WAL = Σ(principal_i × t_i) / Σ(principal_i)
    ///
    /// where the sum runs over principal repayment flows (Amortization,
    /// Notional, PrePayment, RevolvingRepayment). Each is timed at its
    /// balance-effective date (`principal_date`, else the payment date) and
    /// weighted by its par principal (the negated `principal_delta`, else the
    /// cash amount), so a payment lag or a premium/OID cash amount does not
    /// move WAL. t_i is the year fraction from `as_of` to that date; only
    /// positive amounts dated after `as_of` contribute.
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
    ///   the market standard time metric. `docs/REFERENCES.md#sifma-mbs-standard-formulas`
    /// - Fabozzi, *The Handbook of Fixed Income Securities* (8th ed.,
    ///   2012), ch. 24, "Mortgage-Backed Securities", WAL definition. `docs/REFERENCES.md#fabozzi-fixed-income-handbook`
    ///
    /// Returns `Ok(0.0)` if there are no future principal flows.
    ///
    /// # Errors
    ///
    /// Returns `CurrencyMismatch` if contributing repayments use more than one
    /// currency, or an error if the day-count year-fraction calculation fails.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date; only principal repayments whose
    ///   balance-effective date is strictly after it contribute to Act/365F
    ///   weighted average life.
    pub fn wal(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
        // The repaid par is the explicit principal decrease, not the cash
        // (record CORE-CONVENTIONS-015 #3, #4).
        let repayments = self
            .flows
            .iter()
            .filter(|cf| cf.get_kind().is_repayment_like())
            .map(|cf| {
                cf.get_principal_delta()
                    .zip(cf.get_balance_date())
                    .map(|(delta, date)| (date, delta.checked_neg()))
                    .ok_or_else(|| crate::aggregation::missing_principal_delta(cf))
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;
        weighted_average_life_from_principal(repayments, as_of)
    }

    /// Outstanding balance after each unique date, including amortization, PIK, and draws/repays.
    ///
    /// # Balance Changes
    ///
    /// - **Amortization**: Reduces outstanding (stored as positive amounts)
    /// - **PIK**: Increases outstanding (capitalizes into principal)
    /// - **Notional**: Draws are negative (increase outstanding), repays are positive
    ///
    /// Every principal row moves the balance by its explicit delta from the
    /// opening `notional.initial`; an issue funding row whose principal the
    /// opening notional already holds carries a zero delta (record
    /// CORE-CONVENTIONS-015 #4, #5).
    ///
    /// # Negative Balances
    ///
    /// Replays starting from a non-negative opening notional must stay
    /// non-negative at the close of every economic balance date (up to `1e-9`
    /// floating-point rounding dust, which is settled to zero); over-repayments
    /// return [`finstack_quant_core::Error::Validation`].
    ///
    /// # Errors
    ///
    /// Returns a validation error if a non-negative notional goes negative
    /// at the close of any balance date, or on a currency mismatch between
    /// flows and notional or decimal overflow during replay.
    pub fn outstanding_by_date(&self) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        self.replay_balances()
    }

    /// Extract currency-tagged settlement cashflows `(Date, Money)` from this schedule.
    ///
    /// Preserves schedule order and payment dates. PIK capitalizations and
    /// default write-downs are non-cash state changes and are omitted.
    ///
    /// # Returns
    ///
    /// Validated `Vec<(Date, Money)>` in schedule order.
    ///
    /// # Errors
    ///
    /// Returns the schedule validation error for invalid amounts, accrual
    /// metadata, currencies within a row, dates, or single-currency principal movements.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::{schedule_from_dated_flows, ScheduleBuildOpts};
    /// use finstack_quant_core::cashflow::CFKind;
    /// use finstack_quant_core::{currency::Currency, dates::{Date, DayCount}, money::Money};
    /// use time::Month;
    ///
    /// let date = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
    /// let schedule = schedule_from_dated_flows(
    ///     vec![(date, Money::from((100_i64, Currency::USD)))],
    ///     CFKind::Fixed,
    ///     DayCount::Act360,
    ///     ScheduleBuildOpts::default(),
    /// )?;
    /// assert_eq!(schedule.dated_flows()?[0].1.currency(), Currency::USD);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn dated_flows(&self) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        self.validate_native_cash()?;
        Ok(self.settlement_rows())
    }

    pub(crate) fn settlement_rows(&self) -> Vec<(Date, Money)> {
        self.flows
            .iter()
            .filter(|flow| flow.get_kind().is_cash_settlement())
            // A zero-cash row that moves principal is a balance marker, not a
            // settlement; a zero settlement that moves nothing stays.
            .filter(|flow| {
                flow.get_amount().amount() != 0.0
                    || flow
                        .get_principal_delta()
                        .is_none_or(|delta| delta.amount() == 0.0)
            })
            .map(|flow| (flow.get_date(), flow.get_amount()))
            .collect()
    }

    /// Return the outstanding principal at each requested date.
    ///
    /// Replays economic principal dates once, then joins sorted query dates
    /// to the path in linear time. Before its first event, the balance is the
    /// schedule's opening notional; on an event date, its closing balance is used.
    ///
    /// # Arguments
    ///
    /// * `dates` - Nondecreasing balance snapshot dates; duplicate dates return
    ///   duplicate balances. Empty input returns an empty vector.
    ///
    /// # Returns
    ///
    /// Principal balances in the representative notional currency, in the
    /// same order and with the same length as `dates`.
    ///
    /// # Errors
    ///
    /// Returns a validation error for unsorted dates or missing issue metadata,
    /// and propagates currency mismatches or decimal overflow from balance replay.
    pub fn outstanding_at_dates(&self, dates: &[Date]) -> finstack_quant_core::Result<Vec<Money>> {
        if dates.windows(2).any(|pair| pair[0] > pair[1]) {
            return Err(finstack_quant_core::Error::Validation(
                "outstanding snapshot dates must be sorted".into(),
            ));
        }
        if dates.is_empty() {
            return Ok(Vec::new());
        }
        let path = self.outstanding_by_date()?;
        let mut balance = self.notional.initial;
        let mut events = path.iter().peekable();
        let mut balances = Vec::with_capacity(dates.len());
        for &date in dates {
            while let Some(&&(event_date, event_balance)) = events.peek() {
                if event_date > date {
                    break;
                }
                balance = event_balance;
                events.next();
            }
            balances.push(balance);
        }
        Ok(balances)
    }

    fn validate_closing_balances(
        initial: Money,
        closing: &mut [(Date, Money)],
        max_scale: f64,
    ) -> finstack_quant_core::Result<()> {
        if initial.amount_decimal() >= rust_decimal::Decimal::ZERO {
            let ccy = initial.currency();
            let tol = (1e-12 * max_scale).max(1e-9);
            for (date, balance) in closing {
                if balance.amount() < -tol {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "principal repayments exceed outstanding on {date}: {}",
                        balance.amount()
                    )));
                }
                if balance.amount() < 0.0 {
                    *balance = Money::zero(ccy);
                }
            }
        }
        Ok(())
    }

    /// Closing balance after each balance date, in replay order.
    fn replay_balances(&self) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        let order = self.balance_replay_order(|_| true);
        let mut outstanding = self.notional.initial;
        let mut max_scale = outstanding.amount().abs().max(1.0);
        let mut closing: Vec<(Date, Money)> = Vec::new();
        for (idx, date) in order {
            apply_flow_to_outstanding(&mut outstanding, &self.flows[idx])?;
            max_scale = max_scale.max(outstanding.amount().abs());
            match closing.last_mut() {
                Some((last_date, last_balance)) if *last_date == date => {
                    *last_balance = outstanding;
                }
                _ => closing.push((date, outstanding)),
            }
        }
        Self::validate_closing_balances(self.notional.initial, &mut closing, max_scale)?;
        Ok(closing)
    }

    /// Outstanding principal before and after each flow, aligned with
    /// [`Self::get_flows`].
    ///
    /// Replays the same path as [`Self::outstanding_by_date`], one row at a
    /// time. An amortization, PIK, draw or repayment row moves the balance, in
    /// replay order. The issue funding row, whose principal the opening
    /// notional already holds, moves nothing (record CORE-CONVENTIONS-015 #5).
    /// A row that moves nothing keeps
    /// the balance it sits at: for a coupon that is the balance just before its
    /// accrual end, so a coupon paid after a principal date on the accrual end
    /// still shows the principal it accrued on. A balance within half a minor
    /// currency unit of zero reads as zero: it is the float residue of the
    /// amortization amounts, not principal.
    ///
    /// # Returns
    ///
    /// One `(before, after)` pair per flow, in the notional currency.
    ///
    /// # Errors
    ///
    /// Returns a validation error if any end-of-day replayed balance goes
    /// negative for a non-negative initial notional, and propagates currency
    /// mismatches or decimal overflow from balance replay.
    pub fn balances_by_flow(&self) -> finstack_quant_core::Result<Vec<(Money, Money)>> {
        let mut outstanding = self.notional.initial;
        let mut max_scale = outstanding.amount().abs().max(1.0);
        let mut balances = vec![(outstanding, outstanding); self.flows.len()];
        let mut path: Vec<(Date, Money)> = Vec::new();
        let mut closing: Vec<(Date, Money)> = Vec::new();
        for (index, balance_date) in self.balance_replay_order(|_| true) {
            let flow = &self.flows[index];
            let before = outstanding;
            apply_flow_to_outstanding(&mut outstanding, flow)?;
            max_scale = max_scale.max(outstanding.amount().abs());
            balances[index] = (before, outstanding);
            if outstanding != before {
                path.push((balance_date, outstanding));
            }
            match closing.last_mut() {
                Some((last_date, last_balance)) if *last_date == balance_date => {
                    *last_balance = outstanding;
                }
                _ => closing.push((balance_date, outstanding)),
            }
        }
        Self::validate_closing_balances(self.notional.initial, &mut closing, max_scale)?;
        for (flow, balance) in self.flows.iter().zip(&mut balances) {
            if let (Some(accrual), true) = (&flow.get_accrual(), balance.0 == balance.1) {
                let moved = path.partition_point(|(date, _)| *date < accrual.get_end());
                let held = moved
                    .checked_sub(1)
                    .map_or(self.notional.initial, |last| path[last].1);
                *balance = (held, held);
            }
        }
        let currency = self.notional.initial.currency();
        let residue = 0.5 * 10f64.powi(-i32::from(currency.decimals()));
        let settle = |money: &mut Money| {
            if money.amount().abs() < residue {
                *money = Money::from((0_i64, currency));
            }
        };
        for (before, after) in &mut balances {
            settle(before);
            settle(after);
        }
        Ok(balances)
    }

    /// The principal rows `keep` selects with their explicit balance dates, in
    /// balance-replay order (balance date, then schedule order). Rows that move
    /// no principal have no balance date and take no part in replay (record
    /// CORE-CONVENTIONS-015 #6, #11).
    fn balance_replay_order(&self, keep: impl Fn(&CashFlow) -> bool) -> Vec<(usize, Date)> {
        let mut order: Vec<(usize, Date)> = self
            .flows
            .iter()
            .enumerate()
            .filter(|(_, flow)| keep(flow))
            .filter_map(|(index, flow)| flow.get_balance_date().map(|date| (index, date)))
            .collect();
        order.sort_by(|&(left, left_date), &(right, right_date)| {
            left_date
                .cmp(&right_date)
                .then_with(|| compare_flows(&self.flows[left], &self.flows[right]))
        });
        order
    }
}
