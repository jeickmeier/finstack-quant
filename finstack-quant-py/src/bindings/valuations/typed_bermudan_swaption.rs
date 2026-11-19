//! An option to enter a swap on any admissible exercise date.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::swaption::{
    BermudanSwaption, BermudanSwaptionBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption")]
#[pyclass(
    name = "BermudanSwaption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBermudanSwaption {
    pub(crate) inner: BermudanSwaption,
}

instrument_envelope_methods!(
    PyBermudanSwaption(
        "BermudanSwaption",
        module = "finstack_quant.valuations.instruments._typed_bermudan_swaption"
    ),
    BermudanSwaption,
    builder = PyBermudanSwaptionBuilder("BermudanSwaptionBuilder", no_fields),
    seed = BermudanSwaption::builder(),
);
instrument_pricing_methods!(
    PyBermudanSwaption,
    "BermudanSwaption",
    module = "finstack_quant.valuations.instruments._typed_bermudan_swaption"
);

#[pymethods]
impl PyBermudanSwaption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption.to_builder")]
    fn to_builder(&self) -> PyBermudanSwaptionBuilder {
        PyBermudanSwaptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        BermudanSwaption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption.settlement")]
    #[getter]
    fn settlement<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.settlement)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption.exercise_schedule")]
    #[getter]
    fn exercise_schedule<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.exercise_schedule)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption.bermudan_type")]
    #[getter]
    fn bermudan_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.bermudan_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption.underlying_fixed_leg")]
    #[getter]
    fn underlying_fixed_leg(&self) -> super::typed_legs::PyFixedLegSpec {
        super::typed_legs::PyFixedLegSpec::from_inner(self.inner.underlying_fixed_leg.clone())
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaption.underlying_float_leg")]
    #[getter]
    fn underlying_float_leg(&self) -> super::typed_legs::PyFloatLegSpec {
        super::typed_legs::PyFloatLegSpec::from_inner(self.inner.underlying_float_leg.clone())
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaptionBuilder")]
#[pyclass(
    name = "BermudanSwaptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyBermudanSwaptionBuilder {
    inner: Option<BermudanSwaptionBuilder>,
}

#[pymethods]
impl PyBermudanSwaptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: BermudanSwaptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: BermudanSwaptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaptionBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: BermudanSwaptionBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaptionBuilder.settlement")]
    fn settlement<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "settlement")?;
        builder_set!(slf, |b: BermudanSwaptionBuilder| b.settlement(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: BermudanSwaptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaptionBuilder.exercise_schedule")]
    fn exercise_schedule<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "exercise_schedule")?;
        builder_set!(slf, |b: BermudanSwaptionBuilder| b.exercise_schedule(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaptionBuilder.bermudan_type")]
    fn bermudan_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "bermudan_type")?;
        builder_set!(slf, |b: BermudanSwaptionBuilder| b.bermudan_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaptionBuilder.underlying_fixed_leg")]
    fn underlying_fixed_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &super::typed_legs::PyFixedLegSpec,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: BermudanSwaptionBuilder| b
            .underlying_fixed_leg(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaptionBuilder.underlying_float_leg")]
    fn underlying_float_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &super::typed_legs::PyFloatLegSpec,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: BermudanSwaptionBuilder| b
            .underlying_float_leg(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bermudan_swaption", "BermudanSwaptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: BermudanSwaptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyBermudanSwaption>()?;
    module.add_class::<PyBermudanSwaptionBuilder>()?;
    Ok(())
}
