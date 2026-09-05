//! Python bindings for `finstack_quant_cashflows::primitives`.

use finstack_quant_cashflows::primitives::{
    is_cash_settlement_kind, CFKind, CashFlow, CashFlowAccrual,
};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyType};

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

/// Wrapper for [`CFKind`] exposed as `finstack_quant.cashflows.primitives.CFKind`.
#[pyclass(
    name = "CFKind",
    module = "finstack_quant.cashflows.primitives",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyCFKind {
    /// Inner cashflow kind.
    pub(crate) inner: CFKind,
}

impl PyCFKind {
    /// Build from an existing Rust [`CFKind`].
    pub(crate) const fn from_inner(inner: CFKind) -> Self {
        Self { inner }
    }
}

macro_rules! cf_kind_classattrs {
    ($( $(#[$doc:meta])* $name:ident => $variant:ident ),+ $(,)?) => {
        #[pymethods]
        impl PyCFKind {
            $(
                $(#[$doc])*
                #[classattr]
                const $name: PyCFKind = PyCFKind { inner: CFKind::$variant };
            )+
        }
    };
}

cf_kind_classattrs! {
    /// Fixed-rate coupon cash-flow.
    FIXED => Fixed,
    /// Floating-rate coupon cash-flow (or index fixing event).
    FLOAT_RESET => FloatReset,
    /// Inflation-linked coupon cash-flow.
    INFLATION_COUPON => InflationCoupon,
    /// Up-front fee or cost paid at inception.
    FEE => Fee,
    /// Commitment fee on undrawn balance.
    COMMITMENT_FEE => CommitmentFee,
    /// Usage fee on drawn balance.
    USAGE_FEE => UsageFee,
    /// Facility fee on total commitment.
    FACILITY_FEE => FacilityFee,
    /// Principal exchange or notional flow.
    NOTIONAL => Notional,
    /// Payment-in-kind interest capitalization.
    PIK => Pik,
    /// Scheduled amortization (principal repayment).
    AMORTIZATION => Amortization,
    /// Prepayment of principal (unscheduled early repayment).
    PRE_PAYMENT => PrePayment,
    /// Revolving facility draw (borrowing).
    REVOLVING_DRAW => RevolvingDraw,
    /// Revolving facility repayment.
    REVOLVING_REPAYMENT => RevolvingRepayment,
    /// Defaulted notional (principal written down due to credit event).
    DEFAULTED_NOTIONAL => DefaultedNotional,
    /// Recovery cashflow from defaulted principal.
    RECOVERY => Recovery,
    /// Accrued-on-default interest (ISDA standard for CDS).
    ACCRUED_ON_DEFAULT => AccruedOnDefault,
    /// Irregular stub period interest.
    STUB => Stub,
    /// Initial margin posting (collateral transfer out).
    INITIAL_MARGIN_POST => InitialMarginPost,
    /// Initial margin return (collateral returned).
    INITIAL_MARGIN_RETURN => InitialMarginReturn,
    /// Variation margin received.
    VARIATION_MARGIN_RECEIVE => VariationMarginReceive,
    /// Variation margin paid.
    VARIATION_MARGIN_PAY => VariationMarginPay,
    /// Interest accrued on posted margin collateral.
    MARGIN_INTEREST => MarginInterest,
    /// Collateral substitution inflow.
    COLLATERAL_SUBSTITUTION_IN => CollateralSubstitutionIn,
    /// Collateral substitution outflow.
    COLLATERAL_SUBSTITUTION_OUT => CollateralSubstitutionOut,
}

#[pymethods]
impl PyCFKind {
    /// Parse a cashflow kind from its snake_case label (e.g. ``"fixed"``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn parse(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        name.parse::<CFKind>()
            .map(Self::from_inner)
            .map_err(crate::errors::value_error)
    }

    /// Canonical snake_case label of this kind.
    #[getter]
    fn name(&self) -> String {
        self.inner.to_string()
    }

    /// Whether this kind is interest-bearing (Fixed/FloatReset/InflationCoupon/Stub).
    fn is_interest_like(&self) -> bool {
        self.inner.is_interest_like()
    }

    /// Whether this kind changes principal balance (notional, PIK, amortization, …).
    fn is_principal_like(&self) -> bool {
        self.inner.is_principal_like()
    }

    /// Hash consistent with equality on the underlying kind.
    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.inner.hash(&mut hasher);
        hasher.finish()
    }

    /// Snake_case label.
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    /// Debug-style representation.
    fn __repr__(&self) -> String {
        format!("CFKind('{}')", self.inner)
    }
}

/// Extract a [`CFKind`] from a `CFKind` wrapper or a snake_case string.
pub(crate) fn extract_cf_kind(obj: &Bound<'_, PyAny>) -> PyResult<CFKind> {
    if let Ok(kind) = obj.extract::<PyRef<'_, PyCFKind>>() {
        return Ok(kind.inner);
    }
    if let Ok(name) = obj.extract::<String>() {
        return name.parse::<CFKind>().map_err(crate::errors::value_error);
    }
    Err(PyTypeError::new_err("expected CFKind or str"))
}

/// Contractual accrual boundaries and conventions, independent of payment date.
#[pyclass(
    name = "CashFlowAccrual",
    module = "finstack_quant.cashflows.primitives",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyCashFlowAccrual {
    pub(crate) inner: CashFlowAccrual,
}

impl PyCashFlowAccrual {
    /// Build from an existing Rust [`CashFlowAccrual`].
    pub(crate) const fn from_inner(inner: CashFlowAccrual) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCashFlowAccrual {
    /// Construct contractual coupon metadata.
    ///
    /// Parameters
    /// ----------
    /// start : datetime.date or str
    ///     Inclusive accrual start.
    /// end : datetime.date or str
    ///     Exclusive accrual end, independent of payment lag.
    /// day_count : DayCount
    ///     Convention used to calculate the year fraction.
    /// projected_index_rate : float, optional
    ///     Decimal index rate before spread, gearing and constraints.
    /// calendar_id : str, optional
    ///     Registered calendar identifier; required for BUS/252 accrual.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If either date cannot be parsed.
    #[new]
    #[pyo3(signature = (start, end, day_count, projected_index_rate=None, calendar_id=None))]
    fn new(
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        day_count: PyRef<'_, PyDayCount>,
        projected_index_rate: Option<f64>,
        calendar_id: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CashFlowAccrual {
                start: py_to_date(start)?,
                end: py_to_date(end)?,
                day_count: day_count.inner,
                projected_index_rate,
                calendar_id,
            },
        })
    }

    /// Inclusive accrual start as a Python date.
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start)
    }

    /// Exclusive accrual end as a Python date.
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end)
    }

    /// Convention used to calculate accrual year fractions.
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.day_count)
    }

    /// Unconstrained decimal index rate, or None if unprojected.
    #[getter]
    fn projected_index_rate(&self) -> Option<f64> {
        self.inner.projected_index_rate
    }

    /// Registered accrual calendar identifier, or None when unused.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.clone()
    }
}

/// Wrapper for [`CashFlow`] exposed as `finstack_quant.cashflows.primitives.CashFlow`.
#[pyclass(
    name = "CashFlow",
    module = "finstack_quant.cashflows.primitives",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyCashFlow {
    /// Inner dated cashflow.
    pub(crate) inner: CashFlow,
}

impl PyCashFlow {
    /// Build from an existing Rust [`CashFlow`].
    pub(crate) const fn from_inner(inner: CashFlow) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCashFlow {
    /// Construct a dated cashflow.
    ///
    /// The Python positional order is ``(date, amount, kind, reset_date,
    /// accrual_factor, rate)`` so the three required inputs come first; the
    /// Rust constructor ``CashFlow::new`` orders them ``(date, reset_date,
    /// amount, kind, accrual_factor, rate)``. Prefer keyword arguments for
    /// the optional fields.
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date or str
    ///     Payment date.
    /// amount : Money
    ///     Monetary amount including its currency.
    /// kind : CFKind or str
    ///     Cashflow classification.
    /// reset_date : datetime.date or str, optional
    ///     Index reset date for floating coupons.
    /// accrual_factor : float, default 0.0
    ///     Accrual year fraction used for the coupon amount.
    /// rate : float, optional
    ///     Effective annual rate (decimal, ``0.05`` = 5%) used to compute
    ///     this cashflow.
    #[new]
    #[pyo3(
        signature = (date, amount, kind, reset_date=None, accrual_factor=0.0, rate=None),
        text_signature = "(date, amount, kind, reset_date=None, accrual_factor=0.0, rate=None)"
    )]
    fn new(
        date: &Bound<'_, PyAny>,
        amount: PyMoney,
        kind: &Bound<'_, PyAny>,
        reset_date: Option<&Bound<'_, PyAny>>,
        accrual_factor: f64,
        rate: Option<f64>,
    ) -> PyResult<Self> {
        let reset = reset_date.map(py_to_date).transpose()?;
        Ok(Self::from_inner(CashFlow::new(
            py_to_date(date)?,
            reset,
            amount.inner,
            extract_cf_kind(kind)?,
            accrual_factor,
            rate,
        )))
    }

    /// Payment date as ``datetime.date``.
    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.date)
    }

    /// Optional index reset date as ``datetime.date``.
    #[getter]
    fn reset_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner.reset_date.map(|d| date_to_py(py, d)).transpose()
    }

    /// Monetary amount including its currency.
    #[getter]
    fn amount(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.amount)
    }

    /// Cashflow classification.
    #[getter]
    fn kind(&self) -> PyCFKind {
        PyCFKind::from_inner(self.inner.kind)
    }

    /// Accrual year fraction.
    #[getter]
    fn accrual_factor(&self) -> f64 {
        self.inner.accrual_factor
    }

    /// Effective annual rate, when known.
    #[getter]
    fn rate(&self) -> Option<f64> {
        self.inner.rate
    }

    /// Contractual coupon metadata, or None for an unannotated row.
    #[getter]
    fn accrual(&self) -> Option<PyCashFlowAccrual> {
        self.inner
            .accrual
            .clone()
            .map(PyCashFlowAccrual::from_inner)
    }
    /// Explicit principal movement, or None when derived from kind and amount.
    #[getter]
    fn principal_delta(&self) -> Option<PyMoney> {
        self.inner.principal_delta.map(PyMoney::from_inner)
    }
    /// Return a copy carrying the supplied contractual accrual metadata.
    ///
    /// Parameters
    /// ----------
    /// accrual : CashFlowAccrual
    ///     Coupon boundaries, day count, calendar and projected index rate.
    ///
    /// Returns
    /// -------
    /// CashFlow
    ///     A new row with the supplied metadata; call validate to check it.
    fn with_accrual(&self, accrual: PyRef<'_, PyCashFlowAccrual>) -> Self {
        Self::from_inner(self.inner.clone().with_accrual(accrual.inner.clone()))
    }
    /// Return a copy with a principal movement separate from settlement cash.
    ///
    /// Parameters
    /// ----------
    /// delta : Money
    ///     Signed principal change in the cash currency; positive increases it.
    ///
    /// Returns
    /// -------
    /// CashFlow
    ///     A new row with an explicit balance movement; call validate to check currency.
    fn with_principal_delta(&self, delta: PyMoney) -> Self {
        Self::from_inner(self.inner.clone().with_principal_delta(delta.inner))
    }

    /// Validate amount, accrual factor, rate, and reset-date ordering.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a value is non-finite, the accrual factor is negative, or the
    ///     reset date is after the payment date.
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Serialize to the canonical JSON row format.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "failed to serialize CashFlow"))
    }

    /// Deserialize from the canonical JSON row format.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or carries unknown fields.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<CashFlow>(json)
            .map(Self::from_inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid CashFlow JSON"))
    }

    /// Support ``pickle`` through the JSON wire form.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        let reset = self
            .inner
            .reset_date
            .map_or_else(|| "None".to_string(), |d| format!("'{d}'"));
        let rate = self
            .inner
            .rate
            .map_or_else(|| "None".to_string(), |r| r.to_string());
        format!(
            "CashFlow(date='{}', amount={} {}, kind='{}', reset_date={reset}, accrual_factor={}, rate={rate})",
            self.inner.date,
            self.inner.amount.amount(),
            self.inner.amount.currency(),
            self.inner.kind,
            self.inner.accrual_factor,
        )
    }
}

/// Whether a classified flow represents a cash settlement.
///
/// ``PIK`` is a capitalization event and ``DefaultedNotional`` is a write-down;
/// both return ``False``. All settlement kinds return ``True``.
///
/// Parameters
/// ----------
/// kind : CFKind or str
///     Classified cashflow kind to test.
#[pyfunction(name = "is_cash_settlement_kind")]
#[pyo3(text_signature = "(kind)")]
fn py_is_cash_settlement_kind(kind: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(is_cash_settlement_kind(extract_cf_kind(kind)?))
}

/// Register the `finstack_quant.cashflows.primitives` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "primitives")?;
    module.setattr(
        "__doc__",
        "Cashflow primitives: CashFlow, CFKind, settlement classification.",
    )?;
    module.add_class::<PyCFKind>()?;
    module.add_class::<PyCashFlow>()?;
    module.add_class::<PyCashFlowAccrual>()?;
    module.add_function(wrap_pyfunction!(py_is_cash_settlement_kind, &module)?)?;

    let all = PyList::new(
        py,
        [
            "CFKind",
            "CashFlow",
            "CashFlowAccrual",
            "is_cash_settlement_kind",
        ],
    )?;
    module.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &module,
        "primitives",
        "finstack_quant.cashflows",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;
    Ok(())
}
