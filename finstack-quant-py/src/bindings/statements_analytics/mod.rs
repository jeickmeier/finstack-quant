//! Python bindings for the `finstack-quant-statements-analytics` crate.
//!
//! Exposes financial statement analysis: sensitivity, variance, scenario sets,
//! backtesting, goal seek, introspection, DCF/LBO valuation, corporate
//! analysis pipeline, reports, check suites, comparable-company analysis,
//! ECL, the corkscrew and credit-scorecard extensions, and the roll-forward /
//! vintage / real-estate templates.

mod analysis;
mod checks;
mod comps;
mod corkscrew;
mod ecl;
mod reports;
mod scorecards;
mod templates_common;
mod templates_real_estate;
mod templates_roll_forward;
mod templates_vintage;
mod typed;
mod valuation;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Render a unit-variant enum's canonical serde discriminant.
///
/// Single source for status strings (e.g. `CorkscrewStatus::Success` ->
/// `"success"`): the serde `rename_all` on the Rust enum is the wire
/// contract, so the bindings never re-declare the casing by hand.
pub(crate) fn serde_variant_str<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        _ => String::new(),
    }
}

/// Convert between a Python enum wrapper and its Rust twin through their
/// shared serde representation.
///
/// Both sides derive serde with the same `rename_all`, so one round-trip
/// replaces a hand-written `to_rust` / `from_rust` match table per enum.
pub(crate) fn enum_convert<A, B>(value: &A) -> PyResult<B>
where
    A: serde::Serialize,
    B: serde::de::DeserializeOwned,
{
    serde_json::to_value(value)
        .and_then(serde_json::from_value)
        .map_err(|e| crate::errors::serde_json_to_py(e, "enum conversion"))
}

/// Deserialize a canonical serde payload from a JSON string or a plain
/// Python object (dict/list) with the same shape.
pub(crate) fn extract_serde_any<'py, T: serde::de::DeserializeOwned + Send>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
    label: &str,
) -> PyResult<T> {
    if let Ok(json) = obj.extract::<String>() {
        return serde_json::from_str(&json)
            .map_err(|e| crate::errors::serde_json_to_py(e, &format!("invalid {label}")));
    }
    crate::bindings::module_utils::py_to_serde(py, obj, label)
}

/// Python-style rendering of a bool for `__repr__`.
pub(crate) fn py_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

/// Python-style rendering of an optional string for `__repr__`.
pub(crate) fn py_opt_str(value: Option<&str>) -> String {
    match value {
        Some(s) => format!("'{s}'"),
        None => "None".to_string(),
    }
}

/// Register the `statements_analytics` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "statements_analytics")?;
    m.setattr(
        "__doc__",
        "Statement analysis: sensitivity, variance, scenarios, backtesting, goal seek, DCF/LBO, corporate analysis, reports, check suites, introspection, comparable-company analysis, ECL, corkscrew/scorecard extensions, and roll-forward/vintage/real-estate templates.",
    )?;

    analysis::register(py, &m)?;
    typed::register(py, &m)?;
    valuation::register(py, &m)?;
    checks::register(py, &m)?;
    reports::register(py, &m)?;
    ecl::register(py, &m)?;
    comps::register(py, &m)?;
    scorecards::register(py, &m)?;
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
            "CompanyMetrics",
            "CorkscrewAccount",
            "CorkscrewConfig",
            "CorkscrewExtension",
            "CorkscrewReport",
            "CorporateAnalysis",
            "CorporateValuationResult",
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
            "LboCheckMappings",
            "LboResult",
            "LeaseGrowthConvention",
            "LeaseSpec",
            "ManagementFeeBase",
            "ManagementFeeSpec",
            "PLSummaryReport",
            "ParameterSpec",
            "PeerFilter",
            "PeerSet",
            "PeerStats",
            "PropertyTemplateNodes",
            "QualitativeFlags",
            "RegressionResult",
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
            "WeightedEclResult",
            "add_ncf_buildup",
            "add_noi_buildup",
            "add_property_operating_statement",
            "add_rent_roll",
            "add_roll_forward",
            "add_roll_forward_with_opening",
            "add_vintage_buildup",
            "backtest_forecast",
            "classify_stage",
            "compute_ecl",
            "compute_ecl_weighted",
            "compute_multiple",
            "credit_assessment",
            "credit_assessment_report_text",
            "dcf_sensitivity",
            "evaluate_dcf",
            "evaluate_lbo",
            "evaluate_scenario_set",
            "explain_formula",
            "explain_formula_text",
            "generate_tornado_entries",
            "goal_seek",
            "peer_stats",
            "percentile_rank",
            "pl_summary_report",
            "pl_summary_report_text",
            "regression_fair_value",
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
            "validate_scorecard_config",
            "variance_bridge",
            "wacc",
            "z_score",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "statements_analytics",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;

    Ok(())
}
