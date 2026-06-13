//! Internal date-by-date build pipeline for [`CashFlowBuilder`](super::CashFlowBuilder).

use std::sync::Arc;

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::decimal::f64_to_decimal;
use finstack_core::market_data::scalars::ScalarTimeSeries;
use finstack_core::market_data::term_structures::ForwardCurve;
use finstack_core::money::Money;
use rust_decimal::Decimal;

use crate::builder::compiler::{FixedSchedule, FloatSchedule, PeriodicFee};
use crate::builder::emission::{
    emit_amortization_on, emit_fees_on, emit_fixed_coupons_on, emit_float_coupons_on,
    AmortizationParams, ResolvedFloatMarket,
};
use crate::builder::orchestrator::{AmortizationSetup, BuildState, PrincipalEvent};
use crate::builder::Notional;
use crate::primitives::{CFKind, CashFlow};

#[derive(Clone, Copy)]
pub(super) struct BuildContext<'a> {
    pub(super) ccy: Currency,
    pub(super) issue: Date,
    pub(super) maturity: Date,
    /// Business-day-adjusted maturity on which the final principal redemption
    /// is paid. Adjusted with the calendar/BDC of the principal-paying leg
    /// (first fixed, else first floating coupon schedule); equals the raw
    /// `maturity` when no coupon schedule exists.
    pub(super) redemption_date: Date,
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
    fn emit_coupons(&self, d: Date, state: &mut BuildState) -> finstack_core::Result<f64> {
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
    fn emit_amortization(&self, d: Date, state: &mut BuildState) -> finstack_core::Result<()> {
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
            d == self.ctx.maturity,
            &mut state.flows,
        )?;
        Ok(())
    }

    /// Emit amortization scheduled exactly on the issue date.
    pub(super) fn process_issue_amortization(
        &self,
        state: &mut BuildState,
    ) -> finstack_core::Result<()> {
        self.emit_amortization(self.ctx.issue, state)?;
        state
            .outstanding_after
            .insert(self.ctx.issue, state.outstanding);
        Ok(())
    }

    /// Emit fee flows (periodic and fixed).
    fn emit_fees(&self, d: Date, state: &mut BuildState) -> finstack_core::Result<()> {
        emit_fees_on(
            d,
            self.ctx.periodic_fees,
            self.ctx.fixed_fees,
            state.outstanding,
            &state.outstanding_after,
            self.ctx.ccy,
            &mut state.flows,
        )
    }

    /// Process custom principal events (draws/repays) for this date.
    fn process_principal_events(
        &self,
        d: Date,
        state: &mut BuildState,
    ) -> finstack_core::Result<()> {
        for ev in self.ctx.principal_events.iter().filter(|ev| ev.date == d) {
            if ev.delta.amount() != 0.0 || ev.cash.amount() != 0.0 {
                // Sign convention depends on flow kind:
                // - Notional (draws): cash is inflow to borrower, flow is negative (funding outflow)
                // - Amortization: cash is repayment, flow is positive (inflow to lender)
                let flow_amount = match ev.kind {
                    CFKind::Amortization => ev.cash.amount(),
                    _ => -ev.cash.amount(),
                };
                state.flows.push(CashFlow {
                    date: d,
                    reset_date: None,
                    amount: Money::new(flow_amount, ev.cash.currency()),
                    kind: ev.kind,
                    accrual_factor: 0.0,
                    rate: None,
                });
                state.outstanding += f64_to_decimal(ev.delta.amount())?;
            }
        }
        Ok(())
    }

    /// Handle maturity redemption: emit final principal repayment if outstanding > 0.
    ///
    /// The loop still triggers on the raw (unadjusted) maturity date, but the
    /// emitted cashflow is dated on `BuildContext::redemption_date` — the
    /// business-day-adjusted maturity — so the redemption settles on the same
    /// date as the final coupon. `finalize_flows` re-sorts flows by date, so
    /// the shifted date keeps the schedule ordered.
    fn handle_maturity(&self, d: Date, state: &mut BuildState) -> finstack_core::Result<()> {
        if d == self.ctx.maturity && state.outstanding > Decimal::ZERO {
            let outstanding_f64 = finstack_core::decimal::decimal_to_f64(state.outstanding)?;
            state.flows.push(CashFlow {
                date: self.ctx.redemption_date,
                reset_date: None,
                amount: Money::new(outstanding_f64, self.ctx.ccy),
                kind: CFKind::Notional,
                accrual_factor: 0.0,
                rate: None,
            });
            state.outstanding = Decimal::ZERO;
        }
        Ok(())
    }

    /// Process all stages for a single date.
    pub(super) fn process(
        &self,
        d: Date,
        mut state: BuildState,
    ) -> finstack_core::Result<BuildState> {
        let pik_to_add = self.emit_coupons(d, &mut state)?;

        self.emit_amortization(d, &mut state)?;

        // PIK capitalizes after amortization for this date.
        if pik_to_add > 0.0 {
            state.outstanding += f64_to_decimal(pik_to_add)?;
        }

        self.emit_fees(d, &mut state)?;
        self.process_principal_events(d, &mut state)?;
        self.handle_maturity(d, &mut state)?;

        state.outstanding_after.insert(d, state.outstanding);

        Ok(state)
    }
}
