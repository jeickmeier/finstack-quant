//! A fixed-for-floating commodity swap across delivery periods.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::commodity::commodity_swap::{
    CommoditySwap, CommoditySwapBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap")]
#[pyclass(
    name = "CommoditySwap",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCommoditySwap {
    pub(crate) inner: CommoditySwap,
}

instrument_envelope_methods!(
    PyCommoditySwap(
        "CommoditySwap",
        module = "finstack_quant.valuations.instruments._typed_commodity_swap"
    ),
    CommoditySwap,
    builder = PyCommoditySwapBuilder("CommoditySwapBuilder", no_fields),
    seed = CommoditySwap::builder(),
);
instrument_pricing_methods!(
    PyCommoditySwap,
    "CommoditySwap",
    module = "finstack_quant.valuations.instruments._typed_commodity_swap"
);

#[pymethods]
impl PyCommoditySwap {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.to_builder")]
    fn to_builder(&self) -> PyCommoditySwapBuilder {
        PyCommoditySwapBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CommoditySwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.underlying")]
    #[getter]
    fn underlying<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.underlying)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.quantity")]
    #[getter]
    fn quantity(&self) -> f64 {
        self.inner.quantity
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.fixed_price")]
    #[getter]
    fn fixed_price(&self) -> f64 {
        self.inner.fixed_price
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.forward_curve_id")]
    #[getter]
    fn forward_curve_id(&self) -> String {
        self.inner.forward_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.side")]
    #[getter]
    fn side<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.side)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.start_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.frequency")]
    #[getter]
    fn frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.frequency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.calendar_id")]
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.business_day_convention")]
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        super::convert::enum_to_py_string(&self.inner.business_day_convention)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.index_lag_days")]
    #[getter]
    fn index_lag_days(&self) -> Option<i32> {
        self.inner.index_lag_days
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwap.past_fixings")]
    #[getter]
    fn past_fixings<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .past_fixings
            .iter()
            .map(|(d, x)| crate::bindings::date_utils::date_to_py(py, *d).map(|d| (d, *x)))
            .collect()
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder")]
#[pyclass(
    name = "CommoditySwapBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCommoditySwapBuilder {
    inner: Option<CommoditySwapBuilder>,
}

#[pymethods]
impl PyCommoditySwapBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CommoditySwapBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.underlying")]
    fn underlying<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "underlying")?;
        builder_set!(slf, |b: CommoditySwapBuilder| b.underlying(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.quantity")]
    fn quantity<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySwapBuilder| b.quantity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.fixed_price")]
    fn fixed_price<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySwapBuilder| b.fixed_price(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySwapBuilder| b.forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.side")]
    fn side<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "side")?;
        builder_set!(slf, |b: CommoditySwapBuilder| b.side(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.start_date")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CommoditySwapBuilder| b.start_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CommoditySwapBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.frequency")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "frequency")?;
        builder_set!(slf, |b: CommoditySwapBuilder| b.frequency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.calendar_id")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CommoditySwapBuilder| b.calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.business_day_convention")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "business_day_convention")?;
        builder_set!(slf, |b: CommoditySwapBuilder| b
            .business_day_convention(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySwapBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.index_lag_days")]
    fn index_lag_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<i32>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySwapBuilder| b.index_lag_days_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.past_fixings")]
    fn past_fixings<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<(Bound<'_, PyAny>, f64)>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .into_iter()
            .map(|(d, x)| crate::bindings::date_utils::py_to_date(&d).map(|date| (date, x)))
            .collect::<PyResult<_>>()?;
        builder_set!(slf, |b: CommoditySwapBuilder| b.past_fixings(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swap", "CommoditySwapBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CommoditySwapBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCommoditySwap>()?;
    module.add_class::<PyCommoditySwapBuilder>()?;
    Ok(())
}
