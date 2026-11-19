//! Price curve bindings.

use finstack_quant_core::market_data::term_structures::{PriceCurve, PriceCurveKind};

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use super::helpers::{
    CurveSourceOptions, TimePoint, columns_to_dataframe, extract_time_point, required_option,
};
use crate::bindings::core::dates::daycount::extract_day_count;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};

#[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve")]
#[pyclass(
    name = "PriceCurve",
    module = "finstack_quant.core.market_data",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPriceCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<PriceCurve>,
}

impl PyPriceCurve {
    /// Build from an existing `Arc<PriceCurve>`.
    pub(crate) fn from_inner(inner: Arc<PriceCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPriceCurve {
    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.from_dates")]
    #[staticmethod]
    fn from_dates(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        points: Vec<(Bound<'_, PyAny>, f64)>,
        kind: &str,
        day_count: &Bound<'_, PyAny>,
        interp: &str,
        extrapolation: &str,
    ) -> PyResult<Self> {
        let points = points
            .iter()
            .map(|(date, value)| Ok((py_to_date(date)?, *value)))
            .collect::<PyResult<Vec<_>>>()?;
        PriceCurve::from_dates(
            id,
            py_to_date(base_date)?,
            &points,
            kind.parse::<PriceCurveKind>().map_err(core_to_py)?,
            extract_day_count(day_count)?,
            interp.parse::<InterpStyle>().map_err(core_to_py)?,
            extrapolation
                .parse::<ExtrapolationPolicy>()
                .map_err(core_to_py)?,
        )
        .map(|curve| Self::from_inner(Arc::new(curve)))
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.__init__")]
    #[new]
    fn new(options: &Bound<'_, PyDict>) -> PyResult<Self> {
        let source = CurveSourceOptions::extract(options, "PriceCurve", "kind")?;
        let kind: String = required_option(options, "PriceCurve", "kind")?.extract()?;
        PriceCurve::builder(source.id)
            .base_date(source.base_date)
            .knots(source.knots)
            .kind(kind.parse::<PriceCurveKind>().map_err(core_to_py)?)
            .day_count(source.day_count)
            .calendar_id_opt(source.calendar_id)
            .interp(source.interp)
            .extrapolation(source.extrapolation)
            .build()
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.bump_parallel_pct")]
    fn bump_parallel_pct(&self, pct: f64) -> PyResult<Self> {
        self.inner
            .bump_parallel_pct(pct)
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.price")]
    #[pyo3(text_signature = "(self, t)")]
    fn price(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => self.inner.price(t).map_err(core_to_py),
            TimePoint::Date(d) => self.inner.price_on_date(d).map_err(core_to_py),
        }
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        columns_to_dataframe(
            py,
            &[
                ("t", self.inner.knots().to_vec()),
                ("price", self.inner.prices().to_vec()),
            ],
        )
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.id")]
    #[getter]
    fn id(&self) -> &str {
        self.inner.id().as_str()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.base_date")]
    #[getter]
    fn base_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.base_date())
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.kind")]
    #[getter]
    fn kind(&self) -> &'static str {
        self.inner.kind().as_str()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.calendar_id")]
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id().map(str::to_owned)
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.knots")]
    #[getter]
    fn knots(&self) -> Vec<f64> {
        self.inner.knots().to_vec()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.prices")]
    #[getter]
    fn prices(&self) -> Vec<f64> {
        self.inner.prices().to_vec()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.day_count")]
    #[getter]
    fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.interp_style")]
    #[getter]
    fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    #[doc = pydoc!("finstack_quant.core.market_data._curves.price", "PriceCurve.extrapolation")]
    #[getter]
    fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "PriceCurve(id='{}', kind='{}', base_date='{}', knots={})",
            self.inner.id().as_str(),
            self.inner.kind(),
            self.inner.base_date(),
            self.inner.knots().len()
        )
    }
}

wire_methods!(
    PyPriceCurve,
    PriceCurve,
    "PriceCurve",
    module = "finstack_quant.core.market_data._curves.price",
    arc
);
impl_repr_html_via_dataframe!(PyPriceCurve);
