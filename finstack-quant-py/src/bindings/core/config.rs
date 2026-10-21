//! Python bindings for `finstack_quant_core::config`.

use std::collections::BTreeMap;

use crate::bindings::core::currency::extract_currency;
use crate::bindings::macros::{label_enum, typed_or_label, wire_methods};
use crate::bindings::module_utils::py_to_serde;
use crate::errors::core_to_py;
use finstack_quant_core::config::{FinstackConfig, RoundingMode, ToleranceConfig};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

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
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

typed_or_label!(
    pub(crate) fn extract_rounding_mode -> RoundingMode,
    PyRoundingMode,
    "RoundingMode or its lowercase name (e.g. 'bankers')",
    |label| label.parse().map_err(core_to_py)
);

label_enum!(PyRoundingMode, RoundingMode, "RoundingMode", from_name, {
    /// Banker's rounding (ties to even).
    BANKERS => Bankers,
    /// Round halves away from zero.
    AWAY_FROM_ZERO => AwayFromZero,
    /// Round toward zero (truncate).
    TOWARD_ZERO => TowardZero,
    /// Round toward negative infinity.
    FLOOR => Floor,
    /// Round toward positive infinity.
    CEIL => Ceil,
});
wire_methods!(PyRoundingMode, RoundingMode, "RoundingMode", json_only);

/// Numerical tolerance settings for generic floating-point comparisons.
///
/// Parameters
/// ----------
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
/// >>> ToleranceConfig(generic_epsilon=1e-9).generic_epsilon
/// 1e-09
#[pyclass(
    module = "finstack_quant.core.config",
    name = "ToleranceConfig",
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
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
    /// Create tolerance settings, optionally overriding the default epsilon.
    ///
    /// Raises ``ValueError`` if a supplied epsilon is non-finite or not
    /// strictly positive.
    #[new]
    #[pyo3(signature = (generic_epsilon=None))]
    #[pyo3(text_signature = "(generic_epsilon=None)")]
    fn new(generic_epsilon: Option<f64>) -> PyResult<Self> {
        let defaults = ToleranceConfig::default();
        ToleranceConfig::new(generic_epsilon.unwrap_or(defaults.generic_epsilon))
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Epsilon used for generic floating-point comparisons.
    #[getter]
    fn generic_epsilon(&self) -> f64 {
        self.inner.generic_epsilon
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ToleranceConfig(generic_epsilon={:?})",
            self.inner.generic_epsilon
        )
    }
}

wire_methods!(PyToleranceConfig, ToleranceConfig, "ToleranceConfig");

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

    /// Set a versioned registry/config extension from native Python JSON data. Strings are stored literally.
    #[pyo3(text_signature = "(self, key, value)")]
    fn set_extension(
        &mut self,
        py: Python<'_>,
        key: &str,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let value = py_to_serde::<serde_json::Value>(py, value, "config extension")?;
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

    /// Return one extension as native Python data, or `None` if absent.
    #[pyo3(text_signature = "(self, key)")]
    fn get_extension<'py>(
        &self,
        py: Python<'py>,
        key: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .extensions
            .get(key)
            .map(|value| crate::bindings::pandas_utils::serde_to_py(py, value))
            .transpose()
    }

    /// Canonical Rust structural equality (including numeric signed-zero equality).
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(rhs) = other.extract::<PyRef<'_, PyFinstackConfig>>() else {
            return Ok(false);
        };
        Ok(self.inner == rhs.inner)
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

wire_methods!(PyFinstackConfig, FinstackConfig, "FinstackConfig");

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
