//! Deterministic scenario execution engine.
//!
//! Applies [`ScenarioSpec`] operations to a mutable [`ExecutionContext`] in a
//! fixed order, flushing market bumps after each operation so later adapters
//! observe the fully-applied prior state.

mod effects;
mod hierarchy;
mod instrument_shocks;
pub(crate) mod runner;
pub(crate) mod types;

#[cfg(test)]
mod tests;

pub(crate) use effects::ScenarioEffect;
#[cfg(test)]
pub(super) use runner::ScenarioEngine;
pub(crate) use types::HazardApplyEnv;
pub(crate) use types::{
    ApplicationReport, AppliedShock, AppliedShockTarget, ExecutionContext, LevelChange,
    RollForwardReport, ScenarioChangeManifest, ScenarioMarketTarget, ShockMagnitude, ShockNode,
    ShockUnit,
};
