//! Analysis tools for financial statement models.
//!
//! This module provides tools for:
//!
//! - DCF corporate valuation and the orchestrated analysis pipeline
//! - Covenant forecasting and credit coverage metrics
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

pub use backtesting::{backtest_forecast, ForecastMetrics};
pub use credit::{
    compute_credit_context, forecast_breaches, CreditContextMetrics, CreditNumeratorNodes,
    StatementsAdapter,
};
pub use goal_seek::{goal_seek, GoalSeekResult};
pub use introspection::{
    explain_formula, render_tree_ascii, render_tree_detailed, DependencyTracer, DependencyTree,
    Explanation, ExplanationStep,
};
pub use reports::{
    CreditAssessment, CreditAssessmentPoint, CreditAssessmentReport, PLSummaryReport,
};
pub use scenarios::{
    generate_tornado_entries, run_sensitivity, run_variance, variance_bridge, BridgeChart,
    BridgeStep, ParameterSpec, ScenarioDefinition, ScenarioDiff, ScenarioResults, ScenarioSet,
    SensitivityConfig, SensitivityMode, SensitivityResult, SensitivityScenario, TornadoEntry,
    VarianceConfig, VarianceReport, VarianceRow,
};
pub use valuation::{
    dcf_sensitivity, evaluate_dcf, evaluate_lbo, run_corporate_analysis, wacc, CorporateAnalysis,
    CorporateAnalysisBuilder, CorporateAnalysisOptions, CorporateValuationResult, DcfOptions,
    DcfPeriodRow, DcfSensitivityResult, ExitMultipleBump, LboCheckMappings, LboConfig, LboResult,
    LboTranche, DEFAULT_UFCF_NODE,
};

pub use checks::{
    credit_underwriting_checks, lbo_model_checks, render_check_report_html,
    render_check_report_text, three_statement_checks, CapexReconciliation, CoverageFloorCheck,
    CreditMapping, DepreciationReconciliation, DividendReconciliation, EffectiveTaxRateCheck,
    FcfSignCheck, GrowthRateConsistency, InterestExpenseReconciliation, LeverageRangeCheck,
    LiquidityRunwayCheck, ThreeStatementMapping, TrendCheck, TrendDirection,
    WorkingCapitalConsistency,
};

pub use ecl::{
    classify_stage, classify_stage_from_curves, compute_ecl, compute_ecl_from_curve,
    compute_ecl_weighted, compute_ecl_weighted_from_curves, CeclConfig, CeclEngine,
    CeclMethodology, CeclResult, EclBucket, EclConfig, EclConfigBuilder, EclResult, Exposure,
    LgdType, MacroScenario, PdTermStructure, QualitativeFlags, RatingPdMap, RawPdCurve,
    ReversionMethod, Stage, StageResult, StagingConfig, StagingTrigger, WeightedEclResult,
    DEFAULT_REVOLVER_CCF,
};

pub use comps::{
    compute_multiple, compute_peer_multiples, peer_stats, percentile_rank, regression_fair_value,
    score_relative_value, z_score, CompanyId, CompanyMetrics, DimensionScore, MetricExtractor,
    Multiple, PeerFilter, PeerSet, PeerStats, PeriodBasis, RegressionResult, RelativeValueResult,
    ScoreDirection, ScoringDimension,
};
