//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "canonical_api.rs"]
mod canonical_api;
#[path = "cashflow.rs"]
mod cashflow;
#[path = "contract.rs"]
mod contract;
#[path = "dates.rs"]
mod dates;
#[path = "expr.rs"]
mod expr;
#[path = "golden_tests.rs"]
mod golden_tests;
#[path = "infrastructure.rs"]
mod infrastructure;
#[path = "market_data.rs"]
mod market_data;
#[path = "math.rs"]
mod math;
#[path = "money.rs"]
mod money;
#[path = "phase2_strictness.rs"]
mod phase2_strictness;
#[path = "production_audit.rs"]
mod production_audit;
#[path = "serde.rs"]
mod serde;
#[path = "sobol_golden.rs"]
mod sobol_golden;
#[path = "types.rs"]
mod types;
