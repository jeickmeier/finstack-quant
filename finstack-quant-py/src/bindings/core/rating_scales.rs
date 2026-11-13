//! Python bindings for [`finstack_quant_core::rating_scales`].
//!
//! Exposes the caller-supplied credit rating-scale registry, its entries,
//! scorecard scales and their bands. No scale is embedded and nothing falls
//! back to a default; the classes mirror the Rust types one-for-one and all
//! classification and lookup logic stays in Rust.

use super::config::PyFinstackConfig;
use crate::bindings::macros::wire_methods;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::rating_scales::{
    RATING_SCALES_EXTENSION_KEY, RatingLevel, RatingScaleEntry, RatingScaleRegistry,
    ScorecardScale, registry_from_config,
};
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;
use pyo3::types::{PyIterator, PyList, PyModule};

/// One band of a scorecard scale: a label, its representative score and the
/// band's inclusive lower bound.
///
/// Parameters
/// ----------
/// name : str
///     Band label (``"BBB+"``, ``"Baa1"``); outer whitespace is trimmed and the
///     label must be nonblank and not an unrated status (``NR``, ``NOTRATED``,
///     ``NOT RATED``, ``UNRATED``).
/// score : float
///     Representative score, finite in ``[0, 100]`` and not below *min_score*.
/// min_score : float
///     Inclusive lower bound of the band, finite in ``[0, 100]``.
///
/// Raises
/// ------
/// ValueError
///     If *name* is blank or reserved, a score is non-finite or outside
///     ``[0, 100]``, or *score* is below *min_score*.
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
    /// Construct and validate a band. Raises ``ValueError`` on a blank or
    /// reserved name, a score outside ``[0, 100]``, or *score* below *min_score*.
    #[new]
    #[pyo3(text_signature = "(name, score, min_score)")]
    fn new(name: String, score: f64, min_score: f64) -> PyResult<Self> {
        RatingLevel::try_new(name, score, min_score)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Band label, outer whitespace trimmed.
    #[getter]
    fn name(&self) -> &str {
        self.inner.get_name()
    }

    /// Representative score of the band on the 0-100 scale.
    #[getter]
    fn score(&self) -> f64 {
        self.inner.get_score()
    }

    /// Inclusive lower bound of the band on the 0-100 scale.
    #[getter]
    fn min_score(&self) -> f64 {
        self.inner.get_min_score()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RatingLevel(name={:?}, score={}, min_score={})",
            self.inner.get_name(),
            self.inner.get_score(),
            self.inner.get_min_score()
        )
    }

    /// Structural equality on name, score and min_score.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, PyRatingLevel>>()
            .map(|rhs| self.inner == rhs.inner)
            .unwrap_or(false)
    }
}

wire_methods!(PyRatingLevel, RatingLevel, "RatingLevel");

/// A named best-to-worst list of scorecard bands covering ``[0, 100]``.
///
/// For minima ``m0 > m1 > ... > m_last = 0`` the best band is ``[m0, 100]``
/// and each later band is ``[m_i, m_(i-1))``; every representative score
/// classifies to its own band. Distinct from
/// ``finstack_quant.models.credit.migration.RatingScale``, which models the
/// state set of a migration matrix. Supports ``len(scale)``, iteration and
/// indexing over its ``RatingLevel`` rows.
///
/// Parameters
/// ----------
/// scale_name : str
///     Scale name; outer whitespace is trimmed and it must be nonblank.
/// ratings : list[RatingLevel]
///     Bands ordered best-to-worst; ``score`` and ``min_score`` strictly
///     descend, labels are unique after trim and ASCII case folding, and the
///     worst band's ``min_score`` is ``0``.
/// description : str | None
///     Optional human-readable description.
///
/// Raises
/// ------
/// ValueError
///     If the name is blank, *ratings* is empty, labels collide, the order is
///     not strictly best-to-worst, the worst minimum is not ``0``, or a
///     representative score falls outside its band.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.rating_scales import RatingLevel, ScorecardScale
/// >>> scale = ScorecardScale("custom", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
/// >>> [level.name for level in scale]
/// ['A', 'B']
/// >>> scale.classify(80.0).name
/// 'A'
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
    /// Construct and validate a scorecard scale. Raises ``ValueError`` on a
    /// blank name, empty or colliding bands, a non best-to-worst order, a gap
    /// below the worst band, or a representative outside its band.
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

    /// Scale name, outer whitespace trimmed.
    #[getter]
    fn scale_name(&self) -> &str {
        self.inner.get_scale_name()
    }

    /// Optional human-readable description.
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.get_description()
    }

    /// Bands ordered best to worst.
    #[getter]
    fn ratings(&self) -> Vec<PyRatingLevel> {
        self.inner
            .get_ratings()
            .iter()
            .cloned()
            .map(PyRatingLevel::from_inner)
            .collect()
    }

    /// Classify a score into its band; each boundary belongs to the better
    /// band. Raises ``ValueError`` if *score* is non-finite or outside
    /// ``[0, 100]``.
    #[pyo3(text_signature = "(self, score)")]
    fn classify(&self, score: f64) -> PyResult<PyRatingLevel> {
        self.inner
            .classify(score)
            .map(|level| PyRatingLevel::from_inner(level.clone()))
            .map_err(core_to_py)
    }

    /// Best-to-worst position (``0`` is best) of a band label, matched after
    /// trim and ASCII case folding. Raises ``ValueError`` for an unknown label.
    #[pyo3(text_signature = "(self, label)")]
    fn level_position(&self, label: &str) -> PyResult<usize> {
        self.inner.level_position(label).map_err(core_to_py)
    }

    /// Number of bands on this scale.
    fn __len__(&self) -> usize {
        self.inner.get_ratings().len()
    }

    /// ``scale[i]`` — the ``i``-th rating level (negative indices supported).
    fn __getitem__(&self, index: isize) -> PyResult<PyRatingLevel> {
        let len = self.inner.get_ratings().len() as isize;
        let resolved = if index < 0 { index + len } else { index };
        if resolved < 0 || resolved >= len {
            return Err(PyIndexError::new_err("ScorecardScale index out of range"));
        }
        Ok(PyRatingLevel::from_inner(
            self.inner.get_ratings()[resolved as usize].clone(),
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
        Ok(self.inner == rhs.inner)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ScorecardScale(scale_name={:?}, ratings={})",
            self.inner.get_scale_name(),
            self.inner.get_ratings().len()
        )
    }

    /// Rating levels as a pandas ``DataFrame`` with columns
    /// ``name``, ``score``, ``min_score`` (one row per level, best first).
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::table::envelope_to_pandas(py, &self.inner.to_table())
    }
}

wire_methods!(PyScorecardScale, ScorecardScale, "ScorecardScale");

/// One registered scale with its normalized id and provenance.
///
/// Parameters
/// ----------
/// id : str
///     Scale id; normalized by trim and ASCII lowercasing, nonblank.
/// source : str
///     Provenance of the scale's numbers (policy, owner, vintage); trimmed
///     and nonblank.
/// scale : ScorecardScale
///     The scale definition.
///
/// Raises
/// ------
/// ValueError
///     If the normalized id or the source is blank.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.rating_scales import RatingLevel, RatingScaleEntry, ScorecardScale
/// >>> scale = ScorecardScale("Internal", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
/// >>> RatingScaleEntry(" Internal ", "credit policy 2026", scale).id
/// 'internal'
#[pyclass(
    module = "finstack_quant.core.rating_scales",
    name = "RatingScaleEntry",
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyRatingScaleEntry {
    /// Underlying Rust entry.
    pub(crate) inner: RatingScaleEntry,
}

#[pymethods]
impl PyRatingScaleEntry {
    /// Construct and validate an entry. Raises ``ValueError`` if the
    /// normalized id or the source is blank.
    #[new]
    #[pyo3(text_signature = "(id, source, scale)")]
    fn new(id: String, source: String, scale: PyRef<'_, PyScorecardScale>) -> PyResult<Self> {
        RatingScaleEntry::try_new(id, source, scale.inner.clone())
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Normalized scale id.
    #[getter]
    fn id(&self) -> &str {
        self.inner.get_id()
    }

    /// Provenance of the scale's numbers.
    #[getter]
    fn source(&self) -> &str {
        self.inner.get_source()
    }

    /// The scale definition.
    #[getter]
    fn scale(&self) -> PyScorecardScale {
        PyScorecardScale::from_inner(self.inner.get_scale().clone())
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RatingScaleEntry(id={:?}, source={:?})",
            self.inner.get_id(),
            self.inner.get_source()
        )
    }

    /// Structural equality on id, source and scale.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, PyRatingScaleEntry>>()
            .map(|rhs| self.inner == rhs.inner)
            .unwrap_or(false)
    }
}

/// Caller-supplied registry of scorecard rating scales.
///
/// Build one from entries, ``RatingScaleRegistry.from_json(json)`` or
/// ``registry_from_config(config)``; resolve scales strictly by id with
/// ``rating_scale(id)``. There is no embedded or default registry.
///
/// Parameters
/// ----------
/// rating_scales : list[RatingScaleEntry]
///     Entries in registry order; at least one, with unique ids.
///
/// Raises
/// ------
/// ValueError
///     If the list is empty or ids repeat.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.rating_scales import (
/// ...     RatingLevel, RatingScaleEntry, RatingScaleRegistry, ScorecardScale,
/// ... )
/// >>> scale = ScorecardScale("Internal", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
/// >>> registry = RatingScaleRegistry([RatingScaleEntry("internal", "credit policy 2026", scale)])
/// >>> registry.rating_scale(" INTERNAL ").scale_name
/// 'Internal'
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
    /// Construct and validate a registry. Raises ``ValueError`` if
    /// *rating_scales* is empty or ids repeat.
    #[new]
    #[pyo3(text_signature = "(rating_scales)")]
    fn new(rating_scales: Vec<PyRef<'_, PyRatingScaleEntry>>) -> PyResult<Self> {
        RatingScaleRegistry::try_new(rating_scales.iter().map(|e| e.inner.clone()).collect())
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Registered entries in registry order.
    #[getter]
    fn rating_scales(&self) -> Vec<PyRatingScaleEntry> {
        self.inner
            .get_rating_scales()
            .iter()
            .cloned()
            .map(|inner| PyRatingScaleEntry { inner })
            .collect()
    }

    /// Normalized id of every registered scale, in registry order.
    #[pyo3(text_signature = "(self)")]
    fn scale_ids(&self) -> Vec<String> {
        self.inner
            .scale_ids()
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    /// Return ``True`` if *id* is registered after trim and ASCII lowercasing.
    #[pyo3(text_signature = "(self, id)")]
    fn is_known_rating_scale(&self, id: &str) -> bool {
        self.inner.is_known_rating_scale(id)
    }

    /// Resolve a registered entry by id. Raises ``ValueError`` for an
    /// unknown id; no alias or fallback is applied.
    #[pyo3(text_signature = "(self, id)")]
    fn entry(&self, id: &str) -> PyResult<PyRatingScaleEntry> {
        self.inner
            .entry(id)
            .map(|inner| PyRatingScaleEntry {
                inner: inner.clone(),
            })
            .map_err(core_to_py)
    }

    /// Resolve a registered scale by id. Raises ``ValueError`` for an
    /// unknown id; no alias or fallback is applied.
    #[pyo3(text_signature = "(self, id)")]
    fn rating_scale(&self, id: &str) -> PyResult<PyScorecardScale> {
        self.inner
            .rating_scale(id)
            .map(|scale| PyScorecardScale::from_inner(scale.clone()))
            .map_err(core_to_py)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RatingScaleRegistry(scale_ids={:?})",
            self.inner.scale_ids()
        )
    }

    /// Canonical Rust structural equality.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(rhs) = other.extract::<PyRef<'_, PyRatingScaleRegistry>>() else {
            return Ok(false);
        };
        Ok(self.inner == rhs.inner)
    }

    /// Serialize the registry to a JSON string.
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

/// Load the rating-scale registry held in a ``FinstackConfig``.
///
/// Raises ``ValueError`` if the ``core.rating_scales.v1`` extension is absent
/// or invalid; there is no embedded fallback.
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
        "Caller-supplied credit rating-scale registry (scorecard scales) from finstack-quant-core.",
    )?;

    m.add_class::<PyRatingLevel>()?;
    m.add_class::<PyScorecardScale>()?;
    m.add_class::<PyRatingScaleEntry>()?;
    m.add_class::<PyRatingScaleRegistry>()?;

    m.add_function(wrap_pyfunction!(py_registry_from_config, &m)?)?;

    // Surface the extension key as the single Python entry point.
    m.add("RATING_SCALES_EXTENSION_KEY", RATING_SCALES_EXTENSION_KEY)?;

    let all = PyList::new(
        py,
        [
            "RATING_SCALES_EXTENSION_KEY",
            "RatingLevel",
            "RatingScaleEntry",
            "RatingScaleRegistry",
            "ScorecardScale",
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
