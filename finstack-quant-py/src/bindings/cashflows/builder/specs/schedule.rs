//! Spec-type bindings for `finstack_quant_cashflows::builder`.

use crate::bindings::macros::{py_getters, wire_methods};
use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::{PrincipalExchange, RollRule, ScheduleParams};
use pyo3::prelude::*;

use crate::bindings::core::dates::calendar::PyBusinessDayConvention;
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::repr_support::{enum_repr_from_serde as enum_repr, repr_from_serde};
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "RollRule")]
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

    /// Python-style representation (``RollRule.IMM``).
    fn __repr__(&self) -> String {
        enum_repr("RollRule", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "PrincipalExchange")]
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

#[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams")]
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
    ($( $name:ident ),+ $(,)?) => {
        #[pymethods]
        impl PyScheduleParams {
            $(
                #[doc = pydoc!(
                    "finstack_quant.cashflows.builder._specs.schedule",
                    concat!("ScheduleParams.", stringify!($name))
                )]
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
    quarterly_act360,
    semiannual_30360,
    annual_actact,
    usd_sofr_swap,
    usd_corporate_bond,
    usd_treasury,
    eur_estr_swap,
    eur_gov_bond,
    gbp_sonia_swap,
    jpy_tona_swap,
}

wire_methods!(
    PyScheduleParams,
    ScheduleParams,
    "ScheduleParams",
    module = "finstack_quant.cashflows.builder._specs.schedule",
    validate
);

#[pymethods]
impl PyScheduleParams {
    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams.__init__")]
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, frequency, day_count, calendar_id, business_day_convention, stub='short_front', end_of_month=False, payment_lag_days=0, adjust_accrual_dates=False, roll_rule='none')"
    )]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let inner: ScheduleParams =
            crate::bindings::module_utils::fields_from_py(py, fields, "ScheduleParams")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams.validate")]
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("ScheduleParams", &self.inner)
    }
}

py_getters!(PyScheduleParams |s| {
    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams.business_day_convention")]
    business_day_convention -> PyBusinessDayConvention = PyBusinessDayConvention::from_inner(s.inner.business_day_convention);

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams.calendar_id")]
    calendar_id -> String = s.inner.calendar_id.to_string();

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams.stub")]
    stub -> PyStubKind = PyStubKind::from_inner(s.inner.stub);

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams.payment_lag_days")]
    payment_lag_days -> i32 = s.inner.payment_lag_days;

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams.end_of_month")]
    end_of_month -> bool = s.inner.end_of_month;

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams.adjust_accrual_dates")]
    adjust_accrual_dates -> bool = s.inner.adjust_accrual_dates;

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.schedule", "ScheduleParams.roll_rule")]
    roll_rule -> PyRollRule = PyRollRule {
        inner: s.inner.roll_rule,
    };
});
