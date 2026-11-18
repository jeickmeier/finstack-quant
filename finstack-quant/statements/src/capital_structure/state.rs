//! Mutable runtime state for waterfall evaluation.
//!
//! [`CapitalStructureState`] tracks opening/closing balances, PIK mode, and
//! dated unpaid or advance claims across periods. It is mutated by the waterfall engine during
//! sequential evaluation and is therefore the runtime counterpart to the
//! static [`WaterfallSpec`](super::WaterfallSpec).

use crate::capital_structure::CashflowBreakdown;
use crate::capital_structure::principal::{
    PeriodPrincipalFlows, PrincipalAllocation, PrincipalClaim,
};
use crate::capital_structure::residual_schedule::rebuild_residual_interest;
use crate::error::Result;
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Period};
use finstack_quant_core::money::Money;
use indexmap::IndexMap;

/// Capital structure state tracking for dynamic evaluation.
///
/// Maintains balances, PIK mode, and dated unpaid or advance claims across periods.
///
/// This state is mutated by the waterfall engine during sequential evaluation
/// and is therefore the runtime counterpart to the static [`WaterfallSpec`](super::WaterfallSpec).
#[derive(Debug, Clone, Default)]
pub(crate) struct CapitalStructureState {
    /// Opening balances by instrument ID at the start of the current period
    pub(crate) opening_balances: IndexMap<String, Money>,

    /// Closing balances by instrument ID at the end of the current period
    pub(crate) closing_balances: IndexMap<String, Money>,

    /// Current PIK mode by instrument (true = PIK enabled, false = cash)
    pub(crate) pik_mode: IndexMap<String, bool>,

    /// Number of consecutive periods each instrument has been in PIK mode.
    /// Used for hysteresis: PIK stays active until `min_periods_in_pik` is met.
    pub(crate) pik_periods_active: IndexMap<String, usize>,

    /// Unpaid interest/fee shortfall per instrument from the prior period's
    /// available-cash cap. Carried forward as a claim in the next period's
    /// interest category and re-recorded if it remains unpaid.
    pub(crate) interest_shortfall: IndexMap<String, Money>,

    /// Unpaid scheduled-principal claims from prior available-cash caps.
    /// Original payment and economic principal dates survive each carry so
    /// early and delayed settlements affect outstanding exactly once.
    pub(crate) principal_shortfall: IndexMap<String, Vec<PrincipalClaim>>,

    /// Principal cash already paid whose economic reduction occurs in a
    /// future period. These lots reserve repayment capacity until that date;
    /// otherwise an intermediate reporting period could repay the same debt twice.
    pub(crate) principal_advance_payments: IndexMap<String, Vec<PrincipalClaim>>,

    /// Unpaid fee shortfall per instrument from the prior period's
    /// available-cash cap. Carried forward as a claim in the next period's
    /// *fee* category (kept separate from `interest_shortfall` so fee arrears
    /// are not demoted into the interest rung) and re-recorded if unpaid.
    pub(crate) fee_shortfall: IndexMap<String, Money>,

    /// Residual cashflow schedule per instrument, rebuilt after each period's
    /// outstanding change so the next coupon is `outstanding × rate ×
    /// accrual_factor` rather than a scale of the original schedule.
    pub(crate) residual_schedules: IndexMap<String, CashFlowSchedule>,

    /// Dated current-period claims, supplied by market-aware evaluation or
    /// explicitly by callers allocating schedules with separate cash dates.
    pub(crate) period_principal_flows: IndexMap<String, PeriodPrincipalFlows>,
}

impl CapitalStructureState {
    /// Create a new empty state.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Get opening balance for an instrument, defaulting to zero if not present.
    pub(crate) fn get_opening_balance(&self, instrument_id: &str, currency: Currency) -> Money {
        self.opening_balances
            .get(instrument_id)
            .copied()
            .unwrap_or_else(|| Money::from((0_i64, currency)))
    }

    /// Update closing balance for an instrument.
    pub(crate) fn set_closing_balance(&mut self, instrument_id: String, balance: Money) {
        self.closing_balances.insert(instrument_id, balance);
    }

    /// Advance state to next period: closing balances become opening balances.
    ///
    /// Closing balances are cleared after promotion so matured instruments
    /// (balance == 0) do not carry stale data into the next evaluation cycle.
    pub(crate) fn advance_period(&mut self) {
        self.opening_balances = std::mem::take(&mut self.closing_balances);
        self.period_principal_flows.clear();
    }

    /// Set dated principal claims for the current waterfall period.
    ///
    /// Market-aware evaluation supplies these from the residual schedule.
    /// Every instrument in a waterfall needs them, including an empty list
    /// when nothing is due: no claim is synthesized from aggregate principal.
    /// The contractual breakdown's debt balance must be its economic closing
    /// balance before cash allocation.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Exact instrument key in the contractual flow map.
    /// * `period` - Actual half-open reporting interval, including fiscal dates.
    /// * `claims` - Nonnegative current cash-principal claims in native currency;
    ///   payment dates must lie in `period`, while economic dates may precede
    ///   or follow it. Their amounts must sum to contractual principal cash.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error for an empty/reversed period,
    /// negative claims, or cash dates outside the reporting interval.
    pub(crate) fn set_period_principal_flows(
        &mut self,
        instrument_id: impl Into<String>,
        period: &Period,
        claims: Vec<PrincipalClaim>,
    ) -> Result<()> {
        let flows = PeriodPrincipalFlows::new(period, claims)?;
        self.period_principal_flows
            .insert(instrument_id.into(), flows);
        Ok(())
    }

    pub(crate) fn stage_principal(
        &mut self,
        instrument_id: &str,
        contractual: &CashflowBreakdown,
    ) -> Result<PrincipalAllocation> {
        // Scheduled principal is always dated: no claim is synthesized from
        // the period's aggregate principal.
        let period = self
            .period_principal_flows
            .get(instrument_id)
            .ok_or_else(|| {
                crate::error::Error::capital_structure(format!(
                    "instrument '{instrument_id}' has no dated principal claims for the period; \
                 supply each scheduled principal payment with its due and effective dates"
                ))
            })?;
        let allocation = PrincipalAllocation::new(
            period,
            contractual,
            self.principal_shortfall
                .get(instrument_id)
                .cloned()
                .unwrap_or_default(),
            self.principal_advance_payments
                .get(instrument_id)
                .cloned()
                .unwrap_or_default(),
        )?;
        self.principal_shortfall.shift_remove(instrument_id);
        self.principal_advance_payments.shift_remove(instrument_id);
        Ok(allocation)
    }

    /// Reproject future interest only when closing outstanding differs from
    /// the contractual schedule. Preserve all interest earned before the change.
    ///
    /// # Arguments
    ///
    /// * `from_date` - Inclusive period-end snapshot (`period.end - 1 day`).
    ///   Principal changes affect accrual from the following day. Past flows
    ///   remain as accrual anchors; future coupons retain dated amortization,
    ///   draw, and PIK effects. Repayments cannot exceed remaining principal.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when a residual rate cannot be
    /// inferred or a schedule currency does not match the closing balance.
    pub(crate) fn rebuild_residuals(&mut self, from_date: Date) -> Result<()> {
        let updates: Vec<(String, Money)> = self
            .closing_balances
            .iter()
            .filter(|(id, _)| self.residual_schedules.contains_key(*id))
            .map(|(id, closing)| (id.clone(), *closing))
            .collect();
        for (id, closing) in updates {
            if let Some(schedule) = self.residual_schedules.get(&id) {
                let preserved_claims: Vec<_> = self
                    .principal_advance_payments
                    .get(&id)
                    .into_iter()
                    .chain(self.principal_shortfall.get(&id))
                    .flatten()
                    .cloned()
                    .collect();
                let rebuilt =
                    rebuild_residual_interest(schedule, closing, from_date, &preserved_claims)?;
                self.residual_schedules.insert(id, rebuilt);
            }
        }
        Ok(())
    }
}
