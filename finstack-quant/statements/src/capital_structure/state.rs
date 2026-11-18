//! Mutable runtime state for waterfall evaluation.
//!
//! [`CapitalStructureState`] tracks opening/closing balances, coupon elections, and
//! dated unpaid or advance claims across periods. It is mutated by the waterfall engine during
//! sequential evaluation and is therefore the runtime counterpart to the
//! static [`WaterfallSpec`](super::WaterfallSpec).

use crate::capital_structure::CashflowBreakdown;
use crate::capital_structure::principal::{
    PeriodPrincipalFlows, PrincipalAllocation, PrincipalClaim,
};
use crate::error::Result;
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Period;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::loan_terms::{
    CouponElection, PrincipalPosting,
};
use indexmap::IndexMap;

/// Capital structure state tracking for dynamic evaluation.
///
/// Maintains balances, coupon elections, and dated unpaid or advance claims across periods.
///
/// This state is mutated by the waterfall engine during sequential evaluation
/// and is therefore the runtime counterpart to the static [`WaterfallSpec`](super::WaterfallSpec).
#[derive(Debug, Clone, Default)]
pub(crate) struct CapitalStructureState {
    /// Opening balances by instrument ID at the start of the current period
    pub(crate) opening_balances: IndexMap<String, Money>,

    /// Closing balances by instrument ID at the end of the current period
    pub(crate) closing_balances: IndexMap<String, Money>,

    /// Dated coupon elections the PIK toggle has made per loan, in effective
    /// order. The owner recompiles the schedule from its terms and this
    /// ledger; an election's duration is counted from these dates, so a
    /// period evaluated again changes nothing.
    pub(crate) coupon_elections: IndexMap<String, Vec<CouponElection>>,

    /// The PIK toggle's request in the current period: `Some(true)` asks for
    /// PIK, `Some(false)` for the base coupon terms. Consumed when the
    /// schedules are rebuilt.
    pub(crate) period_pik_request: Option<bool>,

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

    /// Schedule per instrument as it stands: the contractual schedule, then
    /// the owner's recompilation after each principal change the waterfall
    /// posts.
    pub(crate) residual_schedules: IndexMap<String, CashFlowSchedule>,

    /// Every principal change the waterfall has posted to an instrument
    /// (prepayments and toggled PIK), each effective the day after its period
    /// close. The owner recompiles the schedule from its terms and this ledger.
    pub(crate) postings: IndexMap<String, Vec<PrincipalPosting>>,

    /// Net principal change the waterfall posted in the current period, by
    /// instrument, in exact money. Consumed when the schedules are rebuilt.
    pub(crate) period_postings: IndexMap<String, Money>,

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
        self.period_postings.clear();
        self.period_pik_request = None;
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
}
