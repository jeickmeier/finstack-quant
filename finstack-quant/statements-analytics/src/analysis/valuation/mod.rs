//! Corporate valuation and orchestration.
//!
//! - [`corporate`] — DCF valuation integrated with statement models
//! - [`lbo`] — leveraged-buyout transaction arithmetic over a statement model
//! - [`orchestrator`] — fluent pipeline combining evaluation, equity, and credit

pub(crate) mod corporate;
pub(crate) mod lbo;
pub(crate) mod orchestrator;

pub use corporate::{
    CorporateDcfRequest, CorporateValuationResult, DEFAULT_UFCF_NODE, DcfDistributionResult,
    DcfInputs, DcfMultipleShocks, DcfOptions, DcfPathQuantile, DcfPathValue, DcfPeriodRow,
    DcfProjection, DcfRateShocks, DcfSensitivityEndpoint, DcfSensitivityGridResult,
    DcfSensitivityGridSpec, DcfSensitivityPair, DcfSensitivityResult, DcfSensitivityShocks,
    DcfShockReport, DcfTerminalShocks, ExitMultipleBump, StatementValuationCut, TerminalFcffBasis,
    WaccInputs, dcf_sensitivity, dcf_sensitivity_grid, dcf_sensitivity_on_results,
    evaluate_corporate_dcf, evaluate_dcf, evaluate_dcf_from_results, evaluate_dcf_monte_carlo,
    wacc,
};
pub use lbo::{LboCheckMappings, LboConfig, LboResult, LboTranche, evaluate_lbo};
pub use orchestrator::{
    CorporateAnalysis, CorporateAnalysisBuilder, CorporateAnalysisOptions, run_corporate_analysis,
};
