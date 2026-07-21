//! Structured-credit tranche analytics: DM, break-even CDR, OAS, metrics,
//! scenario table.
//!
//! These take a tranche id, so they are not reachable through
//! `price_instrument_with_metrics`. Signatures mirror the WASM facade;
//! `OasResult`, `TrancheMetrics`, and `ScenarioTable` are returned as JSON.

use crate::bindings::extract::extract_market;
use crate::errors::display_to_py;
use pyo3::prelude::*;
use pyo3::types::PyModule;

/// Solve a z-spread-equivalent discount margin for a floating-rate tranche.
///
/// Contractual cashflows are projected without changing coupon projection, then
/// a constant additive spread is applied to the discount curve. The result is
/// zero at model PV, negative for a richer (higher) target PV, and positive for
/// a cheaper (lower) target PV; it is not the contractual quoted margin.
///
/// Parameters
/// ----------
/// instrument_json : str
///     Tagged JSON for a ``StructuredCredit`` deal.
/// tranche_id : str
///     Identifier of the floating-rate tranche whose contractual cashflows are
///     spread-discounted.
/// market : MarketContext
///     Market context supplying the discount curve and any forward curves or
///     historical fixings required for cashflow projection.
/// as_of : str
///     Valuation date used for projection and discounting, ``YYYY-MM-DD``.
/// target_pv : float
///     Target present value in the tranche's currency. Values above model PV
///     produce a negative result; values below model PV produce a positive
///     result.
///
/// Returns
/// -------
/// float
///     Z-spread-equivalent discount margin in decimal (``0.015`` = 150 bp).
///
/// Raises
/// ------
/// ValueError
///     If the JSON or date is malformed, the deal is invalid, the tranche is
///     missing or fixed-rate, ``target_pv`` is not finite, required market data
///     is unavailable, or the spread solve fails or exceeds ±5000 bp.
#[pyfunction]
#[pyo3(
    name = "structured_credit_tranche_discount_margin",
    text_signature = "(instrument_json, tranche_id, market, as_of, target_pv)"
)]
fn structured_credit_tranche_discount_margin(
    py: Python<'_>,
    instrument_json: &str,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    target_pv: f64,
) -> PyResult<f64> {
    let market = extract_market(py, market)?;
    let instrument_json = instrument_json.to_owned();
    let tranche_id = tranche_id.to_owned();
    let as_of = as_of.to_owned();
    py.detach(move || {
        finstack_quant_valuations::pricer::structured_credit_tranche_discount_margin_json(
            &instrument_json,
            &tranche_id,
            &market,
            &as_of,
            target_pv,
        )
        .map_err(display_to_py)
    })
}

/// Solve the constant default rate at which a tranche first takes a writedown.
///
/// Parameters
/// ----------
/// instrument_json : str
///     Tagged JSON for a ``StructuredCredit`` deal.
/// tranche_id : str
///     Identifier of the tranche.
/// market : MarketContext
///     Market context supplying curves and fixings.
/// as_of : str
///     Valuation date, ``YYYY-MM-DD``.
///
/// Returns
/// -------
/// float
///     Break-even annual CDR in decimal.
#[pyfunction]
#[pyo3(
    name = "structured_credit_tranche_breakeven_cdr",
    text_signature = "(instrument_json, tranche_id, market, as_of)"
)]
fn structured_credit_tranche_breakeven_cdr(
    py: Python<'_>,
    instrument_json: &str,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
) -> PyResult<f64> {
    let market = extract_market(py, market)?;
    let instrument_json = instrument_json.to_owned();
    let tranche_id = tranche_id.to_owned();
    let as_of = as_of.to_owned();
    py.detach(move || {
        finstack_quant_valuations::pricer::structured_credit_tranche_breakeven_cdr_json(
            &instrument_json,
            &tranche_id,
            &market,
            &as_of,
        )
        .map_err(display_to_py)
    })
}

/// Compute the option-adjusted spread for a tranche at a market price.
///
/// Parameters
/// ----------
/// instrument_json : str
///     Tagged JSON for a ``StructuredCredit`` deal.
/// tranche_id : str
///     Identifier of the tranche.
/// market_price_pct : float
///     Market price as a percentage of original balance (100.0 = par).
/// market : MarketContext
///     Market context supplying curves and fixings.
/// as_of : str
///     Valuation date, ``YYYY-MM-DD``.
/// config_json : str, optional
///     Serialized ``OasConfig``. All fields are currently required when
///     supplied.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``OasResult``.
#[pyfunction]
#[pyo3(
    name = "structured_credit_tranche_oas",
    signature = (instrument_json, tranche_id, market_price_pct, market, as_of, config_json=None),
    text_signature = "(instrument_json, tranche_id, market_price_pct, market, as_of, config_json=None)"
)]
fn structured_credit_tranche_oas(
    py: Python<'_>,
    instrument_json: &str,
    tranche_id: &str,
    market_price_pct: f64,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    config_json: Option<&str>,
) -> PyResult<String> {
    let market = extract_market(py, market)?;
    let instrument_json = instrument_json.to_owned();
    let tranche_id = tranche_id.to_owned();
    let as_of = as_of.to_owned();
    let config_json = config_json.map(str::to_owned);
    py.detach(move || {
        let result = finstack_quant_valuations::pricer::structured_credit_tranche_oas_json(
            &instrument_json,
            &tranche_id,
            market_price_pct,
            &market,
            &as_of,
            config_json.as_deref(),
        )
        .map_err(display_to_py)?;
        serde_json::to_string(&result).map_err(display_to_py)
    })
}

/// Compute the summary risk/pricing metrics for a tranche.
///
/// Parameters
/// ----------
/// instrument_json : str
///     Tagged JSON for a ``StructuredCredit`` deal.
/// tranche_id : str
///     Identifier of the tranche.
/// market : MarketContext
///     Market context supplying curves and fixings.
/// as_of : str
///     Valuation date, ``YYYY-MM-DD``.
/// market_price_pct : float, optional
///     Market price as a percentage of original balance; when omitted the
///     model price is used.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``TrancheMetrics``.
#[pyfunction]
#[pyo3(
    name = "structured_credit_tranche_metrics",
    signature = (instrument_json, tranche_id, market, as_of, market_price_pct=None),
    text_signature = "(instrument_json, tranche_id, market, as_of, market_price_pct=None)"
)]
fn structured_credit_tranche_metrics(
    py: Python<'_>,
    instrument_json: &str,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    market_price_pct: Option<f64>,
) -> PyResult<String> {
    let market = extract_market(py, market)?;
    let instrument_json = instrument_json.to_owned();
    let tranche_id = tranche_id.to_owned();
    let as_of = as_of.to_owned();
    py.detach(move || {
        let result = finstack_quant_valuations::pricer::structured_credit_tranche_metrics_json(
            &instrument_json,
            &tranche_id,
            &market,
            &as_of,
            market_price_pct,
        )
        .map_err(display_to_py)?;
        serde_json::to_string(&result).map_err(display_to_py)
    })
}

/// Price a tranche across a CPR x CDR x severity scenario grid.
///
/// Parameters
/// ----------
/// instrument_json : str
///     Tagged JSON for a ``StructuredCredit`` deal.
/// tranche_id : str
///     Identifier of the tranche.
/// market : MarketContext
///     Market context supplying curves and fixings.
/// as_of : str
///     Valuation date, ``YYYY-MM-DD``.
/// grid_json : str
///     Serialized ``ScenarioGrid``. The grid is capped at 10,000 cells because
///     each cell reprices the entire deal.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``ScenarioTable``.
#[pyfunction]
#[pyo3(
    name = "structured_credit_tranche_scenario_table",
    text_signature = "(instrument_json, tranche_id, market, as_of, grid_json)"
)]
fn structured_credit_tranche_scenario_table(
    py: Python<'_>,
    instrument_json: &str,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    grid_json: &str,
) -> PyResult<String> {
    let market = extract_market(py, market)?;
    let instrument_json = instrument_json.to_owned();
    let tranche_id = tranche_id.to_owned();
    let as_of = as_of.to_owned();
    let grid_json = grid_json.to_owned();
    py.detach(move || {
        let result =
            finstack_quant_valuations::pricer::structured_credit_tranche_scenario_table_json(
                &instrument_json,
                &tranche_id,
                &market,
                &as_of,
                &grid_json,
            )
            .map_err(display_to_py)?;
        serde_json::to_string(&result).map_err(display_to_py)
    })
}

/// Names registered by this module, in the order they appear in `__all__`.
pub(crate) const EXPORTS: &[&str] = &[
    "structured_credit_tranche_breakeven_cdr",
    "structured_credit_tranche_discount_margin",
    "structured_credit_tranche_metrics",
    "structured_credit_tranche_oas",
    "structured_credit_tranche_scenario_table",
];

/// Register the structured-credit tranche analytics on the instruments module.
pub(crate) fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    parent.add_function(wrap_pyfunction!(
        structured_credit_tranche_discount_margin,
        parent
    )?)?;
    parent.add_function(wrap_pyfunction!(
        structured_credit_tranche_breakeven_cdr,
        parent
    )?)?;
    parent.add_function(wrap_pyfunction!(structured_credit_tranche_oas, parent)?)?;
    parent.add_function(wrap_pyfunction!(structured_credit_tranche_metrics, parent)?)?;
    parent.add_function(wrap_pyfunction!(
        structured_credit_tranche_scenario_table,
        parent
    )?)?;
    for &name in EXPORTS {
        parent
            .getattr(name)?
            .setattr("__module__", "finstack_quant.valuations.instruments")?;
    }
    Ok(())
}
