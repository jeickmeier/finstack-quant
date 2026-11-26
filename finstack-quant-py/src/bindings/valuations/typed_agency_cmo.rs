//! An agency collateralized mortgage obligation priced through its waterfall.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::{DealId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::cmo::{AgencyCmo, AgencyCmoBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo")]
#[pyclass(
    name = "AgencyCmo",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAgencyCmo {
    pub(crate) inner: AgencyCmo,
}

instrument_envelope_methods!(
    PyAgencyCmo(
        "AgencyCmo",
        module = "finstack_quant.valuations.instruments._typed_agency_cmo"
    ),
    AgencyCmo,
    builder = PyAgencyCmoBuilder("AgencyCmoBuilder", no_fields),
    seed = AgencyCmo::builder(),
);
instrument_trait_methods!(
    PyAgencyCmo,
    "AgencyCmo",
    module = "finstack_quant.valuations.instruments._typed_agency_cmo"
);

#[pymethods]
impl PyAgencyCmo {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.to_builder")]
    fn to_builder(&self) -> PyAgencyCmoBuilder {
        PyAgencyCmoBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        AgencyCmo::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.agency")]
    #[getter]
    fn agency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.agency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.issue_date")]
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.issue_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.waterfall")]
    #[getter]
    fn waterfall<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.waterfall)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.collateral")]
    #[getter]
    fn collateral<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.collateral)
    }
}

crate::bindings::macros::py_getters!(PyAgencyCmo |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.deal_name")]
    deal_name -> String = s.inner.deal_name.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.reference_tranche_id")]
    reference_tranche_id -> String = s.inner.reference_tranche_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.collateral_wac")]
    collateral_wac -> Option<f64> = s.inner.collateral_wac;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.collateral_wam_months")]
    collateral_wam_months -> Option<u32> = s.inner.collateral_wam_months;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmo.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder")]
#[pyclass(
    name = "AgencyCmoBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyAgencyCmoBuilder {
    inner: Option<AgencyCmoBuilder>,
}

#[pymethods]
impl PyAgencyCmoBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: AgencyCmoBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder.deal_name")]
    fn deal_name<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = DealId::new(value);
        builder_set!(slf, |b: AgencyCmoBuilder| b.deal_name(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder.agency")]
    fn agency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "agency")?;
        builder_set!(slf, |b: AgencyCmoBuilder| b.agency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder.issue_date")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: AgencyCmoBuilder| b.issue_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder.waterfall")]
    fn waterfall<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "waterfall")?;
        builder_set!(slf, |b: AgencyCmoBuilder| b.waterfall(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder.reference_tranche_id")]
    fn reference_tranche_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AgencyCmoBuilder| b.reference_tranche_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder.collateral")]
    fn collateral<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "collateral")?;
        builder_set!(slf, |b: AgencyCmoBuilder| b.collateral_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder.collateral_wac")]
    fn collateral_wac<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyCmoBuilder| b.collateral_wac_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder.collateral_wam_months")]
    fn collateral_wam_months<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<u32>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AgencyCmoBuilder| b
            .collateral_wam_months_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AgencyCmoBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_agency_cmo", "AgencyCmoBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: AgencyCmoBuilder| b.attributes(value))
    }
}

/// Register the agency CMO and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyAgencyCmo>()?;
    module.add_class::<PyAgencyCmoBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["AgencyCmo", "AgencyCmoBuilder"];
