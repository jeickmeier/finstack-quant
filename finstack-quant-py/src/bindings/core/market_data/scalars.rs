//! Python bindings for scalar market time series.

use std::str::FromStr;
use std::sync::Arc;

use finstack_quant_core::market_data::scalars::{
    InflationExpectedRelease, InflationIndex, InflationObservation, ScalarTimeSeries,
    SeriesExtrapolation, SeriesInterpolation,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

use crate::bindings::core::currency::{PyCurrency, extract_currency};
use crate::bindings::core::money::is_python_decimal;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pandas_utils::{dates_to_datetime_index, dict_to_dataframe};
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;

fn observations_to_dataframe<'py>(
    py: Python<'py>,
    observations: Vec<(finstack_quant_core::dates::Date, f64)>,
) -> PyResult<Bound<'py, PyAny>> {
    let dates: Vec<finstack_quant_core::dates::Date> =
        observations.iter().map(|(date, _)| *date).collect();
    let values: Vec<f64> = observations.iter().map(|(_, value)| *value).collect();
    let data = PyDict::new(py);
    data.set_item("value", values)?;
    let index = dates_to_datetime_index(py, &dates)?;
    dict_to_dataframe(py, &data, Some(index))
}

/// Extract an `f64` observation from a `float`, `int` or `decimal.Decimal`
/// (rejecting `bool`).
///
/// A `Decimal` converts to `f64` with ``Decimal.__float__`` and is rejected
/// when non-finite.
pub(crate) fn extract_f64(value: &Bound<'_, PyAny>, field: &str) -> PyResult<f64> {
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "{field} must be float, int, or decimal.Decimal, not bool"
        )));
    }
    if is_python_decimal(value)? {
        let parsed: f64 = value.call_method0("__float__")?.extract()?;
        if !parsed.is_finite() {
            return Err(core_to_py(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::NonFiniteValue {
                    kind: finstack_quant_core::NonFiniteKind::classify(parsed),
                },
            )));
        }
        return Ok(parsed);
    }
    value.extract::<f64>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "{field} must be float, int, or decimal.Decimal"
        ))
    })
}

/// Parse `(date, value)` observation pairs into chronologically ascending Rust values.
fn parse_observations(
    observations: &[(Bound<'_, PyAny>, Bound<'_, PyAny>)],
) -> PyResult<Vec<(finstack_quant_core::dates::Date, f64)>> {
    observations
        .iter()
        .enumerate()
        .map(|(index, (date, value))| {
            Ok((
                py_to_date(date)?,
                extract_f64(value, &format!("observations[{index}] value"))?,
            ))
        })
        .collect()
}

/// Date-indexed scalar market observations with Rust-owned interpolation.
#[pyclass(
    name = "ScalarTimeSeries",
    module = "finstack_quant.core.market_data",
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
    /// Construct a scalar time series from strictly ascending dated observations.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Non-empty trimmed series identifier.
    /// observations : list[tuple[datetime.date | str, float | int | decimal.Decimal]]
    ///     Chronologically strictly ascending ``(date, value)`` pairs; empty and
    ///     single-point series are admitted, duplicate or unsorted dates and
    ///     non-finite values raise ``ValueError``, and ``Decimal`` converts to ``float``.
    /// interpolation : str
    ///     Required in-range fill policy: ``"step"`` or ``"linear"``.
    /// extrapolation : str
    ///     Required right-tail policy after the latest observation: ``"flat"``
    ///     (hold last observation) or ``"error"`` (raise ``KeyError``).
    /// currency : Currency | str, optional
    ///     Currency tag for monetary series; ``None`` for unitless values.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``id`` is blank/untrimmed, dates are unsorted or duplicated, a
    ///     value is non-finite, or ``interpolation``/``extrapolation`` is not
    ///     a recognised label.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import ScalarTimeSeries
    /// >>> series = ScalarTimeSeries("SOFR", [("2025-01-01", 0.04), ("2025-01-03", 0.05)], "linear", "flat")
    /// >>> series.get_value("2025-01-02")
    /// 0.045
    #[new]
    #[pyo3(signature = (id, observations, interpolation, extrapolation, currency=None))]
    fn new(
        id: &str,
        observations: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
        interpolation: &str,
        extrapolation: &str,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let observations = parse_observations(&observations)?;
        let interpolation = SeriesInterpolation::from_str(interpolation).map_err(core_to_py)?;
        let extrapolation = SeriesExtrapolation::from_str(extrapolation).map_err(core_to_py)?;
        let currency = currency.map(extract_currency).transpose()?;
        let inner = ScalarTimeSeries::new(id, observations, currency, interpolation, extrapolation)
            .map_err(core_to_py)?;
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

    /// Interpolation policy name (``"step"`` or ``"linear"``).
    #[getter]
    fn interpolation(&self) -> String {
        self.inner.interpolation().to_string()
    }

    /// Right-tail extrapolation policy name (``"flat"`` or ``"error"``).
    #[getter]
    fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
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

    /// Value on a date under the series interpolation and extrapolation policies.
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date | str
    ///     Lookup date; empty series, dates before the first observation, and
    ///     dates after the last observation under ``"error"`` extrapolation
    ///     raise ``KeyError``.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If the series is empty, ``date`` precedes the first observation, or
    ///     ``date`` exceeds the last observation when ``extrapolation == "error"``.
    /// ValueError
    ///     If ``date`` cannot be parsed as a valid date.
    #[pyo3(text_signature = "(self, date)")]
    fn get_value(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner.get_value(py_to_date(date)?).map_err(core_to_py)
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
    fn get_value_exact(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner
            .get_value_exact(py_to_date(date)?)
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
    /// ascending order of :attr:`observations`; an empty series produces an
    /// empty ``DatetimeIndex`` frame with column ``value``.
    ///
    /// Only the stored observations appear; nothing is interpolated. Use
    /// :meth:`get_value` for values between observation dates — the
    /// interpolation policy stays owned by Rust.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // `ScalarTimeSeries` derives Serialize, but the wire format nests the
        // id/currency/interpolation metadata alongside the observations, so
        // the columns are built explicitly to keep the frame purely the
        // date-indexed series it represents.
        observations_to_dataframe(py, self.inner.observations())
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

wire_methods!(PyScalarTimeSeries, ScalarTimeSeries, "ScalarTimeSeries");

impl_repr_html_via_dataframe!(PyScalarTimeSeries);

#[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationObservation")]
#[pyclass(
    name = "InflationObservation",
    module = "finstack_quant.core.market_data",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInflationObservation {
    inner: InflationObservation,
}

#[pymethods]
impl PyInflationObservation {
    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationObservation.__init__")]
    #[new]
    fn new(
        reference_month: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
        available_on: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: InflationObservation {
                reference_month: py_to_date(reference_month)?,
                value: extract_f64(value, "value")?,
                available_on: py_to_date(available_on)?,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationObservation.reference_month")]
    #[getter]
    fn reference_month<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.reference_month)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationObservation.value")]
    #[getter]
    fn value(&self) -> f64 {
        self.inner.value
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationObservation.available_on")]
    #[getter]
    fn available_on<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.available_on)
    }
}

#[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationExpectedRelease")]
#[pyclass(
    name = "InflationExpectedRelease",
    module = "finstack_quant.core.market_data",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInflationExpectedRelease {
    inner: InflationExpectedRelease,
}

#[pymethods]
impl PyInflationExpectedRelease {
    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationExpectedRelease.__init__")]
    #[new]
    fn new(
        reference_month: &Bound<'_, PyAny>,
        expected_release: &Bound<'_, PyAny>,
        known_on: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: InflationExpectedRelease {
                reference_month: py_to_date(reference_month)?,
                expected_release: py_to_date(expected_release)?,
                known_on: py_to_date(known_on)?,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationExpectedRelease.reference_month")]
    #[getter]
    fn reference_month<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.reference_month)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationExpectedRelease.expected_release")]
    #[getter]
    fn expected_release<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.expected_release)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationExpectedRelease.known_on")]
    #[getter]
    fn known_on<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.known_on)
    }
}

#[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex")]
#[pyclass(
    name = "InflationIndex",
    module = "finstack_quant.core.market_data",
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
    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.__init__")]
    #[new]
    fn new(
        id: &str,
        observations: Vec<PyRef<'_, PyInflationObservation>>,
        currency: &Bound<'_, PyAny>,
        knowledge_date: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        InflationIndex::new(
            id,
            observations.into_iter().map(|obs| obs.inner).collect(),
            extract_currency(currency)?,
            py_to_date(knowledge_date)?,
        )
        .map(|inner| Self::from_inner(Arc::new(inner)))
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.with_expected_releases")]
    fn with_expected_releases(
        &self,
        expected_releases: Vec<PyRef<'_, PyInflationExpectedRelease>>,
    ) -> PyResult<Self> {
        let inner = self
            .inner
            .as_ref()
            .clone()
            .with_expected_releases(expected_releases.into_iter().map(|rel| rel.inner).collect())
            .map_err(core_to_py)?;
        Ok(Self::from_inner(Arc::new(inner)))
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.get_expected_release")]
    fn get_expected_release(
        &self,
        reference_month: &Bound<'_, PyAny>,
    ) -> PyResult<Option<PyInflationExpectedRelease>> {
        self.inner
            .get_expected_release(py_to_date(reference_month)?)
            .map(|rel| rel.map(|inner| PyInflationExpectedRelease { inner }))
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.get_expected_releases")]
    fn get_expected_releases(&self) -> Vec<PyInflationExpectedRelease> {
        self.inner
            .get_expected_releases()
            .iter()
            .map(|&inner| PyInflationExpectedRelease { inner })
            .collect()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.get_observation")]
    fn get_observation(
        &self,
        reference_month: &Bound<'_, PyAny>,
    ) -> PyResult<PyInflationObservation> {
        self.inner
            .get_observation(py_to_date(reference_month)?)
            .map(|inner| PyInflationObservation { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.qualified_monthly_observation")]
    fn qualified_monthly_observation(
        &self,
        year: i32,
        month: u8,
        knowledge_date: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let month = time::Month::try_from(month)
            .map_err(|err| crate::errors::value_error(err.to_string()))?;
        self.inner
            .qualified_monthly_observation(year, month, py_to_date(knowledge_date)?)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.date_range")]
    fn date_range<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
        self.inner
            .date_range()
            .map(|(first, last)| Ok((date_to_py(py, first)?, date_to_py(py, last)?)))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.knowledge_date")]
    #[getter]
    fn knowledge_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.knowledge_date())
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.id")]
    #[getter]
    fn id(&self) -> &str {
        self.inner.id()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.currency")]
    #[getter]
    fn currency(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.currency())
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.observations")]
    #[getter]
    fn observations(&self) -> Vec<PyInflationObservation> {
        self.inner
            .observations()
            .iter()
            .map(|&inner| PyInflationObservation { inner })
            .collect()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._scalars", "InflationIndex.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let observations = self.inner.observations();
        let months: Vec<_> = observations.iter().map(|obs| obs.reference_month).collect();
        let values: Vec<_> = observations.iter().map(|obs| obs.value).collect();
        let available: Vec<_> = observations.iter().map(|obs| obs.available_on).collect();
        let data = PyDict::new(py);
        data.set_item("value", values)?;
        data.set_item("available_on", dates_to_datetime_index(py, &available)?)?;
        dict_to_dataframe(py, &data, Some(dates_to_datetime_index(py, &months)?))
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "InflationIndex(id={:?}, observations={})",
            self.inner.id(),
            self.inner.len()
        )
    }
}

wire_methods!(
    PyInflationIndex,
    InflationIndex,
    "InflationIndex",
    module = "finstack_quant.core.market_data._scalars",
    arc
);

impl_repr_html_via_dataframe!(PyInflationIndex);

pub(super) const EXPORTS: &[&str] = &[
    "InflationIndex",
    "InflationObservation",
    "InflationExpectedRelease",
    "ScalarTimeSeries",
];

/// Register scalar time-series classes on `finstack_quant.core.market_data`.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyScalarTimeSeries>()?;
    m.add_class::<PyInflationIndex>()?;
    m.add_class::<PyInflationObservation>()?;
    m.add_class::<PyInflationExpectedRelease>()?;
    Ok(())
}
