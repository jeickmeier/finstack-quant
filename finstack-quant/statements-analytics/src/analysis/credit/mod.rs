//! Credit analysis tools.
//!
//! - [`crate::analysis::credit::capital_structure`] — debt stack by seniority: leverage and coverage through each payment class, and the maturity wall
//! - [`crate::analysis::credit::covenants`] — covenant bridge between statements and the covenant engine: series evaluation and breach forecasting
//! - [`crate::analysis::credit::credit_context`] — coverage ratios (cash / total / fee-inclusive DSCR, interest coverage, LTV path) from statement data

pub(crate) mod capital_structure;
pub(crate) mod covenants;
pub(crate) mod credit_context;

pub use capital_structure::{
    capital_structure_summary, CapitalStructureSummary, CapitalStructureSummaryConfig,
    CapitalStructureTranche, DebtInstrumentKind, MaturityWallBucket,
};
pub use covenants::{evaluate_covenants, forecast_breaches, StatementsAdapter};
pub use credit_context::{compute_credit_context, CreditContextMetrics, CreditNumeratorNodes};
