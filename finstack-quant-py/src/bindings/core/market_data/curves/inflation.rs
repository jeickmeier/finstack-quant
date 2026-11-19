//! Inflation curve bindings.

use std::sync::Arc;

use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use pyo3::prelude::*;

use super::helpers::{TimePoint, columns_to_dataframe, extract_time_point};
use crate::bindings::core::dates::daycount::extract_day_count;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve")]
#[pyclass(
    name = "InflationCurve",
    module = "finstack_quant.core.market_data",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInflationCurve {
    pub(crate) inner: Arc<InflationCurve>,
}

impl PyInflationCurve {
    pub(crate) fn from_inner(inner: Arc<InflationCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyInflationCurve {
    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.from_dates")]
    #[staticmethod]
    #[pyo3(signature = (id, base_date, points, *, day_count, interp, extrapolation, calendar_id=None))]
    fn from_dates(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        points: Vec<(Bound<'_, PyAny>, f64)>,
        day_count: &Bound<'_, PyAny>,
        interp: &str,
        extrapolation: &str,
        calendar_id: Option<&str>,
    ) -> PyResult<Self> {
        let points = points
            .iter()
            .map(|(date, level)| Ok((py_to_date(date)?, *level)))
            .collect::<PyResult<Vec<_>>>()?;
        InflationCurve::from_dates(
            id,
            py_to_date(base_date)?,
            &points,
            extract_day_count(day_count)?,
            interp.parse::<InterpStyle>().map_err(core_to_py)?,
            extrapolation
                .parse::<ExtrapolationPolicy>()
                .map_err(core_to_py)?,
            calendar_id,
        )
        .map(|curve| Self::from_inner(Arc::new(curve)))
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.__init__")]
    #[new]
    #[pyo3(signature = (id, base_date, knots, *, day_count, interp, extrapolation, calendar_id=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        day_count: &Bound<'_, PyAny>,
        interp: &str,
        extrapolation: &str,
        calendar_id: Option<String>,
    ) -> PyResult<Self> {
        InflationCurve::builder(id)
            .base_date(py_to_date(base_date)?)
            .day_count(extract_day_count(day_count)?)
            .calendar_id_opt(calendar_id)
            .knots(knots)
            .interp(interp.parse::<InterpStyle>().map_err(core_to_py)?)
            .extrapolation(
                extrapolation
                    .parse::<ExtrapolationPolicy>()
                    .map_err(core_to_py)?,
            )
            .build()
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.cpi")]
    fn cpi(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => self.inner.cpi(t).map_err(core_to_py),
            TimePoint::Date(d) => self.inner.cpi_on_date(d).map_err(core_to_py),
        }
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.inflation_rate")]
    fn inflation_rate(&self, t1: f64, t2: f64) -> PyResult<f64> {
        self.inner.inflation_rate(t1, t2).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        columns_to_dataframe(
            py,
            &[
                ("t", self.inner.knots().to_vec()),
                ("cpi", self.inner.cpi_levels().to_vec()),
            ],
        )
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.id")]
    #[getter]
    fn id(&self) -> &str {
        self.inner.id().as_str()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.base_date")]
    #[getter]
    fn base_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.base_date())
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.day_count")]
    #[getter]
    fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.calendar_id")]
    #[getter]
    fn calendar_id(&self) -> Option<&str> {
        self.inner.calendar_id()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.base_cpi")]
    #[getter]
    fn base_cpi(&self) -> f64 {
        self.inner.base_cpi()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.knots")]
    #[getter]
    fn knots(&self) -> Vec<f64> {
        self.inner.knots().to_vec()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.cpi_levels")]
    #[getter]
    fn cpi_levels(&self) -> Vec<f64> {
        self.inner.cpi_levels().to_vec()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.interp_style")]
    #[getter]
    fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.inflation", "InflationCurve.extrapolation")]
    #[getter]
    fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "InflationCurve(id='{}', base_date='{}', base_cpi={}, knots={})",
            self.inner.id().as_str(),
            self.inner.base_date(),
            self.inner.base_cpi(),
            self.inner.knots().len(),
        )
    }
}

wire_methods!(
    PyInflationCurve,
    InflationCurve,
    "InflationCurve",
    module = "finstack_quant.core.market_data._curves.inflation",
    arc
);
impl_repr_html_via_dataframe!(PyInflationCurve);
