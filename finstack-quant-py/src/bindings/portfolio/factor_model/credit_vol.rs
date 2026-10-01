use pyo3::prelude::*;
use pyo3::types::PyDict;

use finstack_quant_portfolio::factor_model::{
    self as fm, CreditVolReport, LevelVolContribution, PositionVolContribution,
};

use crate::bindings::models::factor::credit::PyCreditFactorModel;
use crate::bindings::pandas_utils::dict_to_dataframe;

use super::super::json_bridge::serialize_json;
use super::contributions::PyRiskDecomposition;

/// Aggregated risk contribution for a single hierarchy level.
#[pyclass(
    name = "LevelVolContribution",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyLevelVolContribution {
    pub(crate) inner: LevelVolContribution,
}

impl PyLevelVolContribution {
    fn from_inner(inner: LevelVolContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyLevelVolContribution {
    /// Human-readable hierarchy level name, e.g. ``"Rating"``.
    #[getter]
    fn level_name(&self) -> String {
        self.inner.level_name.clone()
    }

    /// Total contribution of this level across all of its buckets, in the units
    /// of the parent report's risk measure.
    #[getter]
    fn total(&self) -> f64 {
        self.inner.total
    }

    /// Per-bucket contributions keyed by the bucket path.
    ///
    /// Keys are returned in deterministic (sorted) bucket-path order.
    #[getter]
    fn by_bucket<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        for (k, v) in &self.inner.by_bucket {
            d.set_item(k, v)?;
        }
        Ok(d)
    }

    fn __repr__(&self) -> String {
        format!(
            "LevelVolContribution(level_name={:?}, total={}, buckets={})",
            self.inner.level_name,
            self.inner.total,
            self.inner.by_bucket.len(),
        )
    }
}

/// Per-position vol breakdown under :class:`CreditVolReport`.
#[pyclass(
    name = "PositionVolContribution",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPositionVolContribution {
    pub(crate) inner: PositionVolContribution,
}

impl PyPositionVolContribution {
    fn from_inner(inner: PositionVolContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionVolContribution {
    /// Portfolio position identifier.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Factor-driven (systematic) risk for this position, in the units of the
    /// parent report's risk measure.
    #[getter]
    fn factor_total(&self) -> f64 {
        self.inner.factor_total
    }

    /// Issuer-specific (idiosyncratic) risk for this position, allocated from
    /// the report total in proportion to residual variance.
    #[getter]
    fn idiosyncratic(&self) -> f64 {
        self.inner.idiosyncratic
    }

    /// Additive total: :attr:`factor_total` plus :attr:`idiosyncratic`.
    #[getter]
    fn total(&self) -> f64 {
        self.inner.total
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionVolContribution(position_id={:?}, factor_total={}, idiosyncratic={}, total={})",
            self.inner.position_id.as_str(),
            self.inner.factor_total,
            self.inner.idiosyncratic,
            self.inner.total,
        )
    }
}

/// Aggregated vol report grouped by hierarchy level.
#[pyclass(
    name = "CreditVolReport",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyCreditVolReport {
    pub(crate) inner: CreditVolReport,
}

impl PyCreditVolReport {
    fn from_inner(inner: CreditVolReport) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCreditVolReport {
    /// Deserialize from the canonical ``CreditVolReport`` JSON emitted by
    /// :meth:`to_json` (and returned by the WASM ``buildCreditVolReport``).
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: CreditVolReport =
            serde_json::from_str(json).map_err(crate::errors::display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to canonical ``CreditVolReport`` JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Total annualized risk under the chosen measure; matches the source
    /// decomposition's ``total_risk``.
    #[getter]
    fn total(&self) -> f64 {
        self.inner.total
    }

    /// Risk measure serialized as a JSON-compatible string (e.g. ``"variance"``).
    #[getter]
    fn measure_json(&self) -> PyResult<String> {
        serialize_json(&self.inner.measure)
    }

    /// Contribution from the generic (principal-component) ``credit::generic``
    /// factor, in the units of :attr:`measure_json`.
    #[getter]
    fn generic(&self) -> f64 {
        self.inner.generic
    }

    /// Portfolio idiosyncratic risk in the units of :attr:`measure_json`.
    ///
    /// Returns
    /// -------
    /// float
    ///     An Euler-scaled residual contribution, **not** standalone
    ///     residual-only risk, so ``generic + sum(by_level) +
    ///     idiosyncratic_total`` reconciles to :attr:`total`.
    #[getter]
    fn idiosyncratic_total(&self) -> f64 {
        self.inner.idiosyncratic_total
    }

    /// Per-hierarchy-level rollup, positionally aligned to the credit model's
    /// hierarchy levels.
    #[getter]
    fn by_level(&self) -> Vec<PyLevelVolContribution> {
        self.inner
            .by_level
            .iter()
            .cloned()
            .map(PyLevelVolContribution::from_inner)
            .collect()
    }

    /// Optional per-position breakdown; ``None`` when not requested.
    #[getter]
    fn by_position(&self) -> Option<Vec<PyPositionVolContribution>> {
        self.inner.by_position_optional.as_ref().map(|rows| {
            rows.iter()
                .cloned()
                .map(PyPositionVolContribution::from_inner)
                .collect()
        })
    }

    /// Primary table: the per-hierarchy-level rollup.
    ///
    /// Alias of :meth:`to_level_dataframe`. Every tabular result type in the
    /// library answers ``to_dataframe()``; the optional per-position
    /// breakdown stays on :meth:`to_position_dataframe`.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_level_dataframe(py)
    }

    /// Export the per-hierarchy-level rollup as a pandas ``DataFrame``.
    ///
    /// One row per entry of :attr:`by_level`. The per-bucket drill-down stays
    /// on :attr:`LevelVolContribution.by_bucket`, which is already a plain dict.
    ///
    /// Columns: ``level_name``, ``total`` (level contribution in the units of
    /// :attr:`measure_json`).
    fn to_level_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // The columns are built explicitly: the frame leaves out `by_bucket`.
        let rows = &self.inner.by_level;
        let level_names: Vec<&str> = rows.iter().map(|l| l.level_name.as_str()).collect();
        let totals: Vec<f64> = rows.iter().map(|l| l.total).collect();
        let data = PyDict::new(py);
        data.set_item("level_name", level_names)?;
        data.set_item("total", totals)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Export the optional per-position breakdown as a pandas ``DataFrame``.
    ///
    /// One row per entry of :attr:`by_position`. When the report was built
    /// without ``by_position=True`` the result is a zero-row frame that still
    /// carries the full column schema, so downstream ``df[...]`` selections
    /// keep working instead of raising ``KeyError``.
    ///
    /// Columns: ``position_id``, ``factor_total`` (systematic),
    /// ``idiosyncratic`` (issuer-specific), ``total`` (their sum) — all in the
    /// units of :attr:`measure_json`.
    fn to_position_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // Built explicitly so the empty case keeps the full column schema.
        let rows: &[PositionVolContribution] = self
            .inner
            .by_position_optional
            .as_deref()
            .unwrap_or_default();
        let position_ids: Vec<&str> = rows.iter().map(|p| p.position_id.as_str()).collect();
        let factor_totals: Vec<f64> = rows.iter().map(|p| p.factor_total).collect();
        let idiosyncratic: Vec<f64> = rows.iter().map(|p| p.idiosyncratic).collect();
        let totals: Vec<f64> = rows.iter().map(|p| p.total).collect();
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("factor_total", factor_totals)?;
        data.set_item("idiosyncratic", idiosyncratic)?;
        data.set_item("total", totals)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "CreditVolReport(total={}, generic={}, idiosyncratic_total={}, by_level={})",
            self.inner.total,
            self.inner.generic,
            self.inner.idiosyncratic_total,
            self.inner.by_level.len(),
        )
    }
}

/// Build a credit volatility report from a risk decomposition and credit model.
#[pyfunction]
#[pyo3(signature = (decomposition, model, by_position = false))]
pub(super) fn build_credit_vol_report(
    py: Python<'_>,
    decomposition: &PyRiskDecomposition,
    model: &PyCreditFactorModel,
    by_position: bool,
) -> PyResult<PyCreditVolReport> {
    let decomposition = decomposition.inner.clone();
    let model = model.inner.clone();
    let report =
        py.detach(move || fm::build_credit_vol_report(&decomposition, &model, by_position));
    Ok(PyCreditVolReport::from_inner(report))
}
