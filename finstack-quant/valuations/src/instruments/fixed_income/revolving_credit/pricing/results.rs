//! Path-level revolving-credit pricing results.

use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::fixed_income::revolving_credit::cashflow_engine::ThreeFactorPathData;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::results::{MoneyEstimate, MonteCarloResult};

/// Result for a single path valuation.
///
/// Contains the present value, optional 3-factor path data, and the detailed cashflow schedule.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PathResult {
    /// Present value for this path
    pub pv: Money,
    /// 3-factor path data (if from MC)
    pub path_data: Option<ThreeFactorPathData>,
    /// Cashflow schedule for this path
    pub cashflows: CashFlowSchedule,
    /// Value to the lender of the path's draws having been made at the
    /// contractual margin instead of the path's fair spread. The fair spread
    /// is anchored to the margin at the valuation date, so each draw is a
    /// forward loan to maturity worth `−ΔD · (s − s₀) · risky annuity`,
    /// summed over the path's draws. Negative when the spread has widened
    /// since the valuation date (the option the borrower holds has been
    /// exercised against the lender); zero for deterministic schedules and
    /// for any constant spread process. Loan-equivalent draws at default
    /// belong to the default leg and are excluded.
    pub draw_option_cost: Money,
}

/// Enhanced Monte Carlo results with full path details.
///
/// Extends the standard `MonteCarloResult` with individual path results
/// for distribution analysis and visualization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnhancedMonteCarloResult {
    /// Standard MC statistics (mean, std error, CI)
    pub mc_result: MonteCarloResult,
    /// Individual path results for distribution analysis
    pub path_results: Vec<PathResult>,
    /// Monte Carlo estimate of [`PathResult::draw_option_cost`] across paths,
    /// with the same antithetic-aware standard error as the present value.
    pub draw_option_cost: MoneyEstimate,
}
