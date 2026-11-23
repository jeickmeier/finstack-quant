//! Credit analysis tools.
//!
//! - [`crate::analysis::credit::capital_structure`] — debt stack by seniority: leverage and coverage through each payment class, and the maturity wall
//! - [`crate::analysis::credit::covenants`] — covenant bridge between statements and the covenant engine: series evaluation and breach forecasting
//! - [`crate::analysis::credit::credit_context`] — coverage ratios (cash / total / fee-inclusive DSCR, interest coverage, LTV path) from statement data

pub(crate) mod capital_structure;
pub(crate) mod covenants;
pub(crate) mod credit_context;
pub(crate) mod scoring_bridge;
pub(crate) mod verified_covenants;

pub(crate) use credit_context::{
    CreditContextMetrics, CreditNumeratorNodes, compute_credit_context,
};
