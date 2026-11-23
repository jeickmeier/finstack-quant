//! Canonical instrument JSON validation and envelope helpers.

use pyo3::prelude::*;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._json_validation", "validate_instrument_json")]
#[pyfunction]
#[pyo3(signature = (json, metric_pricing_overrides=None))]
#[pyo3(text_signature = "(json, metric_pricing_overrides=None)")]
pub(super) fn validate_instrument_json(
    py: Python<'_>,
    json: &Bound<'_, PyAny>,
    metric_pricing_overrides: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let json = crate::bindings::extract::envelope_json_text(json)?;
    let overrides = super::pricing::metric_pricing_overrides_json(py, metric_pricing_overrides)?;
    finstack_quant_valuations::pricer::validate_instrument_json(&json, overrides.as_deref())
        .map_err(crate::errors::core_to_py)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._json_validation", "validate_typed_instrument_json")]
#[pyfunction]
#[pyo3(text_signature = "(type_tag, json)")]
pub(super) fn validate_typed_instrument_json(
    type_tag: &str,
    json: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let json = crate::bindings::extract::envelope_json_text(json)?;
    finstack_quant_valuations::pricer::validate_typed_instrument_json(type_tag, &json)
        .map_err(crate::errors::core_to_py)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._json_validation", "instrument_envelope_from_spec")]
#[pyfunction]
#[pyo3(text_signature = "(type_tag, spec)")]
pub(super) fn instrument_envelope_from_spec<'py>(
    py: Python<'py>,
    type_tag: &str,
    spec: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let spec = crate::bindings::module_utils::py_to_json_value(py, spec, "spec")?;
    let envelope = finstack_quant_valuations::pricer::instrument_envelope_from_spec(type_tag, spec)
        .map_err(crate::errors::core_to_py)?;
    crate::bindings::json_bridge::serde_to_py(py, &envelope)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._json_validation", "pretty_instrument_json")]
#[pyfunction]
#[pyo3(text_signature = "(json)")]
pub(super) fn pretty_instrument_json(json: &Bound<'_, PyAny>) -> PyResult<String> {
    let json = crate::bindings::extract::envelope_json_text(json)?;
    finstack_quant_valuations::pricer::pretty_instrument_json(&json)
        .map_err(crate::errors::core_to_py)
}
