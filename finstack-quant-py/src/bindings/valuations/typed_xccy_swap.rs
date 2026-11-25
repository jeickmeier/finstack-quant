//! A cross-currency swap exchanging flows in two currencies.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::xccy_swap::{XccySwap, XccySwapBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwap")]
#[pyclass(
    name = "XccySwap",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyXccySwap {
    pub(crate) inner: XccySwap,
}

instrument_envelope_methods!(
    PyXccySwap(
        "XccySwap",
        module = "finstack_quant.valuations.instruments._typed_xccy_swap"
    ),
    XccySwap,
    builder = PyXccySwapBuilder("XccySwapBuilder", no_fields),
    seed = XccySwap::builder(),
);
instrument_pricing_methods!(
    PyXccySwap,
    "XccySwap",
    module = "finstack_quant.valuations.instruments._typed_xccy_swap"
);

#[pymethods]
impl PyXccySwap {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwap.to_builder")]
    fn to_builder(&self) -> PyXccySwapBuilder {
        PyXccySwapBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwap.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        XccySwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwap.leg1")]
    #[getter]
    fn leg1<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.leg1)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwap.leg2")]
    #[getter]
    fn leg2<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.leg2)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwap.notional_exchange")]
    #[getter]
    fn notional_exchange<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.notional_exchange)
    }
}

crate::bindings::macros::py_getters!(PyXccySwap |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwap.reporting_currency")]
    reporting_currency -> String = s.inner.reporting_currency.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwap.allow_calendar_fallback")]
    allow_calendar_fallback -> bool = s.inner.allow_calendar_fallback;
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwapBuilder")]
#[pyclass(
    name = "XccySwapBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyXccySwapBuilder {
    inner: Option<XccySwapBuilder>,
}

#[pymethods]
impl PyXccySwapBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwapBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: XccySwapBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwapBuilder.leg1")]
    fn leg1<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "leg1")?;
        builder_set!(slf, |b: XccySwapBuilder| b.leg1(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwapBuilder.leg2")]
    fn leg2<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "leg2")?;
        builder_set!(slf, |b: XccySwapBuilder| b.leg2(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwapBuilder.notional_exchange")]
    fn notional_exchange<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "notional_exchange")?;
        builder_set!(slf, |b: XccySwapBuilder| b.notional_exchange(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwapBuilder.reporting_currency")]
    fn reporting_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: XccySwapBuilder| b.reporting_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwapBuilder.allow_calendar_fallback")]
    fn allow_calendar_fallback<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: XccySwapBuilder| b.allow_calendar_fallback(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_xccy_swap", "XccySwapBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: XccySwapBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyXccySwap>()?;
    module.add_class::<PyXccySwapBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["XccySwap", "XccySwapBuilder"];
