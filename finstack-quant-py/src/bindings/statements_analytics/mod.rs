//! Python bindings for the `finstack-quant-statements-analytics` crate.
//!
//! Exposes financial statement analysis: sensitivity, variance, scenario sets,
//! backtesting, goal seek, introspection, DCF/LBO valuation, corporate
//! analysis pipeline, reports, check suites, comparable-company analysis,
//! ECL, the corkscrew and credit-scorecard extensions, and the roll-forward /
//! vintage / real-estate templates.

mod analysis;
mod capital_structure;
mod checks;
mod comps;
mod corkscrew;
mod covenants;
mod credit_context;
pub(crate) mod ecl;
mod lbo;
mod relative_value;
mod reports;
mod scorecards;
mod scoring;
pub(crate) mod templates_real_estate;
mod templates_roll_forward;
mod templates_vintage;
mod typed;
pub(crate) mod valuation;

use pyo3::prelude::*;
use pyo3::types::PyList;
mod convert;

pub(crate) use convert::{enum_convert, py_bool, py_opt_str};

/// Register the `statements_analytics` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "statements_analytics")?;
    m.setattr(
        "__doc__",
        "Statement analysis: sensitivity, variance, scenarios, backtesting, goal seek, DCF/LBO, corporate analysis, reports, check suites, introspection, comparable-company analysis, ECL, corkscrew/scorecard extensions, and roll-forward/vintage/real-estate templates.",
    )?;

    analysis::register(py, &m)?;
    typed::register(py, &m)?;
    lbo::register(&m)?;
    valuation::register(py, &m)?;
    checks::register(py, &m)?;
    capital_structure::register(py, &m)?;
    relative_value::register(py, &m)?;
    covenants::register(py, &m)?;
    credit_context::register(&m)?;
    reports::register(py, &m)?;
    ecl::register(py, &m)?;
    comps::register(py, &m)?;
    scorecards::register(py, &m)?;
    scoring::register(&m)?;
    corkscrew::register(py, &m)?;
    templates_vintage::register(py, &m)?;
    templates_roll_forward::register(py, &m)?;
    templates_real_estate::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "AccountType",
            "BridgeChart",
            "BridgeStep",
            "CapitalStructureSummary",
            "CapitalStructureTranche",
            "CompanyMetrics",
            "CorkscrewAccount",
            "CorkscrewConfig",
            "CorkscrewExtension",
            "CorkscrewReport",
            "CorporateAnalysis",
            "CorporateValuationResult",
            "CreditContextSpec",
            "CreditContextReport",
            "CreditContextMetrics",
            "CreditMeasureRow",
            "evaluate_credit_context",
            "StatementScoringMapping",
            "CreditScoringModel",
            "score_statement",
            "score_statement_series",
            "CreditAssessment",
            "CreditAssessmentPoint",
            "CreditMapping",
            "CreditScorecardExtension",
            "DcfSensitivityResult",
            "DependencyTracer",
            "DependencyTree",
            "DimensionScore",
            "EclBucket",
            "EclResult",
            "EquityBridge",
            "Explanation",
            "ExplanationStep",
            "Exposure",
            "ForecastMetrics",
            "FreeRentWindowSpec",
            "GoalSeekResult",
            "GoalSeekSpec",
            "LboCheckMappings",
            "LboResult",
            "LeaseGrowthConvention",
            "LeaseSpec",
            "ManagementFeeBase",
            "ManagementFeeSpec",
            "MaturityWallBucket",
            "PLSummaryReport",
            "ParameterSpec",
            "PeerFilter",
            "PeerSet",
            "PeerStats",
            "PropertyTemplateNodes",
            "QualitativeFlags",
            "RegressionResult",
            "RelativeValueCompany",
            "RelativeValueDimension",
            "RelativeValueMultiple",
            "RelativeValuePoint",
            "RelativeValueReport",
            "RelativeValueResult",
            "RenewalSpec",
            "RentRollOutputNodes",
            "RentStepSpec",
            "ScenarioDiff",
            "ScenarioResults",
            "ScenarioSet",
            "ScorecardConfig",
            "ScorecardMetric",
            "ScorecardReport",
            "ScoringDimension",
            "SensitivityConfig",
            "SensitivityResult",
            "Stage",
            "StageResult",
            "StagingConfig",
            "TerminalValueSpec",
            "ThreeStatementMapping",
            "TornadoEntry",
            "ValuationDiscounts",
            "VarianceConfig",
            "VarianceReport",
            "VarianceRow",
            "ImpairedEclScenario",
            "WeightedImpairedEclResult",
            "compute_impaired_ecl_weighted",
            "ImpairedExposure",
            "RecoveryEvidence",
            "ImpairedEclResult",
            "compute_impaired_ecl",
            "EclScenario",
            "EclScenarioOutcome",
            "WeightedEclResult",
            "add_ncf_buildup",
            "add_noi_buildup",
            "add_property_operating_statement",
            "add_rent_roll",
            "add_roll_forward",
            "add_roll_forward_with_opening",
            "add_vintage_buildup",
            "backtest_forecast",
            "capital_structure_summary",
            "classify_stage",
            "compute_ecl",
            "compute_ecl_weighted",
            "compute_multiple",
            "credit_assessment",
            "credit_assessment_report_text",
            "dcf_sensitivity",
            "StatementCovenantSpec",
            "StatementCovenantAssessment",
            "StatementCovenantForecast",
            "assess_statement_covenants",
            "forecast_statement_covenants",
            "evaluate_covenants",
            "DcfInputs",
            "DcfOptions",
            "StatementValuationCut",
            "TerminalFcffBasis",
            "DcfProjection",
            "CorporateDcfRequest",
            "DcfSensitivityGridSpec",
            "DcfSensitivityGridResult",
            "evaluate_corporate_dcf",
            "evaluate_dcf_from_results",
            "dcf_sensitivity_grid",
            "dcf_sensitivity_on_results",
            "DcfDistributionResult",
            "evaluate_dcf_monte_carlo",
            "evaluate_dcf",
            "LboEarnings",
            "LboDebtFunding",
            "LboFunding",
            "DatedLboRequest",
            "LboFundingResult",
            "LboExitBridge",
            "DatedLboResult",
            "evaluate_dated_lbo",
            "evaluate_lbo",
            "evaluate_scenario_set",
            "evaluate_scenario_set_with_market",
            "explain_formula",
            "forecast_breaches",
            "generate_tornado_entries",
            "goal_seek",
            "peer_stats",
            "percentile_rank",
            "pl_summary_report",
            "regression_fair_value",
            "relative_value_report",
            "render_check_report_html",
            "render_check_report_text",
            "run_checks",
            "run_corporate_analysis",
            "run_credit_underwriting_checks",
            "run_sensitivity",
            "run_three_statement_checks",
            "run_variance",
            "scenario_diff",
            "score_relative_value",
            "variance_bridge",
            "WaccInputs",
            "wacc",
            "z_score",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}
