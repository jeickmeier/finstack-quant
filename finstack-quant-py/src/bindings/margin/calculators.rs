//! Python wrappers for margin calculators (VM + IM result types).

use super::types::{PyCsaSpec, PyImMethodology};
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::{TableBuilder, serde_rows_to_arrow_table};
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_margin as fm;
use pyo3::prelude::*;

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
    pub(super) inner: fm::calculators::VmResult,
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

    /// CSA threshold applied symmetrically to ``|gross_exposure|``.
    #[getter]
    fn threshold(&self) -> f64 {
        self.inner.threshold.amount()
    }

    /// CSA independent amount added to the threshold-adjusted exposure.
    #[getter]
    fn independent_amount(&self) -> f64 {
        self.inner.independent_amount.amount()
    }

    /// Signed collateral balance netted against ``net_exposure`` (positive =
    /// held by us, negative = posted by us).
    #[getter]
    fn collateral_balance(&self) -> f64 {
        self.inner.collateral_balance.amount()
    }

    /// Signed call before the MTA test and rounding:
    /// ``net_exposure - collateral_balance`` (positive = collect).
    #[getter]
    fn unrounded_call(&self) -> f64 {
        self.inner.unrounded_call.amount()
    }

    /// CSA minimum transfer amount; no transfer when
    /// ``abs(unrounded_call) < mta``.
    #[getter]
    fn mta(&self) -> f64 {
        self.inner.mta.amount()
    }

    /// CSA rounding increment applied to a transfer that passes the MTA test
    /// (up for a delivery, down for a return; zero disables rounding).
    #[getter]
    fn rounding_increment(&self) -> f64 {
        self.inner.rounding_increment.amount()
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
}

impl PyVmResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .utf8("date", vec![self.inner.date.to_string()])?
            .utf8(
                "settlement_date",
                vec![self.inner.settlement_date.to_string()],
            )?
            .f64("gross_exposure", vec![self.inner.gross_exposure.amount()])?
            .f64("threshold", vec![self.inner.threshold.amount()])?
            .f64(
                "independent_amount",
                vec![self.inner.independent_amount.amount()],
            )?
            .f64("net_exposure", vec![self.inner.net_exposure.amount()])?
            .f64(
                "collateral_balance",
                vec![self.inner.collateral_balance.amount()],
            )?
            .f64("unrounded_call", vec![self.inner.unrounded_call.amount()])?
            .f64("mta", vec![self.inner.mta.amount()])?
            .f64(
                "rounding_increment",
                vec![self.inner.rounding_increment.amount()],
            )?
            .f64("post_amount", vec![self.inner.post_amount.amount()])?
            .f64("collect_amount", vec![self.inner.collect_amount.amount()])?
            .f64("net_margin", vec![self.inner.net_margin().amount()])?
            .bool("requires_call", vec![self.inner.requires_call()])?
            .utf8(
                "currency",
                vec![self.inner.gross_exposure.currency().to_string()],
            )?
            .build()
    }
}

tabular_methods!(PyVmResult);

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
    inner: fm::calculators::VmCalculator,
}

/// Convert ``list[tuple[date-like, float]]``, ``pandas.Series``, or a 2-column DataFrame into dated
/// exposures in ``currency``.
fn extract_dated_exposures(
    obj: &Bound<'_, PyAny>,
    currency: Currency,
) -> PyResult<Vec<(finstack_quant_core::dates::Date, Money)>> {
    use crate::bindings::dataframe_utils::{
        extract_f64_vec, is_dataframe_like, is_polars_dataframe, table_column_names,
        table_column_to_pylist,
    };
    let items: Vec<(Bound<'_, PyAny>, f64)> = if obj.hasattr("items")? && obj.hasattr("index")? {
        obj.call_method0("items")?
            .try_iter()?
            .map(|item| item?.extract::<(Bound<'_, PyAny>, f64)>())
            .collect::<PyResult<_>>()?
    } else if is_dataframe_like(obj)? {
        let cols = table_column_names(obj)?;
        if cols.len() != 2 {
            return Err(crate::errors::value_error(format!(
                "exposures DataFrame must have 2 columns (date, exposure), got {}",
                cols.len()
            )));
        }
        let dates_py = table_column_to_pylist(obj, &cols[0])?.unwrap_or_default();
        let vals = if is_polars_dataframe(obj)? {
            extract_f64_vec(&obj.get_item(&cols[1])?, &cols[1])?
        } else {
            let vals_py = table_column_to_pylist(obj, &cols[1])?.unwrap_or_default();
            vals_py
                .into_iter()
                .map(|v| v.extract::<f64>())
                .collect::<PyResult<Vec<_>>>()?
        };
        dates_py.into_iter().zip(vals).collect()
    } else {
        obj.extract()?
    };
    items
        .iter()
        .map(|(date, amount)| {
            Ok((
                py_to_date(date)?,
                Money::new(*amount, currency).map_err(core_to_py)?,
            ))
        })
        .collect()
}

#[pymethods]
impl PyVmCalculator {
    /// Create a new VM calculator bound to one CSA specification.
    #[new]
    fn new(csa: &PyCsaSpec) -> Self {
        Self {
            inner: fm::calculators::VmCalculator::new(csa.inner.clone()),
        }
    }

    /// CSA specification this calculator applies.
    #[getter]
    fn csa(&self) -> PyCsaSpec {
        PyCsaSpec {
            inner: self.inner.get_csa().clone(),
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
        let ccy: Currency = currency.parse().map_err(crate::errors::core_to_py)?;
        let exp = Money::new(exposure, ccy).map_err(core_to_py)?;
        let posted = Money::new(posted_collateral, ccy).map_err(core_to_py)?;
        let as_of = py_to_date(as_of)?;
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
    /// Returns an :class:`~finstack_quant.core.table.ArrowTable` with columns
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
    fn generate_margin_calls(
        &self,
        exposures: &Bound<'_, PyAny>,
        initial_collateral: f64,
    ) -> PyResult<PyArrowTable> {
        const COLUMNS: &[(&str, &str)] = &[
            ("call_date", "str"),
            ("settlement_date", "str"),
            ("call_type", "str"),
            ("amount", "float64"),
            ("mtm_trigger", "float64"),
            ("threshold", "float64"),
            ("mta_applied", "float64"),
            ("currency", "str"),
        ];
        let currency = self.inner.get_csa().base_currency;
        let exposures = extract_dated_exposures(exposures, currency)?;
        let initial = Money::new(initial_collateral, currency).map_err(core_to_py)?;
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
        serde_rows_to_arrow_table(&rows, COLUMNS)
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
            .margin_call_dates(py_to_date(start)?, py_to_date(end)?)
            .map_err(core_to_py)?;
        crate::bindings::date_utils::dates_to_pylist(py, &dates)
    }

    fn __repr__(&self) -> String {
        format!(
            "VmCalculator(csa={:?}, currency={}, threshold={:.2}, mta={:.2}, frequency={})",
            self.inner.get_csa().id,
            self.inner.get_csa().base_currency,
            self.inner.get_csa().vm_threshold().amount(),
            self.inner.get_csa().vm_params.mta.amount(),
            self.inner.get_csa().vm_params.frequency,
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
    pub(super) inner: fm::calculators::ImResult,
}

impl PyImResult {
    pub(crate) fn from_inner(inner: fm::calculators::ImResult) -> Self {
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

    /// SIMM aggregation detail as a nested dict, or ``None`` for results not
    /// produced by the SIMM calculator.
    ///
    /// Keys: ``components`` (one per breakdown key, each with ``component``,
    /// ``risk_class``, ``historical_volatility_ratio``, ``margin`` and
    /// ``buckets``; a bucket has ``bucket``, ``k``, ``signed_sum`` and
    /// ``weighted_sensitivities`` rows of ``risk_factor``, ``tenor``,
    /// ``sensitivity``, ``risk_weight``, ``concentration_factor``,
    /// ``weighted_sensitivity``), ``risk_class_margins`` and ``mpor_scale``.
    /// Margins are before the MPOR scale.
    #[getter]
    fn simm_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.simm_detail)
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

    /// Export the per-component breakdown as an :class:`~finstack_quant.core.table.ArrowTable`.
    ///
    /// Columns: ``risk_class``, ``amount``, ``currency``. One row per
    /// component label (SIMM: ``IR_Delta``, ``IR_Vega``, ``FX_Delta``,
    /// ``Curvature``, ...; schedule: the asset class such as
    /// ``interest_rate``), sorted by ``risk_class`` so repeated runs are
    /// byte-identical. Methodologies that publish no breakdown yield a
    /// zero-row table that still carries all three columns.
    ///
    /// Breakdown components do not generally sum to ``amount``: SIMM and
    /// other methodologies aggregate risk classes with correlations.
    fn to_breakdown_table(&self) -> PyResult<PyArrowTable> {
        let risk_classes: Vec<String> = self.inner.breakdown.keys().cloned().collect();
        let amounts: Vec<f64> = self
            .inner
            .breakdown
            .values()
            .map(|money| money.amount())
            .collect();
        let currencies: Vec<String> = self
            .inner
            .breakdown
            .values()
            .map(|money| money.currency().to_string())
            .collect();

        TableBuilder::new()
            .utf8("risk_class", risk_classes)?
            .f64("amount", amounts)?
            .utf8("currency", currencies)?
            .build()
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
}

impl PyImResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("amount", vec![self.inner.amount.amount()])?
            .utf8("currency", vec![self.inner.amount.currency().to_string()])?
            .utf8("methodology", vec![self.inner.methodology.to_string()])?
            .i64("mpor_days", vec![i64::from(self.inner.mpor_days)])?
            .utf8("as_of", vec![self.inner.as_of.to_string()])?
            .bool("approximation", vec![self.inner.approximation])?
            .build()
    }
}

tabular_methods!(PyImResult);

/// Register calculator classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyVmResult>()?;
    m.add_class::<PyVmCalculator>()?;
    m.add_class::<PyImResult>()?;
    Ok(())
}
