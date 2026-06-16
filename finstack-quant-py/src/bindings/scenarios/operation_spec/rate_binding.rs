//! Rate-binding spec wrapper.

use crate::errors::display_to_py;
use finstack_quant_core::types::CurveId;
use finstack_quant_scenarios::spec::RateBindingSpec;
use finstack_quant_statements::types::NodeId;
use pyo3::prelude::*;
use pyo3::types::PyType;

use super::kinds::PyCompounding;

// ---------------------------------------------------------------------------
// RateBindingSpec
// ---------------------------------------------------------------------------

/// Configuration linking a statement rate node to a market curve.
///
/// Mirrors [`finstack_quant_scenarios::spec::RateBindingSpec`].
#[pyclass(
    name = "RateBindingSpec",
    module = "finstack_quant.scenarios",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyRateBindingSpec {
    pub(crate) inner: RateBindingSpec,
}

#[pymethods]
impl PyRateBindingSpec {
    /// Construct a rate-binding specification.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Statement node identifier to receive the extracted rate.
    /// curve_id : str
    ///     Market curve identifier.
    /// tenor : str
    ///     Tenor at which to sample the curve (e.g. ``"1Y"``).
    /// compounding : Compounding, optional
    ///     Output compounding convention. Defaults to ``Compounding.continuous()``.
    /// day_count : str, optional
    ///     Day-count override (e.g. ``"act/360"``). ``None`` uses the curve's
    ///     native day count.
    #[new]
    #[pyo3(signature = (node_id, curve_id, tenor, compounding=None, day_count=None))]
    fn new(
        node_id: &str,
        curve_id: &str,
        tenor: &str,
        compounding: Option<PyRef<'_, PyCompounding>>,
        day_count: Option<String>,
    ) -> Self {
        let compounding = compounding.map(|c| c.inner).unwrap_or_default();
        Self {
            inner: RateBindingSpec {
                node_id: NodeId::from(node_id),
                curve_id: CurveId::from(curve_id),
                tenor: tenor.to_string(),
                compounding,
                day_count,
            },
        }
    }

    #[getter]
    fn node_id(&self) -> String {
        self.inner.node_id.as_str().to_string()
    }

    #[getter]
    fn curve_id(&self) -> String {
        self.inner.curve_id.as_str().to_string()
    }

    #[getter]
    fn tenor(&self) -> String {
        self.inner.tenor.clone()
    }

    #[getter]
    fn compounding(&self) -> PyCompounding {
        PyCompounding {
            inner: self.inner.compounding,
        }
    }

    #[getter]
    fn day_count(&self) -> Option<String> {
        self.inner.day_count.clone()
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize a `RateBindingSpec` from JSON.
    #[classmethod]
    fn from_json(_cls: &Bound<'_, PyType>, json: &str) -> PyResult<Self> {
        let inner: RateBindingSpec = serde_json::from_str(json).map_err(|e| {
            crate::errors::value_error(format!("Invalid RateBindingSpec JSON: {e}"))
        })?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "RateBindingSpec(node_id='{}', curve_id='{}', tenor='{}')",
            self.inner.node_id.as_str(),
            self.inner.curve_id.as_str(),
            self.inner.tenor,
        )
    }
}
