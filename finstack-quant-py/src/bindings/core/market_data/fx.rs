//! Python bindings for `finstack_quant_core::money::fx` FX matrix and conversion policy.

use std::sync::Arc;

use finstack_quant_core::money::fx::{
    CurrencyPair, FxConversionPolicy, FxMatrix, FxPairConvention, FxQuery, FxRateResult,
    invert_fx_rate as rust_invert_fx_rate,
};
use pyo3::prelude::*;
use pyo3::types::PyModule;
use pyo3::wrap_pyfunction;

use crate::bindings::core::currency::{PyCurrency, extract_currency};
use crate::bindings::date_utils::py_to_date;
use crate::bindings::macros::{
    impl_repr_html_via_dataframe, label_enum, typed_or_label, wire_methods,
};
use crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema;
use crate::errors::core_to_py;

typed_or_label!(
    fn extract_fx_policy -> FxConversionPolicy,
    PyFxConversionPolicy,
    "FxConversionPolicy or str",
    |label| label.parse().map_err(core_to_py)
);

/// FX conversion policy enum.
///
/// Wraps [`FxConversionPolicy`] from `finstack-quant-core`.
#[pyclass(
    name = "FxConversionPolicy",
    module = "finstack_quant.core.market_data",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyFxConversionPolicy {
    /// Inner Rust policy.
    pub(crate) inner: FxConversionPolicy,
}

label_enum!(PyFxConversionPolicy, FxConversionPolicy, "FxConversionPolicy", from_name, {
    /// Use spot/forward on the cashflow date.
    CASHFLOW_DATE => CashflowDate,
    /// Use period end date.
    PERIOD_END => PeriodEnd,
});

/// Result of an FX rate query.
///
/// Wraps [`FxRateResult`] from `finstack-quant-core`.
#[pyclass(
    name = "FxRateResult",
    module = "finstack_quant.core.market_data",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFxRateResult {
    /// Inner Rust result.
    pub(crate) inner: FxRateResult,
}

#[pymethods]
impl PyFxRateResult {
    /// The FX conversion rate.
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate
    }

    /// Whether the rate was obtained via triangulation.
    #[getter]
    fn triangulated(&self) -> bool {
        self.inner.triangulated
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``rate``, ``triangulated``.
    ///
    /// One lookup is one flat record, so a one-row frame is the right shape:
    /// ``pd.concat([matrix.rate(*pair, d).to_dataframe() for pair in pairs])``
    /// builds a fixing table, and the ``triangulated`` flag travels with each
    /// rate so a downstream check can refuse derived quotes.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "rate": self.inner.rate,
            "triangulated": self.inner.triangulated,
        });
        serde_object_to_single_row_dataframe_with_schema(py, &row, &["rate", "triangulated"])
    }

    /// The rate as a plain ``float`` (``float(result)``).
    fn __float__(&self) -> f64 {
        self.inner.rate
    }

    fn __repr__(&self) -> String {
        format!(
            "FxRateResult(rate={}, triangulated={})",
            self.inner.rate,
            if self.inner.triangulated {
                "True"
            } else {
                "False"
            },
        )
    }
}

wire_methods!(PyFxRateResult, FxRateResult, "FxRateResult");
impl_repr_html_via_dataframe!(PyFxRateResult);

/// Foreign-exchange rate matrix for currency conversion.
///
/// Wraps [`FxMatrix`] from `finstack-quant-core`. Quote mutations go through
/// `FxMatrix::set_quotes` (interior mutability) so that matrices obtained
/// from a `MarketContext` share state with the underlying context.
#[pyclass(
    name = "FxMatrix",
    module = "finstack_quant.core.market_data",
    skip_from_py_object
)]
pub struct PyFxMatrix {
    /// The matrix used for rate lookups (includes triangulation / caching).
    pub(crate) inner: Arc<FxMatrix>,
}

/// The explicit routing pivot: ``None`` disables routing, otherwise a
/// ``Currency`` or ISO code.
fn optional_pivot(
    value: &Bound<'_, PyAny>,
) -> PyResult<Option<finstack_quant_core::currency::Currency>> {
    if value.is_none() {
        Ok(None)
    } else {
        extract_currency(value).map(Some)
    }
}

/// Extract a numeric FX value as `f64`, rejecting Python `bool`.
fn extract_fx_float(value: &Bound<'_, PyAny>, field: &str) -> PyResult<f64> {
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "{field} must be a number, not bool"
        )));
    }
    value.extract::<f64>()
}

#[pymethods]
impl PyFxMatrix {
    /// Create an empty FX matrix with an explicit routing pivot (Rust
    /// ``FxMatrix::new``); ``None`` selects direct quotes only.
    #[new]
    #[pyo3(text_signature = "(pivot_currency)")]
    fn new(pivot_currency: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: Arc::new(FxMatrix::new(
                Arc::new(finstack_quant_core::money::fx::SimpleFxProvider::new()),
                optional_pivot(pivot_currency)?,
            )),
        })
    }

    /// Set an authoritative FX quote scoped to one date and policy.
    /// Pair-global quotes in either orientation take priority; this fixing
    /// takes priority over provider observations.
    #[pyo3(text_signature = "(self, base, quote, date, policy, rate)")]
    fn set_quote_on(
        &self,
        base: &Bound<'_, PyAny>,
        quote: &Bound<'_, PyAny>,
        date: &Bound<'_, PyAny>,
        policy: &Bound<'_, PyAny>,
        rate: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let base_currency = extract_currency(base)?;
        let quote_currency = extract_currency(quote)?;
        let date = py_to_date(date)?;
        let policy = extract_fx_policy(policy)?;
        let rate = extract_fx_float(rate, "rate")?;
        self.inner
            .set_quote_on(base_currency, quote_currency, date, policy, rate)
            .map_err(core_to_py)
    }

    /// Look up an FX rate.
    /// Pinned fixings precede global quotes, then provider observations;
    /// source priority is resolved before taking reciprocals.
    ///
    /// Parameters
    /// ----------
    /// base : Currency | str
    ///     Base (from) currency.
    /// quote : Currency | str
    ///     Quote (to) currency.
    /// date : datetime.date
    ///     Applicable date for the rate.
    /// policy : FxConversionPolicy | str
    ///     Required conversion policy (no default).
    ///
    /// Returns
    /// -------
    /// FxRateResult
    #[pyo3(text_signature = "(self, base, quote, date, policy)")]
    fn rate(
        &self,
        base: &Bound<'_, PyAny>,
        quote: &Bound<'_, PyAny>,
        date: &Bound<'_, PyAny>,
        policy: &Bound<'_, PyAny>,
    ) -> PyResult<PyFxRateResult> {
        let base_currency = extract_currency(base)?;
        let quote_currency = extract_currency(quote)?;
        let d = py_to_date(date)?;
        let query = FxQuery::new(base_currency, quote_currency, d, extract_fx_policy(policy)?);

        let result = self.inner.rate(query).map_err(core_to_py)?;
        Ok(PyFxRateResult { inner: result })
    }

    /// Set several explicit FX quotes atomically.
    ///
    /// Parameters
    /// ----------
    /// quotes : list[tuple[Currency | str, Currency | str, float]]
    ///     ``(base, quote, rate)`` triples with ``1 base = rate quote``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If any rate is non-finite or non-positive; no quote is applied in that case.
    #[pyo3(text_signature = "(self, quotes)")]
    fn set_quotes(
        &self,
        quotes: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>, Bound<'_, PyAny>)>,
    ) -> PyResult<()> {
        let quotes = quotes
            .iter()
            .map(|(base, quote, rate)| {
                Ok((
                    extract_currency(base)?,
                    extract_currency(quote)?,
                    extract_fx_float(rate, "rate")?,
                ))
            })
            .collect::<PyResult<Vec<_>>>()?;
        self.inner.set_quotes(&quotes).map_err(core_to_py)
    }

    /// Build a matrix from a ``{"EURUSD": 1.1, "GBP/USD": 1.25}`` mapping.
    ///
    /// Parameters
    /// ----------
    /// quotes : dict[str, float]
    ///     Keys are six ASCII-letter ISO pairs (``"EURUSD"``) or
    ///     slash-separated pairs (``"EUR/USD"``), parsed by the Rust
    ///     ``CurrencyPair``; values are ``1 base = rate quote``.
    ///
    /// Returns
    /// -------
    /// FxMatrix
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If a key is not ``str`` or a rate is not a number (or is ``bool``).
    /// ValueError
    ///     If a key is not a currency pair, a code is unknown, or a rate is invalid.
    #[staticmethod]
    #[pyo3(text_signature = "(quotes, pivot_currency)")]
    fn from_dict(
        quotes: &Bound<'_, pyo3::types::PyDict>,
        pivot_currency: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let mut parsed = Vec::with_capacity(quotes.len());
        for (key, value) in quotes.iter() {
            let pair: String = key.extract()?;
            let pair: CurrencyPair = pair.parse().map_err(core_to_py)?;
            parsed.push((pair.base, pair.quote, extract_fx_float(&value, "rate")?));
        }
        FxMatrix::from_quotes(&parsed, optional_pivot(pivot_currency)?)
            .map(|matrix| Self {
                inner: Arc::new(matrix),
            })
            .map_err(core_to_py)
    }

    /// Explicit pair-global quotes as a pandas ``DataFrame``.
    ///
    /// Columns: ``base``, ``quote`` (ISO codes) and ``rate``, in the Rust
    /// snapshot order (by ISO-4217 numeric code of base, then quote).
    /// Date-scoped quotes set with ``set_quote_on`` are not included.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If repeated quote updates prevent a coherent snapshot or the
    ///     provider cannot version its snapshot quotes.
    fn quotes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let state: Vec<(String, String, f64)> = self
            .inner
            .get_serializable_state()
            .map_err(core_to_py)?
            .quotes
            .into_iter()
            .map(|(base, quote, rate)| (base.to_string(), quote.to_string(), rate))
            .collect();
        let data = pyo3::types::PyDict::new(py);
        data.set_item(
            "base",
            state.iter().map(|q| q.0.as_str()).collect::<Vec<_>>(),
        )?;
        data.set_item(
            "quote",
            state.iter().map(|q| q.1.as_str()).collect::<Vec<_>>(),
        )?;
        data.set_item("rate", state.iter().map(|q| q.2).collect::<Vec<_>>())?;
        crate::bindings::pandas_utils::dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> PyResult<String> {
        let state = self.inner.get_serializable_state().map_err(core_to_py)?;
        Ok(format!(
            "FxMatrix(quotes={}, pinned_quotes={})",
            state.quotes.len(),
            state.pinned_quotes.len()
        ))
    }
}

/// Validated quote metadata for one ordered FX pair.
///
/// Wraps [`FxPairConvention`] from `finstack-quant-core`.
#[pyclass(
    name = "FxPairConvention",
    module = "finstack_quant.core.market_data",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug)]
pub struct PyFxPairConvention {
    /// Inner Rust convention.
    pub(crate) inner: FxPairConvention,
}

#[pymethods]
impl PyFxPairConvention {
    /// Quote metadata for the ordered pair `base`/`quote` with an explicit
    /// pip size.
    ///
    /// # Arguments
    ///
    /// * `base` - Base currency (`Currency` or ISO code).
    /// * `quote` - Quote currency (`Currency` or ISO code); must differ from
    ///   `base`.
    /// * `pip_size` - Outright-rate increment per pip; finite and positive.
    #[new]
    #[pyo3(text_signature = "(base, quote, pip_size)")]
    fn new(
        base: &Bound<'_, PyAny>,
        quote: &Bound<'_, PyAny>,
        pip_size: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let pip_size = extract_fx_float(pip_size, "pip_size")?;
        FxPairConvention::new(extract_currency(base)?, extract_currency(quote)?, pip_size)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Base currency of the ordered pair.
    #[getter]
    fn base(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.get_base())
    }

    /// Quote currency of the ordered pair.
    #[getter]
    fn quote(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.get_quote())
    }

    /// Outright-rate increment per pip.
    #[getter]
    fn pip_size(&self) -> f64 {
        self.inner.get_pip_size()
    }

    /// Rate points for a signed number of forward pips.
    ///
    /// # Arguments
    ///
    /// * `pips` - Signed forward points in pips; must be finite.
    #[pyo3(text_signature = "($self, pips)")]
    fn rate_points(&self, pips: &Bound<'_, PyAny>) -> PyResult<f64> {
        let pips = extract_fx_float(pips, "pips")?;
        self.inner.rate_points(pips).map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "FxPairConvention(base={}, quote={}, pip_size={})",
            self.inner.get_base(),
            self.inner.get_quote(),
            self.inner.get_pip_size(),
        )
    }
}

/// Reciprocal of a strictly positive finite FX rate.
///
/// # Arguments
///
/// * `rate` - Outright FX rate to invert, in quote-per-base units. Must be
///   finite and strictly positive; the reciprocal must also be a valid FX rate.
#[pyfunction]
#[pyo3(text_signature = "(rate)")]
fn invert_fx_rate(rate: &Bound<'_, PyAny>) -> PyResult<f64> {
    let rate = extract_fx_float(rate, "rate")?;
    rust_invert_fx_rate(rate).map_err(core_to_py)
}

pub(super) const EXPORTS: &[&str] = &[
    "FxConversionPolicy",
    "FxMatrix",
    "FxPairConvention",
    "FxRateResult",
    "invert_fx_rate",
];

/// Register FX classes and convention helpers on `finstack_quant.core.market_data`.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFxConversionPolicy>()?;
    m.add_class::<PyFxRateResult>()?;
    m.add_class::<PyFxMatrix>()?;
    m.add_class::<PyFxPairConvention>()?;
    m.add_function(wrap_pyfunction!(invert_fx_rate, m)?)?;
    Ok(())
}
