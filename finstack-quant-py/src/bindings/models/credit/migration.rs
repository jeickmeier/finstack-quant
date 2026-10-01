//! Python bindings for `finstack_quant_models::credit::migration`.

use finstack_quant_models::credit::migration::{
    projection, GeneratorMatrix, MigrationSimulator, RatingPath, RatingScale, TransitionMatrix,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::{migration_to_py, serde_json_to_py, value_error};

/// Flatten a matrix argument into row-major data.
///
/// Accepts a flat row-major list, a nested list of rows, or anything with a
/// `tolist()` method (2-D `numpy.ndarray`, `pandas.DataFrame.values`).
fn extract_matrix_data(data: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    let data = if data.hasattr("tolist")? {
        data.call_method0("tolist")?
    } else {
        data.clone()
    };
    if let Ok(flat) = data.extract::<Vec<f64>>() {
        return Ok(flat);
    }
    let rows: Vec<Vec<f64>> = data.extract().map_err(|_| {
        value_error(
            "matrix data must be a flat row-major list of floats, a nested list of rows, \
             or a 2-D numpy array",
        )
    })?;
    let width = rows.first().map_or(0, Vec::len);
    if rows.iter().any(|row| row.len() != width) {
        return Err(value_error("matrix rows must all have the same length"));
    }
    Ok(rows.into_iter().flatten().collect())
}

/// Build a labelled square `pd.DataFrame` (`index` = origin, `columns` = destination).
fn labelled_square_frame<'py>(
    py: Python<'py>,
    scale: &RatingScale,
    data: &nalgebra::DMatrix<f64>,
) -> PyResult<Bound<'py, PyAny>> {
    let labels: Vec<&str> = scale.labels().iter().map(String::as_str).collect();
    let columns = PyDict::new(py);
    for (col, label) in labels.iter().enumerate() {
        let values: Vec<f64> = (0..data.nrows()).map(|row| data[(row, col)]).collect();
        columns.set_item(label, values)?;
    }
    let index = PyList::new(py, &labels)?;
    dict_to_dataframe(py, &columns, Some(index.into_any()))
}

/// Ordinal rating scale (highest grade first) with an optional absorbing
/// default state; the label universe shared by matrices, generators and
/// simulated paths.
#[pyclass(
    module = "finstack_quant.models.credit.migration",
    name = "RatingScale",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyRatingScale {
    pub(crate) inner: RatingScale,
}

impl PyRatingScale {
    fn from_inner(inner: RatingScale) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRatingScale {
    /// The standard whole-letter scale (AAA .. CCC, D), highest grade first.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn standard() -> Self {
        Self::from_inner(RatingScale::standard())
    }

    /// The standard scale with an explicit not-rated state appended.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn standard_with_nr() -> Self {
        Self::from_inner(RatingScale::standard_with_nr())
    }

    /// A notched scale (AA+/AA/AA-, ...) rather than whole grades.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn notched() -> Self {
        Self::from_inner(RatingScale::notched())
    }

    /// A scale built from explicit labels, highest grade first; the last
    /// label is taken as the absorbing default state.
    ///
    /// Parameters
    /// ----------
    /// labels : list[str]
    ///     At least two distinct labels.
    ///
    /// Raises ``ValueError`` for fewer than two labels or duplicates.
    #[staticmethod]
    #[pyo3(text_signature = "(labels)")]
    fn custom(labels: Vec<String>) -> PyResult<Self> {
        RatingScale::custom(labels)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    /// A custom scale with an explicit default (absorbing) state label.
    ///
    /// Parameters
    /// ----------
    /// labels : list[str]
    ///     At least two distinct labels, highest grade first.
    /// default_label : str
    ///     Label of the absorbing default state; must be in ``labels``.
    ///
    /// Raises ``ValueError`` for fewer than two labels or duplicates and
    /// ``KeyError`` when ``default_label`` is not in ``labels``.
    #[staticmethod]
    #[pyo3(text_signature = "(labels, default_label)")]
    fn custom_with_default(labels: Vec<String>, default_label: String) -> PyResult<Self> {
        RatingScale::custom_with_default(labels, default_label)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    /// Number of rating states in the scale.
    #[getter]
    fn n_states(&self) -> usize {
        self.inner.n_states()
    }

    /// Index of a label in the scale, or ``None`` if absent.
    #[pyo3(text_signature = "($self, label)")]
    fn index_of(&self, label: &str) -> Option<usize> {
        self.inner.index_of(label)
    }

    /// Index of a label in the scale; raises ``KeyError`` if absent.
    #[pyo3(text_signature = "($self, label)")]
    fn index_of_required(&self, label: &str) -> PyResult<usize> {
        self.inner.index_of_required(label).map_err(migration_to_py)
    }

    /// Label at a state index, or ``None`` when out of range.
    #[pyo3(text_signature = "($self, index)")]
    fn label_of(&self, index: usize) -> Option<String> {
        self.inner.label_of(index).map(str::to_owned)
    }

    /// Index of the default state, or ``None`` if the scale has none.
    #[pyo3(text_signature = "($self)")]
    fn default_state(&self) -> Option<usize> {
        self.inner.default_state()
    }

    /// Rating labels, highest grade first.
    #[pyo3(text_signature = "($self)")]
    fn labels(&self) -> Vec<String> {
        self.inner.labels().to_vec()
    }

    /// Weighted-average rating factor for a label.
    ///
    /// Raises ``KeyError`` when the label is unknown or has no WARF factor.
    #[pyo3(text_signature = "($self, label)")]
    fn warf(&self, label: &str) -> PyResult<f64> {
        self.inner.warf(label).map_err(migration_to_py)
    }

    /// Nearest rating label for a weighted-average rating factor.
    ///
    /// Raises ``ValueError`` when ``warf`` is non-finite or negative.
    #[pyo3(text_signature = "($self, warf)")]
    fn rating_from_warf(&self, warf: f64) -> PyResult<String> {
        self.inner
            .rating_from_warf(warf)
            .map(str::to_owned)
            .map_err(migration_to_py)
    }

    /// Deserialize a scale from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid RatingScale JSON"))?,
        })
    }

    /// Serialize this scale to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "RatingScale serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __len__(&self) -> usize {
        self.inner.n_states()
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        let labels: Vec<String> = self
            .inner
            .labels()
            .iter()
            .map(|l| format!("{l:?}"))
            .collect();
        let default = match self.inner.default_state() {
            Some(index) => index.to_string(),
            None => "None".to_string(),
        };
        format!(
            "RatingScale(labels=[{}], default_state={default})",
            labels.join(", ")
        )
    }
}

/// Row-stochastic rating transition matrix over a horizon in years.
///
/// Rows are origin states and columns destination states, both in
/// ``scale`` order; entries are probabilities.
#[pyclass(
    module = "finstack_quant.models.credit.migration",
    name = "TransitionMatrix",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyTransitionMatrix {
    pub(crate) inner: TransitionMatrix,
}

impl PyTransitionMatrix {
    fn from_inner(inner: TransitionMatrix) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyTransitionMatrix {
    /// Build a transition matrix.
    ///
    /// Parameters
    /// ----------
    /// scale : RatingScale
    ///     Rating scale defining row/column order.
    /// data : list[float] | list[list[float]] | numpy.ndarray
    ///     Probabilities, row-major flat (``n * n`` values), nested rows, or a
    ///     2-D array.
    /// horizon : float
    ///     Horizon the probabilities cover, in years (> 0).
    ///
    /// Raises ``ValueError`` when the dimension does not match the scale, a
    /// row does not sum to one, an entry is outside [0, 1], the default
    /// state is not absorbing, or the horizon is invalid.
    #[new]
    #[pyo3(text_signature = "(scale, data, horizon)")]
    fn new(scale: &PyRatingScale, data: &Bound<'_, PyAny>, horizon: f64) -> PyResult<Self> {
        let data = extract_matrix_data(data)?;
        TransitionMatrix::new(scale.inner.clone(), &data, horizon)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    /// Build a transition matrix from a labelled square ``pandas.DataFrame``.
    ///
    /// Parameters
    /// ----------
    /// df : pandas.DataFrame
    ///     Square frame whose index (origins) and columns (destinations) carry
    ///     the same labels in scale order.
    /// horizon : float
    ///     Horizon in years (> 0).
    /// scale : RatingScale | None
    ///     Scale to validate against; defaults to
    ///     ``RatingScale.custom(list(df.index))`` (last label absorbing).
    ///
    /// Raises ``ValueError`` when index and columns differ, or the matrix is
    /// invalid for the scale.
    #[staticmethod]
    #[pyo3(signature = (df, horizon, scale = None))]
    #[pyo3(text_signature = "(df, horizon, scale=None)")]
    fn from_dataframe(
        df: &Bound<'_, PyAny>,
        horizon: f64,
        scale: Option<&PyRatingScale>,
    ) -> PyResult<Self> {
        let index: Vec<String> = df.getattr("index")?.call_method0("tolist")?.extract()?;
        let columns: Vec<String> = df.getattr("columns")?.call_method0("tolist")?.extract()?;
        if index != columns {
            return Err(value_error(
                "from_dataframe requires identical index and column labels in scale order",
            ));
        }
        let scale = match scale {
            Some(scale) => scale.inner.clone(),
            None => RatingScale::custom(index).map_err(migration_to_py)?,
        };
        let data = extract_matrix_data(&df.getattr("values")?)?;
        TransitionMatrix::new(scale, &data, horizon)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    /// Transition probability from one labelled state to another.
    ///
    /// Raises ``KeyError`` for an unknown label.
    #[pyo3(signature = (from_, to))]
    #[pyo3(text_signature = "($self, from_, to)")]
    fn probability(&self, from_: &str, to: &str) -> PyResult<f64> {
        self.inner.probability(from_, to).map_err(migration_to_py)
    }

    /// Transition probability by state indices; raises ``ValueError`` when
    /// either index is outside the scale.
    #[pyo3(signature = (from_, to))]
    #[pyo3(text_signature = "($self, from_, to)")]
    fn probability_by_index(&self, from_: usize, to: usize) -> PyResult<f64> {
        self.inner
            .try_probability_by_index(from_, to)
            .map_err(migration_to_py)
    }

    /// One row of transition probabilities, indexed by destination state.
    ///
    /// Raises ``KeyError`` for an unknown label.
    #[pyo3(signature = (from_))]
    #[pyo3(text_signature = "($self, from_)")]
    fn row(&self, from_: &str) -> PyResult<Vec<f64>> {
        self.inner.row(from_).map_err(migration_to_py)
    }

    /// Compose with another matrix on the same scale: ``P(s + t) = P(s) @ P(t)``.
    ///
    /// Raises ``ValueError`` when the scales differ.
    #[pyo3(text_signature = "($self, other)")]
    fn compose(&self, other: &PyTransitionMatrix) -> PyResult<Self> {
        self.inner
            .compose(&other.inner)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    /// Row-major copy of the underlying matrix as nested lists.
    #[pyo3(text_signature = "($self)")]
    fn to_matrix(&self) -> Vec<Vec<f64>> {
        self.inner.to_rows()
    }

    /// Labelled square ``pandas.DataFrame`` (index = origin, columns = destination).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        labelled_square_frame(py, self.inner.scale(), self.inner.as_matrix())
    }

    /// Horizon this matrix covers, in years.
    #[getter]
    fn horizon(&self) -> f64 {
        self.inner.horizon()
    }

    /// Number of rating states in the scale.
    #[getter]
    fn n_states(&self) -> usize {
        self.inner.n_states()
    }

    /// The rating scale defining row/column order.
    #[getter]
    fn scale(&self) -> PyRatingScale {
        PyRatingScale::from_inner(self.inner.scale().clone())
    }

    /// Probability of reaching the default state per origin state, or
    /// ``None`` when the scale has no default state.
    #[pyo3(text_signature = "($self)")]
    fn default_probabilities(&self) -> Option<Vec<f64>> {
        self.inner.default_probabilities()
    }

    /// Deserialize a matrix from canonical JSON (re-validated on load).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid TransitionMatrix JSON"))?,
        })
    }

    /// Serialize this matrix to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "TransitionMatrix serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        format!(
            "TransitionMatrix(n_states={}, horizon={}, labels=[<{} items>])",
            self.inner.n_states(),
            self.inner.horizon(),
            self.inner.n_states()
        )
    }

    /// Render as an HTML table in Jupyter notebooks (delegates to ``to_dataframe``).
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Annualized continuous-time Markov generator ``Q`` (rows sum to zero,
/// non-negative off-diagonals) over a rating scale.
#[pyclass(
    module = "finstack_quant.models.credit.migration",
    name = "GeneratorMatrix",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyGeneratorMatrix {
    pub(crate) inner: GeneratorMatrix,
}

impl PyGeneratorMatrix {
    fn from_inner(inner: GeneratorMatrix) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyGeneratorMatrix {
    /// Build a generator matrix.
    ///
    /// Parameters
    /// ----------
    /// scale : RatingScale
    ///     Rating scale defining row/column order.
    /// data : list[float] | list[list[float]] | numpy.ndarray
    ///     Intensities per year, row-major flat, nested rows, or a 2-D array.
    ///
    /// Raises ``ValueError`` when the dimension does not match the scale, a
    /// row does not sum to zero, an off-diagonal is negative, or the default
    /// state is not absorbing.
    #[new]
    #[pyo3(text_signature = "(scale, data)")]
    fn new(scale: &PyRatingScale, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        let data = extract_matrix_data(data)?;
        GeneratorMatrix::new(scale.inner.clone(), &data)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    /// Embed a transition matrix as a generator via the matrix logarithm
    /// (Israel-Rosenthal-Wei with Kreinin-Sidenius regularization).
    ///
    /// Raises ``RuntimeError`` when no valid generator exists (complex or
    /// non-positive eigenvalues) or the round-trip error exceeds the default
    /// tolerance.
    #[staticmethod]
    #[pyo3(text_signature = "(p)")]
    fn from_transition_matrix(p: &PyTransitionMatrix) -> PyResult<Self> {
        GeneratorMatrix::from_transition_matrix(&p.inner)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    /// Like ``from_transition_matrix`` with an explicit round-trip tolerance.
    ///
    /// Parameters
    /// ----------
    /// p : TransitionMatrix
    ///     Source matrix.
    /// round_trip_tol : float
    ///     Non-negative infinity-norm tolerance on ``exp(Q * h) - P(h)``.
    ///
    /// Raises ``RuntimeError`` when no valid generator exists or the
    /// round-trip error exceeds ``round_trip_tol``.
    #[staticmethod]
    #[pyo3(text_signature = "(p, round_trip_tol)")]
    fn from_transition_matrix_with_tol(
        p: &PyTransitionMatrix,
        round_trip_tol: f64,
    ) -> PyResult<Self> {
        GeneratorMatrix::from_transition_matrix_with_tol(&p.inner, round_trip_tol)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    /// Off-diagonal generator intensity (per year) from one state to another.
    ///
    /// Raises ``KeyError`` for an unknown label.
    #[pyo3(signature = (from_, to))]
    #[pyo3(text_signature = "($self, from_, to)")]
    fn intensity(&self, from_: &str, to: &str) -> PyResult<f64> {
        self.inner.intensity(from_, to).map_err(migration_to_py)
    }

    /// Total intensity of leaving a state (the negated diagonal entry).
    ///
    /// Raises ``KeyError`` for an unknown label.
    #[pyo3(text_signature = "($self, state)")]
    fn exit_rate(&self, state: &str) -> PyResult<f64> {
        self.inner.exit_rate(state).map_err(migration_to_py)
    }

    /// Row-major copy of the underlying matrix as nested lists.
    #[pyo3(text_signature = "($self)")]
    fn to_matrix(&self) -> Vec<Vec<f64>> {
        self.inner.to_rows()
    }

    /// Labelled square ``pandas.DataFrame`` (index = origin, columns = destination).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        labelled_square_frame(py, self.inner.scale(), self.inner.as_matrix())
    }

    /// Number of rating states in the scale.
    #[getter]
    fn n_states(&self) -> usize {
        self.inner.n_states()
    }

    /// The rating scale defining row/column order.
    #[getter]
    fn scale(&self) -> PyRatingScale {
        PyRatingScale::from_inner(self.inner.scale().clone())
    }

    /// L1 mass clamped by Kreinin-Sidenius regularization during extraction.
    #[getter]
    fn regularization_l1(&self) -> f64 {
        self.inner.regularization_l1()
    }

    /// Infinity-norm error from reconstructing the source transition matrix.
    #[getter]
    fn round_trip_error(&self) -> f64 {
        self.inner.round_trip_error()
    }

    /// Deserialize a generator from canonical JSON (re-validated on load).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid GeneratorMatrix JSON"))?,
        })
    }

    /// Serialize this generator to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "GeneratorMatrix serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        format!(
            "GeneratorMatrix(n_states={}, regularization_l1={}, round_trip_error={})",
            self.inner.n_states(),
            self.inner.regularization_l1(),
            self.inner.round_trip_error()
        )
    }

    /// Render as an HTML table in Jupyter notebooks (delegates to ``to_dataframe``).
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// One simulated rating trajectory: piecewise-constant state over
/// ``[0, horizon]`` recorded as ``(time, new_state)`` transitions.
#[pyclass(
    module = "finstack_quant.models.credit.migration",
    name = "RatingPath",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyRatingPath {
    pub(crate) inner: RatingPath,
}

impl PyRatingPath {
    fn from_inner(inner: RatingPath) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRatingPath {
    /// Rating state index occupied at time ``t`` (right-continuous at jumps).
    #[pyo3(text_signature = "($self, t)")]
    fn state_at(&self, t: f64) -> usize {
        self.inner.state_at(t)
    }

    /// Rating label occupied at time ``t``.
    #[pyo3(text_signature = "($self, t)")]
    fn label_at(&self, t: f64) -> String {
        self.inner.label_at(t).to_owned()
    }

    /// Whether the path reached the default state.
    #[pyo3(text_signature = "($self)")]
    fn defaulted(&self) -> bool {
        self.inner.defaulted()
    }

    /// Time of default in years, or ``None`` if the path never defaulted.
    #[pyo3(text_signature = "($self)")]
    fn default_time(&self) -> Option<f64> {
        self.inner.default_time()
    }

    /// Number of recorded transitions, including the initial ``(0.0, s0)`` entry.
    #[pyo3(text_signature = "($self)")]
    fn n_transitions(&self) -> usize {
        self.inner.n_transitions()
    }

    /// Every ``(time, new_state)`` event on the path; the first is always
    /// ``(0.0, initial_state)``.
    #[pyo3(text_signature = "($self)")]
    fn transitions(&self) -> Vec<(f64, usize)> {
        self.inner.transitions().to_vec()
    }

    /// Simulation horizon in years.
    #[getter]
    fn horizon(&self) -> f64 {
        self.inner.horizon()
    }

    /// The rating scale the state indices refer to.
    #[getter]
    fn scale(&self) -> PyRatingScale {
        PyRatingScale::from_inner(self.inner.scale().clone())
    }

    /// Deserialize a path from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid RatingPath JSON"))?,
        })
    }

    /// Serialize this path to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "RatingPath serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        let default_time = match self.inner.default_time() {
            Some(t) => t.to_string(),
            None => "None".to_string(),
        };
        format!(
            "RatingPath(initial={:?}, n_transitions={}, horizon={}, default_time={default_time})",
            self.inner.label_at(0.0),
            self.inner.n_transitions(),
            self.inner.horizon()
        )
    }
}

/// Collection of simulated rating paths from ``MigrationSimulator.simulate``.
///
/// Indexable and iterable like a list of ``RatingPath``; ``to_dataframe()``
/// gives one long frame over all transitions.
#[pyclass(
    module = "finstack_quant.models.credit.migration",
    name = "RatingPaths",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyRatingPaths {
    pub(crate) inner: Vec<RatingPath>,
}

#[pymethods]
impl PyRatingPaths {
    /// The paths as a list of ``RatingPath``.
    #[getter]
    fn paths(&self) -> Vec<PyRatingPath> {
        self.inner
            .iter()
            .cloned()
            .map(PyRatingPath::from_inner)
            .collect()
    }

    /// Fraction of paths that reached the default state.
    #[getter]
    fn default_rate(&self) -> f64 {
        finstack_quant_models::credit::migration::default_rate(&self.inner)
    }

    /// Long-format ``pandas.DataFrame`` of every transition.
    ///
    /// Columns: ``path`` (int), ``time`` (float, years), ``state`` (int),
    /// ``label`` (str); one row per recorded transition (including the
    /// initial state at ``time = 0``), ordered by path then time.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut path_col: Vec<u64> = Vec::new();
        let mut time_col: Vec<f64> = Vec::new();
        let mut state_col: Vec<u64> = Vec::new();
        let mut label_col: Vec<String> = Vec::new();
        for (index, path) in self.inner.iter().enumerate() {
            for (time, state) in path.transitions() {
                path_col.push(index as u64);
                time_col.push(*time);
                state_col.push(*state as u64);
                label_col.push(path.scale().label_of(*state).unwrap_or_default().to_owned());
            }
        }
        let columns = PyDict::new(py);
        columns.set_item("path", path_col)?;
        columns.set_item("time", time_col)?;
        columns.set_item("state", state_col)?;
        columns.set_item("label", label_col)?;
        dict_to_dataframe(py, &columns, None)
    }

    /// Deserialize paths from a canonical JSON array.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid RatingPaths JSON"))?,
        })
    }

    /// Serialize the paths to a compact canonical JSON array.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "RatingPaths serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __getitem__(&self, index: isize) -> PyResult<PyRatingPath> {
        let len = self.inner.len() as isize;
        let resolved = if index < 0 { index + len } else { index };
        if resolved < 0 || resolved >= len {
            return Err(pyo3::exceptions::PyIndexError::new_err(format!(
                "path index {index} out of range for {len} paths"
            )));
        }
        Ok(PyRatingPath::from_inner(
            self.inner[resolved as usize].clone(),
        ))
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        format!(
            "RatingPaths(n_paths={}, default_rate={})",
            self.inner.len(),
            self.default_rate()
        )
    }
}

/// Gillespie CTMC simulator over a generator matrix and horizon.
#[pyclass(
    module = "finstack_quant.models.credit.migration",
    name = "MigrationSimulator",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMigrationSimulator {
    pub(crate) inner: MigrationSimulator,
}

impl PyMigrationSimulator {
    fn from_inner(inner: MigrationSimulator) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyMigrationSimulator {
    /// Build a simulator.
    ///
    /// Parameters
    /// ----------
    /// generator : GeneratorMatrix
    ///     Annualized generator to simulate under.
    /// horizon : float
    ///     Simulation horizon in years (> 0).
    ///
    /// Raises ``ValueError`` when ``horizon`` is non-positive or non-finite.
    #[new]
    #[pyo3(text_signature = "(generator, horizon)")]
    fn new(generator: &PyGeneratorMatrix, horizon: f64) -> PyResult<Self> {
        MigrationSimulator::new(generator.inner.clone(), horizon)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    /// Simulate rating paths from ``initial_state``.
    ///
    /// Determinism: paths are generated with the canonical ``Pcg64`` RNG
    /// seeded from ``seed``; identical seeds reproduce identical paths.
    /// Releases the GIL during simulation.
    ///
    /// Parameters
    /// ----------
    /// initial_state : int
    ///     Starting state index in the generator's scale.
    /// n_paths : int
    ///     Number of paths (> 0).
    /// seed : int
    ///     RNG seed.
    ///
    /// Returns a ``RatingPaths`` collection.
    ///
    /// Raises ``ValueError`` when the state index is out of range or
    /// ``n_paths`` is zero.
    #[pyo3(text_signature = "($self, initial_state, n_paths, seed)")]
    fn simulate(
        &self,
        py: Python<'_>,
        initial_state: usize,
        n_paths: usize,
        seed: u64,
    ) -> PyResult<PyRatingPaths> {
        let paths = py
            .detach(|| self.inner.simulate_seeded(initial_state, n_paths, seed))
            .map_err(migration_to_py)?;
        Ok(PyRatingPaths { inner: paths })
    }

    /// Build an empirical transition matrix by simulating from every state.
    ///
    /// Uses the canonical seeded ``Pcg64`` RNG (see ``simulate``) and
    /// releases the GIL during simulation.
    ///
    /// Parameters
    /// ----------
    /// n_paths_per_state : int
    ///     Paths simulated from each origin state (> 0).
    /// seed : int
    ///     RNG seed.
    ///
    /// Raises ``ValueError`` when ``n_paths_per_state`` is zero.
    #[pyo3(text_signature = "($self, n_paths_per_state, seed)")]
    fn empirical_matrix(
        &self,
        py: Python<'_>,
        n_paths_per_state: usize,
        seed: u64,
    ) -> PyResult<PyTransitionMatrix> {
        let matrix = py.detach(|| self.inner.empirical_matrix_seeded(n_paths_per_state, seed));
        matrix
            .map(PyTransitionMatrix::from_inner)
            .map_err(migration_to_py)
    }

    /// Simulation horizon in years.
    #[getter]
    fn horizon(&self) -> f64 {
        self.inner.horizon()
    }

    /// The generator matrix simulated under.
    #[getter]
    fn generator(&self) -> PyGeneratorMatrix {
        PyGeneratorMatrix::from_inner(self.inner.generator().clone())
    }

    /// Deserialize a simulator from canonical JSON (re-validated on load).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid MigrationSimulator JSON"))?,
        })
    }

    /// Serialize this simulator to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "MigrationSimulator serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        format!(
            "MigrationSimulator(n_states={}, horizon={})",
            self.inner.generator().n_states(),
            self.inner.horizon()
        )
    }
}

/// Project a generator matrix to a transition matrix over a horizon.
///
/// Computes ``P(t) = exp(Q * t)`` — the matrix exponential of the continuous-time
/// generator ``Q``. This is the standard way to obtain rating-migration
/// probabilities for a non-annual horizon (e.g. quarterly, or a 5-year
/// cumulative view) from an annual generator.
///
/// Parameters
/// ----------
/// generator : GeneratorMatrix
///     Continuous-time generator with non-negative off-diagonals and rows
///     summing to zero.
/// t : float
///     Horizon in years. Must be non-negative.
///
/// Returns
/// -------
/// TransitionMatrix
///     Row-stochastic migration probabilities over ``t`` years.
///
/// Raises
/// ------
/// ValueError
///     If ``t`` is negative or the projection fails to produce a valid
///     row-stochastic matrix.
///
/// References
/// ----------
/// Israel, R. B., Rosenthal, J. S., & Wei, J. Z. (2001). "Finding Generators
/// for Markov Chains via Empirical Transition Matrices, with Applications to
/// Credit Ratings." *Mathematical Finance*, 11(2), 245-265.
#[pyfunction]
#[pyo3(text_signature = "(generator, t)")]
fn project(generator: &PyGeneratorMatrix, t: f64) -> PyResult<PyTransitionMatrix> {
    projection::project(&generator.inner, t)
        .map(PyTransitionMatrix::from_inner)
        .map_err(migration_to_py)
}

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "migration")?;
    m.setattr(
        "__doc__",
        "Credit migration models: rating scales, transition matrices, CTMC generators, and seeded simulation.",
    )?;

    m.add_class::<PyRatingScale>()?;
    m.add_class::<PyTransitionMatrix>()?;
    m.add_class::<PyGeneratorMatrix>()?;
    m.add_class::<PyRatingPath>()?;
    m.add_class::<PyRatingPaths>()?;
    m.add_class::<PyMigrationSimulator>()?;
    m.add_function(wrap_pyfunction!(project, &m)?)?;

    let all = PyList::new(
        py,
        [
            "GeneratorMatrix",
            "MigrationSimulator",
            "RatingPath",
            "RatingPaths",
            "RatingScale",
            "TransitionMatrix",
            "project",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "migration",
        "finstack_quant.models.credit",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
