//! Python bindings for scalar market time series.

use std::str::FromStr;
use std::sync::Arc;

use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationInterpolation, InflationLag, ScalarTimeSeries, SeriesInterpolation,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};
use rust_decimal::prelude::ToPrimitive;

use crate::bindings::core::currency::{extract_currency, PyCurrency};
use crate::bindings::core::money::{decimal_from_py, is_python_decimal};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::pandas_utils::{dates_to_datetime_index, dict_to_dataframe};
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
    /// Parameters
    /// ----------
    /// id : str
    ///     Series identifier.
    /// observations : list[tuple[datetime.date | str, float | int | decimal.Decimal]]
    ///     Dated values; ``Decimal`` values must round-trip through ``float`` exactly.
    ///     Dates must be unique; any order is accepted.
    /// currency : Currency | str, optional
    ///     Currency tag for monetary series; ``None`` for unitless values.
    /// interpolation : str, optional
    ///     ``"step"`` (default, last observation carried forward) or ``"linear"``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``observations`` is empty or has duplicate dates, a value is
    ///     non-finite, or ``interpolation`` is not a recognised label.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import ScalarTimeSeries
    /// >>> series = ScalarTimeSeries("SOFR", [("2025-01-01", 0.04), ("2025-01-03", 0.05)], interpolation="linear")
    /// >>> series.value_on("2025-01-02")
    /// 0.045
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

    /// Value on a date under the series interpolation policy.
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date | str
    ///     Lookup date; must lie within the observation range.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``date`` is outside the observed range.
    #[pyo3(text_signature = "(self, date)")]
    fn value_on(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner.value_on(py_to_date(date)?).map_err(core_to_py)
    }

    /// Value on an exact observation date (no interpolation).
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date | str
    ///     Must match a stored observation date exactly.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If no observation exists on ``date``.
    #[pyo3(text_signature = "(self, date)")]
    fn value_on_exact(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner
            .value_on_exact(py_to_date(date)?)
            .map_err(core_to_py)
    }

    /// Earliest observation date.
    #[getter]
    fn first_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .first_date()
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Latest observation date.
    #[getter]
    fn last_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .last_date()
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Export as a pandas ``DataFrame`` indexed by observation date.
    ///
    /// Columns: ``value``.
    ///
    /// The index is a ``DatetimeIndex`` built from the observation dates, so
    /// the frame joins directly against other date-indexed market data and
    /// resamples without a conversion step. Rows follow the chronologically
    /// sorted order of :attr:`observations`; there is always at least one row,
    /// because the constructor rejects an empty observation set.
    ///
    /// Only the stored observations appear; nothing is interpolated. Use
    /// :meth:`value_on` for values between observation dates — the
    /// interpolation policy stays owned by Rust.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // `ScalarTimeSeries` derives Serialize, but the wire format nests the
        // id/currency/interpolation metadata alongside the observations, so
        // the columns are built explicitly to keep the frame purely the
        // date-indexed series it represents.
        let observations = self.inner.observations();
        let dates: Vec<time::Date> = observations.iter().map(|(date, _)| *date).collect();
        let values: Vec<f64> = observations.iter().map(|(_, value)| *value).collect();
        let data = PyDict::new(py);
        data.set_item("value", values)?;
        let index = dates_to_datetime_index(py, &dates)?;
        dict_to_dataframe(py, &data, Some(index))
    }

    /// Serialize the canonical Rust series state to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|err| {
            crate::errors::value_error(format!("failed to serialize ScalarTimeSeries: {err}"))
        })
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

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
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
    /// Underlying Rust inflation index (shared, so `MarketContext` getters
    /// hand out `Arc` clones instead of deep copies).
    pub(crate) inner: Arc<InflationIndex>,
}

impl PyInflationIndex {
    /// Build from an existing Rust inflation index.
    pub(crate) fn from_inner(inner: Arc<InflationIndex>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyInflationIndex {
    /// Construct an inflation index from dated observations.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Index identifier (e.g. ``"US-CPI-U"``).
    /// observations : list[tuple[datetime.date | str, float | int | decimal.Decimal]]
    ///     Dated index levels; ``Decimal`` values must round-trip through ``float`` exactly.
    /// currency : Currency | str
    ///     Currency of the index.
    /// interpolation : str, optional
    ///     ``"step"`` (default, last observation carried forward) or ``"linear"``.
    /// lag : str | int, optional
    ///     Publication lag applied before lookups: ``"none"`` (default),
    ///     ``"3M"``/``"90D"`` market strings, or an integer number of months.
    /// seasonality : list[float], optional
    ///     Twelve multiplicative factors, January through December.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``observations`` is empty or has duplicate dates, a label is
    ///     unknown, or ``seasonality`` does not have exactly 12 entries.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import InflationIndex
    /// >>> index = InflationIndex("US-CPI", [("2025-01-01", 300.0), ("2025-02-01", 301.5)], "USD", lag="3M")
    /// >>> index.lag
    /// '3M'
    #[new]
    #[pyo3(signature = (id, observations, currency, interpolation=None, lag=None, seasonality=None))]
    fn new(
        id: &str,
        observations: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
        currency: &Bound<'_, PyAny>,
        interpolation: Option<&str>,
        lag: Option<&Bound<'_, PyAny>>,
        seasonality: Option<Vec<f64>>,
    ) -> PyResult<Self> {
        let lag = match lag {
            None => None,
            Some(value) => Some(if let Ok(months) = value.extract::<u8>() {
                if months == 0 {
                    InflationLag::None
                } else {
                    InflationLag::Months(months)
                }
            } else {
                let label: String = value.extract().map_err(|_| {
                    crate::errors::value_error(
                        "lag must be an int number of months or a string like \"3M\", \"90D\" or \"none\"",
                    )
                })?;
                label.parse::<InflationLag>().map_err(core_to_py)?
            }),
        };
        let seasonality = match seasonality {
            None => None,
            Some(factors) => Some(<[f64; 12]>::try_from(factors).map_err(|got| {
                crate::errors::value_error(format!(
                    "seasonality must have exactly 12 monthly factors, got {}",
                    got.len()
                ))
            })?),
        };
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
        let mut inner = InflationIndex::new(id, observations, currency)
            .map_err(core_to_py)?
            .with_interpolation(interpolation);
        if let Some(lag) = lag {
            inner = inner.with_lag(lag);
        }
        if let Some(factors) = seasonality {
            inner = inner.with_seasonality(factors).map_err(core_to_py)?;
        }
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Indexation ratio ``value_on(settle_date) / value_on(base_date)`` with lag and seasonality applied.
    ///
    /// Parameters
    /// ----------
    /// base_date : datetime.date | str
    ///     Reference date of the base index level.
    /// settle_date : datetime.date | str
    ///     Date of the uplifted level.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If either lag-adjusted date is outside the observation range.
    #[pyo3(text_signature = "(self, base_date, settle_date)")]
    fn ratio(&self, base_date: &Bound<'_, PyAny>, settle_date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner
            .ratio(py_to_date(base_date)?, py_to_date(settle_date)?)
            .map_err(core_to_py)
    }

    /// Reference CPI for a date under an explicit month lag (bond-style lookup).
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date | str
    ///     Contract date.
    /// lag_months : int
    ///     Calendar months to look back (``3`` for TIPS/gilts). Observations
    ///     must be unique within each reference month; missing months are not
    ///     interpolated. The first of the contract month needs only the first
    ///     lagged observation.
    ///
    /// Returns
    /// -------
    /// float
    ///     Reference index level.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a required monthly CPI observation is missing.
    /// ValueError
    ///     If the contract date cannot be parsed or a required month has multiple observations.
    #[pyo3(text_signature = "(self, date, lag_months)")]
    fn ref_cpi_months_lag(&self, date: &Bound<'_, PyAny>, lag_months: u32) -> PyResult<f64> {
        self.inner
            .ref_cpi_months_lag(py_to_date(date)?, lag_months)
            .map_err(core_to_py)
    }

    /// ``(first_date, last_date)`` of the stored observations.
    ///
    /// Returns
    /// -------
    /// tuple[datetime.date, datetime.date]
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the index has no observations.
    #[pyo3(text_signature = "(self)")]
    fn date_range<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let (first, last) = self.inner.date_range().map_err(core_to_py)?;
        Ok((date_to_py(py, first)?, date_to_py(py, last)?))
    }

    /// Publication lag label: ``"none"``, ``"<n>M"`` or ``"<n>D"``.
    #[getter]
    fn lag(&self) -> String {
        self.inner.lag().to_string()
    }

    /// Twelve monthly seasonality factors (January first), or ``None``.
    #[getter]
    fn seasonality(&self) -> Option<Vec<f64>> {
        self.inner.seasonality().map(|f| f.to_vec())
    }

    /// Inflation-index identifier.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// ISO-4217 currency of this inflation index.
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
        serde_json::to_string(&self.inner).map_err(|err| {
            crate::errors::value_error(format!("failed to serialize InflationIndex: {err}"))
        })
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

pub(super) const EXPORTS: &[&str] = &["InflationIndex", "ScalarTimeSeries"];

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
