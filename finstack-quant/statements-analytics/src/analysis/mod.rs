//! Analysis tools for financial statement models.
//!
//! This module provides tools for:
//!
//! - DCF corporate valuation and the orchestrated analysis pipeline
//! - Covenant forecasting, credit coverage metrics and the debt stack by seniority
//! - Scenario sets, sensitivity sweeps, and variance
//! - Dependency tracing and formula explanation ([`DependencyTracer`], [`explain_formula`])
//! - Formatted P&L summaries and credit assessment reports ([`PLSummaryReport`])
//! - Root-finding for target metric values ([`goal_seek()`])
//! - Forecast accuracy metrics ([`backtest_forecast`])
//! - Domain-level validation checks ([`three_statement_checks`] and the check types)
//! - Expected credit loss (IFRS 9 staging, CECL, portfolio aggregation)
//!
//! ## Where To Start
//!
//! - Use [`crate::analysis::CorporateAnalysisBuilder`] when you want one orchestrated pipeline
//!   that evaluates statements and optionally adds equity plus credit analysis.
//! - Use [`crate::analysis::evaluate_dcf`] for direct DCF valuation from a statement
//!   model.
//! - Use [`crate::analysis::ScenarioSet`] and [`crate::analysis::run_variance`] when comparing multiple
//!   operating cases.
//! - Use [`crate::analysis::forecast_breaches`] and [`crate::analysis::compute_credit_context`] for lender-style
//!   compliance and coverage analysis.
//!
//! ## Conventions
//!
//! - Ratios such as DSCR, coverage, leverage, and valuation multiples are
//!   returned as plain scalars, so `2.0` means `2.0x`.
//! - Percentage-style inputs, such as WACC or growth assumptions, follow the
//!   crate-wide decimal convention: `0.10` means `10%`.
//! - Scenario overrides are deterministic full-period scalar overrides unless a
//!   lower-level API states otherwise.

pub(crate) mod valuation;

pub(crate) mod credit;

pub(crate) mod scenarios;

mod units;

mod checks;

pub(crate) mod ecl;

pub(crate) mod comps;

mod backtesting;
mod goal_seek;
mod introspection;
mod reports;

pub use backtesting::{ForecastMetrics, backtest_forecast};
pub use credit::capital_structure::{
    CapitalStructureSummary, CapitalStructureSummaryConfig, CapitalStructureTranche,
    DebtInstrumentKind, MaturityWallBucket, capital_structure_summary,
};
pub use credit::covenants::{StatementsAdapter, evaluate_covenants, forecast_breaches};
pub use credit::credit_context::{
    CreditContextMetrics, CreditContextReport, CreditContextSpec, CreditMeasure,
    CreditMeasureCoverage, CreditMeasureRow, CreditMeasureValue, CreditNumeratorNodes,
    CreditReferenceValue, compute_credit_context, evaluate_credit_context,
};
pub use credit::scoring_bridge::{
    CreditScoringModel, StatementScoringMapping, score_statement, score_statement_series,
};
pub use credit::verified_covenants::{
    CovenantObservationBinding, CovenantStatementUnit, StatementCovenantAssessment,
    StatementCovenantForecast, StatementCovenantPeriod, StatementCovenantSpec,
    assess_statement_covenants, forecast_statement_covenants,
};
pub use goal_seek::{GoalSeekCell, GoalSeekResult, GoalSeekSearch, GoalSeekSpec, goal_seek};
pub use introspection::{
    DependencyTracer, DependencyTree, Explanation, ExplanationStep, explain_formula,
    render_tree_ascii, render_tree_detailed,
};
pub use reports::{
    CreditAssessment, CreditAssessmentPoint, CreditAssessmentReport, PLSummaryReport,
};
pub use scenarios::scenario_set::{ScenarioDefinition, ScenarioDiff, ScenarioResults, ScenarioSet};
pub use scenarios::sensitivity::{generate_tornado_entries, run_sensitivity};
pub use scenarios::types::{
    ParameterSpec, SensitivityConfig, SensitivityMode, SensitivityResult, SensitivityScenario,
    TornadoEntry,
};
pub use scenarios::variance::{
    BridgeChart, BridgeStep, VarianceConfig, VarianceReport, VarianceRow, run_variance,
    variance_bridge,
};
pub use valuation::corporate::dcf::{
    evaluate_corporate_dcf, evaluate_dcf, evaluate_dcf_from_results,
};
pub use valuation::corporate::path::{
    DcfDistributionResult, DcfPathQuantile, DcfPathValue, evaluate_dcf_monte_carlo,
};
pub use valuation::corporate::sensitivity::{
    DcfMultipleShocks, DcfRateShocks, DcfSensitivityEndpoint, DcfSensitivityGridResult,
    DcfSensitivityGridSpec, DcfSensitivityPair, DcfSensitivityShocks, DcfShockReport,
    DcfTerminalShocks, dcf_sensitivity_grid, dcf_sensitivity_on_results,
};
pub use valuation::corporate::types::{
    CorporateDcfRequest, CorporateValuationResult, DEFAULT_UFCF_NODE, DcfInputs, DcfOptions,
    DcfPeriodRow, DcfProjection, ExitMultipleBump, StatementValuationCut, TerminalFcffBasis,
};
pub use valuation::corporate::wacc::{DcfSensitivityResult, WaccInputs, dcf_sensitivity, wacc};
pub use valuation::lbo::{LboConfig, LboResult, LboTranche, evaluate_lbo};
pub use valuation::lbo_funding::{
    DatedLboRequest, DatedLboResult, LboAcquisition, LboCheckMappings, LboCheckSuite,
    LboDebtFunding, LboEarnings, LboEarningsBasis, LboExitBridge, LboFunding, LboFundingResult,
    LboQualification, evaluate_dated_lbo,
};
pub use valuation::orchestrator::{
    CorporateAnalysis, CorporateAnalysisBuilder, CorporateAnalysisOptions, run_corporate_analysis,
};

pub use checks::consistency::growth_rate::GrowthRateConsistency;
pub use checks::consistency::tax_rate::EffectiveTaxRateCheck;
pub use checks::consistency::working_capital::WorkingCapitalConsistency;
pub use checks::credit::coverage::CoverageFloorCheck;
pub use checks::credit::fcf_sign::FcfSignCheck;
pub use checks::credit::leverage::LeverageRangeCheck;
pub use checks::credit::liquidity::LiquidityRunwayCheck;
pub use checks::credit::trend::{TrendCheck, TrendDirection};
pub use checks::mappings::{CreditMapping, ThreeStatementMapping};
pub use checks::reconciliation::capex::CapexReconciliation;
pub use checks::reconciliation::depreciation::DepreciationReconciliation;
pub use checks::reconciliation::dividends::DividendReconciliation;
pub use checks::reconciliation::interest::InterestExpenseReconciliation;
pub use checks::renderer::{render_check_report_html, render_check_report_text};
pub use checks::suites::{credit_underwriting_checks, lbo_model_checks, three_statement_checks};

pub use ecl::cecl::{CeclConfig, CeclEngine, CeclMethodology, CeclResult, ReversionMethod};
pub use ecl::engine::{
    EclBucket, EclConfig, EclConfigBuilder, EclResult, EclScenarioMeasurement, EclScenarioOutcome,
    LgdType, MacroScenario, WeightedEclResult, compute_ecl_from_curve,
    compute_ecl_weighted_from_curves,
};
pub use ecl::recovery::{
    ImpairedEclMeasurement, ImpairedEclOutcome, ImpairedEclResult, ImpairedEclScenario,
    ImpairedExposure, RecoveryEvidence, WeightedImpairedEclResult, compute_impaired_ecl,
    compute_impaired_ecl_weighted,
};
pub use ecl::request::{EclScenario, classify_stage, compute_ecl, compute_ecl_weighted};
pub use ecl::staging::{StageResult, StagingConfig, StagingTrigger, classify_stage_from_curves};
pub use ecl::types::{
    DEFAULT_REVOLVER_CCF, Exposure, PdTermStructure, QualitativeFlags, RatingPdMap, RawPdCurve,
    Stage,
};

pub use comps::multiples::{compute_multiple, compute_peer_multiples};
pub use comps::peer_set::{PeerFilter, PeerSet};
pub use comps::report::{
    CompositeExclusion, RelativeValueCompany, RelativeValueDimension, RelativeValueMultiple,
    RelativeValuePoint, RelativeValueReport, RelativeValueReportConfig, relative_value_report,
};
pub use comps::scoring::{
    DimensionExclusion, DimensionScore, MetricExtractor, RelativeValueResult, ScoreDirection,
    ScoringDimension, score_relative_value,
};
pub use comps::stats::{
    PeerStats, RegressionResult, peer_stats, percentile_rank, regression_fair_value, z_score,
};
pub use comps::types::{CompanyId, CompanyMetrics, Multiple, PeriodBasis};
