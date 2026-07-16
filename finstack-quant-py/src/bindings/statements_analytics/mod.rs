//! Python bindings for the `finstack-quant-statements-analytics` crate.
//!
//! Exposes financial statement analysis: sensitivity, variance, scenario sets,
//! backtesting, goal seek, introspection, DCF valuation, corporate analysis
//! pipeline, Monte Carlo, reports, comparable-company analysis, ECL, the
//! corkscrew and credit-scorecard extensions, and the roll-forward / vintage /
//! real-estate templates.

mod analysis;
mod comps;
mod corkscrew;
mod ecl;
mod scorecards;
mod templates_common;
mod templates_real_estate;
mod templates_roll_forward;
mod templates_vintage;
mod typed;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `statements_analytics` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "statements_analytics")?;
    m.setattr(
        "__doc__",
        "Statement analysis: sensitivity, variance, scenarios, backtesting, goal seek, DCF, corporate, Monte Carlo, reports, introspection, comparable-company analysis, ECL, corkscrew/scorecard extensions, and roll-forward/vintage/real-estate templates.",
    )?;

    analysis::register(py, &m)?;
    typed::register(py, &m)?;
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
            "SensitivityConfig",
            "VarianceConfig",
            "ScenarioSet",
            "MonteCarloConfig",
            "SensitivityResult",
            "VarianceRow",
            "VarianceReport",
            "ScenarioResultSet",
            "MonteCarloResults",
            "run_sensitivity",
            "generate_tornado_entries",
            "run_variance",
            "evaluate_scenario_set",
            "run_monte_carlo",
            "backtest_forecast",
            "goal_seek",
            "evaluate_dcf",
            "run_corporate_analysis",
            "pl_summary_report",
            "credit_assessment_report",
            "DependencyTracer",
            "direct_dependencies",
            "all_dependencies",
            "dependents",
            "explain_formula",
            "explain_formula_text",
            "run_checks",
            "run_three_statement_checks",
            "run_credit_underwriting_checks",
            "render_check_report_text",
            "render_check_report_html",
            "Exposure",
            "classify_stage",
            "compute_ecl",
            "compute_ecl_weighted",
            // Comparable-company analysis
            "percentile_rank",
            "z_score",
            "peer_stats",
            "regression_fair_value",
            "compute_multiple",
            "score_relative_value",
            // Scorecard extension
            "ScorecardMetric",
            "ScorecardConfig",
            "ScorecardReport",
            "CreditScorecardExtension",
            "validate_scorecard_config",
            // Corkscrew extension
            "AccountType",
            "CorkscrewAccount",
            "CorkscrewConfig",
            "CorkscrewReport",
            "CorkscrewExtension",
            // Vintage template
            "add_vintage_buildup",
            // Roll-forward template
            "add_roll_forward",
            "add_roll_forward_with_opening",
            // Real-estate template
            "SimpleLeaseSpec",
            "RentStepSpec",
            "FreeRentWindowSpec",
            "RenewalSpec",
            "LeaseGrowthConvention",
            "LeaseSpec",
            "RentRollOutputNodes",
            "ManagementFeeBase",
            "ManagementFeeSpec",
            "PropertyTemplateNodes",
            "add_noi_buildup",
            "add_ncf_buildup",
            "add_rent_roll",
            "add_rent_roll_rental_revenue",
            "add_property_operating_statement",
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
