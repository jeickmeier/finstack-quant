//! Scenario tree infrastructure for stochastic structured credit.
//!
//! This module provides:
//! - **ScenarioNode**: Node state with prepayment, default, and recovery
//! - **ScenarioTree**: Recombining lattice representation of scenario paths
//! - **ScenarioTreeConfig**: Tree generation configuration
//!
//! # Design Philosophy
//!
//! The implementation uses a recombining lattice geometry (O(n²) nodes)
//! while still storing path-dependent statistics (burnout, cumulative
//! prepayments/defaults). This provides sufficient accuracy for structured
//! products without the exponential blow-up of a full non-recombining tree.
//!
//! # Usage
//!
//! ```text
//! use finstack_quant_valuations::instruments::fixed_income::structured_credit::pricing::stochastic::tree::{
//!     BranchingSpec, ScenarioTree, ScenarioTreeConfig,
//! };
//!
//! // 60 monthly periods over a 5-year horizon (3-branch recombining lattice)
//! let config = ScenarioTreeConfig::new(60, 5.0, BranchingSpec::fixed(3));
//! let tree = ScenarioTree::build(&config).expect("tree build should succeed");
//!
//! // Compute expected value of remaining pool balance at horizon
//! let ev = tree.expected_value(|node| node.pool_balance);
//! # let _ = ev;
//! ```

mod config;
#[cfg(test)]
mod node;
#[allow(clippy::module_inception)]
#[cfg(test)]
mod tree;

pub(crate) use config::{BranchingSpec, ScenarioTreeConfig};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use node::{ScenarioNode, ScenarioNodeId, ScenarioPath};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use tree::ScenarioTree;
