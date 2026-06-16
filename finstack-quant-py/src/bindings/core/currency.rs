//! Python bindings for [`finstack_quant_core::currency::Currency`].

use std::str::FromStr;

use crate::errors::display_to_py;
use finstack_quant_core::currency::Currency;
use pyo3::basic::CompareOp;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyType};
use pyo3::IntoPyObjectExt;
use strum::IntoEnumIterator;

/// Wrapper for [`Currency`] exposed to Python as `finstack_quant.core.currency.Currency`.
#[pyclass(
    name = "Currency",
    module = "finstack_quant.core.currency",
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyCurrency {
    /// Inner ISO-4217 currency value.
    pub(crate) inner: Currency,
}

impl PyCurrency {
    /// Build a [`PyCurrency`] from an existing [`Currency`].
    pub(crate) const fn from_inner(inner: Currency) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCurrency {
    /// Parse an ISO-4217 alphabetic code (case-insensitive).
    #[new]
    #[pyo3(text_signature = "(code)")]
    fn ctor(code: &str) -> PyResult<Self> {
        Currency::from_str(code)
            .map(Self::from_inner)
            .map_err(|e| crate::errors::value_error(format!("Invalid currency code {code:?}: {e}")))
    }

    /// Construct from an ISO-4217 numeric code (e.g. ``840`` for USD).
    #[classmethod]
    #[pyo3(text_signature = "(cls, code)")]
    fn from_numeric(_cls: &Bound<'_, PyType>, code: u16) -> PyResult<Self> {
        Currency::try_from(code)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Three-letter ISO-4217 code (uppercase).
    #[getter]
    fn code(&self) -> String {
        self.inner.to_string()
    }

    /// ISO-4217 numeric identifier.
    #[getter]
    fn numeric(&self) -> u16 {
        self.inner.numeric()
    }

    /// Typical number of decimal places (minor units) for this currency.
    #[getter]
    fn decimals(&self) -> u8 {
        self.inner.decimals()
    }

    /// Return a debug-style representation.
    fn __repr__(&self) -> String {
        format!("Currency('{}')", self.inner)
    }

    /// ISO alphabetic code.
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    /// Hash based on the ISO numeric currency code.
    fn __hash__(&self) -> isize {
        self.inner.numeric() as isize
    }

    /// Rich comparison; supports another [`PyCurrency`] or an ISO code string.
    fn __richcmp__(
        &self,
        other: Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        if let Ok(r) = other.extract::<PyRef<'_, PyCurrency>>() {
            let ord = self.inner.cmp(&r.inner);
            return op.matches(ord).into_py_any(py);
        }
        if let Ok(s) = other.extract::<String>() {
            let Some(rhs) = Currency::from_str(&s).ok() else {
                return match op {
                    CompareOp::Eq => false.into_py_any(py),
                    CompareOp::Ne => true.into_py_any(py),
                    _ => Ok(py.NotImplemented()),
                };
            };
            let ord = self.inner.cmp(&rhs);
            return op.matches(ord).into_py_any(py);
        }
        Ok(py.NotImplemented())
    }

    /// Serialize this currency to a JSON string.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize a currency from JSON.
    #[classmethod]
    #[pyo3(text_signature = "(cls, json)")]
    fn from_json(_cls: &Bound<'_, PyType>, json: &str) -> PyResult<Self> {
        let inner: Currency = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }
}

/// Extract a [`Currency`] from a [`PyCurrency`] instance or an ISO string code.
pub(crate) fn extract_currency(obj: &Bound<'_, PyAny>) -> PyResult<Currency> {
    if let Ok(ccy) = obj.extract::<PyRef<'_, PyCurrency>>() {
        return Ok(ccy.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return Currency::from_str(&s)
            .map_err(|e| crate::errors::value_error(format!("Invalid currency code {s:?}: {e}")));
    }
    Err(PyTypeError::new_err(
        "expected Currency or str currency code",
    ))
}

/// Build the `finstack_quant.core.currency` submodule and register [`PyCurrency`].
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "currency")?;
    module.setattr(
        "__doc__",
        "ISO-4217 currency bindings (finstack-quant-core).",
    )?;

    module.add_class::<PyCurrency>()?;

    let mut export_names: Vec<String> = vec!["Currency".to_string()];
    for ccy in Currency::iter() {
        let code = ccy.to_string();
        let value = Py::new(py, PyCurrency::from_inner(ccy))?;
        module.add(&code, value)?;
        export_names.push(code);
    }

    let name_refs: Vec<&str> = export_names.iter().map(|s| s.as_str()).collect();
    let all = PyList::new(py, &name_refs)?;
    module.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &module,
        "currency",
        "finstack_quant.core",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
