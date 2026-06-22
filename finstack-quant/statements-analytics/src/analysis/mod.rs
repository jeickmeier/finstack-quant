//! Analysis tools for financial statement models.
//!
//! This module provides tools for:
//!
//! - DCF corporate valuation and the orchestrated analysis pipeline
//! - Covenant forecasting and credit coverage metrics
//! - Scenario sets, sensitivity sweeps, variance, and Monte Carlo
//! - **[`crate::analysis::introspection`]** — Dependency tracing and formula explanation
//! - **[`crate::analysis::reports`]** — Formatted P&L summaries and credit assessment reports
//! - **[`mod@crate::analysis::goal_seek`]** — Root-finding for target metric values
//! - **[`crate::analysis::backtesting`]** — Forecast accuracy metrics
//! - Expected credit loss (IFRS 9 staging, CECL, portfolio aggregation)
//!
//! ## Where To Start
//!
//! - Use [`crate::analysis::CorporateAnalysisBuilder`] when you want one orchestrated pipeline
//!   that evaluates statements and optionally adds equity plus credit analysis.
//! - Use [`crate::analysis::evaluate_dcf_with_market`] for direct DCF valuation from a statement
//!   model.
//! - Use [`crate::analysis::ScenarioSet`] and [`crate::analysis::VarianceAnalyzer`] when comparing multiple
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

// ---- Grouped submodules ----

pub(crate) mod valuation;

pub(crate) mod credit;

pub(crate) mod scenarios;

/// Domain-level validation checks (reconciliation, consistency, credit).
pub mod checks;

pub(crate) mod ecl;

pub(crate) mod comps;

// ---- Flat submodules ----

pub mod backtesting;
pub mod goal_seek;
pub mod introspection;
pub mod reports;

// ---- Type-level re-exports (unchanged public API) ----

pub use backtesting::{backtest_forecast, ForecastMetrics};
pub use credit::{
    compute_credit_context, forecast_breaches, forecast_covenant, forecast_covenants, to_table,
    AdjustedNetDebtSpec, AdjustedNetDebtSpecBuilder, CreditContextMetrics, StatementsAdapter,
};
pub use goal_seek::goal_seek;
pub use introspection::{
    render_tree_ascii, render_tree_detailed, DependencyTracer, DependencyTree, Explanation,
    ExplanationStep, FormulaExplainer,
};
pub use reports::{
    Alignment, CreditAssessment, CreditAssessmentPoint, CreditAssessmentReport, PLSummaryReport,
    Report, TableBuilder,
};
pub use scenarios::{
    generate_tornado_entries, BridgeChart, BridgeStep, MonteCarloConfig, MonteCarloResults,
    ParameterSpec, PercentileSeries, ScenarioDefinition, ScenarioDiff, ScenarioResults,
    ScenarioSet, SensitivityAnalyzer, SensitivityConfig, SensitivityMode, SensitivityResult,
    TornadoEntry, VarianceAnalyzer, VarianceConfig, VarianceReport, VarianceRow,
};
pub use valuation::{
    evaluate_dcf_with_market, CorporateAnalysis, CorporateAnalysisBuilder,
    CorporateValuationResult, CreditInstrumentAnalysis, DcfOptions,
};

// ---- Check-framework re-exports ----

pub use checks::{
    corkscrew_as_checks, credit_underwriting_checks, lbo_model_checks, resolve_check_suite,
    three_statement_checks, CheckReportRenderer, CreditMapping, FormulaCheck,
    ThreeStatementMapping, TrendDirection,
};

// ---- ECL re-exports ----

pub use ecl::{
    binding_default_classify_stage_dpd_30_trigger, binding_default_classify_stage_dpd_90_trigger,
    binding_default_classify_stage_pd_delta_absolute,
    binding_default_compute_ecl_bucket_width_years, binding_default_cure_periods_stage2_to_1,
    binding_default_cure_periods_stage3_to_2, binding_default_exposure_dpd, classify_stage,
    compute_ecl_single, compute_ecl_weighted, compute_ecl_weighted_from_schedules,
    compute_waterfall, default_cecl_config, default_cecl_config_from_config, default_ecl_config,
    default_ecl_config_from_config, default_staging_config, default_staging_config_from_config,
    CeclConfig, CeclEngine, CeclMethodology, CeclResult, EclBucket, EclConfig, EclConfigBuilder,
    EclEngine, EclResult, Exposure, ExposureEclResult, LgdType, MacroScenario, PdTermStructure,
    PortfolioEclResult, ProvisionWaterfall, QualitativeFlags, RawPdCurve, ReversionMethod, Stage,
    StageResult, StagingConfig, StagingTrigger, WeightedEclResult, ECL_POLICY_EXTENSION_KEY,
};

// ---- Comps re-exports ----

pub use comps::{
    compute_multiple, compute_peer_multiples, peer_stats, percentile_rank, regression_fair_value,
    score_relative_value, z_score, CompanyId, CompanyMetrics, DimensionScore, MetricExtractor,
    Multiple, PeerFilter, PeerSet, PeerStats, PeriodBasis, RegressionResult, RelativeValueResult,
    ScoreDirection, ScoringDimension,
};
