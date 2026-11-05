//! Corporate valuation and orchestration.
//!
//! - [`corporate`] — DCF valuation integrated with statement models
//! - [`lbo`] — leveraged-buyout transaction arithmetic over a statement model
//! - [`orchestrator`] — fluent pipeline combining evaluation, equity, and credit

pub(crate) mod corporate;
pub(crate) mod lbo;
pub(crate) mod orchestrator;

pub use corporate::{
    CorporateValuationResult, DEFAULT_UFCF_NODE, DcfOptions, DcfPeriodRow, DcfSensitivityResult,
    ExitMultipleBump, WaccInputs, dcf_sensitivity, evaluate_dcf, wacc,
};
pub use lbo::{LboCheckMappings, LboConfig, LboResult, LboTranche, evaluate_lbo};
pub use orchestrator::{
    CorporateAnalysis, CorporateAnalysisBuilder, CorporateAnalysisOptions, run_corporate_analysis,
};
