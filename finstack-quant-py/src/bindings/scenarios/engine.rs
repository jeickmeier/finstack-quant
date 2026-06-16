//! Python wrappers for scenario engine application.

use crate::bindings::extract::{extract_market, extract_model};
use crate::errors::display_to_py;
use pyo3::prelude::*;
use pyo3::types::PyDict;

fn set_warning_items(
    dict: &Bound<'_, PyDict>,
    warnings: &[finstack_quant_scenarios::Warning],
) -> PyResult<()> {
    let warning_strs: Vec<String> = warnings.iter().map(ToString::to_string).collect();
    dict.set_item("warnings", warning_strs)?;
    let warnings_json = serde_json::to_string(warnings).map_err(display_to_py)?;
    dict.set_item("warnings_json", warnings_json)
}

fn set_report_items(
    dict: &Bound<'_, PyDict>,
    report: &finstack_quant_scenarios::engine::ApplicationReport,
) -> PyResult<()> {
    dict.set_item("operations_applied", report.operations_applied)?;
    dict.set_item("user_operations", report.user_operations)?;
    dict.set_item("expanded_operations", report.expanded_operations)?;
    dict.set_item("rounding_context", report.rounding_context.as_deref())?;
    if let Some(time_roll) = &report.time_roll {
        let time_roll_json = serde_json::to_string(time_roll).map_err(display_to_py)?;
        dict.set_item("time_roll_json", time_roll_json)?;
    }
    set_warning_items(dict, &report.warnings)
}

fn apply_with_context(
    spec: &finstack_quant_scenarios::ScenarioSpec,
    market: &mut finstack_quant_core::market_data::context::MarketContext,
    model: Option<&mut finstack_quant_statements::FinancialModelSpec>,
    as_of: time::Date,
) -> finstack_quant_scenarios::Result<finstack_quant_scenarios::engine::ApplicationReport> {
    let engine = finstack_quant_scenarios::ScenarioEngine::new();
    let mut ctx = finstack_quant_scenarios::ExecutionContext {
        market,
        model,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };
    engine.apply(spec, &mut ctx)
}

/// Apply a scenario to a market context and financial model.
///
/// Parameters
/// ----------
/// scenario_json : str
///     JSON-serialized ``ScenarioSpec``.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// as_of : str
///     Valuation date in ISO 8601 format.
///
/// Returns
/// -------
/// dict
///     Dict with ``market_json`` (modified market), ``model_json`` (modified
///     model), ``operations_applied`` (int), ``user_operations`` (int, count of
///     user-provided operations before hierarchy expansion), ``expanded_operations``
///     (int, count after expansion), ``rounding_context`` (str | None, active
///     rounding-mode stamp), ``time_roll_json`` (str, JSON ``RollForwardReport``;
///     only present when the scenario contained a ``time_roll_forward``
///     operation), and ``warnings`` (list[str]).
///
/// Notes
/// -----
/// This entry point supplies no instrument portfolio and no holiday calendar
/// to the engine, so instrument-scoped operations
/// (``instrument_price_pct_by_*``, ``instrument_spread_bp_by_*``,
/// ``asset_correlation_pts``, ``prepay_default_correlation_pts``) are inert
/// and produce a warning, and ``time_roll_forward`` in ``business_days`` mode
/// adjusts without holiday information.
#[pyfunction]
fn apply_scenario<'py>(
    py: Python<'py>,
    scenario_json: &str,
    market: &Bound<'py, PyAny>,
    model: &Bound<'py, PyAny>,
    as_of: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let spec: finstack_quant_scenarios::ScenarioSpec =
        serde_json::from_str(scenario_json).map_err(display_to_py)?;
    let mut market = extract_market(market)?;
    let mut model = extract_model(model)?;
    let date = super::parse_date(as_of)?;

    // Release the GIL for scenario application: shifts + re-pricing can run for seconds.
    let (report, market, model) = py.detach(|| {
        let report = apply_with_context(&spec, &mut market, Some(&mut model), date);
        (report, market, model)
    });
    let report = report.map_err(display_to_py)?;

    let dict = PyDict::new(py);
    dict.set_item(
        "market_json",
        serde_json::to_string(&market).map_err(display_to_py)?,
    )?;
    dict.set_item(
        "model_json",
        serde_json::to_string(&model).map_err(display_to_py)?,
    )?;
    set_report_items(&dict, &report)?;

    Ok(dict)
}

/// Apply a scenario to a market context only (no model).
///
/// Parameters
/// ----------
/// scenario_json : str
///     JSON-serialized ``ScenarioSpec``.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// as_of : str
///     Valuation date in ISO 8601 format.
///
/// Returns
/// -------
/// dict
///     Dict with ``market_json`` (modified market), ``operations_applied``,
///     ``user_operations``, ``expanded_operations``, ``rounding_context``,
///     ``time_roll_json`` (only when a ``time_roll_forward`` operation ran),
///     and ``warnings``.
///
/// Notes
/// -----
/// As with ``apply_scenario``, no instrument portfolio or holiday calendar is
/// supplied: instrument-scoped operations are inert (with a warning) and
/// business-day time rolls adjust without holiday information.
#[pyfunction]
fn apply_scenario_to_market<'py>(
    py: Python<'py>,
    scenario_json: &str,
    market: &Bound<'py, PyAny>,
    as_of: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let spec: finstack_quant_scenarios::ScenarioSpec =
        serde_json::from_str(scenario_json).map_err(display_to_py)?;
    let mut market = extract_market(market)?;
    let date = super::parse_date(as_of)?;

    let (report, market) = py.detach(|| {
        let report = apply_with_context(&spec, &mut market, None, date);
        (report, market)
    });
    let report = report.map_err(display_to_py)?;

    let dict = PyDict::new(py);
    dict.set_item(
        "market_json",
        serde_json::to_string(&market).map_err(display_to_py)?,
    )?;
    set_report_items(&dict, &report)?;

    Ok(dict)
}

/// Register engine functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(apply_scenario, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(apply_scenario_to_market, m)?)?;
    Ok(())
}
