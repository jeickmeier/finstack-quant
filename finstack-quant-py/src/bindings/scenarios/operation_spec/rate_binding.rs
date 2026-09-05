//! Rate-binding spec wrapper.

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::errors::{display_to_py, scenarios_to_py};
use finstack_quant_core::types::CurveId;
use finstack_quant_scenarios::spec::RateBindingSpec;
use finstack_quant_statements::types::NodeId;
use pyo3::prelude::*;

use super::helpers::extract_compounding;
use super::kinds::PyCompounding;

/// Configuration linking a statement rate node to a market curve.
///
/// Parameters
/// ----------
/// node_id : str
///     Statement node identifier to receive the extracted rate.
/// curve_id : str
///     Market curve identifier.
/// tenor : str
///     Tenor at which to sample the curve (e.g. ``"1Y"``).
/// compounding : Compounding | str, optional
///     Output compounding convention (typed or wire label). Defaults to
///     ``Compounding.continuous()``. The extracted rate is a decimal
///     annualized rate; only the compounding basis changes.
/// day_count : DayCount, optional
///     Output quote day count. ``None`` uses the curve's native day count.
///     Maturity dates and curve-implied accumulation factors are preserved.
///
/// Raises
/// ------
/// ValueError
///     If ``compounding`` is not an accepted label.
///
/// Examples
/// --------
/// >>> from finstack_quant.scenarios import RateBindingSpec
/// >>> binding = RateBindingSpec("interest_rate", "USD-OIS", "1Y", compounding="annual")
/// >>> binding.validate() is None
/// True
/// >>> binding == RateBindingSpec.from_json(binding.to_json())
/// True
#[pyclass(
    name = "RateBindingSpec",
    module = "finstack_quant.scenarios",
    eq,
    frozen,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyRateBindingSpec {
    pub(crate) inner: RateBindingSpec,
}

#[pymethods]
impl PyRateBindingSpec {
    #[new]
    #[pyo3(signature = (node_id, curve_id, tenor, compounding=None, day_count=None))]
    fn new(
        node_id: &str,
        curve_id: &str,
        tenor: &str,
        compounding: Option<&Bound<'_, PyAny>>,
        day_count: Option<PyRef<'_, PyDayCount>>,
    ) -> PyResult<Self> {
        let compounding = compounding
            .map(extract_compounding)
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            inner: RateBindingSpec {
                node_id: NodeId::from(node_id),
                curve_id: CurveId::from(curve_id),
                tenor: tenor.to_string(),
                compounding,
                day_count: day_count.map(|value| value.inner),
            },
        })
    }

    /// Statement node identifier receiving the extracted rate.
    #[getter]
    fn node_id(&self) -> String {
        self.inner.node_id.as_str().to_string()
    }

    /// Market curve identifier sampled for the rate.
    #[getter]
    fn curve_id(&self) -> String {
        self.inner.curve_id.as_str().to_string()
    }

    /// Tenor string at which the curve is sampled.
    #[getter]
    fn tenor(&self) -> String {
        self.inner.tenor.clone()
    }

    /// Output compounding convention.
    #[getter]
    fn compounding(&self) -> PyCompounding {
        PyCompounding {
            inner: self.inner.compounding,
        }
    }

    /// Typed day-count override, or ``None`` for the curve's native day count.
    #[getter]
    fn day_count(&self) -> Option<PyDayCount> {
        self.inner.day_count.map(PyDayCount::from_inner)
    }

    /// Validate identifiers and eagerly parse the tenor.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``node_id`` or ``curve_id`` is blank, or ``tenor`` is not a
    ///     valid tenor string.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(scenarios_to_py)
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a `RateBindingSpec` from JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON does not match the ``RateBindingSpec`` contract.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: RateBindingSpec = serde_json::from_str(json).map_err(|e| {
            crate::errors::value_error(format!("Invalid RateBindingSpec JSON: {e}"))
        })?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "RateBindingSpec(node_id={:?}, curve_id={:?}, tenor={:?})",
            self.inner.node_id.as_str(),
            self.inner.curve_id.as_str(),
            self.inner.tenor,
        )
    }
}
