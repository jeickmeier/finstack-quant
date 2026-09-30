//! `AttributionResultEnvelope` wrapper returned by `attribute_pnl_envelope`.

use crate::bindings::attribution::pnl_attribution::PyPnlAttribution;
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::{display_to_py, serde_json_to_py};
use finstack_quant_attribution::AttributionResultEnvelope;
use pyo3::prelude::*;

/// Versioned attribution result: the ``PnlAttribution`` plus its
/// result-policy audit stamp (Rust ``AttributionResultEnvelope``).
///
/// Returned by ``attribute_pnl_envelope``; ``to_json`` is byte-identical to
/// ``attribute_pnl_envelope_json`` and to the WASM
/// ``attributePnlEnvelopeJson``.
///
/// Examples
/// --------
/// >>> from finstack_quant.attribution import AttributionResultEnvelope
/// >>> try:
/// ...     AttributionResultEnvelope.from_json("{}")
/// ... except ValueError as exc:
/// ...     "missing field" in str(exc)
/// True
#[pyclass(
    name = "AttributionResultEnvelope",
    module = "finstack_quant.attribution",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyAttributionResultEnvelope {
    pub(crate) inner: AttributionResultEnvelope,
}

#[pymethods]
impl PyAttributionResultEnvelope {
    /// The P&L attribution document.
    #[getter]
    fn attribution(&self) -> PyPnlAttribution {
        PyPnlAttribution {
            inner: self.inner.result.attribution.clone(),
        }
    }

    /// Numeric mode, rounding context and FX-policy audit stamp as a dict.
    #[getter]
    fn results_meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.result.results_meta)
    }

    /// Serialize to compact JSON (the ``attribute_pnl_envelope_json`` wire).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the JSON does not match the wire schema.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: AttributionResultEnvelope = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid AttributionResultEnvelope JSON"))?;
        Ok(Self { inner })
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "AttributionResultEnvelope(total_pnl={})",
            self.inner.result.attribution.total_pnl
        )
    }
}
