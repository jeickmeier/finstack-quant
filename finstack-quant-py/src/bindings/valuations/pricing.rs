//! Instrument pricing pipeline: JSON instrument + market → ValuationResult.

use crate::bindings::extract::extract_market;
use crate::errors::display_to_py;
use pyo3::prelude::*;

/// Price an instrument from its tagged JSON and return a ``ValuationResult`` JSON.
///
/// Parameters
/// ----------
/// instrument_json : str
///     Tagged instrument JSON (``{"type": "bond", ...}``).
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// as_of : str
///     Valuation date in ISO 8601 format (``"YYYY-MM-DD"``).
/// model : str
///     Model key: ``"default"`` (default), ``"discounting"``, ``"black76"``, ``"hazard_rate"``,
///     ``"hull_white_1f"``, ``"tree"``, ``"normal"``, ``"monte_carlo_gbm"``,
///     ``"bond_future_clean_price_proxy"``, etc.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``ValuationResult``.
#[pyfunction]
#[pyo3(signature = (instrument_json, market, as_of, model="default"))]
fn price_instrument(
    py: Python<'_>,
    instrument_json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
) -> PyResult<String> {
    let market = extract_market(market)?;
    let instrument_json = instrument_json.to_owned();
    let as_of = as_of.to_owned();
    let model = model.to_owned();

    py.detach(move || {
        let result = finstack_quant_valuations::pricer::price_instrument_json(
            &instrument_json,
            &market,
            &as_of,
            &model,
        )
        .map_err(display_to_py)?;
        serde_json::to_string(&result).map_err(display_to_py)
    })
}

/// Price an instrument with explicit metric requests.
///
/// Parameters
/// ----------
/// instrument_json : str
///     Tagged instrument JSON.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// as_of : str
///     Valuation date.
/// model : str
///     Model key string.
/// metrics : list[str]
///     Metric identifiers to compute (e.g. ``["ytm", "dv01", "modified_duration"]``).
/// pricing_options : str | None
///     Optional JSON string of ``MetricPricingOverrides`` merged into the instrument's
///     ``pricing_overrides`` before pricing.  Supported fields include
///     ``"theta_period"`` (e.g. ``"1D"``, ``"1W"``, ``"1M"``) and
///     ``"breakeven_config"`` (e.g. ``{"target": "z_spread", "mode": "linear"}``).
///     If omitted, the instrument's own overrides (if any) are used unchanged.
/// market_history : str | None
///     Optional JSON string of ``MarketHistory`` scenarios required by ``hvar`` and
///     ``expected_shortfall`` metrics.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``ValuationResult`` including requested metrics.
#[pyfunction]
#[pyo3(signature = (instrument_json, market, as_of, model="default", metrics=vec![], pricing_options=None, market_history=None))]
// PyO3 binding: the argument list mirrors the Python keyword-argument API, so
// it cannot be collapsed into a parameter struct without changing that API.
#[allow(clippy::too_many_arguments)]
fn price_instrument_with_metrics(
    py: Python<'_>,
    instrument_json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
    metrics: Vec<String>,
    pricing_options: Option<&str>,
    market_history: Option<&str>,
) -> PyResult<String> {
    let market = extract_market(market)?;
    let instrument_json = instrument_json.to_owned();
    let as_of = as_of.to_owned();
    let model = model.to_owned();
    let pricing_options = pricing_options.map(str::to_owned);
    let market_history = market_history.map(str::to_owned);

    py.detach(move || {
        let result =
            finstack_quant_valuations::pricer::price_instrument_json_with_metrics_and_history(
                &instrument_json,
                &market,
                &as_of,
                &model,
                &metrics,
                pricing_options.as_deref(),
                market_history.as_deref(),
            )
            .map_err(display_to_py)?;
        serde_json::to_string(&result).map_err(display_to_py)
    })
}

/// List all metric IDs in the standard metric registry.
///
/// Returns
/// -------
/// list[str]
///     All registered metric identifiers (sorted alphabetically).
#[pyfunction]
fn list_standard_metrics() -> Vec<String> {
    finstack_quant_valuations::pricer::list_standard_metrics()
}

/// List all standard metrics organized by group.
///
/// Returns a dict `{ group_name: [metric_id, ...], ... }` where each key
/// is a human-readable group name (e.g. "Pricing", "Greeks", "Sensitivity")
/// and the value is a sorted list of metric ID strings.
///
/// Returns
/// -------
/// dict[str, list[str]]
///     Metrics grouped by category.
#[pyfunction]
fn list_standard_metrics_grouped() -> std::collections::BTreeMap<String, Vec<String>> {
    finstack_quant_valuations::pricer::list_standard_metrics_grouped()
}

/// Per-flow cashflow envelope (DF / survival / PV) for a discountable instrument.
///
/// Supported ``model`` values are ``"discounting"`` (DF-only PV) and
/// ``"hazard_rate"`` (DF × survival + recovery on principal). Any other model
/// key, or an instrument type that isn't priced under the chosen model in the
/// standard registry, raises ``ValueError``. For the supported combinations,
/// the returned envelope's ``total_pv`` reconciles with the instrument's
/// ``base_value``.
///
/// Parameters
/// ----------
/// instrument_json : str
///     Tagged instrument JSON.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// as_of : str
///     Valuation date in ISO 8601 format.
/// model : str
///     ``"discounting"`` or ``"hazard_rate"``.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``InstrumentCashflowEnvelope``. Parse and wrap in a
///     DataFrame via :func:`finstack_quant.valuations.instrument_cashflows`.
#[pyfunction]
fn instrument_cashflows_json(
    py: Python<'_>,
    instrument_json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
) -> PyResult<String> {
    let market = extract_market(market)?;
    let instrument_json = instrument_json.to_owned();
    let as_of = as_of.to_owned();
    let model = model.to_owned();

    py.detach(move || {
        finstack_quant_valuations::pricer::instrument_cashflows_json(
            &instrument_json,
            &market,
            &as_of,
            &model,
        )
        .map_err(display_to_py)
    })
}

/// Register pricing functions on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(price_instrument, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(price_instrument_with_metrics, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_standard_metrics, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_standard_metrics_grouped, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(instrument_cashflows_json, m)?)?;
    Ok(())
}
