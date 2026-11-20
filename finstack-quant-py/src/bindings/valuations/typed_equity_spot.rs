//! An equity holding with a quoted or market-observed share price.

use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::spot::{Equity, EquityBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity")]
#[pyclass(
    name = "Equity",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEquity {
    pub(crate) inner: Equity,
}

instrument_envelope_methods!(
    PyEquity(
        "Equity",
        module = "finstack_quant.valuations.instruments._typed_equity_spot"
    ),
    Equity,
    builder = PyEquityBuilder("EquityBuilder", no_fields),
    seed = Equity::builder(),
);
instrument_pricing_methods!(
    PyEquity,
    "Equity",
    module = "finstack_quant.valuations.instruments._typed_equity_spot"
);

#[pymethods]
impl PyEquity {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity.to_builder")]
    fn to_builder(&self) -> PyEquityBuilder {
        PyEquityBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        Equity::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity.ticker")]
    #[getter]
    fn ticker(&self) -> String {
        self.inner.ticker.clone()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity.currency")]
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity.quantity")]
    #[getter]
    fn quantity(&self) -> Option<f64> {
        self.inner.quantity
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity.quoted_spot")]
    #[getter]
    fn quoted_spot(&self) -> Option<f64> {
        self.inner.quoted_spot
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity.spot_id")]
    #[getter]
    fn spot_id(&self) -> Option<String> {
        self.inner.spot_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity.div_yield_id")]
    #[getter]
    fn div_yield_id(&self) -> Option<String> {
        self.inner.div_yield_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "Equity.discrete_dividends")]
    #[getter]
    fn discrete_dividends<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .discrete_dividends
            .iter()
            .map(|(date, amount)| {
                crate::bindings::date_utils::date_to_py(py, *date)
                    .map(|day| (day.into_any(), *amount))
            })
            .collect()
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "EquityBuilder")]
#[pyclass(
    name = "EquityBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyEquityBuilder {
    inner: Option<EquityBuilder>,
}

#[pymethods]
impl PyEquityBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "EquityBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: EquityBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "EquityBuilder.ticker")]
    fn ticker<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.to_owned();
        builder_set!(slf, |b: EquityBuilder| b.ticker(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "EquityBuilder.currency")]
    fn currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: EquityBuilder| b.currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "EquityBuilder.quantity")]
    fn quantity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: EquityBuilder| b.quantity_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "EquityBuilder.quoted_spot")]
    fn quoted_spot<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: EquityBuilder| b.quoted_spot_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "EquityBuilder.spot_id")]
    fn spot_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: EquityBuilder| b.spot_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "EquityBuilder.div_yield_id")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: EquityBuilder| b.div_yield_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "EquityBuilder.discrete_dividends")]
    fn discrete_dividends<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<(Bound<'_, PyAny>, f64)>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .into_iter()
            .map(|(date, amount)| {
                crate::bindings::date_utils::py_to_date(&date).map(|date| (date, amount))
            })
            .collect::<PyResult<_>>()?;
        builder_set!(slf, |b: EquityBuilder| b.discrete_dividends(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "EquityBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: EquityBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_spot", "EquityBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: EquityBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyEquity>()?;
    module.add_class::<PyEquityBuilder>()?;
    Ok(())
}
