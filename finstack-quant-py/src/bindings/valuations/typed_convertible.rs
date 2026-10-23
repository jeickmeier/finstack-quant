//! Convertible bond Python wrappers: `ConvertibleBond`, its fluent builder
//! and the typed `ConversionSpec` / `CallPutSchedule` term classes.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::bindings::module_utils::{opt_spec_from_py, spec_from_py};
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DilutionEvent, DividendAdjustment,
    PriceTrigger,
};

use super::convert::{
    attributes_from_py, date_repr, enum_to_py_string, float_repr, money_repr, money_to_py, opt_repr,
};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

use finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBondBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConversionSpec")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "ConversionSpec",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyConversionSpec {
    /// Inner canonical Rust conversion spec.
    pub(crate) inner: ConversionSpec,
}

#[pymethods]
impl PyConversionSpec {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConversionSpec.__init__")]
    #[new]
    #[pyo3(signature = (ratio=None, price=None, *, policy, anti_dilution, dividend_adjustment, dilution_events=None))]
    #[pyo3(
        text_signature = "(ratio=None, price=None, *, policy, anti_dilution, dividend_adjustment, dilution_events=None)"
    )]
    fn new(
        py: Python<'_>,
        ratio: Option<f64>,
        price: Option<f64>,
        policy: &Bound<'_, PyAny>,
        anti_dilution: &str,
        dividend_adjustment: &str,
        dilution_events: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let policy: ConversionPolicy = if let Ok(label) = policy.cast::<PyString>() {
            super::convert::enum_from_str(label.to_str()?, "conversion policy")?
        } else if policy.is_instance_of::<PyDict>() {
            spec_from_py(py, policy, "conversion policy")?
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "policy: expected a bare conversion-policy label or a tagged dict",
            ));
        };
        let anti_dilution: AntiDilutionPolicy =
            super::convert::enum_from_str(anti_dilution, "anti_dilution")?;
        let dividend_adjustment: DividendAdjustment =
            super::convert::enum_from_str(dividend_adjustment, "dividend_adjustment")?;
        let dilution_events: Vec<DilutionEvent> =
            opt_spec_from_py(py, dilution_events, "dilution_events")?.unwrap_or_default();
        Ok(Self {
            inner: ConversionSpec {
                ratio,
                price,
                policy,
                anti_dilution,
                dividend_adjustment,
                dilution_events,
            },
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConversionSpec.policy")]
    #[getter]
    fn policy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.policy)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConversionSpec.dilution_events")]
    #[getter]
    fn dilution_events<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.dilution_events)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ConversionSpec(ratio={}, price={}, policy={:?}, anti_dilution={:?}, dividend_adjustment={:?}, dilution_events=<{}>)",
            opt_repr(self.inner.ratio.map(float_repr)),
            opt_repr(self.inner.price.map(float_repr)),
            enum_to_py_string(&self.inner.policy).unwrap_or_default(),
            enum_to_py_string(&self.inner.anti_dilution).unwrap_or_default(),
            enum_to_py_string(&self.inner.dividend_adjustment).unwrap_or_default(),
            self.inner.dilution_events.len(),
        )
    }
}

crate::bindings::macros::py_getters!(PyConversionSpec |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConversionSpec.ratio")]
    ratio -> Option<f64> = s.inner.ratio;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConversionSpec.price")]
    price -> Option<f64> = s.inner.price;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConversionSpec.anti_dilution")]
    anti_dilution -> PyResult<String> = enum_to_py_string(&s.inner.anti_dilution);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConversionSpec.dividend_adjustment")]
    dividend_adjustment -> PyResult<String> = enum_to_py_string(&s.inner.dividend_adjustment);
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CallPutSchedule")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CallPutSchedule",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCallPutSchedule {
    /// Inner canonical Rust schedule.
    pub(crate) inner: CallPutSchedule,
}

/// Coerce `list[dict] | str | None` to call/put windows.
fn windows_from_py(
    py: Python<'_>,
    value: Option<&Bound<'_, PyAny>>,
    what: &str,
) -> PyResult<Vec<CallPut>> {
    Ok(opt_spec_from_py(py, value, what)?.unwrap_or_default())
}

#[pymethods]
impl PyCallPutSchedule {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CallPutSchedule.__init__")]
    #[new]
    #[pyo3(signature = (calls=None, puts=None))]
    #[pyo3(text_signature = "(calls=None, puts=None)")]
    fn new(
        py: Python<'_>,
        calls: Option<&Bound<'_, PyAny>>,
        puts: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CallPutSchedule {
                calls: windows_from_py(py, calls, "calls")?,
                puts: windows_from_py(py, puts, "puts")?,
            },
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CallPutSchedule.calls")]
    #[getter]
    fn calls<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.calls)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CallPutSchedule.puts")]
    #[getter]
    fn puts<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.puts)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CallPutSchedule(calls=<{}>, puts=<{}>)",
            self.inner.calls.len(),
            self.inner.puts.len()
        )
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "ConvertibleBond",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyConvertibleBond {
    /// Inner canonical Rust convertible bond.
    pub(crate) inner:
        finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond,
}

instrument_envelope_methods!(
    PyConvertibleBond("ConvertibleBond"),
    ConvertibleBond,
    builder = PyConvertibleBondBuilder("ConvertibleBondBuilder", fields),
    seed =
        finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond::builder(
        ),
);
instrument_pricing_methods!(PyConvertibleBond, "ConvertibleBond");

#[pymethods]
impl PyConvertibleBond {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond::example(
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.example_mandatory")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_mandatory() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond::example_mandatory()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.parity")]
    #[pyo3(text_signature = "($self, market)")]
    fn parity(&self, py: Python<'_>, market: &Bound<'_, PyAny>) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        self.inner.parity(&market).map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.conversion_premium")]
    #[pyo3(text_signature = "($self, market, bond_price)")]
    fn conversion_premium(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        bond_price: f64,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        self.inner
            .conversion_premium(&market, bond_price)
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.greeks")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn greeks<'py>(
        &self,
        py: Python<'py>,
        market: &Bound<'py, PyAny>,
        as_of: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let greeks = self.inner.greeks(&market, as_of).map_err(core_to_py)?;
        serde_to_py(py, &greeks)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.issue_date")]
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.issue_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.soft_call_trigger")]
    #[getter]
    fn soft_call_trigger<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .soft_call_trigger
            .as_ref()
            .map(|t| serde_to_py(py, t))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.resolved_settlement")]
    #[getter]
    fn resolved_settlement<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .resolved_settlement
            .as_ref()
            .map(|claim| serde_to_py(py, claim))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.cashflow_spec")]
    #[getter]
    fn cashflow_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.cashflow_spec)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Instrument::expiry(&self.inner)
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ConvertibleBond(id={:?}, notional={}, issue_date={}, maturity={}, conversion_ratio={}, spot_id={:?})",
            self.inner.id.as_str(),
            money_repr(self.inner.notional),
            date_repr(self.inner.issue_date),
            date_repr(self.inner.maturity),
            opt_repr(self.inner.conversion_ratio().map(float_repr)),
            self.inner.spot_id.as_str(),
        )
    }
}

crate::bindings::macros::py_getters!(PyConvertibleBond |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.credit_curve_id")]
    credit_curve_id -> Option<String> = s.inner.credit_curve_id.as_ref().map(|id| id.to_string());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.conversion")]
    conversion -> PyConversionSpec = PyConversionSpec { inner: s.inner.conversion.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.conversion_ratio")]
    conversion_ratio -> Option<f64> = s.inner.conversion_ratio();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.effective_conversion_ratio")]
    effective_conversion_ratio -> Option<f64> = s.inner.effective_conversion_ratio();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.spot_id")]
    spot_id -> String = s.inner.spot_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.div_yield_id")]
    div_yield_id -> Option<String> = s.inner.div_yield_id.as_ref().map(|id| id.to_string());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.call_put")]
    call_put -> Option<PyCallPutSchedule> =
        s.inner.call_put.as_ref().map(|inner| PyCallPutSchedule { inner: inner.clone() });

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.settlement_days")]
    settlement_days -> Option<u32> = s.inner.settlement_days;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBond.recovery_rate")]
    recovery_rate -> Option<f64> = s.inner.recovery_rate;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "ConvertibleBondBuilder",
    skip_from_py_object
)]
pub struct PyConvertibleBondBuilder {
    inner: Option<ConvertibleBondBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyConvertibleBondBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            id,
            format!("{value:?}"),
            |b: ConvertibleBondBuilder| b.id(InstrumentId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.notional")]
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
            |b: ConvertibleBondBuilder| b.notional(money)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.issue_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let issue_date = py_to_date(value)?;
        builder_set!(
            slf,
            issue_date,
            date_repr(issue_date),
            |b: ConvertibleBondBuilder| b.issue_date(issue_date)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let maturity = py_to_date(value)?;
        builder_set!(
            slf,
            maturity,
            date_repr(maturity),
            |b: ConvertibleBondBuilder| b.maturity(maturity)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: ConvertibleBondBuilder| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.credit_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn credit_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            credit_curve_id,
            format!("{value:?}"),
            |b: ConvertibleBondBuilder| b.credit_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.conversion")]
    #[pyo3(text_signature = "($self, value)")]
    fn conversion<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let conversion = if let Ok(spec) = value.cast::<PyConversionSpec>() {
            spec.borrow().inner.clone()
        } else {
            spec_from_py(py, value, "conversion")?
        };
        let shown = PyConversionSpec {
            inner: conversion.clone(),
        }
        .__repr__();
        builder_set!(slf, conversion, shown, |b: ConvertibleBondBuilder| b
            .conversion(conversion))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.spot_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn spot_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            spot_id,
            format!("{value:?}"),
            |b: ConvertibleBondBuilder| b.spot_id(PriceId::new(value))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.vol_surface_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: ConvertibleBondBuilder| b.vol_surface_id(CurveId::new(value))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.div_yield_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            div_yield_id,
            format!("{value:?}"),
            |b: ConvertibleBondBuilder| b.div_yield_id(PriceId::new(value))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.call_put")]
    #[pyo3(text_signature = "($self, value)")]
    fn call_put<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let call_put = if let Ok(schedule) = value.cast::<PyCallPutSchedule>() {
            schedule.borrow().inner.clone()
        } else {
            spec_from_py(py, value, "call_put")?
        };
        let shown = format!(
            "CallPutSchedule(calls=<{}>, puts=<{}>)",
            call_put.calls.len(),
            call_put.puts.len()
        );
        builder_set!(slf, call_put, shown, |b: ConvertibleBondBuilder| b
            .call_put(call_put))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.soft_call_trigger")]
    #[pyo3(text_signature = "($self, value)")]
    fn soft_call_trigger<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let trigger: PriceTrigger = spec_from_py(py, value, "soft_call_trigger")?;
        let shown = format!(
            "{{'threshold_pct': {}, 'observation_days': {}, 'required_days_above': {}}}",
            float_repr(trigger.threshold_pct),
            trigger.observation_days,
            trigger.required_days_above
        );
        builder_set!(
            slf,
            soft_call_trigger,
            shown,
            |b: ConvertibleBondBuilder| b.soft_call_trigger(trigger)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.settlement_days")]
    #[pyo3(text_signature = "($self, value)")]
    fn settlement_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            settlement_days,
            value.to_string(),
            |b: ConvertibleBondBuilder| b.settlement_days(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.recovery_rate")]
    #[pyo3(text_signature = "($self, value)")]
    fn recovery_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            recovery_rate,
            float_repr(value),
            |b: ConvertibleBondBuilder| b.recovery_rate(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.resolved_settlement")]
    #[pyo3(text_signature = "($self, value)")]
    fn resolved_settlement<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let claim = spec_from_py(py, value, "resolved_settlement")?;
        builder_set!(
            slf,
            resolved_settlement,
            "{...}".to_string(),
            |b: ConvertibleBondBuilder| b.resolved_settlement(claim)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.cashflow_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn cashflow_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec =
            spec_from_py(py, value, "cashflow_spec")?;
        builder_set!(
            slf,
            cashflow_spec,
            "{...}".to_string(),
            |b: ConvertibleBondBuilder| b.cashflow_spec(spec)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "ConvertibleBondBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        builder_set!(slf, attributes, shown, |b: ConvertibleBondBuilder| b
            .attributes(attrs))
    }
}

/// Register `ConvertibleBond`, its builder and the convertible term classes
/// on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConvertibleBond>()?;
    m.add_class::<PyConvertibleBondBuilder>()?;
    m.add_class::<PyConversionSpec>()?;
    m.add_class::<PyCallPutSchedule>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &[
    "CallPutSchedule",
    "ConversionSpec",
    "ConvertibleBond",
    "ConvertibleBondBuilder",
];

crate::bindings::macros::wire_methods!(
    PyConversionSpec,
    ConversionSpec,
    "ConversionSpec",
    module = "finstack_quant.valuations.instruments"
);
crate::bindings::macros::wire_methods!(
    PyCallPutSchedule,
    CallPutSchedule,
    "CallPutSchedule",
    module = "finstack_quant.valuations.instruments"
);
