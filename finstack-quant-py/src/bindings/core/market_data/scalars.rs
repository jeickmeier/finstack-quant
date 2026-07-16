//! Python bindings for scalar market time series.

use std::str::FromStr;

use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationInterpolation, ScalarTimeSeries, SeriesInterpolation,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use rust_decimal::prelude::ToPrimitive;

use crate::bindings::core::currency::{extract_currency, PyCurrency};
use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::bindings::core::money::{decimal_from_py, is_python_decimal};
use crate::errors::core_to_py;

/// Extract a finite `f64`, rejecting `Decimal` values that cannot round-trip exactly.
pub(super) fn extract_exact_f64(value: &Bound<'_, PyAny>, field: &str) -> PyResult<f64> {
    if is_python_decimal(value)? {
        let decimal = decimal_from_py(value)?;
        let converted = decimal.to_f64().ok_or_else(|| {
            crate::errors::value_error(format!("{field} must be finite and representable as float"))
        })?;
        if !converted.is_finite() {
            return Err(crate::errors::value_error(format!(
                "{field} must be finite"
            )));
        }
        let roundtrip = rust_decimal::Decimal::from_f64_retain(converted).ok_or_else(|| {
            crate::errors::value_error(format!("{field} must be representable as float"))
        })?;
        if roundtrip.normalize() != decimal.normalize() {
            return Err(crate::errors::value_error(format!(
                "{field} Decimal value must be exactly representable as float"
            )));
        }
        return Ok(converted);
    }

    let converted = value.extract::<f64>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "{field} must be float, int, or decimal.Decimal"
        ))
    })?;
    if !converted.is_finite() {
        return Err(crate::errors::value_error(format!(
            "{field} must be finite"
        )));
    }
    Ok(converted)
}

/// Date-indexed scalar market observations with Rust-owned interpolation.
#[pyclass(
    name = "ScalarTimeSeries",
    module = "finstack_quant.core.market_data.scalars",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyScalarTimeSeries {
    /// Underlying Rust series.
    pub(crate) inner: ScalarTimeSeries,
}

impl PyScalarTimeSeries {
    /// Build from an existing Rust series.
    pub(crate) fn from_inner(inner: ScalarTimeSeries) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyScalarTimeSeries {
    /// Construct a scalar time series from dated observations.
    ///
    /// Observation values may be floats, ints, or ``decimal.Decimal`` values.
    /// Decimal values must round-trip through the Rust ``f64`` storage exactly.
    /// ``currency`` accepts either a ``Currency`` wrapper or an ISO code string.
    #[new]
    #[pyo3(signature = (id, observations, currency=None, interpolation=None))]
    fn new(
        id: &str,
        observations: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
        currency: Option<&Bound<'_, PyAny>>,
        interpolation: Option<&str>,
    ) -> PyResult<Self> {
        let observations = observations
            .iter()
            .enumerate()
            .map(|(index, (date, value))| {
                Ok((
                    py_to_date(date)?,
                    extract_exact_f64(value, &format!("observations[{index}] value"))?,
                ))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let currency = currency.map(extract_currency).transpose()?;
        let interpolation = interpolation
            .map(SeriesInterpolation::from_str)
            .transpose()
            .map_err(core_to_py)?
            .unwrap_or_default();
        let inner = ScalarTimeSeries::new(id, observations, currency)
            .map_err(core_to_py)?
            .with_interpolation(interpolation);
        Ok(Self { inner })
    }

    /// Series identifier.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id().as_str()
    }

    /// Optional currency tag.
    #[getter]
    fn currency(&self) -> Option<PyCurrency> {
        self.inner.currency().map(PyCurrency::from_inner)
    }

    /// Interpolation policy name.
    #[getter]
    fn interpolation(&self) -> String {
        self.inner.interpolation().to_string()
    }

    /// Chronologically sorted observations.
    #[getter]
    fn observations<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .observations()
            .into_iter()
            .map(|(date, value)| Ok((date_to_py(py, date)?, value)))
            .collect()
    }

    /// Interpolated value on a date.
    fn value_on(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner.value_on(py_to_date(date)?).map_err(core_to_py)
    }

    /// Serialize the canonical Rust series state to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(|err| {
            crate::errors::value_error(format!("failed to serialize ScalarTimeSeries: {err}"))
        })
    }

    /// Deserialize canonical Rust series state from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| {
                crate::errors::value_error(format!("invalid ScalarTimeSeries JSON: {err}"))
            })
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "ScalarTimeSeries(id={:?}, observations={})",
            self.inner.id().as_str(),
            self.inner.len()
        )
    }
}

/// Inflation index observations with Rust-owned interpolation and validation.
#[pyclass(
    name = "InflationIndex",
    module = "finstack_quant.core.market_data.scalars",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInflationIndex {
    /// Underlying Rust inflation index.
    pub(crate) inner: InflationIndex,
}

impl PyInflationIndex {
    /// Build from an existing Rust inflation index.
    pub(crate) fn from_inner(inner: InflationIndex) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyInflationIndex {
    /// Construct an inflation index from dated observations.
    ///
    /// ``currency`` accepts either a ``Currency`` wrapper or an ISO code string.
    /// ``interpolation`` accepts ``"step"`` or ``"linear"``.
    #[new]
    #[pyo3(signature = (id, observations, currency, interpolation=None))]
    fn new(
        id: &str,
        observations: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
        currency: &Bound<'_, PyAny>,
        interpolation: Option<&str>,
    ) -> PyResult<Self> {
        let observations = observations
            .iter()
            .enumerate()
            .map(|(index, (date, value))| {
                Ok((
                    py_to_date(date)?,
                    extract_exact_f64(value, &format!("observations[{index}] value"))?,
                ))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let currency = extract_currency(currency)?;
        let interpolation = interpolation
            .map(InflationInterpolation::from_str)
            .transpose()
            .map_err(crate::errors::value_error)?
            .unwrap_or_default();
        let inner = InflationIndex::new(id, observations, currency)
            .map_err(core_to_py)?
            .with_interpolation(interpolation);
        Ok(Self { inner })
    }

    /// Inflation-index identifier.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Index currency.
    #[getter]
    fn currency(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.currency)
    }

    /// Interpolation policy name.
    #[getter]
    fn interpolation(&self) -> String {
        self.inner.interpolation().to_string()
    }

    /// Chronologically sorted observations.
    #[getter]
    fn observations<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .observations()
            .into_iter()
            .map(|(date, value)| Ok((date_to_py(py, date)?, value)))
            .collect()
    }

    /// Interpolated index value on a date.
    fn value_on(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner.value_on(py_to_date(date)?).map_err(core_to_py)
    }

    /// Serialize the canonical Rust inflation-index state to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(|err| {
            crate::errors::value_error(format!("failed to serialize InflationIndex: {err}"))
        })
    }

    /// Deserialize canonical Rust inflation-index state from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| {
                crate::errors::value_error(format!("invalid InflationIndex JSON: {err}"))
            })
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "InflationIndex(id={:?}, observations={})",
            self.inner.id,
            self.inner.len()
        )
    }
}

pub(super) const EXPORTS: &[&str] = &["ScalarTimeSeries", "InflationIndex"];

/// Register the `finstack_quant.core.market_data.scalars` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "scalars")?;
    m.setattr(
        "__doc__",
        "Scalar market time-series bindings (finstack-quant-core).",
    )?;
    m.add_class::<PyScalarTimeSeries>()?;
    m.add_class::<PyInflationIndex>()?;
    m.setattr("__all__", PyList::new(py, EXPORTS)?)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "scalars",
        "finstack_quant.core.market_data",
        crate::bindings::module_utils::ParentNameSource::Package,
    )
}
