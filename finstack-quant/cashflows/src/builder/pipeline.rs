//! Internal date-by-date build pipeline for [`CashFlowBuilder`](super::CashFlowBuilder).

use finstack_quant_core::dates::Date;
use finstack_quant_core::decimal::f64_to_decimal;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;

use crate::builder::emission::{
    emit_amortization_on, emit_fees_on, emit_fixed_coupons_on, emit_float_coupons_on,
    ResolvedFloatMarket,
};
use crate::builder::orchestrator::{
    apply_principal_event, BuildState, CompiledCashFlowPlan, PreparedAmortization,
};
use crate::builder::PrincipalExchange;
use finstack_quant_core::cashflow::{CFKind, CashFlow};

impl CompiledCashFlowPlan {
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
