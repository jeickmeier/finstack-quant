//! Python bindings for `finstack_quant_cashflows::accrual`.

use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::accrual::{
    accrued_interest_amount as rust_accrued_interest_amount, AccrualConfig, AccrualIndex,
    AccrualMethod, ExCouponRule,
};
use finstack_quant_core::money::Money;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyType};

use crate::bindings::cashflows::builder::schedule::PyCashFlowSchedule;
use crate::bindings::core::dates::tenor::{extract_tenor, PyTenor};
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::wire_methods;
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.cashflows.accrual", "AccrualMethod")]
#[pyclass(
    name = "AccrualMethod",
    module = "finstack_quant.cashflows.accrual",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyAccrualMethod {
    /// Inner accrual method.
    pub(crate) inner: AccrualMethod,
}

#[pymethods]
impl PyAccrualMethod {
    /// Linear accrual: ``Accrued = Coupon × (elapsed / period)`` (default; ICMA 251.1).
    #[classattr]
    const LINEAR: Self = Self {
        inner: AccrualMethod::Linear,
    };

    /// Compounded accrual: ``Accrued = N × [(1 + r)^f − 1]`` (not ICMA-style).
    #[classattr]
    const COMPOUNDED: Self = Self {
        inner: AccrualMethod::Compounded,
    };

    #[doc = pydoc!("finstack_quant.cashflows.accrual", "AccrualMethod.name")]
    #[getter]
    fn name(&self) -> &'static str {
        match self.inner {
            AccrualMethod::Linear => "linear",
            AccrualMethod::Compounded => "compounded",
        }
    }

    /// Python-style representation (``AccrualMethod.LINEAR``).
    fn __repr__(&self) -> String {
        match self.inner {
            AccrualMethod::Linear => "AccrualMethod.LINEAR".to_string(),
            AccrualMethod::Compounded => "AccrualMethod.COMPOUNDED".to_string(),
        }
    }
}

#[doc = pydoc!("finstack_quant.cashflows.accrual", "ExCouponRule")]
#[pyclass(
    name = "ExCouponRule",
    module = "finstack_quant.cashflows.accrual",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyExCouponRule {
    /// Inner ex-coupon rule.
    pub(crate) inner: ExCouponRule,
}

#[pymethods]
impl PyExCouponRule {
    #[doc = pydoc!("finstack_quant.cashflows.accrual", "ExCouponRule.__init__")]
    #[new]
    #[pyo3(
        signature = (days_before_coupon, calendar_id=None),
        text_signature = "(days_before_coupon, calendar_id=None)"
    )]
    fn new(days_before_coupon: u32, calendar_id: Option<String>) -> Self {
        Self {
            inner: ExCouponRule {
                days_before_coupon,
                calendar_id: calendar_id.map(Into::into),
            },
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.accrual", "ExCouponRule.days_before_coupon")]
    #[getter]
    fn days_before_coupon(&self) -> u32 {
        self.inner.days_before_coupon
    }

    #[doc = pydoc!("finstack_quant.cashflows.accrual", "ExCouponRule.calendar_id")]
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.cashflows.accrual", "ExCouponRule.ex_date")]
    #[pyo3(text_signature = "(self, payment_date)")]
    fn ex_date<'py>(
        &self,
        py: Python<'py>,
        payment_date: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let d = self
            .inner
            .ex_date(py_to_date(payment_date)?)
            .map_err(core_to_py)?;
        date_to_py(py, d)
    }

    #[doc = pydoc!("finstack_quant.cashflows.accrual", "ExCouponRule.is_ex_coupon")]
    #[pyo3(text_signature = "(self, payment_date, settlement_date)")]
    fn is_ex_coupon(
        &self,
        payment_date: &Bound<'_, PyAny>,
        settlement_date: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        self.inner
            .is_ex_coupon(py_to_date(payment_date)?, py_to_date(settlement_date)?)
            .map_err(core_to_py)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ExCouponRule", &self.inner)
    }
}

wire_methods!(PyExCouponRule, ExCouponRule, "ExCouponRule");

#[doc = pydoc!("finstack_quant.cashflows.accrual", "AccrualConfig")]
#[pyclass(
    name = "AccrualConfig",
    module = "finstack_quant.cashflows.accrual",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyAccrualConfig {
    /// Inner accrual configuration.
    pub(crate) inner: AccrualConfig,
}

#[pymethods]
impl PyAccrualConfig {
    #[doc = pydoc!("finstack_quant.cashflows.accrual", "AccrualConfig.__init__")]
    #[new]
    #[pyo3(
        // `include_pik`'s runtime default tracks `AccrualConfig::default()`;
        // the static text_signature literal must be kept in sync by hand.
        signature = (method=None, ex_coupon=None, include_pik=AccrualConfig::default().include_pik, frequency=None),
        text_signature = "(method=None, ex_coupon=None, include_pik=True, frequency=None)"
    )]
    fn new(
        method: Option<PyRef<'_, PyAccrualMethod>>,
        ex_coupon: Option<PyRef<'_, PyExCouponRule>>,
        include_pik: bool,
        frequency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let default = AccrualConfig::default();
        Ok(Self {
            inner: AccrualConfig {
                method: method.map_or(default.method, |m| m.inner.clone()),
                ex_coupon: ex_coupon.map(|r| r.inner.clone()),
                include_pik,
                frequency: frequency.map(extract_tenor).transpose()?,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.accrual", "AccrualConfig.method")]
    #[getter]
    fn method(&self) -> PyAccrualMethod {
        PyAccrualMethod {
            inner: self.inner.method.clone(),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.accrual", "AccrualConfig.ex_coupon")]
    #[getter]
    fn ex_coupon(&self) -> Option<PyExCouponRule> {
        self.inner
            .ex_coupon
            .clone()
            .map(|inner| PyExCouponRule { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.accrual", "AccrualConfig.include_pik")]
    #[getter]
    fn include_pik(&self) -> bool {
        self.inner.include_pik
    }

    #[doc = pydoc!("finstack_quant.cashflows.accrual", "AccrualConfig.frequency")]
    #[getter]
    fn frequency(&self) -> Option<PyTenor> {
        self.inner.frequency.map(PyTenor::from_inner)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("AccrualConfig", &self.inner)
    }
}

wire_methods!(PyAccrualConfig, AccrualConfig, "AccrualConfig");

#[doc = pydoc!("finstack_quant.cashflows.accrual", "AccrualIndex")]
#[pyclass(
    name = "AccrualIndex",
    module = "finstack_quant.cashflows.accrual",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyAccrualIndex {
    /// Inner prebuilt accrual state.
    pub(crate) inner: AccrualIndex,
    /// Currency of the indexed schedule (for repr and Money results).
    currency: finstack_quant_core::currency::Currency,
}

#[pymethods]
impl PyAccrualIndex {
    #[doc = pydoc!("finstack_quant.cashflows.accrual", "AccrualIndex.build")]
    #[classmethod]
    #[pyo3(
        signature = (schedule, config=None),
        text_signature = "(cls, schedule, config=None)"
    )]
    fn build(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        schedule: PyRef<'_, PyCashFlowSchedule>,
        config: Option<PyRef<'_, PyAccrualConfig>>,
    ) -> PyResult<Self> {
        let schedule = schedule.inner.clone();
        let currency = schedule.get_notional().currency();
        let cfg = config.map_or_else(AccrualConfig::default, |c| c.inner.clone());
        py.detach(move || AccrualIndex::build(&schedule, &cfg))
            .map(|inner| Self { inner, currency })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.accrual", "AccrualIndex.accrued_at")]
    #[pyo3(text_signature = "(self, as_of)")]
    fn accrued_at(&self, as_of: &Bound<'_, PyAny>) -> PyResult<PyMoney> {
        self.inner
            .accrued_at(py_to_date(as_of)?)
            .and_then(|amount| Money::new(amount, self.currency))
            .map(PyMoney::from_inner)
            .map_err(core_to_py)
    }

    /// Python-style summary.
    fn __repr__(&self) -> String {
        format!("AccrualIndex(currency='{}')", self.currency)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.accrual", "accrued_interest_amount")]
#[pyfunction]
#[pyo3(
    signature = (schedule, as_of, config=None),
    text_signature = "(schedule, as_of, config=None)"
)]
fn accrued_interest_amount(
    py: Python<'_>,
    schedule: PyRef<'_, PyCashFlowSchedule>,
    as_of: &Bound<'_, PyAny>,
    config: Option<PyRef<'_, PyAccrualConfig>>,
) -> PyResult<PyMoney> {
    let as_of = py_to_date(as_of)?;
    let schedule = schedule.inner.clone();
    let currency = schedule.get_notional().currency();
    let cfg = config.map_or_else(AccrualConfig::default, |c| c.inner.clone());
    py.detach(move || rust_accrued_interest_amount(&schedule, as_of, &cfg))
        .and_then(|amount| Money::new(amount, currency))
        .map(PyMoney::from_inner)
        .map_err(core_to_py)
}

/// Register the `finstack_quant.cashflows.accrual` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = crate::bindings::module_utils::new_submodule(parent, "accrual")?;
    module.setattr(
        "__doc__",
        "Schedule-driven accrued interest: methods, ex-coupon rules, accrual index.",
    )?;
    module.add_class::<PyAccrualMethod>()?;
    module.add_class::<PyExCouponRule>()?;
    module.add_class::<PyAccrualConfig>()?;
    module.add_class::<PyAccrualIndex>()?;
    module.add_function(wrap_pyfunction!(accrued_interest_amount, &module)?)?;

    let all = PyList::new(
        py,
        [
            "AccrualConfig",
            "AccrualIndex",
            "AccrualMethod",
            "ExCouponRule",
            "accrued_interest_amount",
        ],
    )?;
    module.setattr("__all__", all)?;

    crate::bindings::module_utils::attach_submodule(
        parent,
        &module,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;
    Ok(())
}
