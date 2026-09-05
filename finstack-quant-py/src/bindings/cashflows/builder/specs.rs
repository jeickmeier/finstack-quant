//! Spec-type bindings for `finstack_quant_cashflows::builder::specs`.

use finstack_quant_cashflows::builder::{
    AmortizationSpec, CouponType, DefaultModelSpec, FeeAccrualBasis, FeeBase, FeeSpec,
    FixedCouponSpec, FixedWindow, FloatingCouponSpec, FloatingRateFallback, FloatingRateSpec,
    Notional, OvernightCompoundingMethod, OvernightIndexConstraintApplication, PrepaymentModelSpec,
    PrincipalExchange, RecoveryModelSpec, RollRule, ScheduleParams, StepUpCouponSpec,
};
use finstack_quant_cashflows::serde_defaults;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use rust_decimal::Decimal;
use serde_json::Value;

use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::currency::PyCurrency;
use crate::bindings::core::dates::calendar::{
    extract_business_day_convention, PyBusinessDayConvention,
};
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::{extract_tenor, PyTenor};
use crate::bindings::core::money::{decimal_from_py, decimal_to_py, is_python_decimal, PyMoney};
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;

/// `(date, amount)` rows as returned to Python: Python date objects paired with `Money`.
type DateMoneyRows<'py> = Vec<(Bound<'py, PyAny>, PyMoney)>;

/// Extract a `rust_decimal::Decimal` from `decimal.Decimal`, `float`, `int`,
/// or a numeric `str` (parsed losslessly, e.g. ``"0.05"``).
pub(crate) fn decimal_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Decimal> {
    if is_python_decimal(obj)? {
        return decimal_from_py(obj);
    }
    if let Ok(text) = obj.extract::<String>() {
        return text.trim().parse::<Decimal>().map_err(|e| {
            crate::errors::value_error(format!("'{text}' is not a decimal number: {e}"))
        });
    }
    let value: f64 = obj.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("expected decimal.Decimal, float, int, or str")
    })?;
    Decimal::try_from(value).map_err(|e| {
        crate::errors::value_error(format!(
            "value {value} is not representable as Decimal: {e}"
        ))
    })
}

/// Extract a list of `(date, Decimal)` pairs.
pub(crate) fn date_decimal_pairs(
    items: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
) -> PyResult<Vec<(Date, Decimal)>> {
    items
        .iter()
        .map(|(d, v)| Ok((extract_date(d)?, decimal_from_any(v)?)))
        .collect()
}

/// Extract a list of `(date, Money)` pairs.
fn date_money_pairs(items: Vec<(Bound<'_, PyAny>, PyMoney)>) -> PyResult<Vec<(Date, Money)>> {
    items
        .iter()
        .map(|(d, m)| Ok((extract_date(d)?, m.inner)))
        .collect()
}

fn date_money_to_py<'py>(
    py: Python<'py>,
    items: &[(Date, Money)],
) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
    items
        .iter()
        .map(|(d, m)| Ok((date_to_py(py, *d)?, PyMoney::from_inner(*m))))
        .collect()
}

fn decimal_opt_to_py<'py>(
    py: Python<'py>,
    value: Option<Decimal>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    value.map(|v| decimal_to_py(py, v)).transpose()
}

/// Render one serde JSON value Python-style (used by enum reprs).
fn render_value(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("{s:?}"),
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(render_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(fields) => {
            if let (Some(amount), Some(currency)) = (fields.get("amount"), fields.get("currency")) {
                if fields.len() == 2 {
                    return format!("{} {}", render_value(amount), render_value(currency));
                }
            }
            format!(
                "{{{}}}",
                fields
                    .iter()
                    .map(|(k, v)| format!("{k}={}", render_value(v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

/// Python-style repr for an externally-tagged serde enum:
/// unit variants render as ``Type.UPPER_SNAKE``, data variants as
/// ``Type.variant(field=value, ...)``.
fn enum_repr<T: serde::Serialize>(type_name: &str, value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(variant)) => format!("{type_name}.{}", variant.to_ascii_uppercase()),
        Ok(Value::Object(map)) if map.len() == 1 => {
            let (variant, payload) = map
                .iter()
                .next()
                .map_or(("", &Value::Null), |(k, v)| (k.as_str(), v));
            match payload {
                Value::Object(fields) => format!(
                    "{type_name}.{variant}({})",
                    fields
                        .iter()
                        .map(|(k, v)| format!("{k}={}", render_value(v)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                other => format!("{type_name}.{variant}({})", render_value(other)),
            }
        }
        _ => format!("{type_name}(...)"),
    }
}

/// Generate `to_json` / `from_json` / `__reduce__` for a serde-backed wrapper.
macro_rules! wire_methods {
    ($py_type:ident, $rust_type:ty, $name:literal) => {
        #[pymethods]
        impl $py_type {
            /// Serialize to the canonical JSON wire form.
            #[allow(clippy::wrong_self_convention)]
            #[pyo3(text_signature = "(self)")]
            fn to_json(&self) -> PyResult<String> {
                serde_json::to_string(&self.inner).map_err(|e| {
                    crate::errors::serde_json_to_py(e, concat!("failed to serialize ", $name))
                })
            }

            /// Deserialize from the canonical JSON wire form (strict field names).
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the JSON is malformed or carries unknown fields.
            #[staticmethod]
            #[pyo3(text_signature = "(json)")]
            fn from_json(json: &str) -> PyResult<Self> {
                serde_json::from_str::<$rust_type>(json)
                    .map(|inner| Self { inner })
                    .map_err(|e| {
                        crate::errors::serde_json_to_py(e, concat!("invalid ", $name, " JSON"))
                    })
            }

            /// Support ``pickle`` through the JSON wire form.
            fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
                let from_json = py.get_type::<Self>().getattr("from_json")?;
                crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
            }
        }
    };
}

/// Roll-date rule for schedule anchors: ``RollRule.NONE`` (plain tenor
/// stepping), ``RollRule.IMM`` (third Wednesdays of Mar/Jun/Sep/Dec) or
/// ``RollRule.CDS_IMM`` (20th of Mar/Jun/Sep/Dec, Big-Bang front accrual).
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import RollRule
/// >>> RollRule.IMM == RollRule.IMM
/// True
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

/// Whether issue funding and maturity redemption notionals are emitted:
/// ``PrincipalExchange.INITIAL_AND_FINAL`` (bond/loan, default) or
/// ``PrincipalExchange.NONE`` (vanilla swap legs).
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import PrincipalExchange
/// >>> PrincipalExchange.NONE != PrincipalExchange.INITIAL_AND_FINAL
/// True
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

/// Coupon settlement type: ``CouponType.CASH``, ``CouponType.PIK`` or
/// ``CouponType.split(cash_pct, pik_pct)``.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import CouponType
/// >>> CouponType.split(0.5, 0.5).cash_pct
/// Decimal('0.5')
#[pyclass(
    name = "CouponType",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyCouponType {
    /// Inner coupon settlement type.
    pub(crate) inner: CouponType,
}

#[pymethods]
impl PyCouponType {
    /// 100% paid in cash.
    #[classattr]
    const CASH: PyCouponType = PyCouponType {
        inner: CouponType::Cash,
    };
    /// 100% capitalized into principal.
    #[classattr]
    const PIK: PyCouponType = PyCouponType {
        inner: CouponType::Pik,
    };

    /// Split settlement: explicit cash and PIK fractions in ``[0, 1]``.
    ///
    /// Parameters
    /// ----------
    /// cash_pct : Decimal, float or str
    ///     Fraction of the coupon paid in cash.
    /// pik_pct : Decimal, float or str
    ///     Fraction of the coupon capitalized.
    #[staticmethod]
    #[pyo3(text_signature = "(cash_pct, pik_pct)")]
    fn split(cash_pct: &Bound<'_, PyAny>, pik_pct: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: CouponType::Split {
                cash_pct: decimal_from_any(cash_pct)?,
                pik_pct: decimal_from_any(pik_pct)?,
            },
        })
    }

    /// Cash fraction: ``1`` for CASH, ``0`` for PIK, the split value otherwise.
    #[getter]
    fn cash_pct<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let value = match self.inner {
            CouponType::Cash => Decimal::ONE,
            CouponType::Pik => Decimal::ZERO,
            CouponType::Split { cash_pct, .. } => cash_pct,
        };
        decimal_to_py(py, value)
    }

    /// PIK fraction: ``0`` for CASH, ``1`` for PIK, the split value otherwise.
    #[getter]
    fn pik_pct<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let value = match self.inner {
            CouponType::Cash => Decimal::ZERO,
            CouponType::Pik => Decimal::ONE,
            CouponType::Split { pik_pct, .. } => pik_pct,
        };
        decimal_to_py(py, value)
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("CouponType", &self.inner)
    }
}

/// Overnight-index compounding convention for RFR legs.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import OvernightCompoundingMethod
/// >>> OvernightCompoundingMethod.compounded_with_lookback(5)
/// OvernightCompoundingMethod.compounded_with_lookback(lookback_days=5)
#[pyclass(
    name = "OvernightCompoundingMethod",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyOvernightCompoundingMethod {
    /// Inner compounding method.
    pub(crate) inner: OvernightCompoundingMethod,
}

#[pymethods]
impl PyOvernightCompoundingMethod {
    /// Arithmetic average of daily fixings weighted by accrual days.
    #[classattr]
    const SIMPLE_AVERAGE: PyOvernightCompoundingMethod = PyOvernightCompoundingMethod {
        inner: OvernightCompoundingMethod::SimpleAverage,
    };
    /// Compounded in arrears (ISDA 2021 standard; default).
    #[classattr]
    const COMPOUNDED_IN_ARREARS: PyOvernightCompoundingMethod = PyOvernightCompoundingMethod {
        inner: OvernightCompoundingMethod::CompoundedInArrears,
    };

    /// Compounded in arrears with a lookback of ``lookback_days`` business days.
    #[staticmethod]
    #[pyo3(text_signature = "(lookback_days)")]
    fn compounded_with_lookback(lookback_days: u32) -> Self {
        Self {
            inner: OvernightCompoundingMethod::CompoundedWithLookback { lookback_days },
        }
    }

    /// Compounded in arrears with a rate lockout of ``lockout_days`` business days.
    #[staticmethod]
    #[pyo3(text_signature = "(lockout_days)")]
    fn compounded_with_lockout(lockout_days: u32) -> Self {
        Self {
            inner: OvernightCompoundingMethod::CompoundedWithLockout { lockout_days },
        }
    }

    /// Compounded in arrears with an observation shift of ``shift_days`` business days.
    #[staticmethod]
    #[pyo3(text_signature = "(shift_days)")]
    fn compounded_with_observation_shift(shift_days: u32) -> Self {
        Self {
            inner: OvernightCompoundingMethod::CompoundedWithObservationShift { shift_days },
        }
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("OvernightCompoundingMethod", &self.inner)
    }
}

/// Where index floors/caps apply on an overnight leg: ``DAILY`` (each fixing,
/// default) or ``PERIOD`` (once on the compounded period rate).
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import OvernightIndexConstraintApplication
/// >>> OvernightIndexConstraintApplication.DAILY
/// OvernightIndexConstraintApplication.DAILY
#[pyclass(
    name = "OvernightIndexConstraintApplication",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyOvernightIndexConstraintApplication {
    /// Inner constraint-application policy.
    pub(crate) inner: OvernightIndexConstraintApplication,
}

#[pymethods]
impl PyOvernightIndexConstraintApplication {
    /// Apply index floors/caps to each daily fixing before compounding (default).
    #[classattr]
    const DAILY: PyOvernightIndexConstraintApplication = PyOvernightIndexConstraintApplication {
        inner: OvernightIndexConstraintApplication::Daily,
    };
    /// Apply index floors/caps once to the compounded period index rate.
    #[classattr]
    const PERIOD: PyOvernightIndexConstraintApplication = PyOvernightIndexConstraintApplication {
        inner: OvernightIndexConstraintApplication::Period,
    };

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("OvernightIndexConstraintApplication", &self.inner)
    }
}

/// Policy when a floating leg's forward curve is missing: ``ERROR``
/// (default, fail the build), ``SPREAD_ONLY`` (project the spread alone) or
/// ``fixed_rate(rate)`` (use a fixed decimal index rate).
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import FloatingRateFallback
/// >>> FloatingRateFallback.fixed_rate(0.045).rate
/// Decimal('0.045')
#[pyclass(
    name = "FloatingRateFallback",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyFloatingRateFallback {
    /// Inner fallback policy.
    pub(crate) inner: FloatingRateFallback,
}

#[pymethods]
impl PyFloatingRateFallback {
    /// Fail the build when the forward curve lookup fails (default, safest).
    #[allow(non_snake_case)]
    #[classattr]
    fn ERROR() -> Self {
        Self {
            inner: FloatingRateFallback::Error,
        }
    }
    /// Project spread-only when no forward curve is available (explicit opt-in).
    #[allow(non_snake_case)]
    #[classattr]
    fn SPREAD_ONLY() -> Self {
        Self {
            inner: FloatingRateFallback::SpreadOnly,
        }
    }

    /// Use ``rate`` (decimal annual rate, e.g. ``0.045``) as the index component.
    #[staticmethod]
    #[pyo3(text_signature = "(rate)")]
    fn fixed_rate(rate: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: FloatingRateFallback::FixedRate(decimal_from_any(rate)?),
        })
    }

    /// Fixed decimal index rate for ``fixed_rate`` fallbacks, else ``None``.
    #[getter]
    fn rate<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner {
            FloatingRateFallback::FixedRate(rate) => decimal_to_py(py, rate).map(Some),
            _ => Ok(None),
        }
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("FloatingRateFallback", &self.inner)
    }
}

/// How the outstanding balance is sampled for periodic fees:
/// ``POINT_IN_TIME`` (period start, default) or ``TIME_WEIGHTED_AVERAGE``.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import FeeAccrualBasis
/// >>> FeeAccrualBasis.POINT_IN_TIME
/// FeeAccrualBasis.POINT_IN_TIME
#[pyclass(
    name = "FeeAccrualBasis",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyFeeAccrualBasis {
    /// Inner fee accrual basis.
    pub(crate) inner: FeeAccrualBasis,
}

#[pymethods]
impl PyFeeAccrualBasis {
    /// Sample the outstanding balance at accrual-period start (default).
    #[classattr]
    const POINT_IN_TIME: PyFeeAccrualBasis = PyFeeAccrualBasis {
        inner: FeeAccrualBasis::PointInTime,
    };
    /// Time-weighted average outstanding over the accrual period.
    #[classattr]
    const TIME_WEIGHTED_AVERAGE: PyFeeAccrualBasis = PyFeeAccrualBasis {
        inner: FeeAccrualBasis::TimeWeightedAverage,
    };

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("FeeAccrualBasis", &self.inner)
    }
}

/// Schedule-generation parameters for coupons and periodic fees.
///
/// Parameters
/// ----------
/// frequency : Tenor or str
///     Accrual and payment frequency (e.g. ``"3M"``).
/// day_count : DayCount
///     Day-count convention for accrual year fractions.
/// calendar_id : str
///     Holiday calendar id (``"weekends_only"`` for weekend-only rolling);
///     validated at construction.
/// business_day_convention : BusinessDayConvention or str, optional
///     Payment-date rolling convention (default Modified Following, the
///     Rust wire default).
/// stub : StubKind, optional
///     Stub rule (default short-front, the Rust wire default).
/// end_of_month : bool, default False
///     Preserve end-of-month rolling.
/// payment_lag_days : int, default 0
///     Payment lag in business days (non-negative).
/// adjust_accrual_dates : bool, default False
///     Roll accrual boundaries with ``business_day_convention`` (swap/ISDA convention).
/// roll_rule : RollRule, optional
///     IMM/CDS-IMM anchor grid (default none).
///
/// Raises
/// ------
/// ValueError
///     If ``calendar_id`` is unknown, ``payment_lag_days`` is negative, or
///     ``frequency`` / ``business_day_convention`` cannot be parsed.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import ScheduleParams
/// >>> ScheduleParams.usd_sofr_swap().calendar_id
/// 'usny'
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

wire_methods!(PyScheduleParams, ScheduleParams, "ScheduleParams");

#[pymethods]
impl PyScheduleParams {
    /// Construct and validate schedule parameters; see the class docstring.
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
            None => serde_defaults::bdc_modified_following(),
        };
        let inner = ScheduleParams {
            frequency: extract_tenor(frequency)?,
            day_count: day_count.inner,
            business_day_convention,
            calendar_id: calendar_id.to_string(),
            stub: stub.map_or_else(serde_defaults::stub_short_front, |s| s.inner),
            end_of_month,
            payment_lag_days,
            adjust_accrual_dates,
            roll_rule: roll_rule.map_or(RollRule::None, |r| r.inner),
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Fail-fast validation: known calendar id and non-negative payment lag.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``calendar_id`` is unknown or ``payment_lag_days`` is negative.
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Accrual and payment frequency.
    #[getter]
    fn frequency(&self) -> PyTenor {
        PyTenor::from_inner(self.inner.frequency)
    }

    /// Day-count convention.
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.day_count)
    }

    /// Payment-date rolling convention.
    #[getter]
    fn business_day_convention(&self) -> PyBusinessDayConvention {
        PyBusinessDayConvention::from_inner(self.inner.business_day_convention)
    }

    /// Holiday calendar identifier.
    #[getter]
    fn calendar_id(&self) -> String {
        self.inner.calendar_id.clone()
    }

    /// Stub-handling rule.
    #[getter]
    fn stub(&self) -> PyStubKind {
        PyStubKind::from_inner(self.inner.stub)
    }

    /// Payment lag in business days.
    #[getter]
    fn payment_lag_days(&self) -> i32 {
        self.inner.payment_lag_days
    }

    /// Whether end-of-month rolling is preserved.
    #[getter]
    fn end_of_month(&self) -> bool {
        self.inner.end_of_month
    }

    /// Whether accrual boundaries are business-day adjusted.
    #[getter]
    fn adjust_accrual_dates(&self) -> bool {
        self.inner.adjust_accrual_dates
    }

    /// Roll-date rule (IMM / CDS-IMM grid or none).
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

/// Fixed-rate coupon window with a shared schedule.
///
/// Parameters
/// ----------
/// rate : Decimal, float or str
///     Annual coupon rate as a decimal (``0.05`` for 5%).
/// schedule : ScheduleParams
///     Accrual and payment schedule conventions for the window.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import FixedWindow, ScheduleParams
/// >>> FixedWindow(0.05, ScheduleParams.quarterly_act360()).rate
/// Decimal('0.05')
#[pyclass(
    name = "FixedWindow",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFixedWindow {
    /// Inner fixed-rate window.
    pub(crate) inner: FixedWindow,
}

wire_methods!(PyFixedWindow, FixedWindow, "FixedWindow");

#[pymethods]
impl PyFixedWindow {
    /// Construct a fixed window; see the class docstring for parameters.
    #[new]
    #[pyo3(text_signature = "(rate, schedule)")]
    fn new(rate: &Bound<'_, PyAny>, schedule: PyRef<'_, PyScheduleParams>) -> PyResult<Self> {
        Ok(Self {
            inner: FixedWindow {
                rate: decimal_from_any(rate)?,
                schedule: schedule.inner.clone(),
            },
        })
    }

    /// Annual coupon rate as ``decimal.Decimal``.
    #[getter]
    fn rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.rate)
    }

    /// Schedule conventions for this window.
    #[getter]
    fn schedule(&self) -> PyScheduleParams {
        PyScheduleParams::from_inner(self.inner.schedule.clone())
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("FixedWindow", &self.inner)
    }
}

/// Fixed-rate coupon specification.
///
/// Parameters
/// ----------
/// rate : Decimal, float or str
///     Annual coupon rate as a decimal (``0.05`` for 5%).
/// schedule : ScheduleParams
///     Accrual and payment schedule conventions.
/// coupon_type : CouponType, optional
///     Cash (default), PIK, or split settlement.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import FixedCouponSpec, ScheduleParams
/// >>> FixedCouponSpec(0.05, ScheduleParams.semiannual_30360()).coupon_type
/// CouponType.CASH
#[pyclass(
    name = "FixedCouponSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFixedCouponSpec {
    /// Inner fixed-coupon spec.
    pub(crate) inner: FixedCouponSpec,
}

wire_methods!(PyFixedCouponSpec, FixedCouponSpec, "FixedCouponSpec");

#[pymethods]
impl PyFixedCouponSpec {
    /// Construct a fixed coupon spec; see the class docstring for parameters.
    #[new]
    #[pyo3(
        signature = (rate, schedule, coupon_type=None),
        text_signature = "(rate, schedule, coupon_type=None)"
    )]
    fn new(
        rate: &Bound<'_, PyAny>,
        schedule: PyRef<'_, PyScheduleParams>,
        coupon_type: Option<PyRef<'_, PyCouponType>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FixedCouponSpec {
                coupon_type: coupon_type.map_or(CouponType::Cash, |c| c.inner),
                rate: decimal_from_any(rate)?,
                schedule: schedule.inner.clone(),
            },
        })
    }

    /// Annual coupon rate as ``decimal.Decimal``.
    #[getter]
    fn rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.rate)
    }

    /// Schedule conventions.
    #[getter]
    fn schedule(&self) -> PyScheduleParams {
        PyScheduleParams::from_inner(self.inner.schedule.clone())
    }

    /// Cash / PIK / split settlement.
    #[getter]
    fn coupon_type(&self) -> PyCouponType {
        PyCouponType {
            inner: self.inner.coupon_type,
        }
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("FixedCouponSpec", &self.inner)
    }
}

/// Canonical floating-rate specification (index, spread, gearing, floors and
/// caps, reset conventions, overnight compounding, fallback).
///
/// Parameters mirror the Rust struct field-for-field. ``*_bp`` values are
/// basis points and accept ``decimal.Decimal`` (lossless), ``float``, ``int``
/// or a numeric ``str``.
///
/// Parameters
/// ----------
/// index_id : str
///     Forward curve identifier (e.g. ``"USD-SOFR"``).
/// spread_bp : Decimal, float or str
///     Spread over the index in basis points.
/// reset_frequency : Tenor or str
///     Reset frequency (e.g. ``"3M"``).
/// gearing : Decimal, float or str, default 1
///     Multiplier applied to the index (and spread when
///     ``gearing_includes_spread``).
/// gearing_includes_spread : bool, default True
///     Whether gearing also scales the spread.
/// index_floor_bp, all_in_floor_bp, all_in_cap_bp, index_cap_bp : Decimal, float or str, optional
///     Floors and caps in basis points.
/// overnight_index_constraints : OvernightIndexConstraintApplication, optional
///     Where floors/caps apply on overnight legs (default DAILY).
/// index_tenor : Tenor or str, optional
///     Explicit index tenor when it differs from ``reset_frequency``.
/// reset_lag_days : int, default 2
///     Fixing lag in business days (non-negative).
/// fixing_calendar_id : str, optional
///     Calendar for fixing-date rolls.
/// overnight_compounding : OvernightCompoundingMethod, optional
///     Compounding method for overnight indices.
/// overnight_basis : DayCount, optional
///     Day count for overnight compounding.
/// fallback : FloatingRateFallback, optional
///     Behaviour when the forward curve is missing (default ERROR).
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import FloatingRateSpec
/// >>> FloatingRateSpec.sofr(50).index_id
/// 'USD-SOFR'
#[pyclass(
    name = "FloatingRateSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFloatingRateSpec {
    /// Inner floating-rate spec.
    pub(crate) inner: FloatingRateSpec,
}

wire_methods!(PyFloatingRateSpec, FloatingRateSpec, "FloatingRateSpec");

#[pymethods]
impl PyFloatingRateSpec {
    /// Construct a floating-rate spec; see the class docstring for parameters.
    #[new]
    #[pyo3(
        signature = (index_id, spread_bp, reset_frequency, gearing=None, gearing_includes_spread=true, index_floor_bp=None, all_in_floor_bp=None, all_in_cap_bp=None, index_cap_bp=None, overnight_index_constraints=None, index_tenor=None, reset_lag_days=2, fixing_calendar_id=None, overnight_compounding=None, overnight_basis=None, fallback=None),
        text_signature = "(index_id, spread_bp, reset_frequency, gearing=None, gearing_includes_spread=True, index_floor_bp=None, all_in_floor_bp=None, all_in_cap_bp=None, index_cap_bp=None, overnight_index_constraints=None, index_tenor=None, reset_lag_days=2, fixing_calendar_id=None, overnight_compounding=None, overnight_basis=None, fallback=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        index_id: &str,
        spread_bp: &Bound<'_, PyAny>,
        reset_frequency: &Bound<'_, PyAny>,
        gearing: Option<&Bound<'_, PyAny>>,
        gearing_includes_spread: bool,
        index_floor_bp: Option<&Bound<'_, PyAny>>,
        all_in_floor_bp: Option<&Bound<'_, PyAny>>,
        all_in_cap_bp: Option<&Bound<'_, PyAny>>,
        index_cap_bp: Option<&Bound<'_, PyAny>>,
        overnight_index_constraints: Option<PyRef<'_, PyOvernightIndexConstraintApplication>>,
        index_tenor: Option<&Bound<'_, PyAny>>,
        reset_lag_days: i32,
        fixing_calendar_id: Option<String>,
        overnight_compounding: Option<PyRef<'_, PyOvernightCompoundingMethod>>,
        overnight_basis: Option<PyRef<'_, PyDayCount>>,
        fallback: Option<PyRef<'_, PyFloatingRateFallback>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FloatingRateSpec {
                index_id: finstack_quant_core::types::CurveId::from(index_id),
                spread_bp: decimal_from_any(spread_bp)?,
                gearing: gearing.map_or(Ok(Decimal::ONE), decimal_from_any)?,
                gearing_includes_spread,
                index_floor_bp: index_floor_bp.map(decimal_from_any).transpose()?,
                all_in_floor_bp: all_in_floor_bp.map(decimal_from_any).transpose()?,
                all_in_cap_bp: all_in_cap_bp.map(decimal_from_any).transpose()?,
                index_cap_bp: index_cap_bp.map(decimal_from_any).transpose()?,
                overnight_index_constraints: overnight_index_constraints
                    .map_or(OvernightIndexConstraintApplication::Daily, |c| c.inner),
                reset_frequency: extract_tenor(reset_frequency)?,
                index_tenor: index_tenor.map(extract_tenor).transpose()?,
                reset_lag_days,
                fixing_calendar_id,
                overnight_compounding: overnight_compounding.map(|m| m.inner),
                overnight_basis: overnight_basis.map(|d| d.inner),
                fallback: fallback.map_or(FloatingRateFallback::Error, |f| f.inner.clone()),
            },
        })
    }

    /// USD SOFR compounded in arrears (ARRC / ISDA 2021): index ``USD-SOFR``,
    /// quarterly resets, Act/360 daily compounding, no reset lag, USNY fixings.
    ///
    /// Parameters
    /// ----------
    /// spread_bp : Decimal, float or str
    ///     Spread over compounded SOFR in basis points (``50`` = +50 bp).
    #[staticmethod]
    #[pyo3(text_signature = "(spread_bp)")]
    fn sofr(spread_bp: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: FloatingRateSpec::sofr(decimal_from_any(spread_bp)?),
        })
    }

    /// GBP SONIA compounded in arrears: index ``GBP-SONIA``, annual resets,
    /// Act/365F daily compounding, no reset lag, GBLO fixings.
    ///
    /// Parameters
    /// ----------
    /// spread_bp : Decimal, float or str
    ///     Spread over compounded SONIA in basis points.
    #[staticmethod]
    #[pyo3(text_signature = "(spread_bp)")]
    fn sonia(spread_bp: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: FloatingRateSpec::sonia(decimal_from_any(spread_bp)?),
        })
    }

    /// EUR 3M EURIBOR term rate: index ``EUR-EURIBOR-3M``, quarterly resets
    /// fixed in advance with a 2-business-day lag on TARGET2, 3M index tenor.
    ///
    /// Parameters
    /// ----------
    /// spread_bp : Decimal, float or str
    ///     Spread over 3M EURIBOR in basis points.
    #[staticmethod]
    #[pyo3(text_signature = "(spread_bp)")]
    fn euribor_3m(spread_bp: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: FloatingRateSpec::euribor_3m(decimal_from_any(spread_bp)?),
        })
    }

    /// Forward curve identifier.
    #[getter]
    fn index_id(&self) -> String {
        self.inner.index_id.to_string()
    }

    /// Spread over the index in basis points, as ``decimal.Decimal``.
    #[getter]
    fn spread_bp<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.spread_bp)
    }

    /// Gearing multiplier as ``decimal.Decimal``.
    #[getter]
    fn gearing<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.gearing)
    }

    /// Whether gearing also scales the spread.
    #[getter]
    fn gearing_includes_spread(&self) -> bool {
        self.inner.gearing_includes_spread
    }

    /// Index floor in basis points, if any.
    #[getter]
    fn index_floor_bp<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        decimal_opt_to_py(py, self.inner.index_floor_bp)
    }

    /// All-in floor in basis points, if any.
    #[getter]
    fn all_in_floor_bp<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        decimal_opt_to_py(py, self.inner.all_in_floor_bp)
    }

    /// All-in cap in basis points, if any.
    #[getter]
    fn all_in_cap_bp<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        decimal_opt_to_py(py, self.inner.all_in_cap_bp)
    }

    /// Index cap in basis points, if any.
    #[getter]
    fn index_cap_bp<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        decimal_opt_to_py(py, self.inner.index_cap_bp)
    }

    /// Where index floors/caps apply on overnight legs.
    #[getter]
    fn overnight_index_constraints(&self) -> PyOvernightIndexConstraintApplication {
        PyOvernightIndexConstraintApplication {
            inner: self.inner.overnight_index_constraints,
        }
    }

    /// Reset frequency.
    #[getter]
    fn reset_frequency(&self) -> PyTenor {
        PyTenor::from_inner(self.inner.reset_frequency)
    }

    /// Explicit index tenor, if set.
    #[getter]
    fn index_tenor(&self) -> Option<PyTenor> {
        self.inner.index_tenor.map(PyTenor::from_inner)
    }

    /// Fixing lag in business days.
    #[getter]
    fn reset_lag_days(&self) -> i32 {
        self.inner.reset_lag_days
    }

    /// Fixing calendar identifier, if set.
    #[getter]
    fn fixing_calendar_id(&self) -> Option<String> {
        self.inner.fixing_calendar_id.clone()
    }

    /// Overnight compounding method, if set.
    #[getter]
    fn overnight_compounding(&self) -> Option<PyOvernightCompoundingMethod> {
        self.inner
            .overnight_compounding
            .map(|inner| PyOvernightCompoundingMethod { inner })
    }

    /// Overnight compounding day count, if set.
    #[getter]
    fn overnight_basis(&self) -> Option<PyDayCount> {
        self.inner.overnight_basis.map(PyDayCount::from_inner)
    }

    /// Missing-curve fallback policy.
    #[getter]
    fn fallback(&self) -> PyFloatingRateFallback {
        PyFloatingRateFallback {
            inner: self.inner.fallback.clone(),
        }
    }

    /// Validate reset lag and floor/cap ordering.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the reset lag is negative or a floor exceeds its cap.
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("FloatingRateSpec", &self.inner)
    }
}

/// Floating coupon specification composing a ``FloatingRateSpec`` with a
/// schedule and settlement type.
///
/// Parameters
/// ----------
/// rate_spec : FloatingRateSpec
///     Index, spread and reset conventions.
/// schedule : ScheduleParams
///     Accrual and payment schedule conventions.
/// coupon_type : CouponType, optional
///     Cash (default), PIK, or split settlement.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import FloatingCouponSpec, FloatingRateSpec, ScheduleParams
/// >>> spec = FloatingCouponSpec(FloatingRateSpec.sofr(50), ScheduleParams.usd_sofr_swap())
/// >>> spec.rate_spec.index_id
/// 'USD-SOFR'
#[pyclass(
    name = "FloatingCouponSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFloatingCouponSpec {
    /// Inner floating-coupon spec.
    pub(crate) inner: FloatingCouponSpec,
}

wire_methods!(
    PyFloatingCouponSpec,
    FloatingCouponSpec,
    "FloatingCouponSpec"
);

#[pymethods]
impl PyFloatingCouponSpec {
    /// Construct a floating coupon spec; see the class docstring for parameters.
    #[new]
    #[pyo3(
        signature = (rate_spec, schedule, coupon_type=None),
        text_signature = "(rate_spec, schedule, coupon_type=None)"
    )]
    fn new(
        rate_spec: PyRef<'_, PyFloatingRateSpec>,
        schedule: PyRef<'_, PyScheduleParams>,
        coupon_type: Option<PyRef<'_, PyCouponType>>,
    ) -> Self {
        Self {
            inner: FloatingCouponSpec {
                rate_spec: rate_spec.inner.clone(),
                coupon_type: coupon_type.map_or(CouponType::Cash, |c| c.inner),
                schedule: schedule.inner.clone(),
            },
        }
    }

    /// Floating rate specification.
    #[getter]
    fn rate_spec(&self) -> PyFloatingRateSpec {
        PyFloatingRateSpec {
            inner: self.inner.rate_spec.clone(),
        }
    }

    /// Schedule conventions.
    #[getter]
    fn schedule(&self) -> PyScheduleParams {
        PyScheduleParams::from_inner(self.inner.schedule.clone())
    }

    /// Cash / PIK / split settlement.
    #[getter]
    fn coupon_type(&self) -> PyCouponType {
        PyCouponType {
            inner: self.inner.coupon_type,
        }
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("FloatingCouponSpec", &self.inner)
    }
}

/// Step-up / step-down coupon specification.
///
/// Parameters
/// ----------
/// initial_rate : Decimal, float or str
///     Rate until the first step date (decimal, ``0.05`` = 5%).
/// step_schedule : list[tuple[datetime.date, Decimal | float | str]]
///     ``(effective_date, new_rate)`` pairs, strictly increasing by date.
/// schedule : ScheduleParams
///     Accrual and payment schedule conventions.
/// coupon_type : CouponType, optional
///     Cash (default), PIK, or split settlement.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.cashflows.builder import StepUpCouponSpec, ScheduleParams
/// >>> spec = StepUpCouponSpec(0.05, [(datetime.date(2026, 1, 15), 0.06)], ScheduleParams.semiannual_30360())
/// >>> len(spec.step_schedule)
/// 1
#[pyclass(
    name = "StepUpCouponSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyStepUpCouponSpec {
    /// Inner step-up coupon spec.
    pub(crate) inner: StepUpCouponSpec,
}

wire_methods!(PyStepUpCouponSpec, StepUpCouponSpec, "StepUpCouponSpec");

#[pymethods]
impl PyStepUpCouponSpec {
    /// Construct a step-up coupon spec; see the class docstring for parameters.
    #[new]
    #[pyo3(
        signature = (initial_rate, step_schedule, schedule, coupon_type=None),
        text_signature = "(initial_rate, step_schedule, schedule, coupon_type=None)"
    )]
    fn new(
        initial_rate: &Bound<'_, PyAny>,
        step_schedule: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
        schedule: PyRef<'_, PyScheduleParams>,
        coupon_type: Option<PyRef<'_, PyCouponType>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: StepUpCouponSpec {
                coupon_type: coupon_type.map_or(CouponType::Cash, |c| c.inner),
                initial_rate: decimal_from_any(initial_rate)?,
                step_schedule: date_decimal_pairs(step_schedule)?,
                schedule: schedule.inner.clone(),
            },
        })
    }

    /// Rate until the first step date, as ``decimal.Decimal``.
    #[getter]
    fn initial_rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.initial_rate)
    }

    /// ``(effective_date, new_rate)`` pairs.
    #[getter]
    fn step_schedule<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
        self.inner
            .step_schedule
            .iter()
            .map(|(d, r)| Ok((date_to_py(py, *d)?, decimal_to_py(py, *r)?)))
            .collect()
    }

    /// Schedule conventions.
    #[getter]
    fn schedule(&self) -> PyScheduleParams {
        PyScheduleParams::from_inner(self.inner.schedule.clone())
    }

    /// Cash / PIK / split settlement.
    #[getter]
    fn coupon_type(&self) -> PyCouponType {
        PyCouponType {
            inner: self.inner.coupon_type,
        }
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("StepUpCouponSpec", &self.inner)
    }
}

/// Amortization rule: ``NONE`` (bullet), ``linear_to``, ``step_remaining``,
/// ``percent_of_original_per_period`` or ``custom_principal``.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import AmortizationSpec
/// >>> AmortizationSpec.percent_of_original_per_period(0.05).kind
/// 'percent_of_original_per_period'
#[pyclass(
    name = "AmortizationSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct PyAmortizationSpec {
    /// Inner amortization rule.
    pub(crate) inner: AmortizationSpec,
}

wire_methods!(PyAmortizationSpec, AmortizationSpec, "AmortizationSpec");

#[pymethods]
impl PyAmortizationSpec {
    /// No amortization (bullet).
    #[allow(non_snake_case)]
    #[classattr]
    fn NONE() -> Self {
        Self {
            inner: AmortizationSpec::None,
        }
    }

    /// Linear paydown towards ``final_notional`` over all coupon periods.
    #[staticmethod]
    #[pyo3(text_signature = "(final_notional)")]
    fn linear_to(final_notional: PyMoney) -> Self {
        Self {
            inner: AmortizationSpec::LinearTo {
                final_notional: final_notional.inner,
            },
        }
    }

    /// Explicit ``(date, remaining_principal_after_date)`` schedule.
    #[staticmethod]
    #[pyo3(text_signature = "(schedule)")]
    fn step_remaining(schedule: Vec<(Bound<'_, PyAny>, PyMoney)>) -> PyResult<Self> {
        Ok(Self {
            inner: AmortizationSpec::StepRemaining {
                schedule: date_money_pairs(schedule)?,
            },
        })
    }

    /// Fixed percentage of original notional paid each period (``0.05`` = 5%).
    #[staticmethod]
    #[pyo3(text_signature = "(pct)")]
    fn percent_of_original_per_period(pct: f64) -> Self {
        Self {
            inner: AmortizationSpec::PercentOfOriginalPerPeriod { pct },
        }
    }

    /// Custom principal exchanges on specific dates (absolute cash amounts).
    #[staticmethod]
    #[pyo3(text_signature = "(items)")]
    fn custom_principal(items: Vec<(Bound<'_, PyAny>, PyMoney)>) -> PyResult<Self> {
        Ok(Self {
            inner: AmortizationSpec::CustomPrincipal {
                items: date_money_pairs(items)?,
            },
        })
    }

    /// Variant label: ``"none"``, ``"linear_to"``, ``"step_remaining"``,
    /// ``"percent_of_original_per_period"`` or ``"custom_principal"``.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            AmortizationSpec::None => "none",
            AmortizationSpec::LinearTo { .. } => "linear_to",
            AmortizationSpec::StepRemaining { .. } => "step_remaining",
            AmortizationSpec::PercentOfOriginalPerPeriod { .. } => "percent_of_original_per_period",
            AmortizationSpec::CustomPrincipal { .. } => "custom_principal",
        }
    }

    /// Target notional for ``linear_to``, else ``None``.
    #[getter]
    fn final_notional(&self) -> Option<PyMoney> {
        match self.inner {
            AmortizationSpec::LinearTo { final_notional } => {
                Some(PyMoney::from_inner(final_notional))
            }
            _ => None,
        }
    }

    /// ``(date, remaining)`` pairs for ``step_remaining``, else ``None``.
    #[getter]
    fn schedule<'py>(&self, py: Python<'py>) -> PyResult<Option<DateMoneyRows<'py>>> {
        match &self.inner {
            AmortizationSpec::StepRemaining { schedule } => {
                date_money_to_py(py, schedule).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// Per-period percentage for ``percent_of_original_per_period``, else ``None``.
    #[getter]
    fn pct(&self) -> Option<f64> {
        match self.inner {
            AmortizationSpec::PercentOfOriginalPerPeriod { pct } => Some(pct),
            _ => None,
        }
    }

    /// ``(date, amount)`` pairs for ``custom_principal``, else ``None``.
    #[getter]
    fn items<'py>(&self, py: Python<'py>) -> PyResult<Option<DateMoneyRows<'py>>> {
        match &self.inner {
            AmortizationSpec::CustomPrincipal { items } => date_money_to_py(py, items).map(Some),
            _ => Ok(None),
        }
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("AmortizationSpec", &self.inner)
    }
}

/// Notional amount with an optional amortization rule.
///
/// Parameters
/// ----------
/// initial : Money
///     Initial principal amount outstanding at leg inception.
/// amort : AmortizationSpec, optional
///     Amortization rule applied after each period (default: none).
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import Notional
/// >>> Notional.par(1_000_000.0, "USD").initial.amount
/// 1000000.0
#[pyclass(
    name = "Notional",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyNotional {
    /// Inner notional with amortization rule.
    pub(crate) inner: Notional,
}

impl PyNotional {
    /// Build from an existing Rust [`Notional`].
    pub(crate) fn from_inner(inner: Notional) -> Self {
        Self { inner }
    }
}

wire_methods!(PyNotional, Notional, "Notional");

#[pymethods]
impl PyNotional {
    /// Construct a notional; see the class docstring for parameters.
    #[new]
    #[pyo3(signature = (initial, amort=None), text_signature = "(initial, amort=None)")]
    fn new(initial: PyMoney, amort: Option<PyRef<'_, PyAmortizationSpec>>) -> Self {
        Self {
            inner: Notional {
                initial: initial.inner,
                amort: amort.map_or(AmortizationSpec::None, |a| a.inner.clone()),
            },
        }
    }

    /// Plain (non-amortising) notional helper.
    ///
    /// Parameters
    /// ----------
    /// amount : float
    ///     Initial principal amount.
    /// currency : Currency or str
    ///     Currency of the notional, as a ``Currency`` instance or ISO code.
    ///
    /// Returns
    /// -------
    /// Notional
    ///     A bullet notional with no amortization.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If *currency* is not a valid ISO 4217 code.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.cashflows.builder import Notional
    /// >>> Notional.par(1_000_000.0, "USD").initial.amount
    /// 1000000.0
    #[staticmethod]
    #[pyo3(text_signature = "(amount, currency)")]
    fn par(amount: f64, currency: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: Notional::par(amount, extract_currency(currency)?)
                .map_err(crate::errors::core_to_py)?,
        })
    }

    /// Initial principal amount.
    #[getter]
    fn initial(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.initial)
    }

    /// Amortization rule.
    #[getter]
    fn amort(&self) -> PyAmortizationSpec {
        PyAmortizationSpec {
            inner: self.inner.amort.clone(),
        }
    }

    /// Currency of the notional.
    ///
    /// Returns
    /// -------
    /// Currency
    ///     The currency of the initial notional amount.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.cashflows.builder import Notional
    /// >>> Notional.par(1_000_000.0, "USD").currency().code
    /// 'USD'
    #[pyo3(text_signature = "(self)")]
    fn currency(&self, py: Python<'_>) -> PyResult<Py<PyCurrency>> {
        Py::new(py, PyCurrency::from_inner(self.inner.currency()))
    }

    /// Validate the notional and its amortization rule.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the amortization schedule is inconsistent with the initial
    ///     notional (e.g. a currency mismatch or a target above the initial
    ///     amount).
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        format!(
            "Notional(initial={} {}, amort={})",
            self.inner.initial.amount(),
            self.inner.initial.currency(),
            enum_repr("AmortizationSpec", &self.inner.amort)
        )
    }
}

/// Economic balance a periodic fee accrues on: ``FeeBase.DRAWN`` or
/// ``FeeBase.undrawn(facility_limit)``.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import FeeBase
/// >>> FeeBase.DRAWN.kind
/// 'drawn'
#[pyclass(
    name = "FeeBase",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFeeBase {
    /// Inner fee base.
    pub(crate) inner: FeeBase,
}

wire_methods!(PyFeeBase, FeeBase, "FeeBase");

#[pymethods]
impl PyFeeBase {
    /// Fee accrues on the drawn outstanding balance.
    #[allow(non_snake_case)]
    #[classattr]
    fn DRAWN() -> Self {
        Self {
            inner: FeeBase::Drawn,
        }
    }

    /// Fee accrues on undrawn = max(facility_limit - outstanding, 0).
    #[staticmethod]
    #[pyo3(text_signature = "(facility_limit)")]
    fn undrawn(facility_limit: PyMoney) -> Self {
        Self {
            inner: FeeBase::Undrawn {
                facility_limit: facility_limit.inner,
            },
        }
    }

    /// Variant label: ``"drawn"`` or ``"undrawn"``.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            FeeBase::Drawn => "drawn",
            FeeBase::Undrawn { .. } => "undrawn",
        }
    }

    /// Facility limit for ``undrawn`` bases, else ``None``.
    #[getter]
    fn facility_limit(&self) -> Option<PyMoney> {
        match self.inner {
            FeeBase::Undrawn { facility_limit } => Some(PyMoney::from_inner(facility_limit)),
            FeeBase::Drawn => None,
        }
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("FeeBase", &self.inner)
    }
}

/// Fee specification: a one-off ``FeeSpec.fixed(date, amount)`` or a periodic
/// ``FeeSpec.periodic_bp(...)`` accrued in basis points per annum.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.cashflows.builder import FeeSpec
/// >>> from finstack_quant.core.money import Money
/// >>> FeeSpec.fixed(datetime.date(2025, 1, 15), Money(-5_000.0, "USD")).kind
/// 'fixed'
#[pyclass(
    name = "FeeSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFeeSpec {
    /// Inner fee specification.
    pub(crate) inner: FeeSpec,
}

wire_methods!(PyFeeSpec, FeeSpec, "FeeSpec");

#[pymethods]
impl PyFeeSpec {
    /// Fixed fee paid once on ``date``. Negative amounts are rebates.
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date or str
    ///     Payment date.
    /// amount : Money
    ///     Fee amount (negative for rebates).
    #[staticmethod]
    #[pyo3(text_signature = "(date, amount)")]
    fn fixed(date: &Bound<'_, PyAny>, amount: PyMoney) -> PyResult<Self> {
        Ok(Self {
            inner: FeeSpec::Fixed {
                date: extract_date(date)?,
                amount: amount.inner,
            },
        })
    }

    /// Periodic fee quoted in basis points per annum, accrued over generated periods.
    ///
    /// Parameters
    /// ----------
    /// base : FeeBase
    ///     Balance the fee accrues on (drawn / undrawn).
    /// bp : Decimal, float or str
    ///     Fee quote in basis points per annum.
    /// frequency : Tenor or str
    ///     Accrual and payment frequency.
    /// day_count : DayCount
    ///     Day count used to annualize the accrual.
    /// calendar_id : str
    ///     Holiday calendar id (``"weekends_only"`` for weekend-only rolling).
    /// business_day_convention : BusinessDayConvention or str, optional
    ///     Rolling convention for fee dates (default Modified Following).
    /// stub : StubKind, optional
    ///     Stub rule (default short-front, the Rust wire default).
    /// accrual_basis : FeeAccrualBasis, optional
    ///     Balance sampling (default point-in-time).
    #[staticmethod]
    #[pyo3(
        signature = (base, bp, frequency, day_count, calendar_id, business_day_convention=None, stub=None, accrual_basis=None),
        text_signature = "(base, bp, frequency, day_count, calendar_id, business_day_convention=None, stub=None, accrual_basis=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn periodic_bp(
        base: PyRef<'_, PyFeeBase>,
        bp: &Bound<'_, PyAny>,
        frequency: &Bound<'_, PyAny>,
        day_count: PyRef<'_, PyDayCount>,
        calendar_id: &str,
        business_day_convention: Option<&Bound<'_, PyAny>>,
        stub: Option<PyRef<'_, PyStubKind>>,
        accrual_basis: Option<PyRef<'_, PyFeeAccrualBasis>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FeeSpec::PeriodicBp {
                base: base.inner.clone(),
                bp: decimal_from_any(bp)?,
                frequency: extract_tenor(frequency)?,
                day_count: day_count.inner,
                business_day_convention: match business_day_convention {
                    Some(obj) => extract_business_day_convention(obj)?,
                    None => serde_defaults::bdc_modified_following(),
                },
                calendar_id: calendar_id.to_string(),
                stub: stub.map_or_else(serde_defaults::stub_short_front, |s| s.inner),
                accrual_basis: accrual_basis
                    .map_or(FeeAccrualBasis::PointInTime, |a| a.inner.clone()),
            },
        })
    }

    /// Variant label: ``"fixed"`` or ``"periodic_bp"``.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            FeeSpec::Fixed { .. } => "fixed",
            FeeSpec::PeriodicBp { .. } => "periodic_bp",
        }
    }

    /// Payment date of a fixed fee, else ``None``.
    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner {
            FeeSpec::Fixed { date, .. } => date_to_py(py, date).map(Some),
            FeeSpec::PeriodicBp { .. } => Ok(None),
        }
    }

    /// Amount of a fixed fee, else ``None``.
    #[getter]
    fn amount(&self) -> Option<PyMoney> {
        match self.inner {
            FeeSpec::Fixed { amount, .. } => Some(PyMoney::from_inner(amount)),
            FeeSpec::PeriodicBp { .. } => None,
        }
    }

    /// Fee base of a periodic fee, else ``None``.
    #[getter]
    fn base(&self) -> Option<PyFeeBase> {
        match &self.inner {
            FeeSpec::PeriodicBp { base, .. } => Some(PyFeeBase {
                inner: base.clone(),
            }),
            FeeSpec::Fixed { .. } => None,
        }
    }

    /// Basis-point quote of a periodic fee, else ``None``.
    #[getter]
    fn bp<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner {
            FeeSpec::PeriodicBp { bp, .. } => decimal_to_py(py, bp).map(Some),
            FeeSpec::Fixed { .. } => Ok(None),
        }
    }

    /// Accrual frequency of a periodic fee, else ``None``.
    #[getter]
    fn frequency(&self) -> Option<PyTenor> {
        match self.inner {
            FeeSpec::PeriodicBp { frequency, .. } => Some(PyTenor::from_inner(frequency)),
            FeeSpec::Fixed { .. } => None,
        }
    }

    /// Day count of a periodic fee, else ``None``.
    #[getter]
    fn day_count(&self) -> Option<PyDayCount> {
        match self.inner {
            FeeSpec::PeriodicBp { day_count, .. } => Some(PyDayCount::from_inner(day_count)),
            FeeSpec::Fixed { .. } => None,
        }
    }

    /// Business-day convention of a periodic fee, else ``None``.
    #[getter]
    fn business_day_convention(&self) -> Option<PyBusinessDayConvention> {
        match self.inner {
            FeeSpec::PeriodicBp {
                business_day_convention,
                ..
            } => Some(PyBusinessDayConvention::from_inner(business_day_convention)),
            FeeSpec::Fixed { .. } => None,
        }
    }

    /// Calendar id of a periodic fee, else ``None``.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        match &self.inner {
            FeeSpec::PeriodicBp { calendar_id, .. } => Some(calendar_id.clone()),
            FeeSpec::Fixed { .. } => None,
        }
    }

    /// Stub rule of a periodic fee, else ``None``.
    #[getter]
    fn stub(&self) -> Option<PyStubKind> {
        match self.inner {
            FeeSpec::PeriodicBp { stub, .. } => Some(PyStubKind::from_inner(stub)),
            FeeSpec::Fixed { .. } => None,
        }
    }

    /// Balance-sampling basis of a periodic fee, else ``None``.
    #[getter]
    fn accrual_basis(&self) -> Option<PyFeeAccrualBasis> {
        match &self.inner {
            FeeSpec::PeriodicBp { accrual_basis, .. } => Some(PyFeeAccrualBasis {
                inner: accrual_basis.clone(),
            }),
            FeeSpec::Fixed { .. } => None,
        }
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("FeeSpec", &self.inner)
    }
}

/// Prepayment model: constant CPR, PSA curve, or CMBS lockout.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import PrepaymentModelSpec
/// >>> PrepaymentModelSpec.constant_cpr(0.06).cpr
/// 0.06
#[pyclass(
    name = "PrepaymentModelSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPrepaymentModelSpec {
    /// Inner prepayment model.
    pub(crate) inner: PrepaymentModelSpec,
}

wire_methods!(
    PyPrepaymentModelSpec,
    PrepaymentModelSpec,
    "PrepaymentModelSpec"
);

#[pymethods]
impl PyPrepaymentModelSpec {
    /// Constant CPR (no seasoning curve).
    #[staticmethod]
    #[pyo3(text_signature = "(cpr)")]
    fn constant_cpr(cpr: f64) -> Self {
        Self {
            inner: PrepaymentModelSpec::constant_cpr(cpr),
        }
    }

    /// PSA curve with speed multiplier (1.0 = 100% PSA).
    #[staticmethod]
    #[pyo3(text_signature = "(speed_multiplier)")]
    fn psa(speed_multiplier: f64) -> Self {
        Self {
            inner: PrepaymentModelSpec::psa(speed_multiplier),
        }
    }

    /// 100% PSA (standard prepayment assumption).
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn psa_100() -> Self {
        Self {
            inner: PrepaymentModelSpec::psa_100(),
        }
    }

    /// CMBS lockout: zero prepayment for ``lockout_months``, then constant CPR.
    #[staticmethod]
    #[pyo3(text_signature = "(lockout_months, post_lockout_cpr)")]
    fn cmbs_with_lockout(lockout_months: u32, post_lockout_cpr: f64) -> Self {
        Self {
            inner: PrepaymentModelSpec::cmbs_with_lockout(lockout_months, post_lockout_cpr),
        }
    }

    /// Annual constant prepayment rate (decimal).
    #[getter]
    fn cpr(&self) -> f64 {
        self.inner.cpr
    }

    /// Seasoning curve in its JSON wire form (``"constant"``, ``{"psa": ...}``,
    /// ``{"cmbs_lockout": ...}``), or ``None`` when no curve is set.
    #[getter]
    fn curve<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.curve)
    }

    /// Single-month mortality for the supplied seasoning.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the underlying curve parameters are invalid (e.g. a negative
    ///     PSA speed multiplier).
    #[pyo3(text_signature = "(self, seasoning_months)")]
    fn smm(&self, seasoning_months: u32) -> PyResult<f64> {
        self.inner.smm(seasoning_months).map_err(core_to_py)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("PrepaymentModelSpec", &self.inner)
    }
}

/// Default model: constant CDR or SDA curve.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import DefaultModelSpec
/// >>> DefaultModelSpec.cdr_2pct().cdr
/// 0.02
#[pyclass(
    name = "DefaultModelSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyDefaultModelSpec {
    /// Inner default model.
    pub(crate) inner: DefaultModelSpec,
}

wire_methods!(PyDefaultModelSpec, DefaultModelSpec, "DefaultModelSpec");

#[pymethods]
impl PyDefaultModelSpec {
    /// Constant CDR (no seasoning curve).
    #[staticmethod]
    #[pyo3(text_signature = "(cdr)")]
    fn constant_cdr(cdr: f64) -> Self {
        Self {
            inner: DefaultModelSpec::constant_cdr(cdr),
        }
    }

    /// SDA curve with speed multiplier (1.0 = 100% SDA).
    #[staticmethod]
    #[pyo3(text_signature = "(speed_multiplier)")]
    fn sda(speed_multiplier: f64) -> Self {
        Self {
            inner: DefaultModelSpec::sda(speed_multiplier),
        }
    }

    /// 2% CDR baseline.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn cdr_2pct() -> Self {
        Self {
            inner: DefaultModelSpec::cdr_2pct(),
        }
    }

    /// Annual constant default rate (decimal).
    #[getter]
    fn cdr(&self) -> f64 {
        self.inner.cdr
    }

    /// Seasoning curve in its JSON wire form, or ``None`` when no curve is set.
    #[getter]
    fn curve<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.curve)
    }

    /// Monthly default rate for the supplied seasoning.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the underlying curve parameters are invalid (e.g. a negative
    ///     SDA speed multiplier).
    #[pyo3(text_signature = "(self, seasoning_months)")]
    fn mdr(&self, seasoning_months: u32) -> PyResult<f64> {
        self.inner.mdr(seasoning_months).map_err(core_to_py)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("DefaultModelSpec", &self.inner)
    }
}

/// Recovery model with rate (fraction in ``[0, 1]``) and lag in months.
///
/// Parameters
/// ----------
/// rate : float
///     Recovery rate as a fraction of defaulted principal.
/// recovery_lag : int
///     Months between default and recovery receipt.
///
/// Examples
/// --------
/// >>> from finstack_quant.cashflows.builder import RecoveryModelSpec
/// >>> RecoveryModelSpec(0.4, 12).recovery_lag
/// 12
#[pyclass(
    name = "RecoveryModelSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyRecoveryModelSpec {
    /// Inner recovery model.
    pub(crate) inner: RecoveryModelSpec,
}

wire_methods!(PyRecoveryModelSpec, RecoveryModelSpec, "RecoveryModelSpec");

#[pymethods]
impl PyRecoveryModelSpec {
    /// Construct a recovery model; see the class docstring for parameters.
    #[new]
    #[pyo3(text_signature = "(rate, recovery_lag)")]
    fn new(rate: f64, recovery_lag: u32) -> Self {
        Self {
            inner: RecoveryModelSpec::with_lag(rate, recovery_lag),
        }
    }

    /// Recovery rate as a fraction.
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate
    }

    /// Recovery lag in months.
    #[getter]
    fn recovery_lag(&self) -> u32 {
        self.inner.recovery_lag
    }

    /// Validate that the rate is finite and in ``[0, 1]``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the rate is not finite or falls outside ``[0, 1]``.
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("RecoveryModelSpec", &self.inner)
    }
}

/// Add all spec classes to the builder module.
pub(crate) fn add_classes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRollRule>()?;
    module.add_class::<PyPrincipalExchange>()?;
    module.add_class::<PyCouponType>()?;
    module.add_class::<PyOvernightCompoundingMethod>()?;
    module.add_class::<PyOvernightIndexConstraintApplication>()?;
    module.add_class::<PyFloatingRateFallback>()?;
    module.add_class::<PyFeeAccrualBasis>()?;
    module.add_class::<PyScheduleParams>()?;
    module.add_class::<PyFixedWindow>()?;
    module.add_class::<PyFixedCouponSpec>()?;
    module.add_class::<PyFloatingRateSpec>()?;
    module.add_class::<PyFloatingCouponSpec>()?;
    module.add_class::<PyStepUpCouponSpec>()?;
    module.add_class::<PyAmortizationSpec>()?;
    module.add_class::<PyNotional>()?;
    module.add_class::<PyFeeBase>()?;
    module.add_class::<PyFeeSpec>()?;
    module.add_class::<PyPrepaymentModelSpec>()?;
    module.add_class::<PyDefaultModelSpec>()?;
    module.add_class::<PyRecoveryModelSpec>()?;
    Ok(())
}
