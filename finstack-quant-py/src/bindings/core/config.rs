//! Python bindings for `finstack_quant_core::config`.

use crate::bindings::module_utils::py_to_json_value;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::config::{FinstackConfig, RoundingMode, ToleranceConfig};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Error;
use finstack_quant_core::InputError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyType};

/// Wrapper for [`RoundingMode`].
#[pyclass(
    module = "finstack_quant.core.config",
    name = "RoundingMode",
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    /// Parse a rounding mode from a human-readable label (case-insensitive).
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        name.parse::<RoundingMode>()
            .map(Self::from_inner)
            .map_err(|e| core_to_py(Error::Validation(e)))
    }

    fn __repr__(&self) -> String {
        format!("RoundingMode({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __hash__(&self) -> isize {
        self.discriminant() as isize
    }

    fn __eq__(&self, other: Bound<'_, PyAny>) -> PyResult<bool> {
        match other.extract::<PyRef<Self>>() {
            Ok(o) => Ok(self.inner == o.inner),
            Err(_) => Ok(false),
        }
    }
}

impl PyRoundingMode {
    fn discriminant(self) -> u8 {
        match self.inner {
            RoundingMode::Bankers => 0,
            RoundingMode::AwayFromZero => 1,
            RoundingMode::TowardZero => 2,
            RoundingMode::Floor => 3,
            RoundingMode::Ceil => 4,
            #[allow(unreachable_patterns)]
            _ => 255,
        }
    }
}

/// Wrapper for [`ToleranceConfig`].
#[pyclass(
    module = "finstack_quant.core.config",
    name = "ToleranceConfig",
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PyToleranceConfig {
    /// Underlying Rust tolerance configuration.
    pub(crate) inner: ToleranceConfig,
}

impl PyToleranceConfig {
    /// Build a Python wrapper from a Rust [`ToleranceConfig`].
    #[allow(dead_code)]
    pub(crate) fn from_inner(inner: ToleranceConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyToleranceConfig {
    #[new]
    #[pyo3(signature = (rate_epsilon=None, generic_epsilon=None))]
    #[pyo3(text_signature = "(rate_epsilon=None, generic_epsilon=None)")]
    /// Create tolerance settings, optionally overriding default epsilons.
    fn new(rate_epsilon: Option<f64>, generic_epsilon: Option<f64>) -> PyResult<Self> {
        let defaults = ToleranceConfig::default();
        ToleranceConfig::new(
            rate_epsilon.unwrap_or(defaults.rate_epsilon),
            generic_epsilon.unwrap_or(defaults.generic_epsilon),
        )
        .map(|inner| Self { inner })
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

    fn __repr__(&self) -> String {
        format!(
            "ToleranceConfig(rate_epsilon={:?}, generic_epsilon={:?})",
            self.inner.rate_epsilon, self.inner.generic_epsilon
        )
    }
}

/// Wrapper for [`FinstackConfig`].
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
    #[allow(dead_code)]
    pub(crate) fn from_inner(inner: FinstackConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFinstackConfig {
    #[new]
    #[pyo3(signature = (rounding_mode=None, tolerances=None))]
    #[pyo3(text_signature = "(rounding_mode=None, tolerances=None)")]
    /// Create a configuration, optionally overriding rounding mode and tolerances.
    fn new(
        rounding_mode: Option<PyRef<PyRoundingMode>>,
        tolerances: Option<PyRef<PyToleranceConfig>>,
    ) -> Self {
        let mut inner = FinstackConfig::default();
        if let Some(rm) = rounding_mode {
            inner.rounding.mode = rm.inner;
        }
        if let Some(t) = tolerances {
            inner.tolerances = t.inner;
        }
        Self { inner }
    }

    /// Effective output decimal scale for `currency` (ISO-4217 code).
    #[pyo3(text_signature = "(self, currency)")]
    fn output_scale(&self, currency: &str) -> PyResult<u32> {
        let ccy: Currency = currency
            .parse()
            .map_err(|_| core_to_py(InputError::UnknownCurrency.into()))?;
        Ok(self.inner.output_scale(ccy))
    }

    /// Effective ingest decimal scale for `currency` (ISO-4217 code).
    #[pyo3(text_signature = "(self, currency)")]
    fn ingest_scale(&self, currency: &str) -> PyResult<u32> {
        let ccy: Currency = currency
            .parse()
            .map_err(|_| core_to_py(InputError::UnknownCurrency.into()))?;
        Ok(self.inner.ingest_scale(ccy))
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

    /// Deserialize a config from JSON.
    #[classmethod]
    #[pyo3(text_signature = "(cls, json)")]
    fn from_json(_cls: &Bound<'_, PyType>, json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid FinstackConfig JSON"))
    }

    fn __repr__(&self) -> String {
        "FinstackConfig(...)".to_string()
    }
}

/// Register the `finstack_quant.core.config` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "config")?;
    m.setattr(
        "__doc__",
        "Configuration types from finstack-quant-core (rounding, tolerances, FinstackConfig).",
    )?;

    m.add_class::<PyRoundingMode>()?;
    m.add_class::<PyToleranceConfig>()?;
    m.add_class::<PyFinstackConfig>()?;
    let all = PyList::new(py, ["RoundingMode", "ToleranceConfig", "FinstackConfig"])?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "config",
        "finstack_quant.core",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
