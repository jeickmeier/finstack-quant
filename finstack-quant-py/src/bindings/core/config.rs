//! Python bindings for `finstack_quant_core::config`.

use std::collections::BTreeMap;

use crate::bindings::core::currency::extract_currency;
use crate::bindings::module_utils::py_to_json_value;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::config::{FinstackConfig, RoundingMode, ToleranceConfig};
use finstack_quant_core::Error;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyString, PyType};

/// Rounding mode for monetary and rate calculations.
///
/// Enum-style class with class-level constants for each supported mode:
/// ``BANKERS`` (ties to even, the default), ``AWAY_FROM_ZERO``,
/// ``TOWARD_ZERO``, ``FLOOR`` and ``CEIL``. Wherever a rounding mode is
/// accepted, its exact lowercase name (``"bankers"``, ``"away_from_zero"``,
/// ``"toward_zero"``, ``"floor"``, ``"ceil"``) may be passed instead.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.config import RoundingMode
/// >>> RoundingMode.from_name("bankers") == RoundingMode.BANKERS
/// True
#[pyclass(
    module = "finstack_quant.core.config",
    name = "RoundingMode",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyRoundingMode {
    /// Underlying Rust rounding mode.
    pub(crate) inner: RoundingMode,
}

impl PyRoundingMode {
    /// Build a Python wrapper from a Rust [`RoundingMode`].
    pub(crate) fn from_inner(inner: RoundingMode) -> Self {
        Self { inner }
    }
}

/// Extract a [`RoundingMode`] from a `RoundingMode` instance or its exact
/// lowercase name.
pub(crate) fn extract_rounding_mode(obj: &Bound<'_, PyAny>) -> PyResult<RoundingMode> {
    if let Ok(mode) = obj.extract::<PyRef<'_, PyRoundingMode>>() {
        return Ok(mode.inner);
    }
    if let Ok(text) = obj.cast::<PyString>() {
        return text
            .to_str()?
            .parse::<RoundingMode>()
            .map_err(|e| core_to_py(Error::Validation(e)));
    }
    Err(PyTypeError::new_err(
        "expected RoundingMode or its lowercase name (e.g. 'bankers')",
    ))
}

#[pymethods]
impl PyRoundingMode {
    /// Banker's rounding (ties to even).
    #[classattr]
    const BANKERS: PyRoundingMode = PyRoundingMode {
        inner: RoundingMode::Bankers,
    };
    /// Round halves away from zero.
    #[classattr]
    const AWAY_FROM_ZERO: PyRoundingMode = PyRoundingMode {
        inner: RoundingMode::AwayFromZero,
    };
    /// Round toward zero (truncate).
    #[classattr]
    const TOWARD_ZERO: PyRoundingMode = PyRoundingMode {
        inner: RoundingMode::TowardZero,
    };
    /// Round toward negative infinity.
    #[classattr]
    const FLOOR: PyRoundingMode = PyRoundingMode {
        inner: RoundingMode::Floor,
    };
    /// Round toward positive infinity.
    #[classattr]
    const CEIL: PyRoundingMode = PyRoundingMode {
        inner: RoundingMode::Ceil,
    };

    /// Parse a rounding mode from its exact lowercase label (case-sensitive):
    /// ``"bankers"``, ``"away_from_zero"``, ``"toward_zero"``, ``"floor"``,
    /// ``"ceil"``. Raises ``ValueError`` otherwise.
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        name.parse::<RoundingMode>()
            .map(Self::from_inner)
            .map_err(|e| core_to_py(Error::Validation(e)))
    }

    /// Canonical lowercase name (the serde representation), e.g. ``"bankers"``.
    #[getter]
    fn name(&self) -> String {
        self.inner.to_string()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("RoundingMode({:?})", self.inner.to_string())
    }

    /// Return ``str(self)`` — the lowercase name.
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    /// Serialize to JSON (the quoted lowercase name).
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "invalid RoundingMode"))
    }

    /// Deserialize from JSON (a quoted lowercase name).
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid RoundingMode JSON"))
    }

    /// Support ``pickle`` via the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Numerical tolerance settings for rate and generic comparisons.
///
/// Parameters
/// ----------
/// rate_epsilon : float | None
///     Epsilon for rate-style comparisons (decimal rate units); library
///     default when ``None``.
/// generic_epsilon : float | None
///     Epsilon for generic floating-point comparisons; library default when
///     ``None``.
///
/// Raises
/// ------
/// ValueError
///     If a supplied epsilon is non-finite or not strictly positive.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.config import ToleranceConfig
/// >>> ToleranceConfig(rate_epsilon=1e-9).rate_epsilon
/// 1e-09
#[pyclass(
    module = "finstack_quant.core.config",
    name = "ToleranceConfig",
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PyToleranceConfig {
    /// Underlying Rust tolerance configuration.
    pub(crate) inner: ToleranceConfig,
}

impl PyToleranceConfig {
    /// Build a Python wrapper from a Rust [`ToleranceConfig`].
    pub(crate) fn from_inner(inner: ToleranceConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyToleranceConfig {
    /// Create tolerance settings, optionally overriding the default epsilons.
    ///
    /// Raises ``ValueError`` if a supplied epsilon is non-finite or not
    /// strictly positive.
    #[new]
    #[pyo3(signature = (rate_epsilon=None, generic_epsilon=None))]
    #[pyo3(text_signature = "(rate_epsilon=None, generic_epsilon=None)")]
    fn new(rate_epsilon: Option<f64>, generic_epsilon: Option<f64>) -> PyResult<Self> {
        let defaults = ToleranceConfig::default();
        ToleranceConfig::new(
            rate_epsilon.unwrap_or(defaults.rate_epsilon),
            generic_epsilon.unwrap_or(defaults.generic_epsilon),
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
    }

    /// Epsilon used for rate-style comparisons.
    #[getter]
    fn rate_epsilon(&self) -> f64 {
        self.inner.rate_epsilon
    }

    /// Epsilon used for generic floating-point comparisons.
    #[getter]
    fn generic_epsilon(&self) -> f64 {
        self.inner.generic_epsilon
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ToleranceConfig(rate_epsilon={:?}, generic_epsilon={:?})",
            self.inner.rate_epsilon, self.inner.generic_epsilon
        )
    }

    /// Serialize to JSON.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "invalid ToleranceConfig"))
    }

    /// Deserialize from JSON; epsilons are re-validated.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let parsed: ToleranceConfig = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid ToleranceConfig JSON"))?;
        ToleranceConfig::new(parsed.rate_epsilon, parsed.generic_epsilon)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Support ``pickle`` via the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Top-level library configuration: rounding policy, per-currency scale
/// overrides, comparison tolerances and versioned extensions.
///
/// Parameters
/// ----------
/// rounding_mode : RoundingMode | str | None
///     Rounding mode override (object or exact lowercase name); library
///     default (bankers) when ``None``.
/// tolerances : ToleranceConfig | None
///     Tolerance override; library default when ``None``.
///
/// Raises
/// ------
/// ValueError
///     If *rounding_mode* is a string that is not a recognised mode name.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.config import FinstackConfig
/// >>> cfg = FinstackConfig(rounding_mode="floor")
/// >>> cfg.set_output_scale("JPY", 2)
/// >>> (cfg.rounding_mode.name, cfg.output_scale("JPY"), cfg.output_scale_overrides())
/// ('floor', 2, {'JPY': 2})
#[pyclass(
    module = "finstack_quant.core.config",
    name = "FinstackConfig",
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFinstackConfig {
    /// Underlying Rust configuration.
    pub(crate) inner: FinstackConfig,
}

impl PyFinstackConfig {
    /// Build a Python wrapper from a Rust [`FinstackConfig`].
    pub(crate) fn from_inner(inner: FinstackConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFinstackConfig {
    /// Create a configuration, optionally overriding the rounding mode
    /// (``RoundingMode`` or its lowercase name) and tolerances.
    #[new]
    #[pyo3(signature = (rounding_mode=None, tolerances=None))]
    #[pyo3(text_signature = "(rounding_mode=None, tolerances=None)")]
    fn new(
        rounding_mode: Option<&Bound<'_, PyAny>>,
        tolerances: Option<PyRef<PyToleranceConfig>>,
    ) -> PyResult<Self> {
        let mut inner = FinstackConfig::default();
        if let Some(rm) = rounding_mode {
            inner.rounding.mode = extract_rounding_mode(rm)?;
        }
        if let Some(t) = tolerances {
            inner.tolerances = t.inner;
        }
        Ok(Self { inner })
    }

    /// Active rounding mode.
    #[getter]
    fn rounding_mode(&self) -> PyRoundingMode {
        PyRoundingMode::from_inner(self.inner.rounding.mode)
    }

    /// Comparison tolerances.
    #[getter]
    fn tolerances(&self) -> PyToleranceConfig {
        PyToleranceConfig::from_inner(self.inner.tolerances)
    }

    /// Effective output decimal scale for ``currency`` (``Currency`` or ISO code).
    ///
    /// Falls back to the currency's ISO-4217 minor units when no override is set.
    #[pyo3(text_signature = "(self, currency)")]
    fn output_scale(&self, currency: &Bound<'_, PyAny>) -> PyResult<u32> {
        Ok(self.inner.output_scale(extract_currency(currency)?))
    }

    /// Effective ingest decimal scale for ``currency`` (``Currency`` or ISO code).
    ///
    /// Falls back to ``max(6, minor units)`` when no override is set.
    #[pyo3(text_signature = "(self, currency)")]
    fn ingest_scale(&self, currency: &Bound<'_, PyAny>) -> PyResult<u32> {
        Ok(self.inner.ingest_scale(extract_currency(currency)?))
    }

    /// Override the output decimal scale for ``currency`` within ``0..=28``.
    ///
    /// Raises ``ValueError`` for an unknown currency or unsupported scale,
    /// leaving the configuration unchanged.
    #[pyo3(text_signature = "(self, currency, scale)")]
    fn set_output_scale(&mut self, currency: &Bound<'_, PyAny>, scale: u32) -> PyResult<()> {
        let ccy = extract_currency(currency)?;
        self.inner
            .rounding
            .output_scale
            .set_scale(ccy, scale)
            .map_err(core_to_py)
    }

    /// Override the ingest decimal scale for ``currency`` within ``0..=28``.
    ///
    /// Raises ``ValueError`` for an unknown currency or unsupported scale,
    /// leaving the configuration unchanged.
    #[pyo3(text_signature = "(self, currency, scale)")]
    fn set_ingest_scale(&mut self, currency: &Bound<'_, PyAny>, scale: u32) -> PyResult<()> {
        let ccy = extract_currency(currency)?;
        self.inner
            .rounding
            .ingest_scale
            .set_scale(ccy, scale)
            .map_err(core_to_py)
    }

    /// Explicit output-scale overrides as ``{iso_code: scale}``.
    #[pyo3(text_signature = "(self)")]
    fn output_scale_overrides(&self) -> BTreeMap<String, u32> {
        self.inner
            .rounding
            .output_scale
            .get_overrides()
            .iter()
            .map(|(ccy, scale)| (ccy.to_string(), *scale))
            .collect()
    }

    /// Explicit ingest-scale overrides as ``{iso_code: scale}``.
    #[pyo3(text_signature = "(self)")]
    fn ingest_scale_overrides(&self) -> BTreeMap<String, u32> {
        self.inner
            .rounding
            .ingest_scale
            .get_overrides()
            .iter()
            .map(|(ccy, scale)| (ccy.to_string(), *scale))
            .collect()
    }

    /// Set a versioned registry/config extension from a Python dict/list or JSON string.
    #[pyo3(text_signature = "(self, key, value)")]
    fn set_extension(
        &mut self,
        py: Python<'_>,
        key: &str,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let value = py_to_json_value(py, value, "config extension")?;
        self.inner
            .extensions
            .insert(key, value)
            .map_err(core_to_py)?;
        Ok(())
    }

    /// Remove a versioned registry/config extension.
    #[pyo3(text_signature = "(self, key)")]
    fn remove_extension(&mut self, key: &str) -> bool {
        self.inner.extensions.remove(key).is_some()
    }

    /// Return configured extension keys.
    #[pyo3(text_signature = "(self)")]
    fn extension_keys(&self) -> Vec<String> {
        self.inner.extensions.keys().map(str::to_string).collect()
    }

    /// Return one extension as a JSON string, or `None` if absent.
    #[pyo3(text_signature = "(self, key)")]
    fn get_extension_json(&self, key: &str) -> PyResult<Option<String>> {
        self.inner
            .extensions
            .get(key)
            .map(|value| {
                serde_json::to_string(value)
                    .map_err(|err| serde_json_to_py(err, "invalid extension JSON"))
            })
            .transpose()
    }

    /// Return one extension as native Python data, or `None` if absent.
    #[pyo3(text_signature = "(self, key)")]
    fn get_extension<'py>(
        &self,
        py: Python<'py>,
        key: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let Some(json) = self.get_extension_json(key)? else {
            return Ok(None);
        };
        let json_mod = py.import("json")?;
        json_mod.call_method1("loads", (json,)).map(Some)
    }

    /// Serialize this config, including extensions, to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "failed to serialize config"))
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

    /// Deserialize a config from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid FinstackConfig JSON"))
    }

    /// Structural equality via the JSON wire form (rounding, scale overrides,
    /// tolerances and extensions).
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(rhs) = other.extract::<PyRef<'_, PyFinstackConfig>>() else {
            return Ok(false);
        };
        Ok(self.to_json()? == rhs.to_json()?)
    }

    /// Return ``repr(self)`` showing the rounding mode and override counts.
    fn __repr__(&self) -> String {
        format!(
            "FinstackConfig(rounding_mode={:?}, output_scale_overrides={}, ingest_scale_overrides={}, extensions={})",
            self.inner.rounding.mode.to_string(),
            self.inner.rounding.output_scale.get_overrides().len(),
            self.inner.rounding.ingest_scale.get_overrides().len(),
            self.inner.extensions.keys().count()
        )
    }
}

/// Register the `finstack_quant.core.config` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "config")?;
    m.setattr(
        "__doc__",
        "Configuration types from finstack-quant-core (rounding, tolerances, FinstackConfig).",
    )?;

    m.add_class::<PyRoundingMode>()?;
    m.add_class::<PyToleranceConfig>()?;
    m.add_class::<PyFinstackConfig>()?;
    let all = PyList::new(py, ["FinstackConfig", "RoundingMode", "ToleranceConfig"])?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
