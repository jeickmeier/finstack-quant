use pyo3::prelude::*;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::valuations::convert::{
    bps_from_py, enum_to_py_string, money_from_py, money_to_py, opt_repr, rate_decimal_from_py,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::RepLine;

/// Typed wrapper for the Rust `RepLine` (aggregated representative pool line).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "RepLine",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRepLine {
    /// Inner canonical Rust rep line.
    pub(crate) inner: RepLine,
}

#[pymethods]
impl PyRepLine {
    /// Aggregated representative line for pool modeling.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique identifier for the rep line.
    /// balance : Money
    ///     Aggregated balance.
    /// rate : float | Rate
    ///     Weighted average coupon as an annual decimal rate (e.g. ``0.07``
    ///     = 7%).
    /// maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Weighted average maturity date (date-like or ISO-8601 string).
    /// seasoning_months : int
    ///     Weighted average seasoning in months.
    /// day_count : DayCount
    ///     Day count convention.
    /// asset_type : dict
    ///     Canonical Rust AssetType JSON object specifying amortization behavior.
    /// spread_bp : float | Bps, optional
    ///     Weighted average spread over the reference index, in basis
    ///     points (e.g. ``150.0`` = 150bp), for floating-rate lines.
    /// index_id : str, optional
    ///     Reference index identifier, if floating.
    /// index_floor : float, optional
    ///     Floor on the floating index as an annual decimal, applied before
    ///     ``spread_bp``; ignored on fixed-rate lines.
    /// cpr : float, optional
    ///     Constant prepayment rate override, as an annual decimal (e.g.
    ///     ``0.10`` = 10% CPR).
    /// cdr : float, optional
    ///     Constant default rate override, as an annual decimal (e.g.
    ///     ``0.02`` = 2% CDR).
    /// recovery_rate : float, optional
    ///     Recovery rate override, as a decimal fraction (e.g. ``0.45`` =
    ///     45%).
    /// contractual_payment : Money, optional
    ///     Contractual periodic payment for level-pay lines; inferred from
    ///     the balance and remaining schedule when ``None``.
    /// amortization_term_months : int, optional
    ///     Schedule length in months from origination; with
    ///     ``seasoning_months`` the line amortizes over ``term − seasoning``.
    /// io_months : int, optional
    ///     Interest-only window in months from origination.
    ///
    /// Returns
    /// -------
    /// RepLine
    ///     The rep line.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``maturity`` is neither date-like nor a string, or ``rate`` /
    ///     ``spread_bp`` are not numbers / ``Rate`` / ``Bps``.
    /// ValueError
    ///     If a string ``maturity`` is not valid ISO-8601.
    ///
    /// Examples
    /// --------
    /// >>> import datetime
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.core.dates import DayCount
    /// >>> from finstack_quant.core.money import Money
    /// >>> from finstack_quant.valuations.instruments import RepLine
    /// >>> line = RepLine(
    /// ...     "LINE-1", Money(80_000_000.0, Currency("USD")), 0.07,
    /// ...     datetime.date(2031, 1, 15), 12, DayCount.ACT_360, asset_type={"type": "first_lien_loan", "industry": None},
    /// ...     cpr=0.10, cdr=0.02, recovery_rate=0.45,
    /// ... )
    /// >>> "LINE-1" in repr(line)
    /// True
    #[new]
    #[pyo3(signature = (id, balance, rate, maturity, seasoning_months, day_count, *, asset_type,
                        spread_bp = None, index_id = None, index_floor = None, cpr = None,
                        cdr = None, recovery_rate = None, contractual_payment = None,
                        amortization_term_months = None, io_months = None))]
    #[pyo3(
        text_signature = "(id, balance, rate, maturity, seasoning_months, day_count, *, asset_type, \
spread_bp=None, index_id=None, index_floor=None, cpr=None, cdr=None, recovery_rate=None, contractual_payment=None, amortization_term_months=None, io_months=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        id: &str,
        balance: &Bound<'_, PyAny>,
        rate: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        seasoning_months: u32,
        day_count: PyRef<'_, PyDayCount>,
        asset_type: &Bound<'_, PyAny>,
        spread_bp: Option<&Bound<'_, PyAny>>,
        index_id: Option<String>,
        index_floor: Option<f64>,
        cpr: Option<f64>,
        cdr: Option<f64>,
        recovery_rate: Option<f64>,
        contractual_payment: Option<PyRef<'_, PyMoney>>,
        amortization_term_months: Option<u32>,
        io_months: Option<u32>,
    ) -> PyResult<Self> {
        let maturity = extract_date(maturity)?;
        let balance = money_from_py(balance, None, "balance")?;
        let rate = rate_decimal_from_py(rate, "rate")?;
        let spread_bp = spread_bp
            .map(|value| bps_from_py(value, "spread_bp"))
            .transpose()?;
        let mut inner = RepLine::new(
            id,
            balance,
            rate,
            maturity,
            day_count.inner,
            crate::bindings::module_utils::py_to_serde(py, asset_type, "asset_type")?,
        );
        inner.spread_bp = spread_bp;
        inner.index_id = index_id;
        inner.index_floor = index_floor;
        inner.seasoning_months = seasoning_months;
        if let Some(cpr) = cpr {
            inner = inner.with_cpr(cpr);
        }
        if let Some(cdr) = cdr {
            inner = inner.with_cdr(cdr);
        }
        if let Some(recovery_rate) = recovery_rate {
            inner = inner.with_recovery_rate(recovery_rate);
        }
        inner.contractual_payment = contractual_payment.map(|money| money.inner);
        inner.amortization_term_months = amortization_term_months;
        inner.io_months = io_months;
        Ok(Self { inner })
    }

    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Strict JSON object with exactly the fields ``to_json`` writes.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or has the wrong shape.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| crate::errors::serde_json_to_py(err, "invalid RepLine JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to the canonical JSON wire form.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(crate::errors::display_to_py)
    }

    /// Return every field as a plain ``dict`` (canonical serde shape).
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    /// Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Rep line identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Canonical asset classification, including its amortization behavior.
    #[getter]
    fn asset_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.asset_type)
    }

    /// Aggregated balance.
    #[getter]
    fn balance(&self) -> PyMoney {
        money_to_py(self.inner.balance)
    }

    /// Weighted average coupon as an annual decimal rate.
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate
    }

    /// Weighted average spread in basis points, or ``None`` for fixed lines.
    #[getter]
    fn spread_bp(&self) -> Option<f64> {
        self.inner.spread_bp
    }

    /// Reference index identifier, or ``None``.
    #[getter]
    fn index_id(&self) -> Option<String> {
        self.inner.index_id.clone()
    }

    /// Floor on the floating index (annual decimal) applied before the
    /// spread, or ``None``.
    #[getter]
    fn index_floor(&self) -> Option<f64> {
        self.inner.index_floor
    }

    /// Contractual periodic payment, or ``None`` when inferred.
    #[getter]
    fn contractual_payment(&self) -> Option<PyMoney> {
        self.inner.contractual_payment.map(money_to_py)
    }

    /// Amortization schedule length in months from origination, or ``None``.
    #[getter]
    fn amortization_term_months(&self) -> Option<u32> {
        self.inner.amortization_term_months
    }

    /// Interest-only window in months from origination, or ``None``.
    #[getter]
    fn io_months(&self) -> Option<u32> {
        self.inner.io_months
    }

    /// Weighted average maturity as ``datetime.date``.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Weighted average seasoning in months.
    #[getter]
    fn seasoning_months(&self) -> u32 {
        self.inner.seasoning_months
    }

    /// Day count convention (serde name).
    #[getter]
    fn day_count(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.day_count)
    }

    /// CPR override as an annual decimal, or ``None``.
    #[getter]
    fn cpr(&self) -> Option<f64> {
        self.inner.cpr
    }

    /// CDR override as an annual decimal, or ``None``.
    #[getter]
    fn cdr(&self) -> Option<f64> {
        self.inner.cdr
    }

    /// Recovery-rate override as a decimal fraction, or ``None``.
    #[getter]
    fn recovery_rate(&self) -> Option<f64> {
        self.inner.recovery_rate
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RepLine(id='{}', balance={}, currency='{}', rate={}, spread_bp={}, maturity='{}')",
            self.inner.id,
            self.inner.balance.amount(),
            self.inner.balance.currency(),
            self.inner.rate,
            opt_repr(self.inner.spread_bp),
            self.inner.maturity,
        )
    }
}
