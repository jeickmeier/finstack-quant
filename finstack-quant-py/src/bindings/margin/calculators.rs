//! Python wrappers for margin calculators (VM + IM result types).

use super::types::{PyCsaSpec, PyImMethodology};
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::pandas_utils::{
    dict_to_dataframe, serde_rows_to_dataframe_with_schema, ColumnSchema,
};
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_margin as fm;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Variation margin calculation result.
///
/// Sign convention: ``gross_exposure`` is the signed mark-to-market from our
/// side (positive = the counterparty owes us). ``post_amount`` is what we
/// post and ``collect_amount`` what we receive back; at most one is non-zero.
/// Amounts are floats in the CSA base currency.
#[pyclass(
    name = "VmResult",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyVmResult {
    pub(super) inner: fm::VmResult,
}

#[pymethods]
impl PyVmResult {
    /// Deserialize a VM result from canonical JSON; raises ``ValueError`` on
    /// malformed input.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize this result to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Calculation date as ``datetime.date``.
    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.date)
    }

    /// Settlement date of the margin transfer (calculation date plus the CSA
    /// settlement lag, adjusted on the CSA calendar) as ``datetime.date``.
    #[getter]
    fn settlement_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.settlement_date)
    }

    /// Gross mark-to-market exposure (positive = counterparty owes us).
    #[getter]
    fn gross_exposure(&self) -> f64 {
        self.inner.gross_exposure.amount()
    }

    /// Net exposure after threshold and independent amount.
    #[getter]
    fn net_exposure(&self) -> f64 {
        self.inner.net_exposure.amount()
    }

    /// Delivery amount (positive = we post margin).
    #[getter]
    fn post_amount(&self) -> f64 {
        self.inner.post_amount.amount()
    }

    /// Return amount (positive = we receive margin back).
    #[getter]
    fn collect_amount(&self) -> f64 {
        self.inner.collect_amount.amount()
    }

    /// Net desk cash outflow (post minus collect).
    #[getter]
    fn net_margin(&self) -> f64 {
        self.inner.net_margin().amount()
    }

    /// CSA base currency of every amount.
    #[getter]
    fn currency(&self) -> String {
        self.inner.gross_exposure.currency().to_string()
    }

    /// Whether a margin call is required.
    #[getter]
    fn requires_call(&self) -> bool {
        self.inner.requires_call()
    }

    /// Export the result as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``date``, ``settlement_date`` (ISO 8601 strings),
    /// ``gross_exposure``, ``net_exposure``, ``post_amount``,
    /// ``collect_amount``, ``net_margin``, ``requires_call``, ``currency``.
    ///
    /// All amount columns are floats in the single CSA currency reported by
    /// ``currency``; positive ``post_amount`` means we post margin and
    /// positive ``collect_amount`` means we receive margin back.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("date", vec![self.inner.date.to_string()])?;
        data.set_item(
            "settlement_date",
            vec![self.inner.settlement_date.to_string()],
        )?;
        data.set_item("gross_exposure", vec![self.inner.gross_exposure.amount()])?;
        data.set_item("net_exposure", vec![self.inner.net_exposure.amount()])?;
        data.set_item("post_amount", vec![self.inner.post_amount.amount()])?;
        data.set_item("collect_amount", vec![self.inner.collect_amount.amount()])?;
        data.set_item("net_margin", vec![self.inner.net_margin().amount()])?;
        data.set_item("requires_call", vec![self.inner.requires_call()])?;
        data.set_item(
            "currency",
            vec![self.inner.gross_exposure.currency().to_string()],
        )?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "VmResult(date={}, post={:.2}, collect={:.2}, requires_call={}, settlement_date={})",
            self.inner.date,
            self.inner.post_amount.amount(),
            self.inner.collect_amount.amount(),
            if self.inner.requires_call() {
                "True"
            } else {
                "False"
            },
            self.inner.settlement_date,
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Variation margin calculator following ISDA CSA rules.
///
/// Applies the CSA threshold, independent amount, minimum transfer amount
/// and rounding to a signed exposure, dates the settlement on the CSA
/// calendar, and can run a whole MTM series into a margin-call schedule
/// (``generate_margin_calls``) or list the contractual call dates
/// (``margin_call_dates``).
#[pyclass(
    name = "VmCalculator",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyVmCalculator {
    inner: fm::VmCalculator,
    csa: fm::CsaSpec,
}

/// Convert ``list[tuple[date-like, float]] | pandas.Series`` into dated
/// exposures in ``currency``.
fn extract_dated_exposures(
    obj: &Bound<'_, PyAny>,
    currency: Currency,
) -> PyResult<Vec<(time::Date, Money)>> {
    let items: Vec<(Bound<'_, PyAny>, f64)> = if obj.hasattr("items")? && obj.hasattr("index")? {
        obj.call_method0("items")?
            .try_iter()?
            .map(|item| item?.extract::<(Bound<'_, PyAny>, f64)>())
            .collect::<PyResult<_>>()?
    } else {
        obj.extract()?
    };
    items
        .iter()
        .map(|(date, amount)| Ok((extract_date(date)?, money_from_amount(*amount, currency)?)))
        .collect()
}

#[pymethods]
impl PyVmCalculator {
    /// Create a new VM calculator bound to one CSA specification.
    #[new]
    fn new(csa: &PyCsaSpec) -> Self {
        Self {
            inner: fm::VmCalculator::new(csa.inner.clone()),
            csa: csa.inner.clone(),
        }
    }

    /// CSA specification this calculator applies.
    #[getter]
    fn csa(&self) -> PyCsaSpec {
        PyCsaSpec {
            inner: self.csa.clone(),
        }
    }

    /// Calculate variation margin.
    ///
    /// Parameters
    /// ----------
    /// exposure : float
    ///     Signed mark-to-market in ``currency`` (positive = the counterparty
    ///     owes us, negative = we owe them).
    /// posted_collateral : float
    ///     Signed balance in ``currency``: positive held, negative posted, including pending agreed calls.
    /// currency : str
    ///     ISO-4217 code; must equal the CSA base currency.
    /// as_of : datetime.date | str
    ///     Calculation date (``date``, ``datetime``, ``pandas.Timestamp`` or
    ///     ISO ``YYYY-MM-DD``); the settlement date is derived from it.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the currency is unknown or differs from the CSA base currency,
    ///     an amount is non-finite, or the date string is not ISO 8601.
    /// TypeError
    ///     If ``as_of`` is neither a string nor date-like.
    #[pyo3(signature = (exposure, posted_collateral, currency, as_of))]
    fn calculate(
        &self,
        exposure: f64,
        posted_collateral: f64,
        currency: &str,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyVmResult> {
        let ccy: Currency = currency.parse().map_err(display_to_py)?;
        let exp = money_from_amount(exposure, ccy)?;
        let posted = money_from_amount(posted_collateral, ccy)?;
        let as_of = extract_date(as_of)?;
        let result = self
            .inner
            .calculate(exp, posted, as_of)
            .map_err(core_to_py)?;
        Ok(PyVmResult { inner: result })
    }

    /// Run an exposure time series into a margin-call schedule.
    ///
    /// Parameters
    /// ----------
    /// exposures : list[tuple[datetime.date | str, float]] | pandas.Series
    ///     Dated signed exposures in the CSA base currency (positive = the
    ///     counterparty owes us). A ``Series`` contributes its index as the
    ///     dates. Dates are processed in the order given.
    /// initial_collateral : float
    ///     Collateral posted before the first date, in the CSA base currency.
    ///
    /// Returns a pandas ``DataFrame`` with one row per call and columns
    /// ``call_date``, ``settlement_date`` (ISO 8601 strings), ``call_type``
    /// (``"variation_margin_post"`` or ``"variation_margin_collect"``),
    /// ``amount``, ``mtm_trigger``, ``threshold``, ``mta_applied`` (floats in
    /// ``currency``) and ``currency``. Dates without a call produce no row.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an amount is non-finite or a date string is not ISO 8601.
    #[pyo3(signature = (exposures, initial_collateral))]
    fn generate_margin_calls<'py>(
        &self,
        py: Python<'py>,
        exposures: &Bound<'py, PyAny>,
        initial_collateral: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        const COLUMNS: &[ColumnSchema<'_>] = &[
            ("call_date", "str"),
            ("settlement_date", "str"),
            ("call_type", "str"),
            ("amount", "float64"),
            ("mtm_trigger", "float64"),
            ("threshold", "float64"),
            ("mta_applied", "float64"),
            ("currency", "str"),
        ];
        let currency = self.csa.base_currency;
        let exposures = extract_dated_exposures(exposures, currency)?;
        let initial = money_from_amount(initial_collateral, currency)?;
        let calls = self
            .inner
            .generate_margin_calls(&exposures, initial)
            .map_err(core_to_py)?;
        let rows: Vec<serde_json::Value> = calls
            .iter()
            .map(|call| {
                serde_json::json!({
                    "call_date": call.call_date.to_string(),
                    "settlement_date": call.settlement_date.to_string(),
                    "call_type": call.call_type.to_string(),
                    "amount": call.amount.amount(),
                    "mtm_trigger": call.mtm_trigger.amount(),
                    "threshold": call.threshold.amount(),
                    "mta_applied": call.mta_applied.amount(),
                    "currency": call.amount.currency().to_string(),
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, COLUMNS)
    }

    /// Contractual margin-call dates between ``start`` and ``end``.
    ///
    /// Follows the CSA VM frequency on the CSA calendar: daily lists every
    /// business day, weekly/monthly roll from ``start`` with each date
    /// adjusted forward, on-demand returns just the adjusted endpoints.
    ///
    /// Parameters
    /// ----------
    /// start : datetime.date | str
    ///     First date of the window (inclusive).
    /// end : datetime.date | str
    ///     Last date of the window (inclusive).
    ///
    /// Returns a list of ``datetime.date``. Raises ``ValueError`` if a date
    /// string is not ISO 8601 or the CSA calendar is not registered.
    #[pyo3(signature = (start, end))]
    fn margin_call_dates<'py>(
        &self,
        py: Python<'py>,
        start: &Bound<'py, PyAny>,
        end: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let dates = self
            .inner
            .margin_call_dates(extract_date(start)?, extract_date(end)?)
            .map_err(core_to_py)?;
        crate::bindings::pandas_utils::dates_to_pylist(py, &dates)
    }

    fn __repr__(&self) -> String {
        format!(
            "VmCalculator(csa={:?}, currency={}, threshold={:.2}, mta={:.2}, frequency={})",
            self.csa.id,
            self.csa.base_currency,
            self.csa.vm_threshold().amount(),
            self.csa.vm_params.mta.amount(),
            self.csa.vm_params.frequency,
        )
    }
}

/// Initial margin calculation result.
///
/// ``amount`` is a float in ``currency``; ``breakdown_keys`` are
/// methodology-specific component labels — SIMM publishes ``IR_Delta``,
/// ``IR_Vega``, ``Credit_Qualifying_Delta``, ``Credit_Qualifying_Vega``,
/// ``Credit_NonQualifying_Delta``, ``Credit_NonQualifying_Vega``,
/// ``Equity_Delta``, ``Equity_Vega``, ``FX_Delta``, ``FX_Vega``,
/// ``Commodity_Delta``, ``Commodity_Vega`` and ``Curvature``; the schedule
/// calculator publishes the normalised asset class (e.g. ``interest_rate``).
#[pyclass(
    name = "ImResult",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyImResult {
    pub(super) inner: fm::ImResult,
}

impl PyImResult {
    pub(super) fn from_inner(inner: fm::ImResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyImResult {
    /// Deserialize an IM result from canonical JSON; raises ``ValueError``
    /// on malformed input.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize this result to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Calculated initial margin amount.
    #[getter]
    fn amount(&self) -> f64 {
        self.inner.amount.amount()
    }

    /// Currency of the IM amount.
    #[getter]
    fn currency(&self) -> String {
        self.inner.amount.currency().to_string()
    }

    /// Methodology used for calculation.
    #[getter]
    fn methodology(&self) -> PyImMethodology {
        PyImMethodology {
            inner: self.inner.methodology,
        }
    }

    /// Margin period of risk in business days.
    #[getter]
    fn mpor_days(&self) -> u32 {
        self.inner.mpor_days
    }

    /// Whether the amount is a conservative approximation (proxy) rather than
    /// an exact computation under the named methodology.
    #[getter]
    fn approximation(&self) -> bool {
        self.inner.approximation
    }

    /// Calculation date as ``datetime.date``.
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    /// Breakdown component labels present (see the class docstring for the
    /// SIMM and schedule label sets), in canonical sorted order.
    fn breakdown_keys(&self) -> Vec<String> {
        self.inner.breakdown.keys().cloned().collect()
    }

    /// Breakdown amount for one component label, or ``None`` if absent.
    fn breakdown_amount(&self, key: &str) -> Option<f64> {
        self.inner.breakdown.get(key).map(|m| m.amount())
    }

    /// Export the headline result as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``amount``, ``currency``, ``methodology``, ``mpor_days``,
    /// ``as_of``, ``approximation``.
    ///
    /// ``amount`` is a float in ``currency``; ``mpor_days`` is the margin
    /// period of risk in business days; ``as_of`` is an ISO 8601 date string.
    /// ``approximation`` is ``True`` when the amount is a conservative proxy
    /// rather than an exact computation under the named methodology — do not
    /// reconcile an approximated figure against an actual margin call.
    /// Per-component detail lives in ``to_breakdown_dataframe``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("amount", vec![self.inner.amount.amount()])?;
        data.set_item("currency", vec![self.inner.amount.currency().to_string()])?;
        data.set_item("methodology", vec![self.inner.methodology.to_string()])?;
        data.set_item("mpor_days", vec![self.inner.mpor_days])?;
        data.set_item("as_of", vec![self.inner.as_of.to_string()])?;
        data.set_item("approximation", vec![self.inner.approximation])?;
        dict_to_dataframe(py, &data, None)
    }

    /// Export the per-component breakdown as a pandas ``DataFrame``.
    ///
    /// Columns: ``risk_class``, ``amount``, ``currency``. One row per
    /// component label (SIMM: ``IR_Delta``, ``IR_Vega``, ``FX_Delta``,
    /// ``Curvature``, ...; schedule: the asset class such as
    /// ``interest_rate``), sorted by ``risk_class`` so repeated runs are
    /// byte-identical. Methodologies that publish no breakdown yield a
    /// zero-row frame that still carries all three columns.
    ///
    /// Breakdown components do not generally sum to ``amount``: SIMM and
    /// other methodologies aggregate risk classes with correlations.
    fn to_breakdown_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut entries: Vec<(&String, &Money)> = self.inner.breakdown.iter().collect();
        entries.sort_by_key(|(risk_class, _)| *risk_class);

        let risk_classes: Vec<String> = entries.iter().map(|(key, _)| (*key).clone()).collect();
        let amounts: Vec<f64> = entries.iter().map(|(_, money)| money.amount()).collect();
        let currencies: Vec<String> = entries
            .iter()
            .map(|(_, money)| money.currency().to_string())
            .collect();

        let data = PyDict::new(py);
        data.set_item("risk_class", risk_classes)?;
        data.set_item("amount", amounts)?;
        data.set_item("currency", currencies)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "ImResult(amount={:.2}, currency={}, methodology={}, mpor_days={}, as_of={}, approximation={})",
            self.inner.amount.amount(),
            self.inner.amount.currency(),
            self.inner.methodology,
            self.inner.mpor_days,
            self.inner.as_of,
            if self.inner.approximation {
                "True"
            } else {
                "False"
            }
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

pub(super) fn money_from_amount(amount: f64, currency: Currency) -> PyResult<Money> {
    if !amount.is_finite() {
        return Err(crate::errors::value_error(format!(
            "amount must be finite, got {amount}"
        )));
    }
    Money::new(amount, currency).map_err(core_to_py)
}

/// Register calculator classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyVmResult>()?;
    m.add_class::<PyVmCalculator>()?;
    m.add_class::<PyImResult>()?;
    Ok(())
}
