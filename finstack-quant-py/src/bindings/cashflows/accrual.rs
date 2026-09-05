//! Python bindings for `finstack_quant_cashflows::accrual`.

use finstack_quant_cashflows::accrual::{
    accrued_interest_amount, AccrualConfig, AccrualIndex, AccrualMethod, ExCouponRule,
};
use finstack_quant_core::money::Money;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyType};

use crate::bindings::cashflows::builder::schedule::PyCashFlowSchedule;
use crate::bindings::core::dates::tenor::{extract_tenor, PyTenor};
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::errors::core_to_py;

/// Accrual method selector (``AccrualMethod.LINEAR`` / ``AccrualMethod.COMPOUNDED``).
///
/// ``LINEAR`` is the ICMA Rule 251.1 bond convention; ``COMPOUNDED`` uses true
/// exponential compounding within a coupon period and is not ICMA-compliant.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.accrual import AccrualMethod
/// >>> AccrualMethod.LINEAR == AccrualMethod.LINEAR
/// True
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
    #[allow(non_snake_case)]
    fn LINEAR() -> Self {
        Self {
            inner: AccrualMethod::Linear,
        }
    }

    /// Compounded accrual: ``Accrued = N × [(1 + r)^f − 1]`` (not ICMA-style).
    #[classattr]
    #[allow(non_snake_case)]
    fn COMPOUNDED() -> Self {
        Self {
            inner: AccrualMethod::Compounded,
        }
    }

    /// Canonical snake_case label (``"linear"`` / ``"compounded"``).
    #[getter]
    fn name(&self) -> &'static str {
        match self.inner {
            AccrualMethod::Linear => "linear",
            AccrualMethod::Compounded => "compounded",
            _ => "unknown",
        }
    }

    /// Python-style representation (``AccrualMethod.LINEAR``).
    fn __repr__(&self) -> String {
        match self.inner {
            AccrualMethod::Linear => "AccrualMethod.LINEAR".to_string(),
            AccrualMethod::Compounded => "AccrualMethod.COMPOUNDED".to_string(),
            _ => "AccrualMethod(...)".to_string(),
        }
    }
}

/// Ex-coupon convention: the instrument trades ex-coupon from
/// ``days_before_coupon`` days before each payment date.
///
/// Parameters
/// ----------
/// days_before_coupon : int
///     Number of days before the coupon date that go ex (max 366).
/// calendar_id : str, optional
///     Business-day calendar for counting the window; when omitted, calendar
///     days are used.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.accrual import ExCouponRule
/// >>> ExCouponRule(7).days_before_coupon
/// 7
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
    /// Construct an ex-coupon rule; see the class docstring for parameters.
    #[new]
    #[pyo3(
        signature = (days_before_coupon, calendar_id=None),
        text_signature = "(days_before_coupon, calendar_id=None)"
    )]
    fn new(days_before_coupon: u32, calendar_id: Option<String>) -> Self {
        Self {
            inner: ExCouponRule {
                days_before_coupon,
                calendar_id,
            },
        }
    }

    /// Days before the coupon date that go ex.
    #[getter]
    fn days_before_coupon(&self) -> u32 {
        self.inner.days_before_coupon
    }

    /// Optional business-day calendar identifier.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.clone()
    }

    /// Ex-coupon date for a coupon paid on ``payment_date``.
    ///
    /// Parameters
    /// ----------
    /// payment_date : datetime.date or str
    ///     Payment date of the coupon this ex-coupon window precedes.
    ///
    /// Returns
    /// -------
    /// datetime.date
    ///     Ex-coupon date; from this date (inclusive) until ``payment_date``
    ///     (exclusive), the instrument trades ex-coupon.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``days_before_coupon`` exceeds 366.
    /// KeyError
    ///     If the configured calendar id cannot be resolved.
    #[pyo3(text_signature = "(self, payment_date)")]
    fn ex_date<'py>(
        &self,
        py: Python<'py>,
        payment_date: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let d = self
            .inner
            .ex_date(extract_date(payment_date)?)
            .map_err(core_to_py)?;
        date_to_py(py, d)
    }

    /// Serialize to JSON.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "failed to serialize ExCouponRule"))
    }

    /// Deserialize from JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<ExCouponRule>(json)
            .map(|inner| Self { inner })
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid ExCouponRule JSON"))
    }

    /// Support ``pickle`` through the JSON wire form.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ExCouponRule", &self.inner)
    }
}

/// Configuration for schedule-driven interest accrual.
///
/// Parameters
/// ----------
/// method : AccrualMethod, optional
///     Accrual method; defaults to the Rust ``AccrualConfig::default()``
///     method (``AccrualMethod.LINEAR``, ICMA 251.1).
/// ex_coupon : ExCouponRule, optional
///     Ex-coupon window rule; ``None`` disables ex-coupon handling.
/// include_pik : bool, default True
///     Whether PIK (capitalized) interest counts toward the accrued amount.
/// frequency : Tenor or str, optional
///     Coupon frequency (e.g. ``"6M"``); required for the ACT/ACT ICMA day
///     count and ignored otherwise.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.accrual import AccrualConfig
/// >>> AccrualConfig().include_pik
/// True
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
    /// Construct an accrual configuration; see the class docstring for parameters.
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

    /// Accrual method in force.
    #[getter]
    fn method(&self) -> PyAccrualMethod {
        PyAccrualMethod {
            inner: self.inner.method.clone(),
        }
    }

    /// Ex-coupon rule, if any.
    #[getter]
    fn ex_coupon(&self) -> Option<PyExCouponRule> {
        self.inner
            .ex_coupon
            .clone()
            .map(|inner| PyExCouponRule { inner })
    }

    /// Whether PIK interest is included in the accrued amount.
    #[getter]
    fn include_pik(&self) -> bool {
        self.inner.include_pik
    }

    /// Coupon frequency used for ACT/ACT ICMA, if set.
    #[getter]
    fn frequency(&self) -> Option<PyTenor> {
        self.inner.frequency.map(PyTenor::from_inner)
    }

    /// Serialize to JSON.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "failed to serialize AccrualConfig"))
    }

    /// Deserialize from JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<AccrualConfig>(json)
            .map(|inner| Self { inner })
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid AccrualConfig JSON"))
    }

    /// Support ``pickle`` through the JSON wire form.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("AccrualConfig", &self.inner)
    }
}

/// Prebuilt accrual state for repeated ``accrued_at`` queries on one schedule.
///
/// Build with ``AccrualIndex.build(schedule, config=None)``.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.cashflows import schedule_from_dated_flows
/// >>> from finstack_quant.cashflows.accrual import AccrualIndex
/// >>> from finstack_quant.core.dates import DayCount
/// >>> from finstack_quant.core.money import Money
/// >>> schedule = schedule_from_dated_flows([(datetime.date(2025, 6, 15), Money(100.0, "USD"))], "fixed", DayCount.ACT_360)
/// >>> AccrualIndex.build(schedule).accrued_at(datetime.date(2024, 1, 1)).amount
/// 0.0
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
    /// Number of coupon periods indexed (for repr).
    periods: usize,
}

#[pymethods]
impl PyAccrualIndex {
    /// Build reusable accrual state for repeated ``accrued_at`` queries.
    ///
    /// Parameters
    /// ----------
    /// schedule : CashFlowSchedule
    ///     Canonical cashflow schedule containing coupon, PIK, and notional
    ///     flows.
    /// config : AccrualConfig, optional
    ///     Accrual method and ex-coupon configuration bound into the index
    ///     (default linear, PIK included). Build a separate index to accrue
    ///     under a different config.
    ///
    /// Returns
    /// -------
    /// AccrualIndex
    ///     Prebuilt accrual state.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the schedule fails validation, mixes currencies across coupon
    ///     flows, or carries a non-finite accrual factor.
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
        let periods = schedule.coupons().count();
        let cfg = config.map_or_else(AccrualConfig::default, |c| c.inner.clone());
        py.detach(move || AccrualIndex::build(&schedule, &cfg))
            .map(|inner| Self {
                inner,
                currency,
                periods,
            })
            .map_err(core_to_py)
    }

    /// Accrued interest as of ``as_of`` using the prebuilt periods.
    ///
    /// Parameters
    /// ----------
    /// as_of : datetime.date or str
    ///     Accrual cut-off date; dates outside all coupon periods return 0.
    ///
    /// Returns
    /// -------
    /// Money
    ///     Accrued interest in the schedule currency; negative inside an
    ///     active ex-coupon window.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a configured ex-coupon calendar id cannot be resolved.
    #[pyo3(text_signature = "(self, as_of)")]
    fn accrued_at(&self, as_of: &Bound<'_, PyAny>) -> PyResult<PyMoney> {
        self.inner
            .accrued_at(extract_date(as_of)?)
            .and_then(|amount| Money::new(amount, self.currency))
            .map(PyMoney::from_inner)
            .map_err(core_to_py)
    }

    /// Python-style summary.
    fn __repr__(&self) -> String {
        format!(
            "AccrualIndex(periods={}, currency='{}')",
            self.periods, self.currency
        )
    }
}

/// Compute accrued interest for a schedule as of ``as_of``.
///
/// Parameters
/// ----------
/// schedule : CashFlowSchedule
///     Canonical cashflow schedule.
/// as_of : datetime.date or str
///     Accrual cut-off date; dates outside all coupon periods return 0.
/// config : AccrualConfig, optional
///     Accrual method and ex-coupon configuration (default linear, PIK included).
///
/// Returns
/// -------
/// Money
///     Accrued interest in the schedule's notional currency; negative inside
///     an active ex-coupon window.
///
/// Raises
/// ------
/// ValueError
///     If the schedule fails validation, mixes currencies across coupon
///     flows, or carries a non-finite accrual factor.
/// KeyError
///     If a configured ex-coupon calendar id cannot be resolved.
#[pyfunction(name = "accrued_interest_amount")]
#[pyo3(
    signature = (schedule, as_of, config=None),
    text_signature = "(schedule, as_of, config=None)"
)]
fn py_accrued_interest_amount(
    py: Python<'_>,
    schedule: PyRef<'_, PyCashFlowSchedule>,
    as_of: &Bound<'_, PyAny>,
    config: Option<PyRef<'_, PyAccrualConfig>>,
) -> PyResult<PyMoney> {
    let as_of = extract_date(as_of)?;
    let schedule = schedule.inner.clone();
    let currency = schedule.get_notional().currency();
    let cfg = config.map_or_else(AccrualConfig::default, |c| c.inner.clone());
    py.detach(move || accrued_interest_amount(&schedule, as_of, &cfg))
        .and_then(|amount| Money::new(amount, currency))
        .map(PyMoney::from_inner)
        .map_err(core_to_py)
}

/// Register the `finstack_quant.cashflows.accrual` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "accrual")?;
    module.setattr(
        "__doc__",
        "Schedule-driven accrued interest: methods, ex-coupon rules, accrual index.",
    )?;
    module.add_class::<PyAccrualMethod>()?;
    module.add_class::<PyExCouponRule>()?;
    module.add_class::<PyAccrualConfig>()?;
    module.add_class::<PyAccrualIndex>()?;
    module.add_function(wrap_pyfunction!(py_accrued_interest_amount, &module)?)?;

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

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &module,
        "accrual",
        "finstack_quant.cashflows",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;
    Ok(())
}
