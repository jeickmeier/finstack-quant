//! Corporate valuation and orchestration.
//!
//! - [`corporate`] — DCF valuation integrated with statement models
//! - [`lbo`] — leveraged-buyout transaction arithmetic over a statement model
//! - [`orchestrator`] — fluent pipeline combining evaluation, equity, and credit

pub(crate) mod corporate;
pub(crate) mod lbo;
pub(crate) mod orchestrator;

pub use corporate::{
    dcf_sensitivity, evaluate_dcf_with_market, wacc, CorporateValuationResult, DcfOptions,
    DcfSensitivityResult, ExitMultipleBump,
};
pub use lbo::{evaluate_lbo, LboCheckMappings, LboConfig, LboResult, LboTranche};
pub use orchestrator::{CorporateAnalysis, CorporateAnalysisBuilder, CreditInstrumentAnalysis};
