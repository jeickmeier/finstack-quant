//! Python bindings for [`finstack_quant_core::currency::Currency`].

use std::str::FromStr;

use crate::bindings::macros::{typed_or_label, wire_methods};
use finstack_quant_core::currency::Currency;
use pyo3::basic::CompareOp;
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Parse an ISO-4217 alphabetic code (case-insensitive, not trimmed).
    #[new]
    #[pyo3(text_signature = "(code)")]
    fn ctor(code: &str) -> PyResult<Self> {
        Currency::from_str(code)
            .map(Self::from_inner)
            .map_err(crate::errors::core_to_py)
    }

    /// Construct from an ISO-4217 numeric code (e.g. ``840`` for USD).
    #[classmethod]
    #[pyo3(text_signature = "(cls, code)")]
    fn from_numeric(_cls: &Bound<'_, PyType>, code: u16) -> PyResult<Self> {
        Currency::try_from(code)
            .map(Self::from_inner)
            .map_err(crate::errors::core_to_py)
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

    /// Rich comparison; compares typed currency values; other types return NotImplemented.
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
        Ok(py.NotImplemented())
    }
}

wire_methods!(PyCurrency, Currency, "Currency");

typed_or_label!(
    pub(crate) fn extract_currency -> Currency,
    PyCurrency,
    "Currency or str currency code",
    |label| Currency::from_str(label).map_err(crate::errors::core_to_py)
);

/// Build the `finstack_quant.core.currency` submodule and register [`PyCurrency`].
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = crate::bindings::module_utils::new_submodule(parent, "currency")?;
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

    let mut name_refs: Vec<&str> = export_names.iter().map(|s| s.as_str()).collect();
    name_refs.sort_unstable();
    let all = PyList::new(py, &name_refs)?;
    module.setattr("__all__", all)?;

    crate::bindings::module_utils::attach_submodule(
        parent,
        &module,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
