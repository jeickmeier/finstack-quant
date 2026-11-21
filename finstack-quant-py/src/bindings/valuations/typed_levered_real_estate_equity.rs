//! An equity residual after the specified real-estate funding claims.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::real_estate::{
    LeveredRealEstateEquity, LeveredRealEstateEquityBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquity")]
#[pyclass(
    name = "LeveredRealEstateEquity",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyLeveredRealEstateEquity {
    pub(crate) inner: LeveredRealEstateEquity,
}

instrument_envelope_methods!(
    PyLeveredRealEstateEquity(
        "LeveredRealEstateEquity",
        module = "finstack_quant.valuations.instruments._typed_levered_real_estate_equity"
    ),
    LeveredRealEstateEquity,
    builder = PyLeveredRealEstateEquityBuilder("LeveredRealEstateEquityBuilder", no_fields),
    seed = LeveredRealEstateEquity::builder(),
);
instrument_pricing_methods!(
    PyLeveredRealEstateEquity,
    "LeveredRealEstateEquity",
    module = "finstack_quant.valuations.instruments._typed_levered_real_estate_equity"
);

#[pymethods]
impl PyLeveredRealEstateEquity {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquity.to_builder")]
    fn to_builder(&self) -> PyLeveredRealEstateEquityBuilder {
        PyLeveredRealEstateEquityBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquity.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        LeveredRealEstateEquity::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquity.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquity.currency")]
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquity.asset")]
    #[getter]
    fn asset<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.asset)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquity.financing")]
    #[getter]
    fn financing<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.financing)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquityBuilder")]
#[pyclass(
    name = "LeveredRealEstateEquityBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyLeveredRealEstateEquityBuilder {
    inner: Option<LeveredRealEstateEquityBuilder>,
}

#[pymethods]
impl PyLeveredRealEstateEquityBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquityBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: LeveredRealEstateEquityBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquityBuilder.currency")]
    fn currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: LeveredRealEstateEquityBuilder| b.currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquityBuilder.asset")]
    fn asset<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "asset")?;
        builder_set!(slf, |b: LeveredRealEstateEquityBuilder| b.asset(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquityBuilder.financing")]
    fn financing<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "financing")?;
        builder_set!(slf, |b: LeveredRealEstateEquityBuilder| b.financing(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_levered_real_estate_equity", "LeveredRealEstateEquityBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: LeveredRealEstateEquityBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyLeveredRealEstateEquity>()?;
    module.add_class::<PyLeveredRealEstateEquityBuilder>()?;
    Ok(())
}
