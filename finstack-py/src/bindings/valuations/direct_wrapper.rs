//! Shared helpers for direct Python instrument wrappers.

use crate::bindings::extract::extract_market_ref;
use crate::errors::display_to_py;
use finstack_valuations::pricer::{
    canonical_instrument_json, canonical_instrument_json_from_str,
    metric_value_from_instrument_json, present_standard_option_greeks_from_instrument_json,
    pretty_instrument_json, price_instrument_json, price_instrument_json_with_metrics,
    validate_instrument_json,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use serde_json::Value;

fn py_to_json_value<'py>(py: Python<'py>, obj: &Bound<'py, PyAny>, label: &str) -> PyResult<Value> {
    if let Ok(json) = obj.extract::<String>() {
        return serde_json::from_str(&json)
            .map_err(|e| PyValueError::new_err(format!("invalid {label} JSON: {e}")));
    }

    let json_mod = py.import("json")?;
    let json: String = json_mod
        .call_method1("dumps", (obj,))
        .and_then(|value| value.extract())
        .map_err(|e| PyValueError::new_err(format!("invalid {label}: {e}")))?;
    serde_json::from_str(&json)
        .map_err(|e| PyValueError::new_err(format!("invalid {label} JSON: {e}")))
}

pub(super) fn build_from_py(
    py: Python<'_>,
    type_tag: &str,
    spec: Option<&Bound<'_, PyAny>>,
    kwargs: Option<&Bound<'_, PyDict>>,
    spec_label: &str,
    constructor_error: &str,
) -> PyResult<String> {
    if spec.is_some() && kwargs.is_some_and(|d| !d.is_empty()) {
        return Err(PyValueError::new_err(
            "pass either a spec object/JSON or keyword fields, not both",
        ));
    }

    let value = if let Some(spec) = spec {
        py_to_json_value(py, spec, spec_label)?
    } else if let Some(kwargs) = kwargs {
        py_to_json_value(py, kwargs.as_any(), spec_label)?
    } else {
        return Err(PyValueError::new_err(constructor_error.to_string()));
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
    json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
) -> PyResult<String> {
    let market = extract_market_ref(market)?;
    let result = price_instrument_json(json, &market, as_of, model).map_err(display_to_py)?;
    serde_json::to_string(&result).map_err(display_to_py)
}

pub(super) fn price_payload_with_metrics(
    json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
    metrics: Vec<String>,
    pricing_options: Option<&str>,
) -> PyResult<String> {
    let market = extract_market_ref(market)?;
    let result =
        price_instrument_json_with_metrics(json, &market, as_of, model, &metrics, pricing_options)
            .map_err(display_to_py)?;
    serde_json::to_string(&result).map_err(display_to_py)
}

pub(super) fn metric_value(
    json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
    metric: &str,
) -> PyResult<f64> {
    let market = extract_market_ref(market)?;
    metric_value_from_instrument_json(json, &market, as_of, model, metric).map_err(display_to_py)
}

pub(super) fn greeks_dict<'py>(
    py: Python<'py>,
    json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    model: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    let market = extract_market_ref(market)?;
    let pairs = present_standard_option_greeks_from_instrument_json(json, &market, as_of, model)
        .map_err(display_to_py)?;
    for (metric, value) in pairs {
        out.set_item(metric, value)?;
    }
    Ok(out)
}
