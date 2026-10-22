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

use crate::structured_credit::types::StructuredCredit;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

#[cfg(test)]
thread_local! {
    /// Deterministic simulations started on this thread; tests read it to
    /// count waterfall runs per pricing request.
    pub(crate) static SIMULATION_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl StructuredCredit {
    /// Run deterministic cashflow simulation, returning tranche flows and deal accounting.
    ///
    /// The pricing lifecycle validates this deal once and resolves the effective
    /// valuation date before starting the simulation.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context supplying discount and forward curves and
    ///   lifecycle date policy for the collateral and liabilities.
    /// * `as_of` - Requested valuation date; market lifecycle policy can resolve
    ///   an effective date before the simulation begins.
    ///
    /// # Errors
    ///
    /// Propagates validation, missing-curve, schedule and cash-conservation errors.
    pub fn run_simulation(&self, market: &MarketContext, as_of: Date) -> Result<SimulationRun> {
        #[cfg(test)]
        SIMULATION_RUNS.with(|runs| runs.set(runs.get() + 1));
        let lifecycle = finstack_quant_valuations_core::instruments::common_impl::helpers::ValidatedPricingLifecycle::new(self)?;
        let effective_as_of = lifecycle.effective_as_of(market, as_of);
        simulation_engine::simulate_with_source(self, market, effective_as_of, None)
    }
}
