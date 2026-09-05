//! Mutable runtime state for waterfall evaluation.
//!
//! [`CapitalStructureState`] tracks opening/closing balances and cumulative
//! metrics across periods. It is mutated by the waterfall engine during
//! sequential evaluation and is therefore the runtime counterpart to the
//! static [`WaterfallSpec`](super::WaterfallSpec).

use crate::capital_structure::residual_schedule::rebuild_residual_interest;
use crate::error::Result;
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;

/// Capital structure state tracking for dynamic evaluation.
///
/// Maintains opening/closing balances and cumulative metrics across periods.
///
/// This state is mutated by the waterfall engine during sequential evaluation
/// and is therefore the runtime counterpart to the static [`WaterfallSpec`](super::WaterfallSpec).
#[derive(Debug, Clone, Default)]
pub struct CapitalStructureState {
    /// Opening balances by instrument ID at the start of the current period
    pub opening_balances: IndexMap<String, Money>,

    /// Closing balances by instrument ID at the end of the current period
    pub closing_balances: IndexMap<String, Money>,

    /// Cumulative interest paid (cash) by instrument
    pub cumulative_interest_cash: IndexMap<String, Money>,

    /// Cumulative interest accrued (PIK) by instrument
    pub cumulative_interest_pik: IndexMap<String, Money>,

    /// Cumulative principal payments by instrument
    pub cumulative_principal: IndexMap<String, Money>,

    /// Current PIK mode by instrument (true = PIK enabled, false = cash)
    pub pik_mode: IndexMap<String, bool>,

    /// Number of consecutive periods each instrument has been in PIK mode.
    /// Used for hysteresis: PIK stays active until `min_periods_in_pik` is met.
    pub pik_periods_active: IndexMap<String, usize>,

    /// Cumulative principal capitalized via the PIK *toggle* per instrument.
    ///
    /// Toggle-driven capitalization grows the stateful balance beyond the
    /// contractual schedule's notional path. This increment is excluded from
    /// the scale-clamp basis in `calculate_period_flows` so PIK compounding
    /// is not frozen by `SCALE_CLAMP_MAX`, while interest still accrues on
    /// the full stateful balance.
    pub cumulative_toggled_pik: IndexMap<String, Money>,

    /// Unpaid interest/fee shortfall per instrument from the prior period's
    /// available-cash cap. Carried forward as a claim in the next period's
    /// interest category and re-recorded if it remains unpaid.
    pub interest_shortfall: IndexMap<String, Money>,

    /// Unpaid scheduled-principal shortfall per instrument from the prior
    /// period's available-cash cap. Carried forward as a claim in the next
    /// period's amortization category and re-recorded if it remains unpaid.
    pub principal_shortfall: IndexMap<String, Money>,

    /// Unpaid fee shortfall per instrument from the prior period's
    /// available-cash cap. Carried forward as a claim in the next period's
    /// *fee* category (kept separate from `interest_shortfall` so fee arrears
    /// are not demoted into the interest rung) and re-recorded if unpaid.
    pub fee_shortfall: IndexMap<String, Money>,

    /// Net new funding (revolver draws + initial-exchange notional) per
    /// instrument for the *current* period, computed by
    /// `calculate_period_flows`. The waterfall consumes it to recover the
    /// payable balance (`opening + funding`) and the draw-aware closing
    /// balance. Overwritten each period; instruments with no draw record zero.
    pub period_new_funding: IndexMap<String, Money>,

    /// Residual cashflow schedule per instrument, rebuilt after each period's
    /// outstanding change so the next coupon is `outstanding × rate ×
    /// accrual_factor` rather than a scale of the original schedule.
    pub residual_schedules: IndexMap<String, CashFlowSchedule>,
}

impl CapitalStructureState {
    /// Create a new empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get opening balance for an instrument, defaulting to zero if not present.
    pub fn get_opening_balance(&self, instrument_id: &str, currency: Currency) -> Money {
        self.opening_balances
            .get(instrument_id)
            .copied()
            .unwrap_or_else(|| Money::from((0_i64, currency)))
    }

    /// Get closing balance for an instrument, defaulting to zero if not present.
    pub fn get_closing_balance(&self, instrument_id: &str, currency: Currency) -> Money {
        self.closing_balances
            .get(instrument_id)
            .copied()
            .unwrap_or_else(|| Money::from((0_i64, currency)))
    }

    /// Update closing balance for an instrument.
    pub fn set_closing_balance(&mut self, instrument_id: String, balance: Money) {
        self.closing_balances.insert(instrument_id, balance);
    }

    /// Net new funding recorded for an instrument in the current period,
    /// defaulting to zero if not present.
    pub fn get_period_new_funding(&self, instrument_id: &str, currency: Currency) -> Money {
        self.period_new_funding
            .get(instrument_id)
            .copied()
            .unwrap_or_else(|| Money::from((0_i64, currency)))
    }

    /// Advance state to next period: closing balances become opening balances.
    ///
    /// Closing balances are cleared after promotion so matured instruments
    /// (balance == 0) do not carry stale data into the next evaluation cycle.
    pub fn advance_period(&mut self) {
        self.opening_balances = std::mem::take(&mut self.closing_balances);
    }

    /// Rebuild remaining interest on every residual schedule from the current
    /// closing outstanding.
    ///
    /// # Arguments
    ///
    /// * `from_date` - Inclusive period-end snapshot (`period.end - 1 day`).
    ///   Interest flows dated after this date are rewritten onto each
    ///   instrument's closing outstanding; scheduled amort, draws, and fees
    ///   stay in place.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error when a residual rate cannot be
    /// inferred or a schedule currency does not match the closing balance.
    pub fn rebuild_residuals(&mut self, from_date: Date) -> Result<()> {
        let updates: Vec<(String, Money)> = self
            .closing_balances
            .iter()
            .filter(|(id, _)| self.residual_schedules.contains_key(*id))
            .map(|(id, closing)| (id.clone(), *closing))
            .collect();
        for (id, closing) in updates {
            if let Some(schedule) = self.residual_schedules.get(&id) {
                let rebuilt = rebuild_residual_interest(schedule, closing, from_date)?;
                self.residual_schedules.insert(id, rebuilt);
            }
        }
        Ok(())
    }
}
