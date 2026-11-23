//! WASM bindings for the `finstack-quant-statements-analytics` crate.
//!
//! Exposes financial statement analysis functions. Inputs are JSON strings
//! or plain objects; results are structured JavaScript objects, except the
//! `*Text` functions, which return formatted text. The stateful pieces are
//! classes: `DependencyTracer`, `CorkscrewExtension` and
//! `CreditScorecardExtension`.

mod capital_structure;
mod checks;
mod comps;
mod covenants;
mod ecl;
mod handles;
mod members;
mod sensitivity;
mod templates;
mod valuation;

pub use capital_structure::capital_structure_summary;
pub use checks::*;
pub use comps::{
    compute_multiple, peer_stats, percentile_rank, regression_fair_value, relative_value_report,
    score_relative_value, z_score,
};
pub use covenants::{
    assess_statement_covenants, evaluate_covenants, forecast_breaches, forecast_statement_covenants,
};
pub use handles::{JsCorkscrewExtension, JsCreditScorecardExtension, JsDependencyTracer};
pub use sensitivity::*;
pub use valuation::*;
