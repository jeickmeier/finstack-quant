//! Corporate valuation using DCF methodology.
//!
//! This module provides integration between financial statement models and
//! DCF (Discounted Cash Flow) valuation, allowing direct valuation of companies
//! from forecast models.

pub(crate) mod dcf;
pub(crate) mod path;
pub(crate) mod sensitivity;
#[cfg(test)]
mod tests;
pub(crate) mod types;
pub(crate) mod wacc;

use dcf::evaluate_dcf_from_results;
pub(crate) use dcf::{evaluate_dcf_from_results_impl, extract_currency_from_model};
pub(super) use dcf::{monetary_node_money, monetary_node_value, trailing_year_metric};
use types::CorporateDcfRequest;
pub(crate) use types::{
    CorporateValuationResult, DEFAULT_UFCF_NODE, DcfEvalContext, DcfOptions, StatementValuationCut,
};
use wacc::{
    SENSITIVITY_PARAM_EXIT_MULTIPLE, SENSITIVITY_PARAM_TERMINAL_GROWTH, SENSITIVITY_PARAM_WACC,
};
