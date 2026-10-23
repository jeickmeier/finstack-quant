//! Python bindings for `finstack_quant_models::credit::migration`.

use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use finstack_quant_models::credit::migration::{
    self, GeneratorMatrix, MigrationSimulator, RatingPath, RatingScale, TransitionMatrix,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::bindings::pydoc::pydoc;
use crate::errors::kinded_to_py;

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

#[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale")]
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
    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.standard")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn standard() -> Self {
        Self::from_inner(RatingScale::standard())
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.standard_with_nr")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn standard_with_nr() -> Self {
        Self::from_inner(RatingScale::standard_with_nr())
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.notched")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn notched() -> Self {
        Self::from_inner(RatingScale::notched())
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.custom")]
    #[staticmethod]
    #[pyo3(text_signature = "(labels)")]
    fn custom(labels: Vec<String>) -> PyResult<Self> {
        RatingScale::custom(labels)
            .map(Self::from_inner)
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.custom_with_default")]
    #[staticmethod]
    #[pyo3(text_signature = "(labels, default_label)")]
    fn custom_with_default(labels: Vec<String>, default_label: String) -> PyResult<Self> {
        RatingScale::custom_with_default(labels, default_label)
            .map(Self::from_inner)
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.n_states")]
    #[getter]
    fn n_states(&self) -> usize {
        self.inner.n_states()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.index_of")]
    #[pyo3(text_signature = "($self, label)")]
    fn index_of(&self, label: &str) -> Option<usize> {
        self.inner.index_of(label)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.index_of_required")]
    #[pyo3(text_signature = "($self, label)")]
    fn index_of_required(&self, label: &str) -> PyResult<usize> {
        self.inner.index_of_required(label).map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.label_of")]
    #[pyo3(text_signature = "($self, index)")]
    fn label_of(&self, index: usize) -> Option<String> {
        self.inner.label_of(index).map(str::to_owned)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.default_state")]
    #[pyo3(text_signature = "($self)")]
    fn default_state(&self) -> Option<usize> {
        self.inner.default_state()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.labels")]
    #[pyo3(text_signature = "($self)")]
    fn labels(&self) -> Vec<String> {
        self.inner.labels().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.warf")]
    #[pyo3(text_signature = "($self, label)")]
    fn warf(&self, label: &str) -> PyResult<f64> {
        self.inner.warf(label).map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.rating_from_warf")]
    #[pyo3(text_signature = "($self, warf)")]
    fn rating_from_warf(&self, warf: f64) -> PyResult<String> {
        self.inner
            .rating_from_warf(warf)
            .map(str::to_owned)
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.__len__")]
    fn __len__(&self) -> usize {
        self.inner.n_states()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingScale.__repr__")]
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

wire_methods!(
    PyRatingScale,
    RatingScale,
    "RatingScale",
    module = "finstack_quant.models.credit.migration"
);

#[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix")]
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
    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.__init__")]
    #[new]
    #[pyo3(text_signature = "(scale, data, horizon)")]
    fn new(scale: &PyRatingScale, data: &Bound<'_, PyAny>, horizon: f64) -> PyResult<Self> {
        let data = crate::bindings::matrix_input::extract_square_matrix(
            data.py(),
            data,
            scale.inner.n_states(),
            "data",
        )?;
        TransitionMatrix::new(scale.inner.clone(), &data, horizon)
            .map(Self::from_inner)
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.from_dataframe")]
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
        Self::from_labeled(index, columns, &df.getattr("values")?, horizon, scale)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.from_labeled")]
    #[staticmethod]
    #[pyo3(signature = (row_labels, column_labels, data, horizon, scale = None))]
    fn from_labeled(
        row_labels: Vec<String>,
        column_labels: Vec<String>,
        data: &Bound<'_, PyAny>,
        horizon: f64,
        scale: Option<&PyRatingScale>,
    ) -> PyResult<Self> {
        let data = crate::bindings::matrix_input::extract_square_matrix(
            data.py(),
            data,
            row_labels.len(),
            "data",
        )?;
        TransitionMatrix::from_labeled(
            &row_labels,
            &column_labels,
            &data,
            horizon,
            scale.map(|scale| scale.inner.clone()),
        )
        .map(Self::from_inner)
        .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.probability")]
    #[pyo3(signature = (from_, to))]
    #[pyo3(text_signature = "($self, from_, to)")]
    fn probability(&self, from_: &str, to: &str) -> PyResult<f64> {
        self.inner.probability(from_, to).map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.probability_by_index")]
    #[pyo3(signature = (from_, to))]
    #[pyo3(text_signature = "($self, from_, to)")]
    fn probability_by_index(&self, from_: usize, to: usize) -> PyResult<f64> {
        self.inner
            .probability_by_index(from_, to)
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.row")]
    #[pyo3(signature = (from_))]
    #[pyo3(text_signature = "($self, from_)")]
    fn row(&self, from_: &str) -> PyResult<Vec<f64>> {
        self.inner.row(from_).map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.compose")]
    #[pyo3(text_signature = "($self, other)")]
    fn compose(&self, other: &PyTransitionMatrix) -> PyResult<Self> {
        self.inner
            .compose(&other.inner)
            .map(Self::from_inner)
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.to_rows")]
    #[pyo3(text_signature = "($self)")]
    fn to_rows(&self) -> Vec<Vec<f64>> {
        self.inner.to_rows()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        labelled_square_frame(py, self.inner.scale(), self.inner.as_matrix())
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.horizon")]
    #[getter]
    fn horizon(&self) -> f64 {
        self.inner.horizon()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.n_states")]
    #[getter]
    fn n_states(&self) -> usize {
        self.inner.n_states()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.scale")]
    #[getter]
    fn scale(&self) -> PyRatingScale {
        PyRatingScale::from_inner(self.inner.scale().clone())
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.default_probabilities")]
    #[pyo3(text_signature = "($self)")]
    fn default_probabilities(&self) -> Option<Vec<f64>> {
        self.inner.default_probabilities()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "TransitionMatrix.__repr__")]
    fn __repr__(&self) -> String {
        format!(
            "TransitionMatrix(n_states={}, horizon={}, labels=[<{} items>])",
            self.inner.n_states(),
            self.inner.horizon(),
            self.inner.n_states()
        )
    }
}

wire_methods!(
    PyTransitionMatrix,
    TransitionMatrix,
    "TransitionMatrix",
    module = "finstack_quant.models.credit.migration"
);
impl_repr_html_via_dataframe!(PyTransitionMatrix);

#[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix")]
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
    #[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix.__init__")]
    #[new]
    #[pyo3(text_signature = "(scale, data)")]
    fn new(scale: &PyRatingScale, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        let data = crate::bindings::matrix_input::extract_square_matrix(
            data.py(),
            data,
            scale.inner.n_states(),
            "data",
        )?;
        GeneratorMatrix::new(scale.inner.clone(), &data)
            .map(Self::from_inner)
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix.from_transition_matrix")]
    #[staticmethod]
    #[pyo3(signature = (p, round_trip_tol = None))]
    #[pyo3(text_signature = "(p, round_trip_tol=None)")]
    fn from_transition_matrix(
        p: &PyTransitionMatrix,
        round_trip_tol: Option<f64>,
    ) -> PyResult<Self> {
        GeneratorMatrix::from_transition_matrix(&p.inner, round_trip_tol)
            .map(Self::from_inner)
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix.intensity")]
    #[pyo3(signature = (from_, to))]
    #[pyo3(text_signature = "($self, from_, to)")]
    fn intensity(&self, from_: &str, to: &str) -> PyResult<f64> {
        self.inner.intensity(from_, to).map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix.exit_rate")]
    #[pyo3(text_signature = "($self, state)")]
    fn exit_rate(&self, state: &str) -> PyResult<f64> {
        self.inner.exit_rate(state).map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix.to_rows")]
    #[pyo3(text_signature = "($self)")]
    fn to_rows(&self) -> Vec<Vec<f64>> {
        self.inner.to_rows()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        labelled_square_frame(py, self.inner.scale(), self.inner.as_matrix())
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix.n_states")]
    #[getter]
    fn n_states(&self) -> usize {
        self.inner.n_states()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix.scale")]
    #[getter]
    fn scale(&self) -> PyRatingScale {
        PyRatingScale::from_inner(self.inner.scale().clone())
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix.regularization_l1")]
    #[getter]
    fn regularization_l1(&self) -> f64 {
        self.inner.regularization_l1()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix.round_trip_error")]
    #[getter]
    fn round_trip_error(&self) -> f64 {
        self.inner.round_trip_error()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "GeneratorMatrix.__repr__")]
    fn __repr__(&self) -> String {
        format!(
            "GeneratorMatrix(n_states={}, regularization_l1={}, round_trip_error={})",
            self.inner.n_states(),
            self.inner.regularization_l1(),
            self.inner.round_trip_error()
        )
    }
}

wire_methods!(
    PyGeneratorMatrix,
    GeneratorMatrix,
    "GeneratorMatrix",
    module = "finstack_quant.models.credit.migration"
);
impl_repr_html_via_dataframe!(PyGeneratorMatrix);

#[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPath")]
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
    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPath.state_at")]
    #[pyo3(text_signature = "($self, t)")]
    fn state_at(&self, t: f64) -> usize {
        self.inner.state_at(t)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPath.label_at")]
    #[pyo3(text_signature = "($self, t)")]
    fn label_at(&self, t: f64) -> String {
        self.inner.label_at(t).to_owned()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPath.defaulted")]
    #[pyo3(text_signature = "($self)")]
    fn defaulted(&self) -> bool {
        self.inner.defaulted()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPath.default_time")]
    #[pyo3(text_signature = "($self)")]
    fn default_time(&self) -> Option<f64> {
        self.inner.default_time()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPath.n_transitions")]
    #[pyo3(text_signature = "($self)")]
    fn n_transitions(&self) -> usize {
        self.inner.n_transitions()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPath.transitions")]
    #[pyo3(text_signature = "($self)")]
    fn transitions(&self) -> Vec<(f64, usize)> {
        self.inner.transitions().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPath.horizon")]
    #[getter]
    fn horizon(&self) -> f64 {
        self.inner.horizon()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPath.scale")]
    #[getter]
    fn scale(&self) -> PyRatingScale {
        PyRatingScale::from_inner(self.inner.scale().clone())
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPath.__repr__")]
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

wire_methods!(
    PyRatingPath,
    RatingPath,
    "RatingPath",
    module = "finstack_quant.models.credit.migration"
);

#[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPaths")]
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
    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPaths.paths")]
    #[getter]
    fn paths(&self) -> Vec<PyRatingPath> {
        self.inner
            .iter()
            .cloned()
            .map(PyRatingPath::from_inner)
            .collect()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPaths.default_rate")]
    #[getter]
    fn default_rate(&self) -> f64 {
        migration::default_rate(&self.inner)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPaths.to_dataframe")]
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

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPaths.__len__")]
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPaths.__getitem__")]
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

    #[doc = pydoc!("finstack_quant.models.credit.migration", "RatingPaths.__repr__")]
    fn __repr__(&self) -> String {
        format!(
            "RatingPaths(n_paths={}, default_rate={})",
            self.inner.len(),
            self.default_rate()
        )
    }
}

wire_methods!(
    PyRatingPaths,
    Vec<RatingPath>,
    "RatingPaths",
    module = "finstack_quant.models.credit.migration"
);

#[doc = pydoc!("finstack_quant.models.credit.migration", "MigrationSimulator")]
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
    #[doc = pydoc!("finstack_quant.models.credit.migration", "MigrationSimulator.__init__")]
    #[new]
    #[pyo3(text_signature = "(generator, horizon)")]
    fn new(generator: &PyGeneratorMatrix, horizon: f64) -> PyResult<Self> {
        MigrationSimulator::new(generator.inner.clone(), horizon)
            .map(Self::from_inner)
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "MigrationSimulator.simulate")]
    #[pyo3(text_signature = "($self, initial_state, n_paths, seed)")]
    fn simulate(
        &self,
        py: Python<'_>,
        initial_state: usize,
        n_paths: usize,
        seed: u64,
    ) -> PyResult<PyRatingPaths> {
        let paths = py
            .detach(|| self.inner.simulate(initial_state, n_paths, seed))
            .map_err(kinded_to_py)?;
        Ok(PyRatingPaths { inner: paths })
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "MigrationSimulator.empirical_matrix")]
    #[pyo3(text_signature = "($self, n_paths_per_state, seed)")]
    fn empirical_matrix(
        &self,
        py: Python<'_>,
        n_paths_per_state: usize,
        seed: u64,
    ) -> PyResult<PyTransitionMatrix> {
        let matrix = py.detach(|| self.inner.empirical_matrix(n_paths_per_state, seed));
        matrix
            .map(PyTransitionMatrix::from_inner)
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "MigrationSimulator.horizon")]
    #[getter]
    fn horizon(&self) -> f64 {
        self.inner.horizon()
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "MigrationSimulator.generator")]
    #[getter]
    fn generator(&self) -> PyGeneratorMatrix {
        PyGeneratorMatrix::from_inner(self.inner.generator().clone())
    }

    #[doc = pydoc!("finstack_quant.models.credit.migration", "MigrationSimulator.__repr__")]
    fn __repr__(&self) -> String {
        format!(
            "MigrationSimulator(n_states={}, horizon={})",
            self.inner.generator().n_states(),
            self.inner.horizon()
        )
    }
}

wire_methods!(
    PyMigrationSimulator,
    MigrationSimulator,
    "MigrationSimulator",
    module = "finstack_quant.models.credit.migration"
);

#[doc = pydoc!("finstack_quant.models.credit.migration", "project")]
#[pyfunction]
#[pyo3(text_signature = "(generator, t)")]
fn project(generator: &PyGeneratorMatrix, t: f64) -> PyResult<PyTransitionMatrix> {
    migration::project(&generator.inner, t)
        .map(PyTransitionMatrix::from_inner)
        .map_err(kinded_to_py)
}

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "migration")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.credit.migration", "__module__"),
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
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
