use numpy::{PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use finstack_quant_models::factor::risk::{
    parametric_es_decomposition_view, DecompositionConfig, HistoricalPositionDecomposer,
    ParametricEsDecompositionView, ParametricPositionDecomposer, PositionEsContributionView,
};

use crate::bindings::pandas_utils::{dict_to_dataframe, serde_object_to_single_row_dataframe};
use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, value_error};

use super::super::json_bridge::{deserialize_json, serialize_json};
use super::super::matrix_input::{extract_position_pnls, extract_square_matrix, PositionPnlMatrix};
use super::contributions::PyPositionRiskDecomposition;

/// `True` when `obj` is a `pandas.DataFrame` (checked by module + type name so
/// pandas is not imported when it is not already loaded).
fn is_dataframe(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let ty = obj.get_type();
    let name: String = ty.getattr("__name__")?.extract()?;
    if name != "DataFrame" {
        return Ok(false);
    }
    let module: String = ty.getattr("__module__")?.extract()?;
    Ok(module.starts_with("pandas"))
}

/// Column labels of a DataFrame as strings.
fn dataframe_columns(frame: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let columns = frame.getattr("columns")?;
    columns
        .try_iter()?
        .map(|c| c?.str()?.extract::<String>())
        .collect()
}

/// `frame.to_numpy(dtype="float64")` as a NumPy array object.
fn dataframe_to_f64_array<'py>(
    py: Python<'py>,
    frame: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let kwargs = PyDict::new(py);
    kwargs.set_item("dtype", "float64")?;
    frame
        .call_method("to_numpy", (), Some(&kwargs))
        .map_err(|_| value_error("DataFrame must be convertible to a float64 array"))
}

/// Resolve `position_ids` and the square `covariance` matrix.
///
/// `covariance` accepts a nested list, a 2-D NumPy array, or a
/// `pandas.DataFrame`; for a DataFrame `position_ids` may be `None`, in
/// which case the frame's column labels are used.
fn extract_covariance_input(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    covariance: &Bound<'_, PyAny>,
    n_weights: usize,
) -> PyResult<(Vec<String>, Vec<f64>)> {
    if is_dataframe(covariance)? {
        let columns = dataframe_columns(covariance)?;
        let ids = position_ids.unwrap_or(columns);
        let array = dataframe_to_f64_array(py, covariance)?;
        let flat = extract_square_matrix(py, &array, n_weights, "covariance")?;
        return Ok((ids, flat));
    }
    let ids = position_ids.ok_or_else(|| {
        value_error("position_ids is required unless covariance is a pandas.DataFrame")
    })?;
    let flat = extract_square_matrix(py, covariance, n_weights, "covariance")?;
    Ok((ids, flat))
}

/// Resolve `position_ids` and the position × scenario P&L matrix.
///
/// Accepted shapes for `position_pnls`:
///
/// - `pandas.DataFrame` — rows are scenarios, columns are positions
///   (`position_ids` defaults to the column labels);
/// - nested list or 2-D NumPy array shaped `n_positions × n_scenarios`
///   (position-major), the Rust `flatten_position_pnls` layout WASM shares.
pub(super) fn extract_pnl_input(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    position_pnls: &Bound<'_, PyAny>,
) -> PyResult<(Vec<String>, PositionPnlMatrix)> {
    if is_dataframe(position_pnls)? {
        let columns = dataframe_columns(position_pnls)?;
        let ids = position_ids.unwrap_or(columns);
        let array = dataframe_to_f64_array(py, position_pnls)?;
        let array = array
            .extract::<PyReadonlyArray2<'_, f64>>()
            .map_err(|_| value_error("position_pnls DataFrame must be two-dimensional"))?;
        let shape = array.shape();
        if shape[1] != ids.len() {
            return Err(value_error(format!(
                "position_pnls DataFrame has {} columns but {} position ids; \
                 rows are scenarios and columns are positions",
                shape[1],
                ids.len()
            )));
        }
        let data: Vec<f64> = array.as_array().iter().copied().collect();
        let matrix = PositionPnlMatrix::from_scenario_major(data, shape[0]);
        return Ok((ids, matrix));
    }
    let ids = position_ids.ok_or_else(|| {
        value_error("position_ids is required unless position_pnls is a pandas.DataFrame")
    })?;
    let matrix = extract_position_pnls(py, position_pnls, ids.len())?;
    Ok((ids, matrix))
}

/// Decompose portfolio VaR/ES into position contributions via parametric
/// Euler allocation, returning a typed ``PositionRiskDecomposition``.
///
/// Args:
///     position_ids: Position identifiers aligned with ``weights``; may be
///         ``None`` when ``covariance`` is a ``pandas.DataFrame`` (its column
///         labels are used).
///     weights: Portfolio weights or exposures, one per position.
///     covariance: Square position-return covariance matrix aligned with
///         ``position_ids`` — nested list, 2-D NumPy array, or
///         ``pandas.DataFrame``.
///     confidence: Tail confidence as a decimal probability strictly inside
///         ``(0.5, 1)``. ``None`` uses the Rust
///         ``DecompositionConfig.parametric_95()`` preset (``0.95``).
///     compute_incremental: Whether to compute leave-one-out incremental VaR
///         (one full repricing per position). Defaults to ``False``.
///
/// Returns:
///     ``PositionRiskDecomposition`` with portfolio VaR/ES (losses negative)
///     and per-position VaR and ES contributions.
///
/// Raises:
///     ValueError: If dimensions disagree, the covariance is not symmetric
///         positive semidefinite, or ``confidence`` is outside ``(0.5, 1)``.
#[pyfunction]
#[pyo3(signature = (position_ids, weights, covariance, confidence = None, compute_incremental = false))]
pub(super) fn parametric_var_decomposition(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    weights: Vec<f64>,
    covariance: &Bound<'_, PyAny>,
    confidence: Option<f64>,
    compute_incremental: bool,
) -> PyResult<PyPositionRiskDecomposition> {
    let n = weights.len();
    let (position_ids, cov_flat) = extract_covariance_input(py, position_ids, covariance, n)?;
    let mut config = confidence.map_or_else(
        DecompositionConfig::parametric_95,
        DecompositionConfig::parametric,
    );
    if compute_incremental {
        config = config.with_incremental();
    }

    let result = py
        .detach(move || {
            ParametricPositionDecomposer.decompose_positions(
                &weights,
                &cov_flat,
                &position_ids,
                &config,
            )
        })
        .map_err(core_to_py)?;

    Ok(PyPositionRiskDecomposition::from_inner(result))
}

/// Decompose portfolio expected shortfall through the parametric Euler
/// allocation and return the ES reporting view.
///
/// This is the ES twin of ``parametric_var_decomposition``: it runs the same
/// engine and projects the result onto ``portfolio_es`` and the per-position
/// ``component_es`` / ``pct_contribution`` rows. Use
/// ``parametric_var_decomposition`` when the joined VaR + ES table is needed.
///
/// Args:
///     position_ids: Position identifiers aligned with ``weights``; may be
///         ``None`` when ``covariance`` is a ``pandas.DataFrame``.
///     weights: Portfolio weights or exposures, one per position.
///     covariance: Square covariance matrix — nested list, 2-D NumPy array,
///         or ``pandas.DataFrame``.
///     confidence: ES tail confidence as a decimal probability strictly
///         inside ``(0.5, 1)``. ``None`` uses the Rust
///         ``DecompositionConfig.parametric_95()`` preset (``0.95``).
///
/// Returns:
///     ``ParametricEsDecompositionView`` with ``portfolio_var``,
///     ``portfolio_es`` (losses negative) and per-position ES rows.
///
/// Raises:
///     ValueError: If dimensions disagree, the covariance is malformed, or
///         ``confidence`` is outside ``(0.5, 1)``.
#[pyfunction]
#[pyo3(signature = (position_ids, weights, covariance, confidence = None))]
pub(super) fn parametric_es_decomposition(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    weights: Vec<f64>,
    covariance: &Bound<'_, PyAny>,
    confidence: Option<f64>,
) -> PyResult<PyParametricEsDecompositionView> {
    let n = weights.len();
    let (position_ids, cov_flat) = extract_covariance_input(py, position_ids, covariance, n)?;
    let config = confidence.map_or_else(
        DecompositionConfig::parametric_95,
        DecompositionConfig::parametric,
    );

    let view = py
        .detach(move || {
            ParametricPositionDecomposer
                .decompose_positions(&weights, &cov_flat, &position_ids, &config)
                .map(|decomposition| parametric_es_decomposition_view(&decomposition))
        })
        .map_err(core_to_py)?;

    Ok(PyParametricEsDecompositionView::from_inner(view))
}

/// Decompose portfolio VaR and ES from per-position scenario P&Ls via
/// historical simulation, returning a typed ``PositionRiskDecomposition``.
///
/// Args:
///     position_ids: Position identifiers; may be ``None`` when
///         ``position_pnls`` is a ``pandas.DataFrame`` (column labels are
///         used).
///     position_pnls: P&L matrix. A ``pandas.DataFrame`` is read as rows =
///         scenarios, columns = positions. A nested list or 2-D NumPy array is
///         read as ``n_positions x n_scenarios`` (position-major, one row per
///         position). Losses are negative.
///     confidence: Tail confidence as a decimal probability strictly inside
///         ``(0.5, 1)``. ``None`` uses the Rust
///         ``DecompositionConfig.historical_95()`` preset (``0.95``).
///
/// Returns:
///     ``PositionRiskDecomposition`` with historical VaR/ES totals and
///     per-position contributions (marginal and incremental VaR are ``None``).
///
/// Raises:
///     ValueError: If the matrix is empty, ragged, does not have one row per
///         position id, too few scenarios resolve the tail, or ``confidence``
///         is outside ``(0.5, 1)``.
#[pyfunction]
#[pyo3(signature = (position_ids, position_pnls, confidence = None))]
pub(super) fn historical_var_decomposition(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    position_pnls: &Bound<'_, PyAny>,
    confidence: Option<f64>,
) -> PyResult<PyPositionRiskDecomposition> {
    let (position_ids, position_pnls) = extract_pnl_input(py, position_ids, position_pnls)?;
    let n = position_ids.len();
    let n_scenarios = position_pnls.n_scenarios();

    let config = confidence.map_or_else(
        DecompositionConfig::historical_95,
        DecompositionConfig::historical,
    );
    let result = py
        .detach(move || {
            let flat = position_pnls.into_scenario_major(n);
            HistoricalPositionDecomposer.decompose_from_pnls(
                &flat,
                &position_ids,
                n_scenarios,
                &config,
            )
        })
        .map_err(core_to_py)?;

    Ok(PyPositionRiskDecomposition::from_inner(result))
}

/// Look up a position by id inside a ``PositionRiskDecomposition`` and
/// return its component VaR.
///
/// Args:
///     decomp: Decomposition to search.
///     position_id: Position identifier.
///
/// Raises:
///     KeyError: If ``position_id`` is not in the decomposition.
#[pyfunction]
#[pyo3(signature = (decomp, position_id))]
pub(super) fn position_component_var(
    decomp: &PyPositionRiskDecomposition,
    position_id: &str,
) -> PyResult<f64> {
    decomp
        .inner
        .var_contributions
        .iter()
        .find(|c| c.position_id.as_str() == position_id)
        .map(|c| c.component_var)
        .ok_or_else(|| {
            PyKeyError::new_err(format!("position '{position_id}' not in decomposition"))
        })
}

/// One position's row in a ``ParametricEsDecompositionView``.
///
/// Example:
///     >>> from finstack_quant.models.factor.risk import PositionEsContributionView
///     >>> row = PositionEsContributionView.from_json(
///     ...     '{"position_id":"A","component_es":-1.5,"marginal_es":null,"pct_contribution":0.6}'
///     ... )
///     >>> (row.position_id, row.pct_contribution)
///     ('A', 0.6)
#[pyclass(
    name = "PositionEsContributionView",
    module = "finstack_quant.models.factor.risk",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyPositionEsContributionView {
    pub(crate) inner: PositionEsContributionView,
}

impl PyPositionEsContributionView {
    fn from_inner(inner: PositionEsContributionView) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionEsContributionView {
    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: PositionEsContributionView = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Portfolio position identifier.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.clone()
    }

    /// Component Expected Shortfall allocated to the position (portfolio
    /// currency; losses negative).
    #[getter]
    fn component_es(&self) -> f64 {
        self.inner.component_es
    }

    /// Marginal ES, or ``None`` when the engine produced no gradient.
    #[getter]
    fn marginal_es(&self) -> Option<f64> {
        self.inner.marginal_es
    }

    /// Share of portfolio ES contributed by this position, as a fraction
    /// (not a percentage).
    #[getter]
    fn pct_contribution(&self) -> f64 {
        self.inner.pct_contribution
    }

    /// Export this row as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``component_es``, ``marginal_es``,
    /// ``pct_contribution``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("PositionEsContributionView", &self.inner)
    }
}

/// Expected-shortfall view of a parametric position decomposition, as
/// returned by ``parametric_es_decomposition``.
///
/// Example:
///     >>> from finstack_quant.models.factor.risk import parametric_es_decomposition
///     >>> es = parametric_es_decomposition(["A", "B"], [1.0, 2.0], [[0.04, 0.0], [0.0, 0.01]])
///     >>> (es.n_positions, round(es.portfolio_es, 6))
///     (2, -0.583423)
#[pyclass(
    name = "ParametricEsDecompositionView",
    module = "finstack_quant.models.factor.risk",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyParametricEsDecompositionView {
    pub(crate) inner: ParametricEsDecompositionView,
}

impl PyParametricEsDecompositionView {
    fn from_inner(inner: ParametricEsDecompositionView) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyParametricEsDecompositionView {
    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: ParametricEsDecompositionView = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Total portfolio VaR (portfolio currency; losses negative).
    #[getter]
    fn portfolio_var(&self) -> f64 {
        self.inner.portfolio_var
    }

    /// Total portfolio Expected Shortfall (portfolio currency; losses
    /// negative; at or beyond VaR in the loss tail).
    #[getter]
    fn portfolio_es(&self) -> f64 {
        self.inner.portfolio_es
    }

    /// Tail confidence as a decimal probability.
    #[getter]
    fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    /// Number of positions in the decomposition.
    #[getter]
    fn n_positions(&self) -> usize {
        self.inner.n_positions
    }

    /// Per-position ES rows.
    #[getter]
    fn contributions(&self) -> Vec<PyPositionEsContributionView> {
        self.inner
            .contributions
            .iter()
            .cloned()
            .map(PyPositionEsContributionView::from_inner)
            .collect()
    }

    /// Export the per-position ES rows as a pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``component_es``, ``marginal_es``,
    /// ``pct_contribution`` (fraction, not percentage). The portfolio
    /// scalars stay on their getters and are not repeated per row.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = &self.inner.contributions;
        let position_ids: Vec<&str> = rows.iter().map(|c| c.position_id.as_str()).collect();
        let component_es: Vec<f64> = rows.iter().map(|c| c.component_es).collect();
        let marginal_es: Vec<Option<f64>> = rows.iter().map(|c| c.marginal_es).collect();
        let pct: Vec<f64> = rows.iter().map(|c| c.pct_contribution).collect();
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("component_es", component_es)?;
        data.set_item("marginal_es", marginal_es)?;
        data.set_item("pct_contribution", pct)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("ParametricEsDecompositionView", &self.inner)
    }

    /// Render as an HTML table in Jupyter notebooks (delegates to
    /// ``to_dataframe``; returns ``None`` if the frame cannot be built).
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}
