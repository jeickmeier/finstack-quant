//! Internal date-by-date build pipeline and the compiled plan it runs.
//!
//! [`CompiledCashFlowPlan`] is produced by `CashFlowBuilder::compile_plan`
//! and projected here into a [`CashFlowSchedule`].

use super::compiler::{FixedSchedule, FloatSchedule, PeriodicFee};
use super::date_generation::CompiledPeriod;
use super::emission::{
    emit_amortization_on, emit_fees_on, emit_fixed_coupons_on, emit_float_coupons_on,
    ResolvedFloatMarket,
};
use super::orchestrator::{
    apply_principal_event, coupon_periods, initialize_build_state, AmortizationSetup, BuildState,
    PreparedAmortization,
};
use super::schedule::{CashFlowMeta, CashFlowSchedule, CashflowRepresentation};
use super::specs::PrincipalEventSpec;
use crate::builder::{AmortizationSpec, Notional, PrincipalExchange};
use crate::fixings::ProjectedFixing;
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::decimal::f64_to_decimal;
use finstack_quant_core::market_data::fixings::fixing_series_id;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::InputError;
use rust_decimal::Decimal;
use std::sync::Arc;
use tracing::debug;

impl CompiledCashFlowPlan {
    /// Every fixed, floating and periodic-fee period, in schedule order.
    fn all_periods(&self) -> impl Iterator<Item = &CompiledPeriod> {
        coupon_periods(&self.fixed_schedules, &self.float_schedules)
            .chain(self.periodic_fees.iter().flat_map(|s| &s.periods))
    }

    /// Emit amortization scheduled exactly on the issue date.
    pub(super) fn process_issue_amortization(
        &self,
        state: &mut BuildState,
    ) -> finstack_quant_core::Result<()> {
        emit_amortization_on(
            self.issue,
            self.notional.initial.currency(),
            &self.amort_setup,
            state,
        )?;
        if let Some((_, balance)) = state.outstanding_history.last_mut() {
            *balance = state.outstanding;
        }
        self.initialize_linear_between(self.issue, state);
        Ok(())
    }

    /// Fix equal installments from the balance after all start-date movements.
    fn initialize_linear_between(&self, d: Date, state: &mut BuildState) {
        if let PreparedAmortization::LinearBetween { start, count, .. } = &self.amort_setup.rule {
            if *start == d {
                state.linear_between_installment =
                    Some(state.outstanding / Decimal::from(*count as u64));
            }
        }
    }

    /// Process custom principal events (draws/repays) for this date.
    fn process_principal_events(
        &self,
        d: Date,
        state: &mut BuildState,
    ) -> finstack_quant_core::Result<Decimal> {
        let first = self.principal_events.partition_point(|ev| ev.date < d);
        let mut funded = Decimal::ZERO;
        for ev in self.principal_events[first..]
            .iter()
            .take_while(|ev| ev.date == d)
        {
            apply_principal_event(ev, &mut state.outstanding, &mut state.flows)?;
            if ev.kind == CFKind::Notional && ev.delta.amount() > 0.0 {
                funded = funded
                    .checked_add(f64_to_decimal(ev.delta.amount())?)
                    .ok_or_else(|| {
                        finstack_quant_core::Error::Validation(
                            "funding exceeds the supported decimal range".into(),
                        )
                    })?;
            }
        }
        Ok(funded)
    }

    /// Handle maturity redemption: emit final principal repayment if outstanding > 0.
    ///
    /// Computes at the terminal accrual boundary after PIK capitalization.
    /// Cash settles on the independently adjusted redemption payment date.
    fn handle_maturity(&self, d: Date, state: &mut BuildState) -> finstack_quant_core::Result<()> {
        if d == self.redemption_effective_date
            && self.principal_exchange == PrincipalExchange::InitialAndFinal
            && state.outstanding > Decimal::ZERO
        {
            let outstanding_f64 = state.outstanding.as_f64();
            state.flows.push(
                CashFlow::new(
                    self.redemption_date,
                    None,
                    Money::new(outstanding_f64, self.notional.initial.currency())?,
                    CFKind::Notional,
                    0.0,
                    None,
                )
                .with_principal_date(d),
            );
            state.outstanding = Decimal::ZERO;
        }
        Ok(())
    }

    /// Process all stages for a single date.
    pub(super) fn process_date(
        &self,
        d: Date,
        state: &mut BuildState,
        market: ResolvedFloatMarket<'_>,
    ) -> finstack_quant_core::Result<()> {
        let ccy = self.notional.initial.currency();
        let pik_to_add = emit_fixed_coupons_on(d, &self.fixed_schedules, ccy, state)?
            + emit_float_coupons_on(d, &self.float_schedules, ccy, market, state)?;
        if pik_to_add > 0.0 {
            state.outstanding = state
                .outstanding
                .checked_add(f64_to_decimal(pik_to_add)?)
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "outstanding principal exceeds the supported decimal range".into(),
                    )
                })?;
        }
        let funded = self.process_principal_events(d, state)?;
        emit_amortization_on(d, ccy, &self.amort_setup, state)?;
        emit_fees_on(
            d,
            &self.periodic_fees,
            state.outstanding,
            &state.outstanding_history,
            ccy,
            &mut state.flows,
        )?;
        self.initialize_linear_between(d, state);
        if let PreparedAmortization::LinearBetween { start, end, .. } = &self.amort_setup.rule {
            if funded > Decimal::ZERO && d > *start && d < *end {
                // Each later draw repays equally over its remaining dates.
                // Add after today's payment: a same-day draw first amortizes
                // at the next contractual boundary, never at its funding date.
                let remaining = self
                    .amort_setup
                    .payment_dates
                    .keys()
                    .filter(|&&date| date > d && date <= *end)
                    .count();
                if remaining > 0 {
                    *state
                        .linear_between_installment
                        .get_or_insert(Decimal::ZERO) += funded / Decimal::from(remaining as u64);
                }
            }
        }
        self.handle_maturity(d, state)?;

        debug_assert!(
            state
                .outstanding_history
                .last()
                .is_none_or(|(last, _)| *last <= d),
            "outstanding_history requires ascending build dates"
        );
        match state.outstanding_history.last_mut() {
            Some((last_date, last_value)) if *last_date == d => *last_value = state.outstanding,
            _ => state.outstanding_history.push((d, state.outstanding)),
        }

        Ok(())
    }
}

impl CompiledCashFlowPlan {
    /// Every date the build loop visits, sorted and deduplicated.
    pub(super) fn collect_all_dates(&self) -> finstack_quant_core::Result<Vec<Date>> {
        let estimated_periods: usize = self
            .fixed_schedules
            .iter()
            .map(|s| s.periods.len())
            .chain(self.float_schedules.iter().map(|s| s.periods.len()))
            .chain(self.periodic_fees.iter().map(|s| s.periods.len()))
            .sum();
        let mut dates = Vec::with_capacity(3 + estimated_periods * 3 + self.principal_events.len());
        dates.extend([self.issue, self.maturity, self.redemption_date]);
        for entry in self.all_periods() {
            let period = &entry.period;
            dates.extend([
                period.accrual_start,
                period.accrual_end,
                period.payment_date,
            ]);
        }
        dates.extend(
            self.notional
                .amort
                .dated_amounts()
                .iter()
                .map(|(date, _)| *date),
        );
        for ev in &self.principal_events {
            dates.push(ev.date);
        }
        if let AmortizationSpec::LinearBetween { start, .. } = &self.notional.amort {
            dates.push(*start);
        }
        dates.sort_unstable();
        dates.dedup();
        if dates.len() < 2 {
            return Err(InputError::TooFewPoints.into());
        }
        Ok(dates)
    }
}

pub(super) struct CompiledCashFlowPlan {
    pub(super) notional: Notional,
    pub(super) issue: Date,
    pub(super) maturity: Date,
    pub(super) fixed_schedules: Vec<FixedSchedule>,
    pub(super) float_schedules: Vec<FloatSchedule>,
    pub(super) periodic_fees: Vec<PeriodicFee>,
    pub(super) fixed_fees: Vec<(Date, Money)>,
    pub(super) principal_events: Vec<PrincipalEventSpec>,
    pub(super) dates: Vec<Date>,
    pub(super) amort_setup: AmortizationSetup,
    pub(super) redemption_date: Date,
    pub(super) redemption_effective_date: Date,
    pub(super) principal_exchange: PrincipalExchange,
}

impl CompiledCashFlowPlan {
    pub(super) fn project(
        &self,
        curves: Option<&finstack_quant_core::market_data::context::MarketContext>,
        observation_date: Option<Date>,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let accrual_start = self
            .all_periods()
            .map(|entry| entry.period.accrual_start)
            .min()
            .unwrap_or(self.issue);
        let mut state = initialize_build_state(
            self.issue,
            accrual_start,
            &self.notional,
            self.dates.len(),
            &self.principal_events,
            self.principal_exchange,
        )?;
        // Fixed fees are emitted once, here. The final canonical sort places
        // them by date, so they need no entries in the date grid.
        for (fee_date, amount) in &self.fixed_fees {
            if amount.amount() != 0.0 {
                state.flows.push(CashFlow::new(
                    *fee_date,
                    None,
                    *amount,
                    CFKind::Fee,
                    0.0,
                    None,
                ));
            }
        }

        let (resolved_curves, resolved_fixings): (
            Vec<Option<Arc<ForwardCurve>>>,
            Vec<Option<ScalarTimeSeries>>,
        ) = if let Some(mkt) = curves {
            self.float_schedules
                .iter()
                .map(|schedule| {
                    let forward_curve_id = schedule.spec.rate_spec.forward_curve_id.as_str();
                    (
                        mkt.get_forward(forward_curve_id).ok(),
                        mkt.get_series(fixing_series_id(forward_curve_id))
                            .ok()
                            .cloned(),
                    )
                })
                .unzip()
        } else {
            (
                vec![None; self.float_schedules.len()],
                vec![None; self.float_schedules.len()],
            )
        };

        for (schedule, resolved_curve) in self.float_schedules.iter().zip(&resolved_curves) {
            let (Some(spec_tenor), Some(curve)) = (
                schedule.spec.rate_spec.index_tenor,
                resolved_curve.as_deref(),
            ) else {
                continue;
            };
            if schedule
                .spec
                .rate_spec
                .compounding
                .is_some_and(|c| c.is_overnight())
            {
                continue;
            }
            let spec_years = spec_tenor.to_years();
            let curve_years = curve.tenor();
            if curve_years > 0.0
                && spec_years > 0.0
                && (spec_years - curve_years).abs() / curve_years > 0.1
            {
                tracing::warn!(
                    forward_curve_id = %schedule.spec.rate_spec.forward_curve_id,
                    spec_index_tenor_years = spec_years,
                    curve_tenor_years = curve_years,
                    "floating leg index_tenor disagrees with the resolved forward curve's \
                     tenor; projection uses the curve's embedded tenor — check the curve id"
                );
            }
        }

        self.process_issue_amortization(&mut state)?;
        for &d in self.dates.iter().filter(|&&d| d > self.issue) {
            self.process_date(
                d,
                &mut state,
                ResolvedFloatMarket {
                    curves: &resolved_curves,
                    observation_date,
                    fixings: &resolved_fixings,
                },
            )?;
        }

        let flows = state.flows;
        let (meta, out_day_count) = finalize_flows(
            &self.fixed_schedules,
            &self.float_schedules,
            self.issue,
            self.maturity,
            state.projected_fixings,
        );
        debug!(flows = flows.len(), "cashflow schedule: project complete");
        let schedule = CashFlowSchedule::from_flows_unchecked(
            flows,
            self.notional.clone(),
            out_day_count,
            meta,
        );
        schedule.validate()?;
        Ok(schedule)
    }
}

/// Build the metadata and representative day count of a compiled schedule.
fn finalize_flows(
    fixed: &[FixedSchedule],
    floating: &[FloatSchedule],
    issue_date: Date,
    maturity_date: Date,
    mut projected_fixings: Vec<ProjectedFixing>,
) -> (CashFlowMeta, DayCount) {
    crate::fixings::normalize_projected_fixings(&mut projected_fixings);

    let mut cals: Vec<String> = fixed
        .iter()
        .map(|schedule| schedule.spec.schedule.calendar_id.to_string())
        .chain(
            floating
                .iter()
                .map(|schedule| schedule.spec.schedule.calendar_id.to_string()),
        )
        .collect();
    cals.sort_unstable();
    cals.dedup();
    let meta = CashFlowMeta {
        projected_fixings,
        calendar_ids: cals,
        commitment: None,
        issue_date: Some(issue_date),
        maturity: Some(maturity_date),
        representation: CashflowRepresentation::default(),
    };

    let out_day_count = if let Some(schedule) = fixed.first() {
        schedule.spec.schedule.day_count
    } else if let Some(schedule) = floating.first() {
        schedule.spec.schedule.day_count
    } else {
        DayCount::Act365F
    };
    (meta, out_day_count)
}
