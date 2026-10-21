//! Credit analysis tools.
//!
//! - [`crate::analysis::credit::covenants`] — covenant forecasting bridge between statements and the covenant engine
//! - [`crate::analysis::credit::credit_context`] — coverage ratios (cash / total / fee-inclusive DSCR, interest coverage, LTV path) from statement data

pub(crate) mod covenants;
pub(crate) mod credit_context;

pub use covenants::{forecast_breaches, StatementsAdapter};
pub use credit_context::{compute_credit_context, CreditContextMetrics, CreditNumeratorNodes};
