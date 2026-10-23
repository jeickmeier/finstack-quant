//! Python bindings for the `finstack-quant-valuations` crate.
//!
//! Exposes the [`PyValuationResult`] envelope for pricing output,
//! JSON-based instrument loading and the standard pricer pipeline.

pub(crate) mod composite;
pub(crate) mod convert;
pub(crate) mod instruments;
pub(crate) mod market;
mod merton_mc;
mod pricing;
mod schema;
pub(crate) mod typed_asset_backed_facility;
pub(crate) mod typed_convertible;
pub(crate) mod typed_credit;
pub(crate) mod typed_equity;
pub(crate) mod typed_fx;
mod typed_legs;
pub(crate) mod typed_macros;
pub(crate) mod typed_rates;
pub(crate) mod typed_revolving_credit;
pub(crate) mod typed_structured_credit;
mod var;

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    serde_to_py,
};
use crate::bindings::pydoc::pydoc;
use crate::errors::display_to_py;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult")]
#[pyclass(
    name = "ValuationResult",
    module = "finstack_quant.valuations",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyValuationResult {
    pub(crate) inner: finstack_quant_valuations::results::ValuationResult,
}

impl PyValuationResult {
    fn long_rows_dataframe<'py>(
        py: Python<'py>,
        rows: &[finstack_quant_valuations::results::ValuationLongRow],
    ) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(
            py,
            rows,
            &[
                ("metric", "str"),
                ("curve", "str"),
                ("bucket", "str"),
                ("value", "float64"),
            ],
        )
    }
}

#[pymethods]
impl PyValuationResult {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_valuations::results::ValuationResult = serde_json::from_str(json)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid ValuationResult JSON"))?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.as_of")]
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.as_of)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.metrics")]
    #[getter]
    fn metrics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.measures {
            dict.set_item(key.as_str(), *value)?;
        }
        Ok(dict)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.get_metric")]
    #[pyo3(text_signature = "($self, key)")]
    fn get_metric(&self, key: &str) -> Option<f64> {
        self.inner.metric_str(key)
    }

    /// ``result[key]``: scalar measure by key.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``key`` is not a measure of this result; the message lists the
    ///     five closest keys present.
    fn __getitem__(&self, key: &str) -> PyResult<f64> {
        self.inner.metric_str(key).ok_or_else(|| {
            let closest = self.inner.closest_metric_keys(key, 5);
            let hint = if closest.is_empty() {
                String::new()
            } else {
                format!("; closest keys: {}", closest.join(", "))
            };
            pyo3::exceptions::PyKeyError::new_err(format!(
                "metric '{key}' is not on this result ({} measures){hint}",
                self.inner.measures.len()
            ))
        })
    }

    /// ``key in result``: whether a measure with this key (canonical wire form) is present.
    fn __contains__(&self, key: &str) -> bool {
        self.inner.metric_str(key).is_some()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.metric_series")]
    #[pyo3(text_signature = "($self, base)")]
    fn metric_series(&self, base: &str) -> PyResult<Vec<(Vec<String>, f64)>> {
        let base = base.parse().map_err(crate::errors::core_to_py)?;
        Ok(self.inner.metric_series(&base))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.metric_series_dataframe")]
    #[pyo3(text_signature = "($self, base)")]
    fn metric_series_dataframe<'py>(
        &self,
        py: Python<'py>,
        base: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::long_rows_dataframe(py, &self.inner.to_series_rows(base))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.to_long_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_long_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Self::long_rows_dataframe(py, &self.inner.to_long_rows())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.metric_units")]
    #[pyo3(text_signature = "($self)")]
    fn metric_units<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (key, unit) in self.inner.metric_units() {
            dict.set_item(key, unit.as_str())?;
        }
        Ok(dict)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.all_covenants_passed")]
    #[pyo3(text_signature = "($self)")]
    fn all_covenants_passed(&self) -> bool {
        self.inner.all_covenants_passed()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.failed_covenants")]
    #[pyo3(text_signature = "($self)")]
    fn failed_covenants(&self) -> Vec<String> {
        self.inner
            .failed_covenants()
            .into_iter()
            .map(String::from)
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.covenants")]
    #[getter]
    fn covenants<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .covenants
            .as_ref()
            .map(|covenants| serde_to_py(py, covenants))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.provenance")]
    #[getter]
    fn provenance<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .provenance
            .as_ref()
            .map(|provenance| serde_to_py(py, provenance))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.explanation")]
    #[getter]
    fn explanation<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .explanation
            .as_ref()
            .map(|trace| serde_to_py(py, trace))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = self.inner.to_row();
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &["instrument_id", "as_of_date", "pv", "currency"],
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult._repr_html_")]
    fn _repr_html_<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_dataframe(py)?.call_method0("_repr_html_")
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.meta")]
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.details")]
    #[getter]
    fn details<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .details
            .as_ref()
            .map(|details| serde_to_py(py, details))
            .transpose()
    }

    /// Structural equality: two results are equal when their JSON documents match.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.cast::<Self>() else {
            return Ok(false);
        };
        let lhs = serde_json::to_value(&self.inner).map_err(display_to_py)?;
        let rhs = serde_json::to_value(&other.borrow().inner).map_err(display_to_py)?;
        Ok(lhs == rhs)
    }

    fn __repr__(&self) -> String {
        format!(
            "ValuationResult(id={:?}, price={:.4}, currency={}, metrics={})",
            self.inner.instrument_id,
            self.inner.value.amount(),
            self.inner.value.currency(),
            self.inner.measures.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyValuationResult |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.instrument_id")]
    instrument_id -> &str = &s.inner.instrument_id;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.schema_version")]
    schema_version -> u32 = s.inner.schema_version.into();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.price")]
    get_price -> f64 = s.inner.value.amount();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.value")]
    value -> crate::bindings::core::money::PyMoney = convert::money_to_py(s.inner.value);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations", "ValuationResult.currency")]
    currency -> String = s.inner.value.currency().to_string();
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "validate_instrument_json")]
#[pyfunction]
#[pyo3(signature = (json, metric_pricing_overrides=None))]
#[pyo3(text_signature = "(json, metric_pricing_overrides=None)")]
fn validate_instrument_json(
    py: Python<'_>,
    json: &Bound<'_, PyAny>,
    metric_pricing_overrides: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let json = crate::bindings::extract::envelope_json_text(json)?;
    let overrides = pricing::metric_pricing_overrides_json(py, metric_pricing_overrides)?;
    finstack_quant_valuations::pricer::validate_instrument_json(&json, overrides.as_deref())
        .map_err(crate::errors::core_to_py)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "validate_typed_instrument_json")]
#[pyfunction]
#[pyo3(text_signature = "(type_tag, json)")]
fn validate_typed_instrument_json(type_tag: &str, json: &Bound<'_, PyAny>) -> PyResult<String> {
    let json = crate::bindings::extract::envelope_json_text(json)?;
    finstack_quant_valuations::pricer::validate_typed_instrument_json(type_tag, &json)
        .map_err(crate::errors::core_to_py)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "instrument_envelope_from_spec")]
#[pyfunction]
#[pyo3(text_signature = "(type_tag, spec)")]
fn instrument_envelope_from_spec<'py>(
    py: Python<'py>,
    type_tag: &str,
    spec: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let spec = crate::bindings::module_utils::py_to_json_value(py, spec, "spec")?;
    let envelope = finstack_quant_valuations::pricer::instrument_envelope_from_spec(type_tag, spec)
        .map_err(crate::errors::core_to_py)?;
    crate::bindings::pandas_utils::serde_to_py(py, &envelope)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "pretty_instrument_json")]
#[pyfunction]
#[pyo3(text_signature = "(json)")]
fn pretty_instrument_json(json: &Bound<'_, PyAny>) -> PyResult<String> {
    let json = crate::bindings::extract::envelope_json_text(json)?;
    finstack_quant_valuations::pricer::pretty_instrument_json(&json)
        .map_err(crate::errors::core_to_py)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "bond_from_cashflows_json")]
#[pyfunction]
#[pyo3(
    signature = (instrument_id, schedule_json, discount_curve_id, quoted_clean_price_pct = None),
    text_signature = "(instrument_id, schedule_json, discount_curve_id, quoted_clean_price_pct=None)"
)]
fn bond_from_cashflows_json(
    py: Python<'_>,
    instrument_id: &str,
    schedule_json: &str,
    discount_curve_id: &str,
    quoted_clean_price_pct: Option<f64>,
) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_valuations::instruments::fixed_income::bond_from_cashflows_json(
            instrument_id,
            schedule_json,
            discount_curve_id,
            quoted_clean_price_pct,
        )
        .map_err(crate::errors::core_to_py)
    })
}

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "valuations")?;
    m.setattr("__doc__", pydoc!("finstack_quant.valuations", "__module__"))?;

    m.add_class::<PyValuationResult>()?;
    composite::register(py, &m)?;
    schema::register(py, &m)?;
    register_instruments(py, &m)?;
    register_market(py, &m)?;

    let all = PyList::new(
        py,
        [
            "ValuationResult",
            "composite",
            "instruments",
            "market",
            "schema",
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

fn register_instruments(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "instruments")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.valuations.instruments", "__module__"),
    )?;

    m.add_function(wrap_pyfunction!(validate_instrument_json, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_typed_instrument_json, &m)?)?;
    m.add_function(wrap_pyfunction!(instrument_envelope_from_spec, &m)?)?;
    m.add_function(wrap_pyfunction!(pretty_instrument_json, &m)?)?;
    m.add_function(wrap_pyfunction!(bond_from_cashflows_json, &m)?)?;
    instruments::register(py, &m)?;
    merton_mc::register(py, &m)?;
    typed_legs::register(py, &m)?;
    typed_rates::register(py, &m)?;
    typed_credit::register(py, &m)?;
    typed_convertible::register(py, &m)?;
    typed_equity::register(py, &m)?;
    typed_fx::register(py, &m)?;
    typed_structured_credit::register(py, &m)?;
    typed_revolving_credit::register(py, &m)?;
    typed_asset_backed_facility::register(py, &m)?;
    pricing::register(py, &m)?;
    var::register(&m)?;
    typed_structured_credit::tranche_analytics::register(&m)?;
    let mut exports = vec![
        "validate_instrument_json",
        "validate_typed_instrument_json",
        "instrument_envelope_from_spec",
        "pretty_instrument_json",
        "bond_from_cashflows_json",
    ];
    exports.extend_from_slice(merton_mc::EXPORTS);
    exports.extend_from_slice(typed_structured_credit::tranche_analytics::EXPORTS);
    exports.extend_from_slice(instruments::EXPORTS);
    exports.extend_from_slice(typed_legs::EXPORTS);
    exports.extend_from_slice(typed_rates::EXPORTS);
    exports.extend_from_slice(typed_credit::EXPORTS);
    exports.extend_from_slice(typed_convertible::EXPORTS);
    exports.extend_from_slice(typed_equity::EXPORTS);
    exports.extend_from_slice(typed_fx::EXPORTS);
    exports.extend_from_slice(typed_structured_credit::EXPORTS);
    exports.extend_from_slice(typed_revolving_credit::EXPORTS);
    exports.extend_from_slice(typed_asset_backed_facility::EXPORTS);
    exports.extend_from_slice(pricing::EXPORTS);
    exports.extend_from_slice(var::EXPORTS);
    exports.sort_unstable();
    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;
    Ok(())
}

fn register_market(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "market")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.valuations.market", "__module__"),
    )?;

    market::register(py, &m)?;
    let mut exports = market::EXPORTS.to_vec();
    exports.sort_unstable();
    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;
    Ok(())
}
