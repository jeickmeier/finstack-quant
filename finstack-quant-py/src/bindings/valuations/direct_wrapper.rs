//! Shared helpers for direct Python instrument wrappers.

use crate::bindings::extract::extract_market;
use crate::bindings::module_utils::py_to_json_value;
use crate::errors::display_to_py;
use finstack_quant_valuations::pricer::{
    canonical_instrument_json, canonical_instrument_json_from_str,
    metric_value_from_instrument_json, present_standard_option_greeks_from_instrument_json,
    pretty_instrument_json, price_instrument_json, price_instrument_json_with_metrics_and_history,
    validate_instrument_json,
};
use finstack_quant_valuations::results::ValuationResult;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};

pub(super) fn build_from_py(
    py: Python<'_>,
    type_tag: &str,
    spec: Option<&Bound<'_, PyAny>>,
    kwargs: Option<&Bound<'_, PyDict>>,
    spec_label: &str,
    constructor_error: &str,
) -> PyResult<String> {
    if spec.is_some() && kwargs.is_some_and(|d| !d.is_empty()) {
        return Err(crate::errors::value_error(
            "pass either a spec object/JSON or keyword fields, not both",
        ));
    }

    let value = if let Some(spec) = spec {
        py_to_json_value(py, spec, spec_label)?
    } else if let Some(kwargs) = kwargs {
        py_to_json_value(py, kwargs.as_any(), spec_label)?
    } else {
        return Err(crate::errors::value_error(constructor_error.to_string()));
    };
    canonical_instrument_json(type_tag, value).map_err(display_to_py)
}

pub(super) fn from_json_payload(type_tag: &str, json: &str) -> PyResult<String> {
    canonical_instrument_json_from_str(type_tag, json).map_err(display_to_py)
}

pub(super) fn pretty_json(json: &str) -> PyResult<String> {
    pretty_instrument_json(json).map_err(display_to_py)
}

pub(super) fn validate_payload(json: &str) -> PyResult<()> {
    validate_instrument_json(json)
        .map(|_| ())
        .map_err(display_to_py)
}

pub(super) fn price_payload(
    py: Python<'_>,
    json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
) -> PyResult<String> {
    let result = price_payload_result(py, json, market, as_of, model)?;
    serde_json::to_string(&result).map_err(display_to_py)
}

pub(super) fn price_payload_result(
    py: Python<'_>,
    json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
) -> PyResult<ValuationResult> {
    let market = extract_market(market)?;
    let json = json.to_owned();
    let as_of = as_of.to_owned();
    let model = model.to_owned();
    py.detach(move || price_instrument_json(&json, &market, &as_of, &model).map_err(display_to_py))
}

// PyO3 binding helper: the argument list mirrors the Python keyword-argument
// API, so it cannot be collapsed into a parameter struct without changing it.
#[allow(clippy::too_many_arguments)]
pub(super) fn price_payload_with_metrics(
    py: Python<'_>,
    json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
    metrics: Vec<String>,
    pricing_options: Option<&str>,
    market_history: Option<&str>,
) -> PyResult<String> {
    let market = extract_market(market)?;
    let json = json.to_owned();
    let as_of = as_of.to_owned();
    let model = model.to_owned();
    let pricing_options = pricing_options.map(str::to_owned);
    let market_history = market_history.map(str::to_owned);
    py.detach(move || {
        let result = price_instrument_json_with_metrics_and_history(
            &json,
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

pub(super) fn metric_value(
    py: Python<'_>,
    json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
    metric: &str,
) -> PyResult<f64> {
    let market = extract_market(market)?;
    let json = json.to_owned();
    let as_of = as_of.to_owned();
    let model = model.to_owned();
    let metric = metric.to_owned();
    py.detach(move || {
        metric_value_from_instrument_json(&json, &market, &as_of, &model, &metric)
            .map_err(display_to_py)
    })
}

pub(super) fn greeks_dict<'py>(
    py: Python<'py>,
    json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let market = extract_market(market)?;
    let json = json.to_owned();
    let as_of = as_of.to_owned();
    let model = model.to_owned();
    let pairs = py.detach(move || {
        present_standard_option_greeks_from_instrument_json(&json, &market, &as_of, &model)
            .map_err(display_to_py)
    })?;
    let out = PyDict::new(py);
    for (metric, value) in pairs {
        out.set_item(metric, value)?;
    }
    Ok(out)
}
