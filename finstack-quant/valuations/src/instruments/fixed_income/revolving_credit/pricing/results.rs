//! Path-level revolving-credit pricing results.

use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::fixed_income::revolving_credit::cashflow_engine::ThreeFactorPathData;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::results::{MoneyEstimate, MonteCarloResult};

/// Result for a single path valuation.
///
/// Contains the present value, optional 3-factor path data, and the detailed cashflow schedule.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct EnhancedMonteCarloResult {
    /// Standard MC statistics (mean, std error, CI)
    pub mc_result: MonteCarloResult,
    /// Individual path results for distribution analysis
    pub path_results: Vec<PathResult>,
    /// Monte Carlo estimate of [`PathResult::draw_option_cost`] across paths,
    /// with the same antithetic-aware standard error as the present value.
    pub draw_option_cost: MoneyEstimate,
}

impl EnhancedMonteCarloResult {
    /// Project path identity, PV and draw-option cost in simulation order, retaining columns for an empty path set.
    ///
    /// Amounts are in the Monte Carlo estimate currency. Mixed path currencies
    /// return an error before scalar export; path identities must fit signed 64-bit integers.
    pub fn to_table(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_core::table::TableEnvelope> {
        use finstack_quant_core::table::{TableColumn, TableColumnData, TableEnvelope};
        let currency = self.mc_result.estimate.mean.currency();
        for path in &self.path_results {
            for amount in [path.pv, path.draw_option_cost] {
                if amount.currency() != currency {
                    return Err(finstack_quant_core::Error::CurrencyMismatch {
                        expected: currency,
                        actual: amount.currency(),
                    });
                }
            }
        }
        let ids = (0..self.path_results.len())
            .map(|index| {
                i64::try_from(index).map_err(|_| {
                    finstack_quant_core::Error::Validation("path identity exceeds i64".into())
                })
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;
        TableEnvelope::new(vec![
            TableColumn::new("path", TableColumnData::Int64(ids)),
            TableColumn::new(
                "pv",
                TableColumnData::Float64(
                    self.path_results
                        .iter()
                        .map(|path| path.pv.amount())
                        .collect(),
                ),
            ),
            TableColumn::new(
                "draw_option_cost",
                TableColumnData::Float64(
                    self.path_results
                        .iter()
                        .map(|path| path.draw_option_cost.amount())
                        .collect(),
                ),
            ),
        ])
    }
}
