//! Shared caplet/floorlet payoff inputs used by Black and normal pricing.

use finstack_quant_core::currency::Currency;

/// Inputs for pricing a single caplet or floorlet.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CapletFloorletInputs {
    /// True for caplet, false for floorlet.
    pub(crate) is_cap: bool,
    /// Notional amount.
    pub(crate) notional: f64,
    /// Strike rate, as decimal.
    pub(crate) strike: f64,
    /// Forward rate, as decimal.
    pub(crate) forward: f64,
    /// Discount factor to payment date.
    pub(crate) discount_factor: f64,
    /// Annualized volatility in the model convention.
    pub(crate) volatility: f64,
    /// Time to fixing date in years.
    pub(crate) time_to_fixing: f64,
    /// Accrual year fraction for the period.
    pub(crate) accrual_year_fraction: f64,
    /// Currency for the cashflow.
    pub(crate) currency: Currency,
}
