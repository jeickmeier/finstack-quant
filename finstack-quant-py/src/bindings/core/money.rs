//! Python bindings for [`finstack_quant_core::money::Money`].

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use finstack_quant_core::config::RoundingMode;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::{FormatOpts, Money};
use finstack_quant_core::Error;
use pyo3::basic::CompareOp;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyFloat, PyInt, PyList, PyString, PyTuple, PyType};
use pyo3::IntoPyObjectExt;

use crate::bindings::core::config::{extract_rounding_mode, PyFinstackConfig};
use crate::bindings::core::currency::{extract_currency, PyCurrency};
use crate::errors::{core_to_py, display_to_py, value_error};

static DECIMAL_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();

fn decimal_type<'py>(py: Python<'py>) -> PyResult<&'py Bound<'py, PyType>> {
    DECIMAL_TYPE.import(py, "decimal", "Decimal")
}

/// A currency-tagged monetary amount.
///
/// Immutable, Decimal-backed value type combining a precision-preserving
/// amount with an ISO-4217 currency. Arithmetic is checked: addition,
/// subtraction, ordering and ``Money / Money`` require matching currencies
/// (``ValueError`` otherwise), and non-finite inputs are rejected.
/// ``amount_decimal`` exposes the stored amount losslessly; ``amount`` is its
/// ``float`` view.
///
/// Parameters
/// ----------
/// amount : decimal.Decimal | float | int | str
///     Finite monetary amount. ``Decimal`` and ``str`` (``"1234.56"``) are
///     parsed exactly; ``float``/``int`` go through IEEE 754.
/// currency : Currency | str
///     ISO-4217 currency (object or alphabetic code string).
/// config : FinstackConfig | None
///     When given, the amount is rounded on ingest using that config's
///     rounding mode and per-currency ingest scale.
///
/// Raises
/// ------
/// ValueError
///     If *amount* is not finite / not parsable or *currency* is invalid.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.money import Money
/// >>> Money("100.50", "USD").format()
/// 'USD 100.50'
#[pyclass(
    name = "Money",
    module = "finstack_quant.core.money",
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PyMoney {
    /// Inner currency-tagged amount.
    pub(crate) inner: Money,
}

impl PyMoney {
    /// Build a [`PyMoney`] from an existing [`Money`].
    pub(crate) const fn from_inner(inner: Money) -> Self {
        Self { inner }
    }
}

/// Parse `obj` as a [`PyMoney`] and return the wrapped [`Money`].
fn extract_money(obj: &Bound<'_, PyAny>) -> PyResult<Money> {
    obj.extract::<PyRef<'_, PyMoney>>()
        .map(|m| m.inner)
        .map_err(|_| PyTypeError::new_err("expected Money"))
}

/// Convert a Python ``decimal.Decimal`` into a `rust_decimal::Decimal` without
/// going through `f64`.
pub(crate) fn decimal_from_py(obj: &Bound<'_, PyAny>) -> PyResult<rust_decimal::Decimal> {
    if !is_python_decimal(obj)? {
        return Err(PyTypeError::new_err("expected decimal.Decimal"));
    }
    let s: String = obj.str()?.extract()?;
    parse_decimal_str(&s)
}

fn parse_decimal_str(s: &str) -> PyResult<rust_decimal::Decimal> {
    rust_decimal::Decimal::from_str(s.trim())
        .map_err(|e| value_error(format!("Invalid Decimal value {s:?}: {e}")))
}

/// Convert a Rust decimal into Python ``decimal.Decimal`` without using `f64`.
pub(crate) fn decimal_to_py<'py>(
    py: Python<'py>,
    value: rust_decimal::Decimal,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(decimal_type(py)?.call1((value.to_string(),))?.into_any())
}

/// Return true if `obj` is an instance of `decimal.Decimal` (or any subclass).
///
/// Uses the Python `isinstance` check rather than a string compare on
/// `type(obj).__name__`, so `MyDecimal(Decimal)` subclasses and any third-party
/// `Decimal`-named classes are distinguished correctly. The import resolves
/// through a cached `decimal.Decimal` type after the first call.
pub(crate) fn is_python_decimal(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = obj.py();
    obj.is_instance(decimal_type(py)?)
}

/// Build a [`Money`] from a Python amount that may be `float`, `int`,
/// `decimal.Decimal` or a decimal string. `Decimal`/`str` inputs preserve
/// full precision; numeric inputs follow IEEE 754 semantics and later
/// ``amount`` accessors expose an ``f64`` view.
pub(crate) fn money_from_amount(obj: &Bound<'_, PyAny>, ccy: Currency) -> PyResult<Money> {
    money_from_amount_with_config(obj, ccy, None)
}

fn money_from_amount_with_config(
    obj: &Bound<'_, PyAny>,
    ccy: Currency,
    cfg: Option<&PyFinstackConfig>,
) -> PyResult<Money> {
    const TYPE_MSG: &str = "Money amount must be float, int, str, or decimal.Decimal";
    let from_f64 = |amount: f64| match cfg {
        Some(cfg) => Money::new_with_config(amount, ccy, &cfg.inner),
        None => Money::new(amount, ccy),
    };
    let from_decimal = |amount| match cfg {
        Some(cfg) => Money::from_decimal_with_config(amount, ccy, &cfg.inner),
        None => Money::from_decimal(amount, ccy),
    };
    if obj.is_instance_of::<PyFloat>() || obj.is_instance_of::<PyInt>() {
        let amount: f64 = obj.extract().map_err(|_| PyTypeError::new_err(TYPE_MSG))?;
        return from_f64(amount).map_err(core_to_py);
    }
    if is_python_decimal(obj)? {
        let d = decimal_from_py(obj)?;
        return from_decimal(d).map_err(core_to_py);
    }
    if let Ok(text) = obj.cast::<PyString>() {
        let d = parse_decimal_str(text.to_str()?)?;
        return from_decimal(d).map_err(core_to_py);
    }
    let amount: f64 = obj.extract().map_err(|_| PyTypeError::new_err(TYPE_MSG))?;
    from_f64(amount).map_err(core_to_py)
}

fn currency_mismatch(lhs: Money, rhs: Money) -> PyErr {
    core_to_py(Error::CurrencyMismatch {
        expected: lhs.currency(),
        actual: rhs.currency(),
    })
}

#[pymethods]
impl PyMoney {
    /// Construct from a finite ``amount`` and a ``Currency`` or ISO code string.
    ///
    /// ``amount`` may be a ``float``, ``int``, ``decimal.Decimal`` or a decimal
    /// string such as ``"1234.56"``. ``Decimal``/``str`` inputs are parsed
    /// without going through ``f64``. When ``config`` is given the amount is
    /// rounded on ingest with that config's rounding mode and ingest scale.
    #[new]
    #[pyo3(signature = (amount, currency, config=None))]
    #[pyo3(text_signature = "(amount, currency, config=None)")]
    fn new(
        amount: &Bound<'_, PyAny>,
        currency: &Bound<'_, PyAny>,
        config: Option<PyRef<'_, PyFinstackConfig>>,
    ) -> PyResult<Self> {
        let ccy = extract_currency(currency)?;
        money_from_amount_with_config(amount, ccy, config.as_deref()).map(Self::from_inner)
    }

    /// Zero amount in the given currency.
    #[classmethod]
    #[pyo3(text_signature = "(cls, currency)")]
    fn zero(_cls: &Bound<'_, PyType>, currency: &Bound<'_, PyAny>) -> PyResult<Self> {
        let ccy = extract_currency(currency)?;
        Money::new(0.0, ccy)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Construct from a ``decimal.Decimal`` amount, preserving full precision.
    ///
    /// This requires an actual ``decimal.Decimal`` instance.
    #[classmethod]
    #[pyo3(text_signature = "(cls, amount, currency)")]
    fn from_decimal(
        _cls: &Bound<'_, PyType>,
        amount: &Bound<'_, PyAny>,
        currency: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let ccy = extract_currency(currency)?;
        let d = decimal_from_py(amount)?;
        Money::from_decimal(d, ccy)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Numeric amount as ``float`` (derived from the internal decimal representation).
    #[getter]
    fn amount(&self) -> f64 {
        self.inner.amount()
    }

    /// Lossless amount as a Python ``decimal.Decimal``.
    ///
    /// The internal Rust ``Decimal`` is rendered to a string and parsed by
    /// ``decimal.Decimal``, so no ``float`` round-trip occurs.
    #[getter]
    fn amount_decimal<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.amount_decimal())
    }

    /// ISO-4217 currency of this money amount.
    #[getter]
    fn currency(&self, py: Python<'_>) -> PyResult<Py<PyCurrency>> {
        Py::new(py, PyCurrency::from_inner(self.inner.currency()))
    }

    /// Format the amount.
    ///
    /// ``decimals`` defaults to the currency's ISO minor units; ``group`` is
    /// an optional thousands separator (``","``); ``rounding`` is a
    /// ``RoundingMode`` or its name (default bankers).
    #[pyo3(signature = (decimals=None, show_currency=true, group=None, rounding=None))]
    #[pyo3(text_signature = "(self, decimals=None, show_currency=True, group=None, rounding=None)")]
    fn format(
        &self,
        decimals: Option<usize>,
        show_currency: bool,
        group: Option<&str>,
        rounding: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<String> {
        let group = match group {
            None => None,
            Some(sep) => {
                let mut chars = sep.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Some(c),
                    _ => return Err(value_error("group must be a single character such as ','")),
                }
            }
        };
        let rounding = match rounding {
            Some(mode) => extract_rounding_mode(mode)?,
            None => RoundingMode::Bankers,
        };
        Ok(self.inner.format_with(FormatOpts {
            decimals,
            show_currency,
            group,
            rounding,
        }))
    }

    /// Return a debug-style representation.
    fn __repr__(&self) -> String {
        format!(
            "Money({}, '{}')",
            self.inner.amount_decimal(),
            self.inner.currency()
        )
    }

    /// Human-readable amount with currency (ISO minor units).
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    /// Hash combining the exact Decimal amount and currency.
    fn __hash__(&self) -> isize {
        let mut hasher = DefaultHasher::new();
        self.inner.amount_decimal().hash(&mut hasher);
        self.inner.currency().hash(&mut hasher);
        hasher.finish() as isize
    }

    /// Rich comparison; ordering requires matching currencies.
    fn __richcmp__(
        &self,
        other: Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(rhs) = other.extract::<PyRef<'_, PyMoney>>() else {
            return Ok(py.NotImplemented());
        };
        match op {
            CompareOp::Eq => Ok((self.inner == rhs.inner).into_py_any(py)?),
            CompareOp::Ne => Ok((self.inner != rhs.inner).into_py_any(py)?),
            CompareOp::Lt | CompareOp::Le | CompareOp::Gt | CompareOp::Ge => {
                if self.inner.currency() != rhs.inner.currency() {
                    return Err(currency_mismatch(self.inner, rhs.inner));
                }
                let ord = self.inner.amount_decimal().cmp(&rhs.inner.amount_decimal());
                Ok(op.matches(ord).into_py_any(py)?)
            }
        }
    }

    /// Serialize to JSON.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: Money = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// :meth:`to_json` / :meth:`from_json`, so an unpickled value is exactly
    /// what the wire format defines — no separate state format to drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// ``(amount, currency_code)`` tuple.
    #[allow(clippy::wrong_self_convention)]
    fn to_tuple(&self) -> (f64, String) {
        (self.inner.amount(), self.inner.currency().to_string())
    }

    /// Build from ``(amount, currency)``; ``amount`` may be ``float``, ``int``,
    /// ``Decimal`` or ``str`` and ``currency`` a ``Currency`` or ISO code.
    #[classmethod]
    #[pyo3(text_signature = "(cls, tup)")]
    fn from_tuple(_cls: &Bound<'_, PyType>, tup: &Bound<'_, PyTuple>) -> PyResult<Self> {
        if tup.len() != 2 {
            return Err(value_error("expected a (amount, currency) tuple"));
        }
        let ccy = extract_currency(&tup.get_item(1)?)?;
        money_from_amount(&tup.get_item(0)?, ccy).map(Self::from_inner)
    }

    /// Convert using an already-resolved positive FX rate.
    #[pyo3(text_signature = "(target, rate)")]
    fn convert_at_rate(&self, target: &Bound<'_, PyAny>, rate: f64) -> PyResult<Self> {
        let target = extract_currency(target)?;
        self.inner
            .convert_at_rate(target, rate)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Add two amounts (same currency); maps [`Money::checked_add`] errors to Python.
    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let rhs = extract_money(other)?;
        self.inner
            .checked_add(rhs)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Subtract two amounts (same currency).
    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let rhs = extract_money(other)?;
        self.inner
            .checked_sub(rhs)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Scale by a scalar ``float``.
    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let scalar: f64 = other.extract()?;
        self.inner
            .checked_mul_f64(scalar)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Divide by a scalar ``float`` (``-> Money``) or by a same-currency
    /// ``Money`` (``-> float`` ratio). Raises ``ValueError`` on zero divisor.
    fn __truediv__<'py>(
        &self,
        py: Python<'py>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if let Ok(rhs) = other.extract::<PyRef<'_, PyMoney>>() {
            let ratio = self.inner.checked_div(rhs.inner).map_err(core_to_py)?;
            return ratio.into_bound_py_any(py);
        }
        let scalar: f64 = other.extract()?;
        if scalar == 0.0 {
            return Err(value_error("division by zero"));
        }
        self.inner
            .checked_div_f64(scalar)
            .map(Self::from_inner)
            .map_err(core_to_py)?
            .into_bound_py_any(py)
    }

    /// Unary negation.
    fn __neg__(&self) -> Self {
        Self::from_inner(self.inner.checked_neg())
    }

    /// Absolute value (same currency).
    fn __abs__(&self) -> Self {
        Self::from_inner(self.inner.abs())
    }

    /// ``float(money)`` — the ``amount`` view.
    fn __float__(&self) -> f64 {
        self.inner.amount()
    }

    /// ``round(money, n)`` — bankers-round the amount to ``n`` decimal places
    /// (``n`` defaults to the currency's minor units).
    #[pyo3(signature = (ndigits=None))]
    fn __round__(&self, ndigits: Option<i32>) -> PyResult<Self> {
        self.inner
            .round(ndigits)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Right-add; supports ``Money + Money`` and ``0 + money``.
    fn __radd__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(rhs) = other.extract::<PyRef<'_, PyMoney>>() {
            return rhs
                .inner
                .checked_add(self.inner)
                .map(Self::from_inner)
                .map_err(core_to_py);
        }
        let scalar: f64 = other.extract()?;
        if scalar == 0.0 {
            Ok(*self)
        } else {
            Err(PyTypeError::new_err(
                "unsupported right operand for Money addition (expected Money or 0)",
            ))
        }
    }

    /// Right-subtract; supports ``0 - money`` for Python sum-style identities.
    fn __rsub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let scalar: f64 = other.extract()?;
        if scalar != 0.0 {
            return Err(PyTypeError::new_err(
                "unsupported right operand for Money subtraction (expected 0)",
            ));
        }
        Ok(Self::from_inner(self.inner.checked_neg()))
    }

    /// Right-multiply by a scalar.
    fn __rmul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let scalar: f64 = other.extract()?;
        self.inner
            .checked_mul_f64(scalar)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    // Note: `PyMoney` is `frozen`, so in-place ops are not provided.
    // Python's `+=`/`-=`/`*=`/`/=` will fall back to the non-in-place dunders
    // (`__add__`, etc.) and rebind the variable to a fresh `Money`.
}

/// Register the `finstack_quant.core.money` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "money")?;
    module.setattr(
        "__doc__",
        "Currency-tagged money bindings (finstack-quant-core).",
    )?;
    module.add_class::<PyMoney>()?;

    let all = PyList::new(py, ["Money"])?;
    module.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &module,
        "money",
        "finstack_quant.core",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
