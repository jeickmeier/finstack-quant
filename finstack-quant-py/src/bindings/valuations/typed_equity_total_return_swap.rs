//! A swap exchanging equity total return for contractual financing.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::equity_trs::{
    EquityTotalReturnSwap, EquityTotalReturnSwapBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap")]
#[pyclass(
    name = "EquityTotalReturnSwap",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEquityTotalReturnSwap {
    pub(crate) inner: EquityTotalReturnSwap,
}

instrument_envelope_methods!(
    PyEquityTotalReturnSwap(
        "EquityTotalReturnSwap",
        module = "finstack_quant.valuations.instruments._typed_equity_total_return_swap"
    ),
    EquityTotalReturnSwap,
    builder = PyEquityTotalReturnSwapBuilder("EquityTotalReturnSwapBuilder", no_fields),
    seed = EquityTotalReturnSwap::builder(),
);
instrument_pricing_methods!(
    PyEquityTotalReturnSwap,
    "EquityTotalReturnSwap",
    module = "finstack_quant.valuations.instruments._typed_equity_total_return_swap"
);

#[pymethods]
impl PyEquityTotalReturnSwap {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.to_builder")]
    fn to_builder(&self) -> PyEquityTotalReturnSwapBuilder {
        PyEquityTotalReturnSwapBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        EquityTotalReturnSwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.underlying")]
    #[getter]
    fn underlying<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.underlying)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.financing_leg")]
    #[getter]
    fn financing_leg<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.financing_leg)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.schedule")]
    #[getter]
    fn schedule<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.schedule)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.side")]
    #[getter]
    fn side<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.side)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.initial_level")]
    #[getter]
    fn initial_level(&self) -> Option<f64> {
        self.inner.initial_level
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.past_fixings")]
    #[getter]
    fn past_fixings<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .past_fixings
            .iter()
            .map(|(d, x)| crate::bindings::date_utils::date_to_py(py, *d).map(|d| (d, *x)))
            .collect()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.margin_spec")]
    #[getter]
    fn margin_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.margin_spec)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.dividend_tax_rate")]
    #[getter]
    fn dividend_tax_rate(&self) -> f64 {
        self.inner.dividend_tax_rate
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.dividend_settlement")]
    #[getter]
    fn dividend_settlement<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.dividend_settlement)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwap.discrete_dividends")]
    #[getter]
    fn discrete_dividends<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .discrete_dividends
            .iter()
            .map(|(d, x)| crate::bindings::date_utils::date_to_py(py, *d).map(|d| (d, *x)))
            .collect()
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder")]
#[pyclass(
    name = "EquityTotalReturnSwapBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyEquityTotalReturnSwapBuilder {
    inner: Option<EquityTotalReturnSwapBuilder>,
}

#[pymethods]
impl PyEquityTotalReturnSwapBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.underlying")]
    fn underlying<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "underlying")?;
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b.underlying(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.financing_leg")]
    fn financing_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "financing_leg")?;
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b
            .financing_leg(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.schedule")]
    fn schedule<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "schedule")?;
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b.schedule(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.side")]
    fn side<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "side")?;
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b.side(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.initial_level")]
    fn initial_level<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b
            .initial_level_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.past_fixings")]
    fn past_fixings<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<(Bound<'_, PyAny>, f64)>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .into_iter()
            .map(|(d, x)| crate::bindings::date_utils::py_to_date(&d).map(|date| (date, x)))
            .collect::<PyResult<_>>()?;
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b.past_fixings(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.margin_spec")]
    fn margin_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "margin_spec")?;
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b
            .margin_spec_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.dividend_tax_rate")]
    fn dividend_tax_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b
            .dividend_tax_rate(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.dividend_settlement")]
    fn dividend_settlement<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "dividend_settlement")?;
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b
            .dividend_settlement(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.discrete_dividends")]
    fn discrete_dividends<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<(Bound<'_, PyAny>, f64)>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .into_iter()
            .map(|(d, x)| crate::bindings::date_utils::py_to_date(&d).map(|date| (date, x)))
            .collect::<PyResult<_>>()?;
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b
            .discrete_dividends(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_equity_total_return_swap", "EquityTotalReturnSwapBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: EquityTotalReturnSwapBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyEquityTotalReturnSwap>()?;
    module.add_class::<PyEquityTotalReturnSwapBuilder>()?;
    Ok(())
}
