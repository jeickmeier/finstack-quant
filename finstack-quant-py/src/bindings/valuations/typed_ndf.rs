//! A non-deliverable FX forward with an explicit fixing regime.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fx::ndf::{Ndf, NdfBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf")]
#[pyclass(
    name = "Ndf",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyNdf {
    pub(crate) inner: Ndf,
}

instrument_envelope_methods!(
    PyNdf(
        "Ndf",
        module = "finstack_quant.valuations.instruments._typed_ndf"
    ),
    Ndf,
    builder = PyNdfBuilder("NdfBuilder", no_fields),
    seed = Ndf::builder(),
);
instrument_pricing_methods!(
    PyNdf,
    "Ndf",
    module = "finstack_quant.valuations.instruments._typed_ndf"
);

#[pymethods]
impl PyNdf {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.to_builder")]
    fn to_builder(&self) -> PyNdfBuilder {
        PyNdfBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        Ndf::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.fixing_date")]
    #[getter]
    fn fixing_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.fixing_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.quote_convention")]
    #[getter]
    fn quote_convention<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.quote_convention)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.fixing_source")]
    #[getter]
    fn fixing_source<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.fixing_source)
    }
}

crate::bindings::macros::py_getters!(PyNdf |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.base_currency")]
    base_currency -> String = s.inner.base_currency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.settlement_currency")]
    settlement_currency -> String = s.inner.settlement_currency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.contract_rate")]
    contract_rate -> Option<f64> = s.inner.contract_rate;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.domestic_discount_curve_id")]
    domestic_discount_curve_id -> String = s.inner.domestic_discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.foreign_discount_curve_id")]
    foreign_discount_curve_id -> Option<String> = s
        .inner
        .foreign_discount_curve_id
        .as_ref()
        .map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.observed_fixing")]
    observed_fixing -> Option<f64> = s.inner.observed_fixing;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.quoted_spot")]
    quoted_spot -> Option<f64> = s.inner.quoted_spot;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.quoted_forward")]
    quoted_forward -> Option<f64> = s.inner.quoted_forward;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.base_calendar_id")]
    base_calendar_id -> Option<String> = s
        .inner
        .base_calendar_id
        .as_ref()
        .map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "Ndf.settlement_calendar_id")]
    settlement_calendar_id -> Option<String> = s
        .inner
        .settlement_calendar_id
        .as_ref()
        .map(ToString::to_string);
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder")]
#[pyclass(
    name = "NdfBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyNdfBuilder {
    inner: Option<NdfBuilder>,
}

#[pymethods]
impl PyNdfBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: NdfBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.base_currency")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: NdfBuilder| b.base_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.settlement_currency")]
    fn settlement_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: NdfBuilder| b.settlement_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.fixing_date")]
    fn fixing_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: NdfBuilder| b.fixing_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: NdfBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: NdfBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.contract_rate")]
    fn contract_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: NdfBuilder| b.contract_rate_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.domestic_discount_curve_id")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: NdfBuilder| b.domestic_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.quote_convention")]
    fn quote_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "quote_convention")?;
        builder_set!(slf, |b: NdfBuilder| b.quote_convention(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.foreign_discount_curve_id")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: NdfBuilder| b.foreign_discount_curve_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.observed_fixing")]
    fn observed_fixing<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: NdfBuilder| b.observed_fixing_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.fixing_source")]
    fn fixing_source<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "fixing_source")?;
        builder_set!(slf, |b: NdfBuilder| b.fixing_source_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.quoted_spot")]
    fn quoted_spot<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: NdfBuilder| b.quoted_spot_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.quoted_forward")]
    fn quoted_forward<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: NdfBuilder| b.quoted_forward_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.base_calendar_id")]
    fn base_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: NdfBuilder| b.base_calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.settlement_calendar_id")]
    fn settlement_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: NdfBuilder| b.settlement_calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_ndf", "NdfBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: NdfBuilder| b.attributes(value))
    }
}

/// Register the NDF and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyNdf>()?;
    module.add_class::<PyNdfBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["Ndf", "NdfBuilder"];
