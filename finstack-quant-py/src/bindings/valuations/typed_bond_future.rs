//! Typed deliverable bond futures based on the native contract builder.

use crate::bindings::date_utils::py_to_date;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    BondFuture, BondFutureBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::instruments::PyBond;
use super::typed_listed::PyListedFutureTerms;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture")]
#[pyclass(
    name = "BondFuture",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBondFuture {
    pub(crate) inner: BondFuture,
}

instrument_envelope_methods!(
    PyBondFuture(
        "BondFuture",
        module = "finstack_quant.valuations.instruments._typed_bond_future"
    ),
    BondFuture,
    builder = PyBondFutureBuilder("BondFutureBuilder", no_fields),
    seed = BondFuture::builder(),
);
instrument_pricing_methods!(
    PyBondFuture,
    "BondFuture",
    module = "finstack_quant.valuations.instruments._typed_bond_future"
);

#[pymethods]
impl PyBondFuture {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture.to_builder")]
    fn to_builder(&self) -> PyBondFutureBuilder {
        PyBondFutureBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        BondFuture::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture.delivery_start")]
    #[getter]
    fn delivery_start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.delivery_start)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture.contract_specs")]
    #[getter]
    fn contract_specs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.contract_specs)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture.deliverable_basket")]
    #[getter]
    fn deliverable_basket<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.deliverable_basket)
    }
}

crate::bindings::macros::py_getters!(PyBondFuture |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture.terms")]
    terms -> PyListedFutureTerms = PyListedFutureTerms {
        inner: s.inner.terms.clone(),
    };

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture.ctd_bond_id")]
    ctd_bond_id -> Option<String> = s.inner.ctd_bond_id.as_ref().map(|id| id.to_string());

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture.ctd_bond")]
    ctd_bond -> Option<PyBond> = s.inner.ctd_bond.as_ref().map(|bond| PyBond {
        inner: bond.clone(),
    });

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFuture.repo_curve_id")]
    repo_curve_id -> Option<String> = s.inner.repo_curve_id.as_ref().map(|id| id.to_string());
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFutureBuilder")]
#[pyclass(
    name = "BondFutureBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyBondFutureBuilder {
    inner: Option<BondFutureBuilder>,
}

#[pymethods]
impl PyBondFutureBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFutureBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: BondFutureBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFutureBuilder.terms")]
    fn terms<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyListedFutureTerms,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: BondFutureBuilder| b.terms(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFutureBuilder.delivery_start")]
    fn delivery_start<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_date(value)?;
        builder_set!(slf, |b: BondFutureBuilder| b.delivery_start(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFutureBuilder.contract_specs")]
    fn contract_specs<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "contract_specs")?;
        builder_set!(slf, |b: BondFutureBuilder| b.contract_specs(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFutureBuilder.deliverable_basket")]
    fn deliverable_basket<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "deliverable_basket")?;
        builder_set!(slf, |b: BondFutureBuilder| b.deliverable_basket(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFutureBuilder.ctd_bond_id")]
    fn ctd_bond_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: BondFutureBuilder| b.ctd_bond_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFutureBuilder.ctd_bond")]
    fn ctd_bond<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyBond,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner.clone();
        builder_set!(slf, |b: BondFutureBuilder| b.ctd_bond(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFutureBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CurveId::new(value);
        builder_set!(slf, |b: BondFutureBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFutureBuilder.repo_curve_id")]
    fn repo_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CurveId::new(value);
        builder_set!(slf, |b: BondFutureBuilder| b.repo_curve_id(value))
    }
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_bond_future", "BondFutureBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: BondFutureBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyBondFuture>()?;
    module.add_class::<PyBondFutureBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["BondFuture", "BondFutureBuilder"];
