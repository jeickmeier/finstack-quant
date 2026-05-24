//! Python bindings for [`finstack_core::rating_scales`].
//!
//! Exposes the shared credit rating-scale registry (scorecard scales such as
//! S&P / Moody's / Fitch), the rating-level threshold rows, and the
//! [`UnknownScalePolicy`] enum used by scorecards. The classes here mirror the
//! Rust types one-for-one; arithmetic and lookup logic stays in Rust.

use super::config::PyFinstackConfig;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_core::rating_scales::{
    embedded_registry, registry_from_config, RatingLevel, RatingScaleRegistry, ScorecardScale,
    UnknownScalePolicy, RATING_SCALES_EXTENSION_KEY,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyType};

/// Wrapper for [`UnknownScalePolicy`].
#[pyclass(
    module = "finstack.core.rating_scales",
    name = "UnknownScalePolicy",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

    /// Parse a policy name (``error``, ``fallback_to_default``,
    /// ``warn_and_fallback``; case-insensitive).
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        match name.to_ascii_lowercase().as_str() {
            "error" => Ok(Self::from_inner(UnknownScalePolicy::Error)),
            "fallback_to_default" => Ok(Self::from_inner(UnknownScalePolicy::FallbackToDefault)),
            "warn_and_fallback" => Ok(Self::from_inner(UnknownScalePolicy::WarnAndFallback)),
            other => Err(crate::errors::value_error(format!(
                "unknown UnknownScalePolicy variant {other:?}"
            ))),
        }
    }

    /// Canonical snake_case name (matches the serde representation).
    #[getter]
    fn name(&self) -> &'static str {
        match self.inner {
            UnknownScalePolicy::Error => "error",
            UnknownScalePolicy::FallbackToDefault => "fallback_to_default",
            UnknownScalePolicy::WarnAndFallback => "warn_and_fallback",
        }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("UnknownScalePolicy({})", self.name())
    }

    /// Return ``str(self)``.
    fn __str__(&self) -> String {
        self.name().to_string()
    }

    /// Serialize to a JSON string.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "invalid UnknownScalePolicy"))
    }

    /// Deserialize a policy from JSON.
    #[classmethod]
    #[pyo3(text_signature = "(cls, json)")]
    fn from_json(_cls: &Bound<'_, PyType>, json: &str) -> PyResult<Self> {
        let inner: UnknownScalePolicy = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid UnknownScalePolicy JSON"))?;
        Ok(Self::from_inner(inner))
    }
}

/// Wrapper for [`RatingLevel`].
#[pyclass(
    module = "finstack.core.rating_scales",
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
    #[new]
    #[pyo3(text_signature = "(name, score, min_score)")]
    /// Construct a rating level from its name and score thresholds.
    fn new(name: String, score: f64, min_score: f64) -> Self {
        Self::from_inner(RatingLevel {
            name,
            score,
            min_score,
        })
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

    /// Serialize to a JSON string.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "invalid RatingLevel"))
    }

    /// Deserialize a rating level from JSON.
    #[classmethod]
    #[pyo3(text_signature = "(cls, json)")]
    fn from_json(_cls: &Bound<'_, PyType>, json: &str) -> PyResult<Self> {
        let inner: RatingLevel = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid RatingLevel JSON"))?;
        Ok(Self::from_inner(inner))
    }
}

/// Wrapper for [`ScorecardScale`].
#[pyclass(
    module = "finstack.core.rating_scales",
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
    #[new]
    #[pyo3(signature = (scale_name, ratings, description = None))]
    /// Construct a scorecard scale.
    fn new(
        scale_name: String,
        ratings: Vec<PyRef<'_, PyRatingLevel>>,
        description: Option<String>,
    ) -> Self {
        let levels: Vec<RatingLevel> = ratings.iter().map(|r| r.inner.clone()).collect();
        Self::from_inner(ScorecardScale {
            scale_name,
            description,
            ratings: levels,
        })
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
    #[pyo3(text_signature = "(self)")]
    fn get_ratings(&self) -> Vec<PyRatingLevel> {
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

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ScorecardScale(scale_name={:?}, ratings={})",
            self.inner.scale_name,
            self.inner.ratings.len()
        )
    }

    /// Serialize to a JSON string.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "invalid ScorecardScale"))
    }

    /// Deserialize a scorecard scale from JSON.
    #[classmethod]
    #[pyo3(text_signature = "(cls, json)")]
    fn from_json(_cls: &Bound<'_, PyType>, json: &str) -> PyResult<Self> {
        let inner: ScorecardScale = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid ScorecardScale JSON"))?;
        Ok(Self::from_inner(inner))
    }
}

/// Wrapper for [`RatingScaleRegistry`].
#[pyclass(
    module = "finstack.core.rating_scales",
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
    fn get_default_scorecard_score(&self) -> f64 {
        self.inner.default_scorecard_score()
    }

    /// Configured default rating-scale id.
    #[pyo3(text_signature = "(self)")]
    fn get_default_scale_id(&self) -> &str {
        self.inner.default_scale_id()
    }

    /// Configured unknown-scale policy.
    #[pyo3(text_signature = "(self)")]
    fn get_unknown_scale_policy(&self) -> PyUnknownScalePolicy {
        PyUnknownScalePolicy::from_inner(self.inner.unknown_scale_policy())
    }

    /// Return ``True`` if ``name`` is a known scale id or alias.
    #[pyo3(text_signature = "(self, name)")]
    fn is_known_rating_scale(&self, name: &str) -> bool {
        self.inner.is_known_rating_scale(name)
    }

    /// Resolve a scale name or alias to a [`ScorecardScale`].
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

    /// Serialize the registry to a JSON string.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "invalid RatingScaleRegistry"))
    }

    /// Deserialize a registry from JSON. The payload is validated.
    #[classmethod]
    #[pyo3(text_signature = "(cls, json)")]
    fn from_json(_cls: &Bound<'_, PyType>, json: &str) -> PyResult<Self> {
        let inner: RatingScaleRegistry = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid RatingScaleRegistry JSON"))?;
        // Round-trip through the canonical loader so validation runs.
        let validated_json = serde_json::to_string(&inner)
            .map_err(|err| serde_json_to_py(err, "invalid RatingScaleRegistry"))?;
        let validated: RatingScaleRegistry = serde_json::from_str(&validated_json)
            .map_err(|err| serde_json_to_py(err, "invalid RatingScaleRegistry JSON"))?;
        Ok(Self::from_inner(validated))
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

/// Load a rating-scale registry from a [`FinstackConfig`].
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

/// Build the `finstack.core.rating_scales` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "rating_scales")?;
    m.setattr(
        "__doc__",
        "Shared credit rating-scale registry (scorecard scales) from finstack-core.",
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
            "UnknownScalePolicy",
            "RatingLevel",
            "ScorecardScale",
            "RatingScaleRegistry",
            "embedded_registry",
            "registry_from_config",
            "RATING_SCALES_EXTENSION_KEY",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "rating_scales",
        "finstack.core",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
