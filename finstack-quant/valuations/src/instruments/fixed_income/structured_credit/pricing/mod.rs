//! Pricing and cashflow projection for structured credit instruments.
//!
//! This module contains pure functions for:
//! - Deterministic cashflow simulation
//! - Waterfall execution
//! - Coverage test evaluation
//! - Diversion rule processing
//! - Stochastic pricing

pub(crate) mod coverage_tests;
pub(crate) mod diversion;
pub(crate) mod resolve;
pub(crate) mod simulation_engine;
pub(crate) mod stochastic;
pub(crate) mod waterfall;

pub use resolve::{
    apply_accumulation_lockout, apply_shifting_interest, apply_step_down, resolve_waterfall,
};
pub use waterfall::{execute_waterfall, execute_waterfall_with_workspace};

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
pub fn run_simulation(
    instrument: &StructuredCredit,
    context: &MarketContext,
    as_of: Date,
) -> Result<HashMap<String, TrancheCashflows>> {
    let mut source = DeterministicPoolFlowSource;
    simulation_engine::run_simulation_with_source(instrument, context, as_of, &mut source)
}

/// Generate aggregated deterministic cashflows for all tranches.
pub fn generate_cashflows(
    instrument: &StructuredCredit,
    context: &MarketContext,
    as_of: Date,
) -> Result<DatedFlows> {
    let full_results = run_simulation(instrument, context, as_of)?;
    simulation_engine::aggregate_tranche_cashflows(&full_results)
}

/// Generate deterministic cashflows for a specific tranche.
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
#[allow(unused_imports)] // May be used by external bindings
pub use stochastic::CorrelationStructure;
#[allow(unused_imports)] // May be used by external bindings
pub use stochastic::StochasticDefaultSpec;
#[allow(unused_imports)] // May be used by external bindings
pub use stochastic::StochasticPrepaySpec;
