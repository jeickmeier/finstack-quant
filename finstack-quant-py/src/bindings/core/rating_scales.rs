//! Python bindings for [`finstack_quant_core::rating_scales`].
//!
//! Exposes the shared credit rating-scale registry (scorecard scales such as
//! S&P / Moody's / Fitch), the rating-level threshold rows, and the
//! [`UnknownScalePolicy`] enum used by scorecards. The classes here mirror the
//! Rust types one-for-one; arithmetic and lookup logic stays in Rust.

use super::config::PyFinstackConfig;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::rating_scales::{
    embedded_registry, registry_from_config, RatingLevel, RatingScaleRegistry, ScorecardScale,
    UnknownScalePolicy, RATING_SCALES_EXTENSION_KEY,
};
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;
use pyo3::types::{PyIterator, PyList, PyModule, PyType};

/// Policy applied when a scorecard names an unknown rating scale.
///
/// Enum-style class: ``ERROR`` rejects unknown names, ``FALLBACK_TO_DEFAULT``
/// resolves them to the registry's default scale, and ``WARN_AND_FALLBACK``
/// does the same while leaving warning emission to the caller.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.rating_scales import UnknownScalePolicy
/// >>> UnknownScalePolicy.from_name("error") == UnknownScalePolicy.ERROR
/// True
#[pyclass(
    module = "finstack_quant.core.rating_scales",
    name = "UnknownScalePolicy",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct PyUnknownScalePolicy {
    /// Underlying Rust policy variant.
    pub(crate) inner: UnknownScalePolicy,
}

impl PyUnknownScalePolicy {
    /// Build a Python wrapper from a Rust [`UnknownScalePolicy`].
    pub(crate) const fn from_inner(inner: UnknownScalePolicy) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyUnknownScalePolicy {
    /// Reject unknown scale names.
    #[classattr]
    const ERROR: PyUnknownScalePolicy = PyUnknownScalePolicy {
        inner: UnknownScalePolicy::Error,
    };
    /// Fall back to the configured default scale for unknown names.
    #[classattr]
    const FALLBACK_TO_DEFAULT: PyUnknownScalePolicy = PyUnknownScalePolicy {
        inner: UnknownScalePolicy::FallbackToDefault,
    };
    /// Fall back to the default scale and let callers emit a warning.
    #[classattr]
    const WARN_AND_FALLBACK: PyUnknownScalePolicy = PyUnknownScalePolicy {
        inner: UnknownScalePolicy::WarnAndFallback,
    };

    /// Parse a policy from its exact lowercase snake_case name
    /// (case-sensitive): ``"error"``, ``"fallback_to_default"``,
    /// ``"warn_and_fallback"``. Raises ``ValueError`` otherwise.
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        finstack_quant_core::wire::serde_parse(name)
            .map(Self::from_inner)
            .map_err(crate::errors::core_to_py)
    }

    /// Canonical snake_case name (the serde representation).
    #[getter]
    fn name(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner).map_err(crate::errors::core_to_py)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "UnknownScalePolicy({:?})",
            self.name().unwrap_or_else(|_| "?".to_string())
        )
    }

    /// Return ``str(self)``.
    fn __str__(&self) -> String {
        self.name().unwrap_or_else(|_| "?".to_string())
    }

    /// Serialize to a JSON string.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "invalid UnknownScalePolicy"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a policy from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        UnknownScalePolicy::from_json(json)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }
}

/// A single rating threshold row on a scorecard scale.
///
/// Parameters
/// ----------
/// name : str
///     Rating label (``"BBB+"``, ``"Baa1"``); must not be blank.
/// score : float
///     Representative score on the inclusive 0-100 scorecard scale.
/// min_score : float
///     Minimum score (inclusive 0-100) that qualifies for this rating.
///
/// Raises
/// ------
/// ValueError
///     If *name* is blank or either score is non-finite or outside 0-100.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.rating_scales import RatingLevel
/// >>> RatingLevel("BBB", 70.0, 65.0).min_score
/// 65.0
#[pyclass(
    module = "finstack_quant.core.rating_scales",
    name = "RatingLevel",
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyRatingLevel {
    /// Underlying Rust rating level.
    pub(crate) inner: RatingLevel,
}

impl PyRatingLevel {
    /// Build a Python wrapper from a Rust [`RatingLevel`].
    pub(crate) fn from_inner(inner: RatingLevel) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRatingLevel {
    /// Construct and validate a rating level from its name and score
    /// thresholds. Raises ``ValueError`` on a blank name or a score outside
    /// the inclusive 0-100 range.
    #[new]
    #[pyo3(text_signature = "(name, score, min_score)")]
    fn new(name: String, score: f64, min_score: f64) -> PyResult<Self> {
        RatingLevel::try_new(name, score, min_score)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Rating name (e.g. ``"AAA"`` or ``"Aaa"``).
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Numeric score on the 0-100 scorecard scale.
    #[getter]
    fn score(&self) -> f64 {
        self.inner.score
    }

    /// Minimum score threshold for this rating.
    #[getter]
    fn min_score(&self) -> f64 {
        self.inner.min_score
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RatingLevel(name={:?}, score={}, min_score={})",
            self.inner.name, self.inner.score, self.inner.min_score
        )
    }

    /// Structural equality on name, score and min_score.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, PyRatingLevel>>()
            .map(|rhs| {
                self.inner.name == rhs.inner.name
                    && self.inner.score == rhs.inner.score
                    && self.inner.min_score == rhs.inner.min_score
            })
            .unwrap_or(false)
    }

    /// Serialize to a JSON string.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "invalid RatingLevel"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a rating level from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        RatingLevel::from_json(json)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }
}

/// A named, ordered (best-to-worst) list of scorecard rating thresholds.
///
/// Distinct from ``finstack_quant.models.credit.migration.RatingScale``, which
/// models the state set of a migration matrix. Supports ``len(scale)``,
/// iteration and indexing over its ``RatingLevel`` rows.
///
/// Parameters
/// ----------
/// scale_name : str
///     Scale identifier (``"S&P"``, ``"Moody's"``).
/// ratings : list[RatingLevel]
///     Levels ordered best-to-worst; ``score`` and ``min_score`` must strictly
///     descend and names must be unique.
/// description : str | None
///     Optional human-readable description.
///
/// Raises
/// ------
/// ValueError
///     If *ratings* is empty, contains duplicate names, or is not strictly
///     ordered best-to-worst.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.rating_scales import RatingLevel, ScorecardScale
/// >>> scale = ScorecardScale("custom", [RatingLevel("A", 90.0, 85.0), RatingLevel("B", 70.0, 65.0)])
/// >>> [level.name for level in scale]
/// ['A', 'B']
#[pyclass(
    module = "finstack_quant.core.rating_scales",
    name = "ScorecardScale",
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyScorecardScale {
    /// Underlying Rust scorecard scale.
    pub(crate) inner: ScorecardScale,
}

impl PyScorecardScale {
    /// Build a Python wrapper from a Rust [`ScorecardScale`].
    pub(crate) fn from_inner(inner: ScorecardScale) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyScorecardScale {
    /// Construct and validate a scorecard scale. Raises ``ValueError`` if
    /// ``ratings`` is empty, has duplicate names, or is not strictly ordered
    /// best-to-worst.
    #[new]
    #[pyo3(signature = (scale_name, ratings, description = None))]
    #[pyo3(text_signature = "(scale_name, ratings, description=None)")]
    fn new(
        scale_name: String,
        ratings: Vec<PyRef<'_, PyRatingLevel>>,
        description: Option<String>,
    ) -> PyResult<Self> {
        let levels: Vec<RatingLevel> = ratings.iter().map(|r| r.inner.clone()).collect();
        ScorecardScale::try_new(scale_name, description, levels)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Scale name (e.g. ``"S&P"`` or ``"Moody's"``).
    #[getter]
    fn scale_name(&self) -> &str {
        &self.inner.scale_name
    }

    /// Optional human-readable description.
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    /// Ordered list of rating levels from best to worst.
    #[getter]
    fn ratings(&self) -> Vec<PyRatingLevel> {
        self.inner
            .ratings
            .iter()
            .cloned()
            .map(PyRatingLevel::from_inner)
            .collect()
    }

    /// Number of rating levels on this scale.
    fn __len__(&self) -> usize {
        self.inner.ratings.len()
    }

    /// ``scale[i]`` — the ``i``-th rating level (negative indices supported).
    fn __getitem__(&self, index: isize) -> PyResult<PyRatingLevel> {
        let len = self.inner.ratings.len() as isize;
        let resolved = if index < 0 { index + len } else { index };
        if resolved < 0 || resolved >= len {
            return Err(PyIndexError::new_err("ScorecardScale index out of range"));
        }
        Ok(PyRatingLevel::from_inner(
            self.inner.ratings[resolved as usize].clone(),
        ))
    }

    /// Iterate over rating levels best-to-worst.
    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyIterator>> {
        let levels = PyList::new(py, self.ratings())?;
        levels.into_any().try_iter()
    }

    /// Structural equality on name, description and ratings.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(rhs) = other.extract::<PyRef<'_, PyScorecardScale>>() else {
            return Ok(false);
        };
        Ok(self.to_json()? == rhs.to_json()?)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ScorecardScale(scale_name={:?}, ratings={})",
            self.inner.scale_name,
            self.inner.ratings.len()
        )
    }

    /// Rating levels as a pandas ``DataFrame`` with columns
    /// ``name``, ``score``, ``min_score`` (one row per level, best first).
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema(
            py,
            &self.inner.ratings,
            &[
                ("name", "str"),
                ("score", "float64"),
                ("min_score", "float64"),
            ],
        )
    }

    /// Serialize to a JSON string.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "invalid ScorecardScale"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a scorecard scale from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        ScorecardScale::from_json(json)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }
}

/// Versioned registry of scorecard rating scales and defaults.
///
/// Obtain one via ``embedded_registry()`` (the bundled default) or
/// ``registry_from_config(config)``; resolve scales by id or alias with
/// ``rating_scale(name)``.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.rating_scales import embedded_registry
/// >>> registry = embedded_registry()
/// >>> registry.rating_scale("Fitch").scale_name
/// 'S&P'
#[pyclass(
    module = "finstack_quant.core.rating_scales",
    name = "RatingScaleRegistry",
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyRatingScaleRegistry {
    /// Underlying Rust registry.
    pub(crate) inner: RatingScaleRegistry,
}

impl PyRatingScaleRegistry {
    /// Build a Python wrapper from a Rust [`RatingScaleRegistry`].
    pub(crate) fn from_inner(inner: RatingScaleRegistry) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRatingScaleRegistry {
    /// Configured default scorecard score for threshold gaps.
    #[pyo3(text_signature = "(self)")]
    fn default_scorecard_score(&self) -> f64 {
        self.inner.default_scorecard_score()
    }

    /// Configured default rating-scale id.
    #[pyo3(text_signature = "(self)")]
    fn default_scale_id(&self) -> &str {
        self.inner.default_scale_id()
    }

    /// Configured unknown-scale policy.
    #[pyo3(text_signature = "(self)")]
    fn unknown_scale_policy(&self) -> PyUnknownScalePolicy {
        PyUnknownScalePolicy::from_inner(self.inner.unknown_scale_policy())
    }

    /// Primary id of every registered scale, in registry order (aliases excluded).
    #[pyo3(text_signature = "(self)")]
    fn scale_ids(&self) -> Vec<String> {
        self.inner
            .scale_ids()
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    /// Return ``True`` if ``name`` is a known scale id or alias.
    #[pyo3(text_signature = "(self, name)")]
    fn is_known_rating_scale(&self, name: &str) -> bool {
        self.inner.is_known_rating_scale(name)
    }

    /// Resolve a scale name or alias to a ``ScorecardScale``.
    ///
    /// Honours the registry's unknown-scale policy: depending on the policy
    /// this may fall back to the default scale or raise ``ValueError``.
    #[pyo3(text_signature = "(self, name)")]
    fn rating_scale(&self, name: &str) -> PyResult<PyScorecardScale> {
        self.inner
            .rating_scale(name)
            .map(|scale| PyScorecardScale::from_inner(scale.clone()))
            .map_err(core_to_py)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RatingScaleRegistry(default_scale_id={:?}, default_score={})",
            self.inner.default_scale_id(),
            self.inner.default_scorecard_score()
        )
    }

    /// Structural equality via the JSON wire form.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(rhs) = other.extract::<PyRef<'_, PyRatingScaleRegistry>>() else {
            return Ok(false);
        };
        Ok(self.to_json()? == rhs.to_json()?)
    }

    /// Serialize the registry to a JSON string.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "invalid RatingScaleRegistry"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a registry from JSON. The payload is validated.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        RatingScaleRegistry::from_json(json)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }
}

/// Return the embedded (built-in) rating-scale registry.
#[pyfunction]
#[pyo3(name = "embedded_registry", text_signature = "()")]
fn py_embedded_registry() -> PyResult<PyRatingScaleRegistry> {
    embedded_registry()
        .map(|reg| PyRatingScaleRegistry::from_inner(reg.clone()))
        .map_err(core_to_py)
}

/// Load a rating-scale registry from a ``FinstackConfig``.
///
/// Falls back to the embedded registry when the config does not override the
/// ``core.rating_scales.v1`` extension key.
#[pyfunction]
#[pyo3(name = "registry_from_config", text_signature = "(config)")]
fn py_registry_from_config(config: PyRef<'_, PyFinstackConfig>) -> PyResult<PyRatingScaleRegistry> {
    registry_from_config(&config.inner)
        .map(PyRatingScaleRegistry::from_inner)
        .map_err(core_to_py)
}

/// Build the `finstack_quant.core.rating_scales` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "rating_scales")?;
    m.setattr(
        "__doc__",
        "Shared credit rating-scale registry (scorecard scales) from finstack-quant-core.",
    )?;

    m.add_class::<PyUnknownScalePolicy>()?;
    m.add_class::<PyRatingLevel>()?;
    m.add_class::<PyScorecardScale>()?;
    m.add_class::<PyRatingScaleRegistry>()?;

    m.add_function(wrap_pyfunction!(py_embedded_registry, &m)?)?;
    m.add_function(wrap_pyfunction!(py_registry_from_config, &m)?)?;

    // Surface the extension key as the single Python entry point.
    m.add("RATING_SCALES_EXTENSION_KEY", RATING_SCALES_EXTENSION_KEY)?;

    let all = PyList::new(
        py,
        [
            "RATING_SCALES_EXTENSION_KEY",
            "RatingLevel",
            "RatingScaleRegistry",
            "ScorecardScale",
            "UnknownScalePolicy",
            "embedded_registry",
            "registry_from_config",
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
