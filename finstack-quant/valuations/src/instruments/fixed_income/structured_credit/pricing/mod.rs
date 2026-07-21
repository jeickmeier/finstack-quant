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
pub(crate) mod stochastic;
pub(crate) mod waterfall;

pub use resolve::resolve_waterfall;
pub use waterfall::execute_waterfall;

use crate::cashflow::traits::DatedFlows;
use crate::instruments::fixed_income::structured_credit::pricing::simulation_engine::DeterministicPoolFlowSource;
use crate::instruments::fixed_income::structured_credit::types::{
    StructuredCredit, TrancheCashflows,
};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;

/// Run full deterministic cashflow simulation for a structured credit instrument.
///
/// # Arguments
///
/// * `instrument` - Validated structured-credit deal containing asset pool,
///   tranche structure, waterfall, and deterministic credit/prepayment model.
/// * `context` - Market context used for rate resets, discounting, and other
///   projection dependencies.
/// * `as_of` - Requested valuation date; lifecycle policy may resolve an
///   effective date from the market context.
pub fn run_simulation(
    instrument: &StructuredCredit,
    context: &MarketContext,
    as_of: Date,
) -> Result<HashMap<String, TrancheCashflows>> {
    let lifecycle =
        crate::instruments::common_impl::helpers::ValidatedPricingLifecycle::new(instrument)?;
    let effective_as_of = lifecycle.effective_as_of(context, as_of);
    let mut source = DeterministicPoolFlowSource;
    simulation_engine::run_simulation_with_source(instrument, context, effective_as_of, &mut source)
}

/// Generate aggregated deterministic cashflows for all tranches.
///
/// # Arguments
///
/// * `instrument` - Validated structured-credit deal to project through its
///   deterministic pool and waterfall model.
/// * `context` - Market context used for rate resets and projection inputs.
/// * `as_of` - Requested valuation date used to select known versus projected
///   cashflows.
pub fn generate_cashflows(
    instrument: &StructuredCredit,
    context: &MarketContext,
    as_of: Date,
) -> Result<DatedFlows> {
    let full_results = run_simulation(instrument, context, as_of)?;
    simulation_engine::aggregate_tranche_cashflows(&full_results)
}

/// Generate deterministic cashflows for a specific tranche.
///
/// # Arguments
///
/// * `instrument` - Validated structured-credit deal to project through its
///   deterministic pool and waterfall model.
/// * `tranche_id` - Identifier of the requested tranche within `instrument`.
/// * `context` - Market context used for rate resets and projection inputs.
/// * `as_of` - Requested valuation date used to select known versus projected
///   cashflows.
pub fn generate_tranche_cashflows(
    instrument: &StructuredCredit,
    tranche_id: &str,
    context: &MarketContext,
    as_of: Date,
) -> Result<TrancheCashflows> {
    let mut full_results = run_simulation(instrument, context, as_of)?;
    simulation_engine::take_tranche_cashflows(&mut full_results, tranche_id)
}

// Re-export stochastic types (accessible via stochastic module if needed)
pub use stochastic::CorrelationStructure;
pub use stochastic::StochasticDefaultSpec;
pub use stochastic::StochasticPrepaySpec;
