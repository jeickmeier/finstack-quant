//! Credit analysis tools.
//!
//! - [`crate::analysis::credit::capital_structure`] — debt stack by seniority: leverage and coverage through each payment class, and the maturity wall
//! - [`crate::analysis::credit::covenants`] — covenant bridge between statements and the covenant engine: series evaluation and breach forecasting
//! - [`crate::analysis::credit::credit_context`] — coverage ratios (cash / total / fee-inclusive DSCR, interest coverage, LTV path) from statement data

pub(crate) mod capital_structure;
pub(crate) mod covenants;
pub(crate) mod credit_context;

pub use capital_structure::{
    CapitalStructureSummary, CapitalStructureSummaryConfig, CapitalStructureTranche,
    DebtInstrumentKind, MaturityWallBucket, capital_structure_summary,
};
pub use covenants::{StatementsAdapter, evaluate_covenants, forecast_breaches};
pub use credit_context::{
    CreditContextMetrics, CreditContextReport, CreditContextSpec, CreditMeasure,
    CreditMeasureCoverage, CreditMeasureRow, CreditMeasureValue, CreditNumeratorNodes,
    CreditReferenceValue, compute_credit_context, evaluate_credit_context,
};

mod scoring_bridge;
pub use scoring_bridge::{
    CreditScoringModel, StatementScoringMapping, score_statement, score_statement_series,
};

mod verified_covenants;
pub use verified_covenants::{
    CovenantObservationBinding, CovenantStatementUnit, StatementCovenantAssessment,
    StatementCovenantForecast, StatementCovenantPeriod, StatementCovenantSpec,
    assess_statement_covenants, forecast_statement_covenants,
};
