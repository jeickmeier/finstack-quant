//! Scenario / yield table for structured-credit tranches.
//!
//! Sweeps a grid of behavioral assumptions (CPR × CDR × severity) and reprices
//! a single tranche in each cell, returning a structured table of price, WAL and
//! principal writedown. This is the standard way structured-credit tranches are
//! quoted and stress-analysed on a trading desk.

use crate::cashflow::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::instruments::Instrument;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use serde::{Deserialize, Serialize};

use super::calculate_tranche_wal;

/// Grid of behavioral scenarios to evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioGrid {
    /// Annual CPR values (decimal) to sweep.
    pub cprs: Vec<f64>,
    /// Annual CDR values (decimal) to sweep.
    pub cdrs: Vec<f64>,
    /// Loss severities (decimal); recovery = `1 - severity`.
    pub severities: Vec<f64>,
    /// Recovery lag (months) applied in every scenario. When `None`, the deal's
    /// own recovery lag is used; set it to override (e.g. `Some(0)` for
    /// immediate recoveries).
    #[serde(default)]
    pub recovery_lag: Option<u32>,
}

/// One evaluated cell of the scenario table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioCell {
    /// Annual CPR (decimal) for this cell.
    pub cpr: f64,
    /// Annual CDR (decimal) for this cell.
    pub cdr: f64,
    /// Loss severity (decimal) for this cell.
    pub severity: f64,
    /// Tranche price as a percentage of original balance.
    pub price: f64,
    /// Weighted-average life in years.
    pub wal: f64,
    /// Total principal writedown in currency units.
    pub writedown: f64,
}

/// Scenario table for a single tranche.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioTable {
    /// Identifier of the tranche evaluated.
    pub tranche_id: String,
    /// Evaluated cells, in CPR-major, then CDR, then severity order.
    pub cells: Vec<ScenarioCell>,
}

/// Build a scenario table for one tranche across a CPR × CDR × severity grid.
///
/// Each cell clones the deal, overrides the deterministic prepayment, default
/// and recovery assumptions, reprices the tranche, and records its price (as a
/// percentage of original balance), WAL and principal writedown.
///
/// # Arguments
///
/// * `deal` - The structured-credit deal owning the tranche.
/// * `tranche_id` - Identifier of the tranche to evaluate.
/// * `context` - Market context for cashflow projection and discounting.
/// * `as_of` - Valuation date.
/// * `grid` - The scenario grid to sweep.
///
/// # Errors
///
/// Returns an error if the tranche is missing or a cell fails to reprice.
pub fn scenario_table(
    deal: &StructuredCredit,
    tranche_id: &str,
    context: &MarketContext,
    as_of: Date,
    grid: &ScenarioGrid,
) -> Result<ScenarioTable> {
    deal.validate_for_pricing()?;
    let original_balance = deal
        .tranches
        .tranches
        .iter()
        .find(|t| t.id.as_str() == tranche_id)
        .ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: format!("tranche:{tranche_id}"),
            })
        })?
        .original_balance
        .amount();

    let mut cells = Vec::with_capacity(grid.cprs.len() * grid.cdrs.len() * grid.severities.len());
    for &cpr in &grid.cprs {
        for &cdr in &grid.cdrs {
            for &severity in &grid.severities {
                let mut scenario = deal.clone();
                scenario.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(cpr);
                scenario.credit_model.default_spec = DefaultModelSpec::constant_cdr(cdr);
                // Inherit the deal's recovery lag unless the grid overrides it.
                let lag = grid
                    .recovery_lag
                    .unwrap_or(deal.credit_model.recovery_spec.recovery_lag);
                scenario.credit_model.recovery_spec =
                    RecoveryModelSpec::with_lag((1.0 - severity).clamp(0.0, 1.0), lag);

                let cashflows = scenario.get_tranche_cashflows(tranche_id, context, as_of)?;
                let pv = scenario.value_tranche(tranche_id, context, as_of)?;
                let price = if original_balance > 0.0 {
                    pv.amount() / original_balance * 100.0
                } else {
                    0.0
                };
                let wal = calculate_tranche_wal(&cashflows, as_of)?;

                cells.push(ScenarioCell {
                    cpr,
                    cdr,
                    severity,
                    price,
                    wal,
                    writedown: cashflows.total_writedown.amount(),
                });
            }
        }
    }

    Ok(ScenarioTable {
        tranche_id: tranche_id.to_string(),
        cells,
    })
}
