//! Internal date-by-date build pipeline for [`CashFlowBuilder`](super::CashFlowBuilder).

use std::sync::Arc;

use finstack_quant_core::dates::Date;
use finstack_quant_core::decimal::f64_to_decimal;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;

use crate::builder::emission::{
    emit_amortization_on, emit_fees_on, emit_fixed_coupons_on, emit_float_coupons_on,
    FloatEmissionOutput, ResolvedFloatMarket,
};
use crate::builder::orchestrator::{
    apply_principal_event, BuildState, CompiledCashFlowPlan, PreparedAmortization,
};
use crate::builder::PrincipalExchange;
use crate::primitives::{CFKind, CashFlow};

/// Processes cashflows for a single schedule date.
pub(super) struct DateProcessor<'a> {
    plan: &'a CompiledCashFlowPlan,
    observation_date: Option<Date>,
    resolved_curves: &'a [Option<Arc<ForwardCurve>>],
    /// Per-float-schedule historical fixing series (`FIXING:{forward_curve_id}`),
    /// aligned with `resolved_curves`; used for seasoned coupons whose
    /// observation dates precede the curve base date.
    resolved_fixings: &'a [Option<ScalarTimeSeries>],
}

impl<'a> DateProcessor<'a> {
    pub(super) fn new(
        plan: &'a CompiledCashFlowPlan,
        observation_date: Option<Date>,
        resolved_curves: &'a [Option<Arc<ForwardCurve>>],
        resolved_fixings: &'a [Option<ScalarTimeSeries>],
    ) -> Self {
        Self {
            plan,
            observation_date,
            resolved_curves,
            resolved_fixings,
        }
    }

    /// Emit fixed and floating coupons, returning total PIK amount to capitalize.
    fn emit_coupons(&self, d: Date, state: &mut BuildState) -> finstack_quant_core::Result<f64> {
        let ccy = self.plan.notional.initial.currency();
        let pik_f = emit_fixed_coupons_on(
            d,
            &self.plan.fixed_schedules,
            &state.outstanding_history,
            state.outstanding,
            ccy,
            &mut state.flows,
        )?;
        let pik_fl = emit_float_coupons_on(
            d,
            &self.plan.float_schedules,
            &state.outstanding_history,
            state.outstanding,
            ccy,
            ResolvedFloatMarket {
                curves: self.resolved_curves,
                observation_date: self.observation_date,
                fixings: self.resolved_fixings,
            },
            FloatEmissionOutput {
                flows: &mut state.flows,
                projected_fixings: &mut state.projected_fixings,
            },
        )?;
        Ok(pik_f + pik_fl)
    }

    /// Emit amortization flows based on the amortization spec.
    fn emit_amortization(
        &self,
        d: Date,
        state: &mut BuildState,
    ) -> finstack_quant_core::Result<()> {
        let first_flow = state.flows.len();
        let amort_setup = &self.plan.amort_setup;
        emit_amortization_on(
            d,
            self.plan.notional.initial.currency(),
            &mut state.outstanding,
            amort_setup,
            state.linear_between_installment,
            amort_setup.final_amortization_date == Some(d),
            &mut state.flows,
        )?;
        for flow in &mut state.flows[first_flow..] {
            flow.principal_date = Some(d);
            flow.date = amort_setup.payment_dates.get(&d).copied().unwrap_or(d);
        }
        Ok(())
    }

    /// Emit amortization scheduled exactly on the issue date.
    pub(super) fn process_issue_amortization(
        &self,
        state: &mut BuildState,
    ) -> finstack_quant_core::Result<()> {
        self.emit_amortization(self.plan.issue, state)?;
        if let Some((_, balance)) = state.outstanding_history.last_mut() {
            *balance = state.outstanding;
        }
        self.initialize_linear_between(self.plan.issue, state);
        Ok(())
    }

    /// Fix equal installments from the balance after all start-date movements.
    fn initialize_linear_between(&self, d: Date, state: &mut BuildState) {
        if let PreparedAmortization::LinearBetween { start, count, .. } =
            &self.plan.amort_setup.rule
        {
            if *start == d {
                state.linear_between_installment =
                    Some(state.outstanding / Decimal::from(*count as u64));
            }
        }
    }

    /// Emit fee flows (periodic and fixed).
    fn emit_fees(&self, d: Date, state: &mut BuildState) -> finstack_quant_core::Result<()> {
        emit_fees_on(
            d,
            &self.plan.periodic_fees,
            &self.plan.fixed_fees,
            state.outstanding,
            &state.outstanding_history,
            self.plan.notional.initial.currency(),
            &mut state.flows,
        )
    }

    /// Process custom principal events (draws/repays) for this date.
    fn process_principal_events(
        &self,
        d: Date,
        state: &mut BuildState,
    ) -> finstack_quant_core::Result<Decimal> {
        let first = self.plan.principal_events.partition_point(|ev| ev.date < d);
        let mut funded = Decimal::ZERO;
        for ev in self.plan.principal_events[first..]
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
        if d == self.plan.redemption_effective_date
            && self.plan.principal_exchange == PrincipalExchange::InitialAndFinal
            && state.outstanding > Decimal::ZERO
        {
            let outstanding_f64 = finstack_quant_core::decimal::decimal_to_f64(state.outstanding)?;
            state.flows.push(
                CashFlow::new(
                    self.plan.redemption_date,
                    None,
                    Money::new(outstanding_f64, self.plan.notional.initial.currency())?,
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
    pub(super) fn process(
        &self,
        d: Date,
        mut state: BuildState,
    ) -> finstack_quant_core::Result<BuildState> {
        let pik_to_add = self.emit_coupons(d, &mut state)?;
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
        let funded = self.process_principal_events(d, &mut state)?;
        self.emit_amortization(d, &mut state)?;
        self.emit_fees(d, &mut state)?;
        self.initialize_linear_between(d, &mut state);
        if let PreparedAmortization::LinearBetween { start, end, .. } = &self.plan.amort_setup.rule
        {
            if funded > Decimal::ZERO && d > *start && d < *end {
                // Each later draw repays equally over its remaining dates.
                // Add after today's payment: a same-day draw first amortizes
                // at the next contractual boundary, never at its funding date.
                let remaining = self
                    .plan
                    .amort_setup
                    .amort_dates
                    .iter()
                    .filter(|&&date| date > d && date <= *end)
                    .count();
                if remaining > 0 {
                    *state
                        .linear_between_installment
                        .get_or_insert(Decimal::ZERO) += funded / Decimal::from(remaining as u64);
                }
            }
        }
        self.handle_maturity(d, &mut state)?;

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

        Ok(state)
    }
}
