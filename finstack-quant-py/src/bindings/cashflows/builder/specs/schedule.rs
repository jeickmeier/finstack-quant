//! Spec-type bindings for `finstack_quant_cashflows::builder`.

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::{PrincipalExchange, RollRule, ScheduleParams};
use finstack_quant_core::dates::BusinessDayConvention;
use pyo3::prelude::*;

use crate::bindings::core::dates::calendar::{
    extract_business_day_convention, PyBusinessDayConvention,
};
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::{extract_tenor, PyTenor};
use crate::bindings::repr_support::{enum_repr_from_serde as enum_repr, repr_from_serde};
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.cashflows.builder", "RollRule")]
#[pyclass(
    name = "RollRule",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyRollRule {
    /// Inner roll-date rule.
    pub(crate) inner: RollRule,
}

#[pymethods]
impl PyRollRule {
    /// Plain tenor stepping (default).
    #[classattr]
    const NONE: PyRollRule = PyRollRule {
        inner: RollRule::None,
    };
    /// Standard IMM dates: third Wednesday of Mar/Jun/Sep/Dec.
    #[classattr]
    const IMM: PyRollRule = PyRollRule {
        inner: RollRule::Imm,
    };
    /// CDS IMM dates: 20th of Mar/Jun/Sep/Dec with Big-Bang front accrual.
    #[classattr]
    const CDS_IMM: PyRollRule = PyRollRule {
        inner: RollRule::CdsImm,
    };

    /// Python-style representation (``RollRule.IMM``).
    fn __repr__(&self) -> String {
        enum_repr("RollRule", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "PrincipalExchange")]
#[pyclass(
    name = "PrincipalExchange",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyPrincipalExchange {
    /// Inner principal-exchange policy.
    pub(crate) inner: PrincipalExchange,
}

#[pymethods]
impl PyPrincipalExchange {
    /// Do not emit issue or redemption notionals.
    #[classattr]
    const NONE: PyPrincipalExchange = PyPrincipalExchange {
        inner: PrincipalExchange::None,
    };
    /// Emit issue funding and maturity redemption (default).
    #[classattr]
    const INITIAL_AND_FINAL: PyPrincipalExchange = PyPrincipalExchange {
        inner: PrincipalExchange::InitialAndFinal,
    };

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("PrincipalExchange", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams")]
#[pyclass(
    name = "ScheduleParams",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyScheduleParams {
    /// Inner schedule-generation parameters.
    pub(crate) inner: ScheduleParams,
}

impl PyScheduleParams {
    /// Build from an existing Rust [`ScheduleParams`].
    pub(crate) fn from_inner(inner: ScheduleParams) -> Self {
        Self { inner }
    }
}

macro_rules! schedule_params_presets {
    ($( $(#[$doc:meta])* $name:ident ),+ $(,)?) => {
        #[pymethods]
        impl PyScheduleParams {
            $(
                $(#[$doc])*
                #[staticmethod]
                #[pyo3(text_signature = "()")]
                fn $name() -> Self {
                    Self::from_inner(ScheduleParams::$name())
                }
            )+
        }
    };
}

schedule_params_presets! {
    /// Quarterly, Act/360, Modified Following, weekends-only calendar.
    quarterly_act360,
    /// Semi-annual, 30/360, Modified Following, weekends-only calendar.
    semiannual_30360,
    /// Annual, Act/Act, Following, weekends-only calendar.
    annual_actact,
    /// USD SOFR swap: quarterly, Act/360, MF, USNY, T+2 lag, adjusted accruals.
    usd_sofr_swap,
    /// USD corporate bond: semi-annual, 30/360, Following, USNY.
    usd_corporate_bond,
    /// USD Treasury: semi-annual, Act/Act ISMA, Following, USNY.
    usd_treasury,
    /// EUR ESTR swap: annual, Act/360, MF, TARGET2, T+2 lag, adjusted accruals.
    eur_estr_swap,
    /// EUR government bond: annual, Act/Act ISMA, Following, TARGET2.
    eur_gov_bond,
    /// GBP SONIA swap: annual, Act/365F, MF, GBLO, no lag, adjusted accruals.
    gbp_sonia_swap,
    /// JPY TONA swap: annual, Act/365F, MF, JPTO, T+2 lag, adjusted accruals.
    jpy_tona_swap,
}

wire_methods!(PyScheduleParams, ScheduleParams, "ScheduleParams", validate);

#[pymethods]
impl PyScheduleParams {
    #[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams.__init__")]
    #[new]
    #[pyo3(
        signature = (frequency, day_count, calendar_id, business_day_convention=None, stub=None, end_of_month=false, payment_lag_days=0, adjust_accrual_dates=false, roll_rule=None),
        text_signature = "(frequency, day_count, calendar_id, business_day_convention=None, stub=None, end_of_month=False, payment_lag_days=0, adjust_accrual_dates=False, roll_rule=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        frequency: &Bound<'_, PyAny>,
        day_count: PyRef<'_, PyDayCount>,
        calendar_id: &str,
        business_day_convention: Option<&Bound<'_, PyAny>>,
        stub: Option<PyRef<'_, PyStubKind>>,
        end_of_month: bool,
        payment_lag_days: i32,
        adjust_accrual_dates: bool,
        roll_rule: Option<PyRef<'_, PyRollRule>>,
    ) -> PyResult<Self> {
        let business_day_convention = match business_day_convention {
            Some(obj) => extract_business_day_convention(obj)?,
            None => BusinessDayConvention::default(),
        };
        let inner = ScheduleParams {
            frequency: extract_tenor(frequency)?,
            day_count: day_count.inner,
            business_day_convention,
            calendar_id: calendar_id.into(),
            stub: stub.map_or(ScheduleParams::DEFAULT_STUB, |s| s.inner),
            end_of_month,
            payment_lag_days,
            adjust_accrual_dates,
            roll_rule: roll_rule.map(|r| r.inner).unwrap_or_default(),
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams.validate")]
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams.frequency")]
    #[getter]
    fn frequency(&self) -> PyTenor {
        PyTenor::from_inner(self.inner.frequency)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams.day_count")]
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.day_count)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams.business_day_convention")]
    #[getter]
    fn business_day_convention(&self) -> PyBusinessDayConvention {
        PyBusinessDayConvention::from_inner(self.inner.business_day_convention)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams.calendar_id")]
    #[getter]
    fn calendar_id(&self) -> String {
        self.inner.calendar_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams.stub")]
    #[getter]
    fn stub(&self) -> PyStubKind {
        PyStubKind::from_inner(self.inner.stub)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams.payment_lag_days")]
    #[getter]
    fn payment_lag_days(&self) -> i32 {
        self.inner.payment_lag_days
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams.end_of_month")]
    #[getter]
    fn end_of_month(&self) -> bool {
        self.inner.end_of_month
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams.adjust_accrual_dates")]
    #[getter]
    fn adjust_accrual_dates(&self) -> bool {
        self.inner.adjust_accrual_dates
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "ScheduleParams.roll_rule")]
    #[getter]
    fn roll_rule(&self) -> PyRollRule {
        PyRollRule {
            inner: self.inner.roll_rule,
        }
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("ScheduleParams", &self.inner)
    }
}
