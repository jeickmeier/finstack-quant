//! A commodity forward with contractual delivery and settlement.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::commodity::commodity_forward::{
    CommodityForward, CommodityForwardBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward")]
#[pyclass(
    name = "CommodityForward",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCommodityForward {
    pub(crate) inner: CommodityForward,
}

instrument_envelope_methods!(
    PyCommodityForward(
        "CommodityForward",
        module = "finstack_quant.valuations.instruments._typed_commodity_forward"
    ),
    CommodityForward,
    builder = PyCommodityForwardBuilder("CommodityForwardBuilder", no_fields),
    seed = CommodityForward::builder(),
);
instrument_trait_methods!(
    PyCommodityForward,
    "CommodityForward",
    module = "finstack_quant.valuations.instruments._typed_commodity_forward"
);

#[pymethods]
impl PyCommodityForward {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.to_builder")]
    fn to_builder(&self) -> PyCommodityForwardBuilder {
        PyCommodityForwardBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CommodityForward::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.underlying")]
    #[getter]
    fn underlying<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.underlying)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.settlement")]
    #[getter]
    fn settlement<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.settlement)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.position")]
    #[getter]
    fn position<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.position)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.convention")]
    #[getter]
    fn convention<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.convention)
    }
}

crate::bindings::macros::py_getters!(PyCommodityForward |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.quantity")]
    quantity -> f64 = s.inner.quantity;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.multiplier")]
    multiplier -> f64 = s.inner.multiplier;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.contract_price")]
    contract_price -> Option<f64> = s.inner.contract_price;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.quoted_forward")]
    quoted_forward -> Option<f64> = s.inner.quoted_forward;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.forward_curve_id")]
    forward_curve_id -> String = s.inner.forward_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.spot_id")]
    spot_id -> Option<String> = s.inner.spot_id.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.exchange")]
    exchange -> Option<String> = s.inner.exchange.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.contract_month")]
    contract_month -> Option<String> = s.inner.contract_month.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.settlement_days")]
    settlement_days -> Option<u32> = s.inner.settlement_days;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.settlement_calendar_id")]
    settlement_calendar_id -> Option<String> = s.inner .settlement_calendar_id .as_ref() .map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForward.settlement_business_day_convention")]
    settlement_business_day_convention -> PyResult<Option<String>> = s.inner .settlement_business_day_convention .as_ref() .map(super::convert::enum_to_py_string) .transpose();
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder")]
#[pyclass(
    name = "CommodityForwardBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCommodityForwardBuilder {
    inner: Option<CommodityForwardBuilder>,
}

#[pymethods]
impl PyCommodityForwardBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CommodityForwardBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.underlying")]
    fn underlying<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "underlying")?;
        builder_set!(slf, |b: CommodityForwardBuilder| b.underlying(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.quantity")]
    fn quantity<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommodityForwardBuilder| b.quantity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.multiplier")]
    fn multiplier<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommodityForwardBuilder| b.multiplier(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CommodityForwardBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.settlement")]
    fn settlement<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "settlement")?;
        builder_set!(slf, |b: CommodityForwardBuilder| b.settlement(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.position")]
    fn position<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "position")?;
        builder_set!(slf, |b: CommodityForwardBuilder| b.position(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.contract_price")]
    fn contract_price<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommodityForwardBuilder| b
            .contract_price_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.quoted_forward")]
    fn quoted_forward<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommodityForwardBuilder| b
            .quoted_forward_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommodityForwardBuilder| b.forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.spot_id")]
    fn spot_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CommodityForwardBuilder| b.spot_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommodityForwardBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.exchange")]
    fn exchange<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CommodityForwardBuilder| b.exchange_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.contract_month")]
    fn contract_month<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CommodityForwardBuilder| b
            .contract_month_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.convention")]
    fn convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "convention")?;
        builder_set!(slf, |b: CommodityForwardBuilder| b.convention_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.settlement_days")]
    fn settlement_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<u32>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommodityForwardBuilder| b
            .settlement_days_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.settlement_calendar_id")]
    fn settlement_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CommodityForwardBuilder| b
            .settlement_calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.settlement_business_day_convention")]
    fn settlement_business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "settlement_business_day_convention")?;
        builder_set!(slf, |b: CommodityForwardBuilder| b
            .settlement_business_day_convention_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_forward", "CommodityForwardBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CommodityForwardBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCommodityForward>()?;
    module.add_class::<PyCommodityForwardBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["CommodityForward", "CommodityForwardBuilder"];
