//! Shared cashflow simulation engine for structured credit instruments.
//!
//! This module provides pure functions for running period-by-period
//! cashflow simulation through the waterfall engine. Deterministic and
//! stochastic pricing differ only in how they source pool SMM/MDR/recovery
//! assumptions for each legal payment period.

pub(super) mod conservation;
mod delinquency;
pub(crate) mod diagnostics;
mod exercise;
mod hedges;
mod instrument_flows;
mod instrument_paths;
pub(crate) mod orchestration;
pub(crate) mod period_helpers;
pub(super) mod pool_flow_source;
pub(super) mod pool_flows;
pub(super) mod recovery;
mod reserve;
mod simulate_period;
pub(crate) mod state;
mod triggers;

pub(crate) use diagnostics::{
    NonWaterfallPayment, NonWaterfallSource, PeriodDiagnostics, SimulationDiagnostics,
};

pub(crate) use instrument_flows::PreparedInstrumentSchedules;
pub(crate) use instrument_paths::InstrumentPathFlowSource;
pub(crate) use orchestration::SimulationRun;
pub(crate) use orchestration::{
    PreparedDealSimulation, prepare_deal_simulation, simulate_prepared, simulate_with_source,
};
pub(crate) use pool_flow_source::{
    DeterministicPoolFlowSource, OasPathFlowSource, PathShocks, PerNameDefaultEngine,
    PerNamePeriodInput, PeriodPoolShock, PoolFlowRequest, PoolFlowSource, StochasticPathFlowSource,
};

#[cfg(test)]
mod tests;
