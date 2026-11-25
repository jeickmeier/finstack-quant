//! A note redeeming when its accumulated coupon reaches the contractual target.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::exotics::tarn::{Tarn, TarnBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn")]
#[pyclass(
    name = "Tarn",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTarn {
    pub(crate) inner: Tarn,
}

instrument_envelope_methods!(
    PyTarn(
        "Tarn",
        module = "finstack_quant.valuations.instruments._typed_tarn"
    ),
    Tarn,
    builder = PyTarnBuilder("TarnBuilder", no_fields),
    seed = Tarn::builder(),
);
instrument_pricing_methods!(
    PyTarn,
    "Tarn",
    module = "finstack_quant.valuations.instruments._typed_tarn"
);

#[pymethods]
impl PyTarn {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.to_builder")]
    fn to_builder(&self) -> PyTarnBuilder {
        PyTarnBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        Tarn::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.fixed_rate")]
    #[getter]
    fn fixed_rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.fixed_rate)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.start_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.payment_dates")]
    #[getter]
    fn payment_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        {
            let dates = &self.inner.payment_dates;
            dates
                .iter()
                .map(|date| crate::bindings::date_utils::date_to_py(py, *date))
                .collect::<PyResult<Vec<_>>>()
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.index_tenor")]
    #[getter]
    fn index_tenor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.index_tenor)
    }
}

crate::bindings::macros::py_getters!(PyTarn |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.coupon_floor")]
    coupon_floor -> f64 = s.inner.coupon_floor;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.target_coupon")]
    target_coupon -> f64 = s.inner.target_coupon;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.forward_curve_id")]
    forward_curve_id -> String = s.inner.forward_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.vol_surface_id")]
    vol_surface_id -> Option<String> = s.inner.vol_surface_id.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "Tarn.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount = crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder")]
#[pyclass(
    name = "TarnBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyTarnBuilder {
    inner: Option<TarnBuilder>,
}

#[pymethods]
impl PyTarnBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: TarnBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.fixed_rate")]
    fn fixed_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::money::decimal_from_py(value)?;
        builder_set!(slf, |b: TarnBuilder| b.fixed_rate(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.coupon_floor")]
    fn coupon_floor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: TarnBuilder| b.coupon_floor(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.target_coupon")]
    fn target_coupon<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: TarnBuilder| b.target_coupon(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: TarnBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.start_date")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: TarnBuilder| b.start_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.payment_dates")]
    fn payment_dates<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = {
            let dates = value;
            dates
                .iter()
                .map(crate::bindings::date_utils::py_to_date)
                .collect::<PyResult<Vec<_>>>()?
        };
        builder_set!(slf, |b: TarnBuilder| b.payment_dates(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.index_tenor")]
    fn index_tenor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "index_tenor")?;
        builder_set!(slf, |b: TarnBuilder| b.index_tenor(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: TarnBuilder| b.forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: TarnBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: TarnBuilder| b.vol_surface_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: TarnBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_tarn", "TarnBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: TarnBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyTarn>()?;
    module.add_class::<PyTarnBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["Tarn", "TarnBuilder"];
