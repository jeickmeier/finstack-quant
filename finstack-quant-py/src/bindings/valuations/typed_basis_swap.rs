//! A basis swap exchanging flows on the specified floating-rate indices.

use crate::bindings::core::money::PyMoney;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::basis_swap::{BasisSwap, BasisSwapBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwap")]
#[pyclass(
    name = "BasisSwap",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBasisSwap {
    pub(crate) inner: BasisSwap,
}

instrument_envelope_methods!(
    PyBasisSwap(
        "BasisSwap",
        module = "finstack_quant.valuations.instruments._typed_basis_swap"
    ),
    BasisSwap,
    builder = PyBasisSwapBuilder("BasisSwapBuilder", no_fields),
    seed = BasisSwap::builder(),
);
instrument_trait_methods!(
    PyBasisSwap,
    "BasisSwap",
    module = "finstack_quant.valuations.instruments._typed_basis_swap"
);

#[pymethods]
impl PyBasisSwap {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwap.to_builder")]
    fn to_builder(&self) -> PyBasisSwapBuilder {
        PyBasisSwapBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwap.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        BasisSwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }
}

crate::bindings::macros::py_getters!(PyBasisSwap |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwap.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwap.primary_leg")]
    primary_leg -> super::typed_legs::PyFloatLegSpec = super::typed_legs::PyFloatLegSpec::from_inner(s.inner.primary_leg.clone());
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwap.reference_leg")]
    reference_leg -> super::typed_legs::PyFloatLegSpec = super::typed_legs::PyFloatLegSpec::from_inner(s.inner.reference_leg.clone());
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwap.allow_calendar_fallback")]
    allow_calendar_fallback -> bool = s.inner.allow_calendar_fallback;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwap.allow_same_curve")]
    allow_same_curve -> bool = s.inner.allow_same_curve;
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwapBuilder")]
#[pyclass(
    name = "BasisSwapBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyBasisSwapBuilder {
    inner: Option<BasisSwapBuilder>,
}

#[pymethods]
impl PyBasisSwapBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwapBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: BasisSwapBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwapBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: BasisSwapBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwapBuilder.primary_leg")]
    fn primary_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &super::typed_legs::PyFloatLegSpec,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: BasisSwapBuilder| b.primary_leg(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwapBuilder.reference_leg")]
    fn reference_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &super::typed_legs::PyFloatLegSpec,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: BasisSwapBuilder| b.reference_leg(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwapBuilder.allow_calendar_fallback")]
    fn allow_calendar_fallback<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: BasisSwapBuilder| b.allow_calendar_fallback(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwapBuilder.allow_same_curve")]
    fn allow_same_curve<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: BasisSwapBuilder| b.allow_same_curve(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basis_swap", "BasisSwapBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: BasisSwapBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyBasisSwap>()?;
    module.add_class::<PyBasisSwapBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["BasisSwap", "BasisSwapBuilder"];
