//! Corporate valuation and orchestration.
//!
//! - [`corporate`] — DCF valuation integrated with statement models
//! - [`orchestrator`] — fluent pipeline combining evaluation, equity, and credit

pub(crate) mod corporate;
pub(crate) mod orchestrator;

pub use corporate::{evaluate_dcf_with_market, CorporateValuationResult, DcfOptions};
pub use orchestrator::{CorporateAnalysis, CorporateAnalysisBuilder, CreditInstrumentAnalysis};
