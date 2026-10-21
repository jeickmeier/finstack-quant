//! Pricing and cashflow projection for structured credit instruments.
//!
//! This module contains pure functions for:
//! - Deterministic cashflow simulation
//! - Waterfall execution
//! - Coverage test evaluation
//! - Diversion rule processing
//! - Stochastic pricing

pub(crate) mod coverage_tests;
pub(crate) mod resolve;
pub(crate) mod simulation_engine;
pub use simulation_engine::{
    CoverageTestDiagnostic, PeriodDiagnostics, SimulationDiagnostics, SimulationRun,
};
pub(crate) mod stochastic;
pub(crate) mod waterfall;

pub use waterfall::execute_waterfall;

use crate::structured_credit::types::{StructuredCredit, TrancheCashflows};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;

#[cfg(test)]
thread_local! {
    /// Deterministic simulations started on this thread; tests read it to
    /// count waterfall runs per pricing request.
    pub(crate) static SIMULATION_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Run full deterministic cashflow simulation for a structured credit instrument.
///
/// # Arguments
///
/// * `instrument` - Validated structured-credit deal containing asset pool,
///   tranche structure, waterfall, and deterministic credit/prepayment model.
/// * `market` - Market context used for rate resets, discounting, and other
///   projection dependencies.
/// * `as_of` - Requested valuation date; lifecycle policy may resolve an
///   effective date from the market context.
pub fn run_simulation(
    instrument: &StructuredCredit,
    market: &MarketContext,
    as_of: Date,
) -> Result<HashMap<String, TrancheCashflows>> {
    run_simulation_with_diagnostics(instrument, market, as_of).map(|run| run.tranches)
}

/// Run the deterministic cashflow simulation and return the tranche results
/// together with the deal-level accounting (reserve balance path, reserve
/// interest, draw funding).
///
/// # Arguments
///
/// * `instrument` - Validated structured-credit deal containing asset pool,
///   tranche structure, waterfall, and deterministic credit/prepayment model.
/// * `market` - Market context used for rate resets, discounting, and other
///   projection dependencies.
/// * `as_of` - Requested valuation date; lifecycle policy may resolve an
///   effective date from the market context.
///
/// # Errors
///
/// Propagates validation, schedule and cash-conservation errors from the
/// simulation engine.
pub fn run_simulation_with_diagnostics(
    instrument: &StructuredCredit,
    market: &MarketContext,
    as_of: Date,
) -> Result<simulation_engine::SimulationRun> {
    #[cfg(test)]
    SIMULATION_RUNS.with(|runs| runs.set(runs.get() + 1));
    let lifecycle =
        finstack_quant_valuations_core::instruments::common_impl::helpers::ValidatedPricingLifecycle::new(instrument)?;
    let effective_as_of = lifecycle.effective_as_of(market, as_of);
    simulation_engine::simulate_with_source(instrument, market, effective_as_of, None)
}
