//! Python bindings for `finstack_quant_core::credit::migration`.

use finstack_quant_core::credit::migration::{
    projection, GeneratorMatrix, MigrationSimulator, RatingPath, RatingScale, TransitionMatrix,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use rand::SeedableRng;
use rand_pcg::Pcg64;

use crate::errors::migration_to_py;

fn matrix_rows(data: &nalgebra::DMatrix<f64>) -> Vec<Vec<f64>> {
    (0..data.nrows())
        .map(|row| (0..data.ncols()).map(|col| data[(row, col)]).collect())
        .collect()
}

#[pyclass(
    module = "finstack_quant.core.credit.migration",
    name = "RatingScale",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyRatingScale {
    inner: RatingScale,
}

impl PyRatingScale {
    fn from_inner(inner: RatingScale) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRatingScale {
    #[staticmethod]
    fn standard() -> Self {
        Self::from_inner(RatingScale::standard())
    }

    #[staticmethod]
    fn standard_with_nr() -> Self {
        Self::from_inner(RatingScale::standard_with_nr())
    }

    #[staticmethod]
    fn notched() -> Self {
        Self::from_inner(RatingScale::notched())
    }

    #[staticmethod]
    fn custom(labels: Vec<String>) -> PyResult<Self> {
        RatingScale::custom(labels)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    #[staticmethod]
    fn custom_with_default(labels: Vec<String>, default_label: String) -> PyResult<Self> {
        RatingScale::custom_with_default(labels, default_label)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    fn n_states(&self) -> usize {
        self.inner.n_states()
    }

    fn index_of(&self, label: &str) -> Option<usize> {
        self.inner.index_of(label)
    }

    fn default_state(&self) -> Option<usize> {
        self.inner.default_state()
    }

    fn labels(&self) -> Vec<String> {
        self.inner.labels().to_vec()
    }

    fn warf(&self, label: &str) -> PyResult<f64> {
        self.inner.warf(label).map_err(migration_to_py)
    }

    fn rating_from_warf(&self, warf: f64) -> PyResult<String> {
        self.inner
            .rating_from_warf(warf)
            .map(str::to_owned)
            .map_err(migration_to_py)
    }
}

#[pyclass(
    module = "finstack_quant.core.credit.migration",
    name = "TransitionMatrix",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyTransitionMatrix {
    inner: TransitionMatrix,
}

impl PyTransitionMatrix {
    fn from_inner(inner: TransitionMatrix) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyTransitionMatrix {
    #[new]
    fn new(scale: &PyRatingScale, data: Vec<f64>, horizon: f64) -> PyResult<Self> {
        TransitionMatrix::new(scale.inner.clone(), &data, horizon)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    fn probability(&self, from: &str, to: &str) -> PyResult<f64> {
        self.inner.probability(from, to).map_err(migration_to_py)
    }

    fn row(&self, from: &str) -> PyResult<Vec<f64>> {
        self.inner.row(from).map_err(migration_to_py)
    }

    fn to_matrix(&self) -> Vec<Vec<f64>> {
        matrix_rows(self.inner.as_matrix())
    }

    fn horizon(&self) -> f64 {
        self.inner.horizon()
    }

    fn n_states(&self) -> usize {
        self.inner.n_states()
    }

    fn default_probabilities(&self) -> Option<Vec<f64>> {
        self.inner.default_probabilities()
    }
}

#[pyclass(
    module = "finstack_quant.core.credit.migration",
    name = "GeneratorMatrix",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyGeneratorMatrix {
    inner: GeneratorMatrix,
}

impl PyGeneratorMatrix {
    fn from_inner(inner: GeneratorMatrix) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyGeneratorMatrix {
    #[new]
    fn new(scale: &PyRatingScale, data: Vec<f64>) -> PyResult<Self> {
        GeneratorMatrix::new(scale.inner.clone(), &data)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    #[staticmethod]
    fn from_transition_matrix(p: &PyTransitionMatrix) -> PyResult<Self> {
        GeneratorMatrix::from_transition_matrix(&p.inner)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    fn intensity(&self, from: &str, to: &str) -> PyResult<f64> {
        self.inner.intensity(from, to).map_err(migration_to_py)
    }

    fn exit_rate(&self, state: &str) -> PyResult<f64> {
        self.inner.exit_rate(state).map_err(migration_to_py)
    }

    fn to_matrix(&self) -> Vec<Vec<f64>> {
        matrix_rows(self.inner.as_matrix())
    }

    fn n_states(&self) -> usize {
        self.inner.n_states()
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
}

#[pyclass(
    module = "finstack_quant.core.credit.migration",
    name = "RatingPath",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyRatingPath {
    inner: RatingPath,
}

impl PyRatingPath {
    fn from_inner(inner: RatingPath) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRatingPath {
    fn state_at(&self, t: f64) -> usize {
        self.inner.state_at(t)
    }

    fn label_at(&self, t: f64) -> String {
        self.inner.label_at(t).to_owned()
    }

    fn defaulted(&self) -> bool {
        self.inner.defaulted()
    }

    fn default_time(&self) -> Option<f64> {
        self.inner.default_time()
    }

    fn n_transitions(&self) -> usize {
        self.inner.n_transitions()
    }

    fn transitions(&self) -> Vec<(f64, usize)> {
        self.inner.transitions().to_vec()
    }

    fn horizon(&self) -> f64 {
        self.inner.horizon()
    }
}

#[pyclass(
    module = "finstack_quant.core.credit.migration",
    name = "MigrationSimulator",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyMigrationSimulator {
    inner: MigrationSimulator,
}

impl PyMigrationSimulator {
    fn from_inner(inner: MigrationSimulator) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyMigrationSimulator {
    #[new]
    fn new(generator: &PyGeneratorMatrix, horizon: f64) -> PyResult<Self> {
        MigrationSimulator::new(generator.inner.clone(), horizon)
            .map(Self::from_inner)
            .map_err(migration_to_py)
    }

    /// Simulate rating paths from `initial_state`.
    ///
    /// Determinism: paths are generated with the canonical `Pcg64` RNG
    /// (rand_pcg) seeded from `seed`, matching the Rust simulator's
    /// reference RNG; identical seeds reproduce identical paths.
    /// Releases the GIL (detaches) during simulation.
    fn simulate(
        &self,
        py: Python<'_>,
        initial_state: usize,
        n_paths: usize,
        seed: u64,
    ) -> PyResult<Vec<PyRatingPath>> {
        let paths = py
            .detach(|| {
                let mut rng = Pcg64::seed_from_u64(seed);
                self.inner.simulate(initial_state, n_paths, &mut rng)
            })
            .map_err(migration_to_py)?;
        Ok(paths.into_iter().map(PyRatingPath::from_inner).collect())
    }

    /// Build an empirical transition matrix by simulation.
    ///
    /// Uses the canonical seeded `Pcg64` RNG (see [`Self::simulate`]) and
    /// releases the GIL during simulation.
    fn empirical_matrix(
        &self,
        py: Python<'_>,
        n_paths_per_state: usize,
        seed: u64,
    ) -> PyResult<PyTransitionMatrix> {
        let matrix = py.detach(|| {
            let mut rng = Pcg64::seed_from_u64(seed);
            self.inner.empirical_matrix(n_paths_per_state, &mut rng)
        });
        matrix
            .map(PyTransitionMatrix::from_inner)
            .map_err(migration_to_py)
    }

    fn horizon(&self) -> f64 {
        self.inner.horizon()
    }
}

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
    m.add_class::<PyMigrationSimulator>()?;
    m.add_function(wrap_pyfunction!(project, &m)?)?;

    let all = PyList::new(
        py,
        [
            "RatingScale",
            "TransitionMatrix",
            "GeneratorMatrix",
            "RatingPath",
            "MigrationSimulator",
            "project",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "migration",
        "finstack_quant.core.credit",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
