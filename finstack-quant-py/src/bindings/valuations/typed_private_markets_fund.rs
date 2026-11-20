//! A private-markets fund valued from its dated commitments and cash calls.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::pe_fund::{
    PrivateMarketsFund, PrivateMarketsFundBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFund")]
#[pyclass(
    name = "PrivateMarketsFund",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPrivateMarketsFund {
    pub(crate) inner: PrivateMarketsFund,
}

instrument_envelope_methods!(
    PyPrivateMarketsFund(
        "PrivateMarketsFund",
        module = "finstack_quant.valuations.instruments._typed_private_markets_fund"
    ),
    PrivateMarketsFund,
    builder = PyPrivateMarketsFundBuilder("PrivateMarketsFundBuilder", no_fields),
    seed = PrivateMarketsFund::builder(),
);
instrument_pricing_methods!(
    PyPrivateMarketsFund,
    "PrivateMarketsFund",
    module = "finstack_quant.valuations.instruments._typed_private_markets_fund"
);

#[pymethods]
impl PyPrivateMarketsFund {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFund.to_builder")]
    fn to_builder(&self) -> PyPrivateMarketsFundBuilder {
        PyPrivateMarketsFundBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFund.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        PrivateMarketsFund::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFund.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFund.currency")]
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFund.waterfall_spec")]
    #[getter]
    fn waterfall_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.waterfall_spec)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFund.events")]
    #[getter]
    fn events<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.events)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFund.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> Option<String> {
        self.inner
            .discount_curve_id
            .as_ref()
            .map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFund.unrealized_nav")]
    #[getter]
    fn unrealized_nav(&self) -> Option<PyMoney> {
        self.inner.unrealized_nav.map(PyMoney::from_inner)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFundBuilder")]
#[pyclass(
    name = "PrivateMarketsFundBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyPrivateMarketsFundBuilder {
    inner: Option<PrivateMarketsFundBuilder>,
}

#[pymethods]
impl PyPrivateMarketsFundBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFundBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: PrivateMarketsFundBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFundBuilder.currency")]
    fn currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: PrivateMarketsFundBuilder| b.currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFundBuilder.waterfall_spec")]
    fn waterfall_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "waterfall_spec")?;
        builder_set!(slf, |b: PrivateMarketsFundBuilder| b.waterfall_spec(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFundBuilder.events")]
    fn events<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "events")?;
        builder_set!(slf, |b: PrivateMarketsFundBuilder| b.events(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFundBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: PrivateMarketsFundBuilder| b
            .discount_curve_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFundBuilder.unrealized_nav")]
    fn unrealized_nav<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(|money| money.inner);
        builder_set!(slf, |b: PrivateMarketsFundBuilder| b
            .unrealized_nav_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_private_markets_fund", "PrivateMarketsFundBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: PrivateMarketsFundBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyPrivateMarketsFund>()?;
    module.add_class::<PyPrivateMarketsFundBuilder>()?;
    Ok(())
}
