//! An autocallable equity-linked claim with explicit trigger and settlement terms.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::autocallable::{
    Autocallable, AutocallableBuilder,
};
use pyo3::prelude::*;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable")]
#[pyclass(
    name = "Autocallable",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAutocallable {
    pub(crate) inner: Autocallable,
}

instrument_envelope_methods!(
    PyAutocallable(
        "Autocallable",
        module = "finstack_quant.valuations.instruments._typed_autocallable"
    ),
    Autocallable,
    builder = PyAutocallableBuilder("AutocallableBuilder", no_fields),
    seed = Autocallable::builder(),
);
instrument_pricing_methods!(
    PyAutocallable,
    "Autocallable",
    module = "finstack_quant.valuations.instruments._typed_autocallable"
);

#[pymethods]
impl PyAutocallable {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.to_builder")]
    fn to_builder(&self) -> PyAutocallableBuilder {
        PyAutocallableBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        Autocallable::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.observation_dates")]
    #[getter]
    fn observation_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        {
            let dates = &self.inner.observation_dates;
            dates
                .iter()
                .map(|date| crate::bindings::date_utils::date_to_py(py, *date))
                .collect::<PyResult<Vec<_>>>()
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.payment_dates")]
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

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.final_payoff_type")]
    #[getter]
    fn final_payoff_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.final_payoff_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.path_model")]
    #[getter]
    fn path_model<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.path_model)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.past_fixings")]
    #[getter]
    fn past_fixings<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .past_fixings
            .iter()
            .map(|(d, x)| crate::bindings::date_utils::date_to_py(py, *d).map(|d| (d, *x)))
            .collect()
    }
}

crate::bindings::macros::py_getters!(PyAutocallable |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.underlying_ticker")]
    underlying_ticker -> String = s.inner.underlying_ticker.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.autocall_barriers")]
    autocall_barriers -> Vec<f64> = s.inner.autocall_barriers.clone();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.coupon_barriers")]
    coupon_barriers -> Vec<f64> = s.inner.coupon_barriers.clone();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.coupons")]
    coupons -> Vec<f64> = s.inner.coupons.clone();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.memory_coupons")]
    memory_coupons -> bool = s.inner.memory_coupons;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.final_barrier")]
    final_barrier -> f64 = s.inner.final_barrier;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.cap_level")]
    cap_level -> f64 = s.inner.cap_level;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount = crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.spot_id")]
    spot_id -> String = s.inner.spot_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.div_yield_id")]
    div_yield_id -> Option<String> = s.inner.div_yield_id.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "Autocallable.initial_level")]
    initial_level -> Option<f64> = s.inner.initial_level;
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder")]
#[pyclass(
    name = "AutocallableBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyAutocallableBuilder {
    inner: Option<AutocallableBuilder>,
}

#[pymethods]
impl PyAutocallableBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: AutocallableBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.underlying_ticker")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AutocallableBuilder| b.underlying_ticker(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.observation_dates")]
    fn observation_dates<'py>(
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
        builder_set!(slf, |b: AutocallableBuilder| b.observation_dates(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.payment_dates")]
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
        builder_set!(slf, |b: AutocallableBuilder| b.payment_dates(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: AutocallableBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.autocall_barriers")]
    fn autocall_barriers<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AutocallableBuilder| b.autocall_barriers(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.coupon_barriers")]
    fn coupon_barriers<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AutocallableBuilder| b.coupon_barriers(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.coupons")]
    fn coupons<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AutocallableBuilder| b.coupons(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.memory_coupons")]
    fn memory_coupons<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AutocallableBuilder| b.memory_coupons(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.final_barrier")]
    fn final_barrier<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AutocallableBuilder| b.final_barrier(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.final_payoff_type")]
    fn final_payoff_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "final_payoff_type")?;
        builder_set!(slf, |b: AutocallableBuilder| b.final_payoff_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.cap_level")]
    fn cap_level<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AutocallableBuilder| b.cap_level(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: AutocallableBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: AutocallableBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AutocallableBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.spot_id")]
    fn spot_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AutocallableBuilder| b.spot_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AutocallableBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.path_model")]
    fn path_model<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "path_model")?;
        builder_set!(slf, |b: AutocallableBuilder| b.path_model(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.div_yield_id")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: AutocallableBuilder| b.div_yield_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.initial_level")]
    fn initial_level<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AutocallableBuilder| b.initial_level_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.past_fixings")]
    fn past_fixings<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<(Bound<'_, PyAny>, f64)>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .into_iter()
            .map(|(d, x)| crate::bindings::date_utils::py_to_date(&d).map(|date| (date, x)))
            .collect::<PyResult<_>>()?;
        builder_set!(slf, |b: AutocallableBuilder| b.past_fixings(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_autocallable", "AutocallableBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: AutocallableBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyAutocallable>()?;
    module.add_class::<PyAutocallableBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["Autocallable", "AutocallableBuilder"];
