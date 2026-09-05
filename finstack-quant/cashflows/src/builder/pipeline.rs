//! Internal date-by-date build pipeline for [`CashFlowBuilder`](super::CashFlowBuilder).

use std::sync::Arc;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::decimal::f64_to_decimal;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;

use crate::builder::compiler::{FixedSchedule, FloatSchedule, PeriodicFee};
use crate::builder::emission::{
    emit_amortization_on, emit_fees_on, emit_fixed_coupons_on, emit_float_coupons_on,
    AmortizationParams, ResolvedFloatMarket,
};
use crate::builder::orchestrator::{AmortizationSetup, BuildState, PrincipalEvent};
use crate::builder::{Notional, PrincipalExchange};
use crate::primitives::{CFKind, CashFlow};

#[derive(Clone, Copy)]
pub(super) struct BuildContext<'a> {
    pub(super) ccy: Currency,
    pub(super) issue: Date,
    /// Date on which the final principal redemption is paid: BDC-adjust
    /// `maturity` on the principal-paying leg, then apply that leg's payment
    /// lag. Equals the raw `maturity` when no coupon schedule exists.
    pub(super) redemption_date: Date,
    /// Whether to emit the maturity balloon as `CFKind::Notional`.
    pub(super) principal_exchange: PrincipalExchange,
    pub(super) notional: &'a Notional,
    pub(super) fixed_schedules: &'a [FixedSchedule],
    pub(super) float_schedules: &'a [FloatSchedule],
    pub(super) periodic_fees: &'a [PeriodicFee],
    pub(super) fixed_fees: &'a [(Date, Money)],
    pub(super) principal_events: &'a [PrincipalEvent],
}

/// Processes cashflows for a single schedule date.
pub(super) struct DateProcessor<'a> {
    ctx: &'a BuildContext<'a>,
    amort_setup: &'a AmortizationSetup,
    resolved_curves: &'a [Option<Arc<ForwardCurve>>],
    /// Per-float-schedule historical fixing series (`FIXING:{index_id}`),
    /// aligned with `resolved_curves`; used for seasoned coupons whose
    /// observation dates precede the curve base date.
    resolved_fixings: &'a [Option<ScalarTimeSeries>],
}

impl<'a> DateProcessor<'a> {
    pub(super) fn new(
        ctx: &'a BuildContext<'a>,
        amort_setup: &'a AmortizationSetup,
        resolved_curves: &'a [Option<Arc<ForwardCurve>>],
        resolved_fixings: &'a [Option<ScalarTimeSeries>],
    ) -> Self {
        Self {
            ctx,
            amort_setup,
            resolved_curves,
            resolved_fixings,
        }
    }

    /// Emit fixed and floating coupons, returning total PIK amount to capitalize.
    fn emit_coupons(&self, d: Date, state: &mut BuildState) -> finstack_quant_core::Result<f64> {
        let pik_f = emit_fixed_coupons_on(
            d,
            self.ctx.fixed_schedules,
            &state.outstanding_after,
            state.outstanding,
            self.ctx.ccy,
            &mut state.flows,
        )?;
        let pik_fl = emit_float_coupons_on(
            d,
            self.ctx.float_schedules,
            &state.outstanding_after,
            state.outstanding,
            self.ctx.ccy,
            ResolvedFloatMarket {
                curves: self.resolved_curves,
                fixings: self.resolved_fixings,
            },
            &mut state.flows,
        )?;
        Ok(pik_f + pik_fl)
    }

    /// Emit amortization flows based on the amortization spec.
    fn emit_amortization(
        &self,
        d: Date,
        state: &mut BuildState,
    ) -> finstack_quant_core::Result<()> {
        let amort_params = AmortizationParams {
            ccy: self.ctx.ccy,
            amort_dates: &self.amort_setup.amort_dates,
            linear_delta: self.amort_setup.linear_delta,
            percent_per: self.amort_setup.percent_per,
            step_remaining_map: &self.amort_setup.step_remaining_map,
            custom_principal_map: &self.amort_setup.custom_principal_map,
        };
        emit_amortization_on(
            d,
            self.ctx.notional,
            &mut state.outstanding,
            &amort_params,
            d == self.ctx.redemption_date,
            &mut state.flows,
        )?;
        Ok(())
    }

    /// Emit amortization scheduled exactly on the issue date.
    pub(super) fn process_issue_amortization(
        &self,
        state: &mut BuildState,
    ) -> finstack_quant_core::Result<()> {
        self.emit_amortization(self.ctx.issue, state)?;
        if let Some((_, balance)) = state.outstanding_history.last_mut() {
            *balance = state.outstanding;
        }
        state
            .outstanding_after
            .insert(self.ctx.issue, state.outstanding);
        Ok(())
    }

    /// Emit fee flows (periodic and fixed).
    fn emit_fees(&self, d: Date, state: &mut BuildState) -> finstack_quant_core::Result<()> {
        emit_fees_on(
            d,
            self.ctx.periodic_fees,
            self.ctx.fixed_fees,
            state.outstanding,
            &state.outstanding_history,
            self.ctx.ccy,
            &mut state.flows,
        )
    }

    /// Process custom principal events (draws/repays) for this date.
    fn process_principal_events(
        &self,
        d: Date,
        state: &mut BuildState,
    ) -> finstack_quant_core::Result<()> {
        let first = self.ctx.principal_events.partition_point(|ev| ev.date < d);
        for ev in self.ctx.principal_events[first..]
            .iter()
            .take_while(|ev| ev.date == d)
        {
            if ev.delta.amount() != 0.0 || ev.cash.amount() != 0.0 {
                // Draws are negative (lender outflow); amortization is positive (lender inflow).
                let flow_amount = match ev.kind {
                    CFKind::Amortization => ev.cash.amount(),
                    _ => -ev.cash.amount(),
                };
                state.flows.push(
                    CashFlow::new(
                        d,
                        None,
                        Money::new(flow_amount, ev.cash.currency())?,
                        ev.kind,
                        0.0,
                        None,
                    )
                    .with_principal_delta(ev.delta),
                );
                state.outstanding += f64_to_decimal(ev.delta.amount())?;
                if state.outstanding < Decimal::ZERO {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "principal event on {} would make outstanding balance negative ({})",
                        d, state.outstanding
                    )));
                }
            }
        }
        Ok(())
    }

    /// Handle maturity redemption: emit final principal repayment if outstanding > 0.
    ///
    /// Triggers on `redemption_date` (adjusted maturity plus payment lag),
    /// after same-day coupons, amortization, PIK capitalization, and fees, so
    /// a lagged final PIK is included in the balloon. Outstanding is not
    /// zeroed on the raw maturity when that date is earlier than the lagged
    /// coupon.
    fn handle_maturity(&self, d: Date, state: &mut BuildState) -> finstack_quant_core::Result<()> {
        if d == self.ctx.redemption_date
            && self.ctx.principal_exchange == PrincipalExchange::InitialAndFinal
            && state.outstanding > Decimal::ZERO
        {
            let outstanding_f64 = finstack_quant_core::decimal::decimal_to_f64(state.outstanding)?;
            state.flows.push(CashFlow::new(
                d,
                None,
                Money::new(outstanding_f64, self.ctx.ccy)?,
                CFKind::Notional,
                0.0,
                None,
            ));
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

        self.emit_amortization(d, &mut state)?;

        if pik_to_add > 0.0 {
            state.outstanding += f64_to_decimal(pik_to_add)?;
        }

        self.emit_fees(d, &mut state)?;
        self.process_principal_events(d, &mut state)?;
        self.handle_maturity(d, &mut state)?;

        state.outstanding_after.insert(d, state.outstanding);
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
