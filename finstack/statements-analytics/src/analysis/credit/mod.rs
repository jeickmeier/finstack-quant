//! Credit analysis tools.
//!
//! - [`crate::analysis::credit::covenants`] — covenant forecasting bridge between statements and the covenant engine
//! - [`crate::analysis::credit::credit_context`] — coverage ratios (DSCR, interest coverage, LTV) from statement data
//! - [`crate::analysis::credit::adjusted_net_debt`] — rating-agency Adjusted Net Debt bridge

pub(crate) mod adjusted_net_debt;
pub(crate) mod covenants;
pub(crate) mod credit_context;

pub use adjusted_net_debt::{AdjustedNetDebtSpec, AdjustedNetDebtSpecBuilder};
pub use covenants::{
    forecast_breaches, forecast_covenant, forecast_covenants, to_table, StatementsAdapter,
};
pub use credit_context::{compute_credit_context, CreditContextMetrics};
