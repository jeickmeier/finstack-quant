//! Credit default swap Python wrappers and fluent builder.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::market::conventions::ids::CdsDocClause;
use finstack_quant_valuations::market::conventions::CdsConvention;
use finstack_quant_valuations::market::conventions::CdsValuationConvention;
use rust_decimal::prelude::ToPrimitive;

use super::super::convert::enum_from_str;
use super::super::convert::{
    attributes_from_py, date_repr, dated_money_from_py, enum_to_py_string, money_repr, money_to_py,
    opt_repr,
};
use super::super::typed_legs::{PyPremiumLegSpec, PyProtectionLegSpec};
use super::super::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};

use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwapBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CreditDefaultSwap",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCreditDefaultSwap {
    /// Inner canonical Rust CDS.
    pub(crate) inner:
        finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap,
}

instrument_envelope_methods!(
    PyCreditDefaultSwap("CreditDefaultSwap"),
    CreditDefaultSwap,
    builder = PyCreditDefaultSwapBuilder("CreditDefaultSwapBuilder", fields),
    seed =
        finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap::builder(
        ),
);
instrument_pricing_methods!(PyCreditDefaultSwap, "CreditDefaultSwap");

#[pymethods]
impl PyCreditDefaultSwap {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap::example()
                .map_err(core_to_py)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.par_spread")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn par_spread(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        self.inner.par_spread(&market, as_of).map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.upfront")]
    #[getter]
    fn upfront<'py>(&self, py: Python<'py>) -> PyResult<Option<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .upfront
            .map(|(date, money)| Ok((date_to_py(py, date)?, money_to_py(money))))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.protection_effective_date")]
    #[getter]
    fn protection_effective_date<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .protection_effective_date
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.protection_start")]
    #[getter]
    fn protection_start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.protection_start())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.margin_spec")]
    #[getter]
    fn margin_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .margin_spec
            .as_ref()
            .map(|spec| crate::bindings::pandas_utils::serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Instrument::expiry(&self.inner)
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CreditDefaultSwap(id={:?}, side={:?}, notional={}, coupon_bp={}, start={}, end={}, convention={:?})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.side).unwrap_or_default(),
            money_repr(self.inner.notional),
            opt_repr(self.inner.premium_leg.coupon_bp.to_f64()),
            date_repr(self.inner.premium_leg.start),
            date_repr(self.inner.premium_leg.end),
            enum_to_py_string(&self.inner.convention).unwrap_or_default(),
        )
    }
}

crate::bindings::macros::py_getters!(PyCreditDefaultSwap |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.side")]
    side -> PyResult<String> = enum_to_py_string(&s.inner.side);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.convention")]
    convention -> PyResult<String> = enum_to_py_string(&s.inner.convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.premium_leg")]
    premium_leg -> PyPremiumLegSpec = PyPremiumLegSpec { inner: s.inner.premium_leg.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.protection_leg")]
    protection_leg -> PyProtectionLegSpec =
        PyProtectionLegSpec { inner: s.inner.protection_leg.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.valuation_convention")]
    valuation_convention -> PyResult<String> = enum_to_py_string(&s.inner.valuation_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.doc_clause")]
    doc_clause -> PyResult<Option<String>> =
        s.inner.doc_clause.map(|clause| enum_to_py_string(&clause)).transpose();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwap.doc_clause_effective")]
    doc_clause_effective -> PyResult<String> = enum_to_py_string(&s.inner.doc_clause_effective());
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CreditDefaultSwapBuilder",
    skip_from_py_object
)]
pub struct PyCreditDefaultSwapBuilder {
    inner: Option<CreditDefaultSwapBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyCreditDefaultSwapBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            id,
            format!("{value:?}"),
            |b: CreditDefaultSwapBuilder| b.id(InstrumentId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.notional")]
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        builder_set!(
            slf,
            notional,
            money_repr(money),
            |b: CreditDefaultSwapBuilder| b.notional(money)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.side")]
    #[pyo3(text_signature = "($self, value)")]
    fn side<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let side = enum_from_str(value, "side")?;
        builder_set!(
            slf,
            side,
            format!("{value:?}"),
            |b: CreditDefaultSwapBuilder| b.side(side)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn convention<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let convention = enum_from_str::<CdsConvention>(value, "convention")?;
        builder_set!(
            slf,
            convention,
            format!("{value:?}"),
            |b: CreditDefaultSwapBuilder| b.convention(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.premium_leg")]
    #[pyo3(text_signature = "($self, value)")]
    fn premium_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyPremiumLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let leg = value.inner.clone();
        let shown = format!(
            "PremiumLegSpec(coupon_bp={}, start={}, end={})",
            leg.coupon_bp,
            date_repr(leg.start),
            date_repr(leg.end)
        );
        builder_set!(slf, premium_leg, shown, |b: CreditDefaultSwapBuilder| b
            .premium_leg(leg))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.protection_leg")]
    #[pyo3(text_signature = "($self, value)")]
    fn protection_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyProtectionLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let leg = value.inner.clone();
        let shown = format!(
            "ProtectionLegSpec(credit_curve_id={:?}, recovery_rate={})",
            leg.credit_curve_id.as_str(),
            leg.recovery_rate
        );
        builder_set!(slf, protection_leg, shown, |b: CreditDefaultSwapBuilder| b
            .protection_leg(leg))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.valuation_convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn valuation_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention: CdsValuationConvention = enum_from_str(value, "valuation_convention")?;
        builder_set!(
            slf,
            valuation_convention,
            format!("{value:?}"),
            |b: CreditDefaultSwapBuilder| b.valuation_convention(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.upfront")]
    #[pyo3(text_signature = "($self, value)")]
    fn upfront<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let (date, money) = dated_money_from_py(value, None, "upfront")?;
        let shown = format!("({}, {})", date_repr(date), money_repr(money));
        builder_set!(slf, upfront, shown, |b: CreditDefaultSwapBuilder| b
            .upfront((date, money)))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.doc_clause")]
    #[pyo3(text_signature = "($self, value)")]
    fn doc_clause<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let clause: CdsDocClause = enum_from_str(value, "doc_clause")?;
        builder_set!(
            slf,
            doc_clause,
            format!("{value:?}"),
            |b: CreditDefaultSwapBuilder| b.doc_clause(clause)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.protection_effective_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn protection_effective_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(
            slf,
            protection_effective_date,
            date_repr(date),
            |b: CreditDefaultSwapBuilder| b.protection_effective_date(date)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.margin_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn margin_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec = crate::bindings::module_utils::py_to_serde(py, value, "margin_spec")?;
        let shown = value.repr()?.to_string();
        builder_set!(slf, margin_spec, shown, |b: CreditDefaultSwapBuilder| b
            .margin_spec(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CreditDefaultSwapBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        builder_set!(slf, attributes, shown, |b: CreditDefaultSwapBuilder| b
            .attributes(attrs))
    }
}
