//! Python bindings for `finstack_quant_core::money::fx` FX matrix and conversion policy.

use std::sync::Arc;

use finstack_quant_core::money::fx::{
    fx_market_pair as rust_fx_market_pair, fx_pair_convention as rust_fx_pair_convention,
    fx_pip_size as rust_fx_pip_size, invert_fx_rate as rust_invert_fx_rate, FxConversionPolicy,
    FxMatrix, FxPairConvention, FxQuery, FxQuoteConvention, FxRateResult, SimpleFxProvider,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use pyo3::wrap_pyfunction;

use crate::bindings::core::currency::{extract_currency, PyCurrency};
use crate::bindings::date_utils::py_to_date;
use crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema;
use crate::errors::{core_to_py, display_to_py};

/// Parse an [`FxConversionPolicy`] from a string.
fn parse_fx_policy(s: &str) -> PyResult<FxConversionPolicy> {
    s.parse::<FxConversionPolicy>()
        .map_err(|e| crate::errors::value_error(format!("Invalid FxConversionPolicy {s:?}: {e}")))
}

fn extract_fx_policy(value: &Bound<'_, PyAny>) -> PyResult<FxConversionPolicy> {
    if let Ok(wrapper) = value.extract::<PyRef<'_, PyFxConversionPolicy>>() {
        Ok(wrapper.inner)
    } else if let Ok(s) = value.extract::<String>() {
        parse_fx_policy(&s)
    } else {
        Err(crate::errors::value_error(
            "policy must be FxConversionPolicy or str",
        ))
    }
}

/// FX conversion policy enum.
///
/// Wraps [`FxConversionPolicy`] from `finstack-quant-core`.
#[pyclass(
    name = "FxConversionPolicy",
    module = "finstack_quant.core.market_data.fx",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug)]
pub struct PyFxConversionPolicy {
    /// Inner Rust policy.
    pub(crate) inner: FxConversionPolicy,
}

impl PyFxConversionPolicy {
    pub(crate) fn from_inner(inner: FxConversionPolicy) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFxConversionPolicy {
    /// Use spot/forward on the cashflow date.
    #[classattr]
    const CASHFLOW_DATE: PyFxConversionPolicy = PyFxConversionPolicy {
        inner: FxConversionPolicy::CashflowDate,
    };
    /// Use period end date.
    #[classattr]
    const PERIOD_END: PyFxConversionPolicy = PyFxConversionPolicy {
        inner: FxConversionPolicy::PeriodEnd,
    };
    /// Use an average over the period.
    #[classattr]
    const PERIOD_AVERAGE: PyFxConversionPolicy = PyFxConversionPolicy {
        inner: FxConversionPolicy::PeriodAverage,
    };

    /// Parse from a string label (e.g. ``"cashflow_date"``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, pyo3::types::PyType>, name: &str) -> PyResult<Self> {
        parse_fx_policy(name).map(Self::from_inner)
    }

    fn __repr__(&self) -> String {
        format!("FxConversionPolicy({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Result of an FX rate query.
///
/// Wraps [`FxRateResult`] from `finstack-quant-core`.
#[pyclass(
    name = "FxRateResult",
    module = "finstack_quant.core.market_data.fx",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFxRateResult {
    /// Inner Rust result.
    inner: FxRateResult,
}

#[pymethods]
impl PyFxRateResult {
    /// Deserialize an FX lookup result from canonical JSON.
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

/// Foreign-exchange rate matrix for currency conversion.
///
/// Wraps [`FxMatrix`] from `finstack-quant-core`. Quote mutations go through
/// `FxMatrix::set_quote` (interior mutability) so that matrices obtained
/// from a `MarketContext` share state with the underlying context.
#[pyclass(
    name = "FxMatrix",
    module = "finstack_quant.core.market_data.fx",
    skip_from_py_object
)]
pub struct PyFxMatrix {
    /// The matrix used for rate lookups (includes triangulation / caching).
    pub(crate) inner: Arc<FxMatrix>,
}

#[pymethods]
impl PyFxMatrix {
    /// Create an empty FX matrix.
    #[new]
    fn new() -> Self {
        let provider = Arc::new(SimpleFxProvider::new());
        let matrix = FxMatrix::new(provider);
        Self {
            inner: Arc::new(matrix),
        }
    }

    /// Set an explicit FX quote.
    ///
    /// Parameters
    /// ----------
    /// base : Currency | str
    ///     Base (from) currency.
    /// quote : Currency | str
    ///     Quote (to) currency.
    /// rate : float
    ///     The conversion rate ``1 base = rate quote``.
    #[pyo3(text_signature = "(self, base, quote, rate)")]
    fn set_quote(
        &self,
        base: &Bound<'_, PyAny>,
        quote: &Bound<'_, PyAny>,
        rate: f64,
    ) -> PyResult<()> {
        let base_currency = extract_currency(base)?;
        let quote_currency = extract_currency(quote)?;
        self.inner
            .set_quote(base_currency, quote_currency, rate)
            .map_err(core_to_py)?;
        Ok(())
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
        rate: f64,
    ) -> PyResult<()> {
        let base_currency = extract_currency(base)?;
        let quote_currency = extract_currency(quote)?;
        let date = py_to_date(date)?;
        let policy = extract_fx_policy(policy)?;
        self.inner
            .set_quote_on(base_currency, quote_currency, date, policy, rate)
            .map_err(core_to_py)
    }

    /// Look up an FX rate.
    /// Global quotes precede pinned fixings, then provider observations;
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
    /// policy : str, optional
    ///     Conversion policy (default ``"cashflow_date"``).
    ///
    /// Returns
    /// -------
    /// FxRateResult
    #[pyo3(signature = (base, quote, date, policy=None))]
    #[pyo3(text_signature = "(self, base, quote, date, policy=None)")]
    fn rate(
        &self,
        base: &Bound<'_, PyAny>,
        quote: &Bound<'_, PyAny>,
        date: &Bound<'_, PyAny>,
        policy: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyFxRateResult> {
        let base_currency = extract_currency(base)?;
        let quote_currency = extract_currency(quote)?;
        let d = py_to_date(date)?;
        let pol = match policy {
            Some(p) => extract_fx_policy(p)?,
            None => FxConversionPolicy::CashflowDate,
        };

        let query = FxQuery::with_policy(base_currency, quote_currency, d, pol);

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
    fn set_quotes(&self, quotes: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>, f64)>) -> PyResult<()> {
        let quotes = quotes
            .iter()
            .map(|(base, quote, rate)| {
                Ok((extract_currency(base)?, extract_currency(quote)?, *rate))
            })
            .collect::<PyResult<Vec<_>>>()?;
        self.inner.set_quotes(&quotes).map_err(core_to_py)
    }

    /// Build a matrix from a ``{"EURUSD": 1.1, "GBP/USD": 1.25}`` mapping.
    ///
    /// Parameters
    /// ----------
    /// quotes : dict[str, float]
    ///     Keys are six-letter ISO pairs (``"EURUSD"``) or slash-separated
    ///     pairs (``"EUR/USD"``); values are ``1 base = rate quote``.
    ///
    /// Returns
    /// -------
    /// FxMatrix
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a key is not a currency pair, a code is unknown, or a rate is invalid.
    #[staticmethod]
    #[pyo3(text_signature = "(quotes)")]
    fn from_dict(quotes: &Bound<'_, pyo3::types::PyDict>) -> PyResult<Self> {
        let matrix = Self::new();
        let mut parsed = Vec::with_capacity(quotes.len());
        for (key, value) in quotes.iter() {
            let pair: String = key.extract()?;
            let (base, quote) = match pair.split_once('/') {
                Some((b, q)) => (b.to_string(), q.to_string()),
                None if pair.len() == 6 => (pair[..3].to_string(), pair[3..].to_string()),
                None => {
                    return Err(crate::errors::value_error(format!(
                        "invalid FX pair {pair:?}: expected \"EURUSD\" or \"EUR/USD\""
                    )))
                }
            };
            parsed.push((
                crate::bindings::module_utils::parse_currency(&base)?,
                crate::bindings::module_utils::parse_currency(&quote)?,
                value.extract::<f64>()?,
            ));
        }
        matrix.inner.set_quotes(&parsed).map_err(core_to_py)?;
        Ok(matrix)
    }

    /// Explicit pair-global quotes as a pandas ``DataFrame``.
    ///
    /// Columns: ``base``, ``quote`` (ISO codes) and ``rate``. Date-scoped
    /// quotes set with ``set_quote_on`` are not included.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    fn quotes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut state: Vec<(String, String, f64)> = self
            .inner
            .get_serializable_state()
            .quotes
            .into_iter()
            .map(|(base, quote, rate)| (base.to_string(), quote.to_string(), rate))
            .collect();
        state.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
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

    fn __repr__(&self) -> String {
        let state = self.inner.get_serializable_state();
        format!(
            "FxMatrix(quotes={}, pinned_quotes={})",
            state.quotes.len(),
            state.pinned_quotes.len()
        )
    }
}

/// Parse an [`FxQuoteConvention`] from a string.
fn parse_fx_quote_convention(s: &str) -> PyResult<FxQuoteConvention> {
    s.parse::<FxQuoteConvention>()
        .map_err(|e| crate::errors::value_error(format!("Invalid FxQuoteConvention {s:?}: {e}")))
}

/// USD quotation style for a market FX pair (Direct or Indirect versus USD).
///
/// Wraps [`FxQuoteConvention`] from `finstack-quant-core`.
#[pyclass(
    name = "FxQuoteConvention",
    module = "finstack_quant.core.market_data.fx",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyFxQuoteConvention {
    /// Inner Rust convention.
    pub(crate) inner: FxQuoteConvention,
}

impl PyFxQuoteConvention {
    pub(crate) fn from_inner(inner: FxQuoteConvention) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFxQuoteConvention {
    /// USD is the quote currency (EURUSD, GBPUSD).
    #[classattr]
    const DIRECT: PyFxQuoteConvention = PyFxQuoteConvention {
        inner: FxQuoteConvention::Direct,
    };
    /// USD is the base currency (USDJPY, USDCAD).
    #[classattr]
    const INDIRECT: PyFxQuoteConvention = PyFxQuoteConvention {
        inner: FxQuoteConvention::Indirect,
    };

    /// Parse from a string label (``"direct"`` or ``"indirect"``).
    ///
    /// # Arguments
    ///
    /// * `name` - Convention label: ``"direct"`` or ``"indirect"``.
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, pyo3::types::PyType>, name: &str) -> PyResult<Self> {
        parse_fx_quote_convention(name).map(Self::from_inner)
    }

    fn __repr__(&self) -> String {
        format!("FxQuoteConvention({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Market convention for one FX pair after Bloomberg/Reuters CCY1 ordering.
///
/// Wraps [`FxPairConvention`] from `finstack-quant-core`. Instances come from
/// [`fx_pair_convention`].
#[pyclass(
    name = "FxPairConvention",
    module = "finstack_quant.core.market_data.fx",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug)]
pub struct PyFxPairConvention {
    /// Inner Rust convention.
    inner: FxPairConvention,
}

impl PyFxPairConvention {
    fn from_inner(inner: FxPairConvention) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFxPairConvention {
    /// Market CCY1 (one unit of this currency).
    #[getter]
    fn base(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.base)
    }

    /// Market CCY2 (units of this currency per one unit of CCY1).
    #[getter]
    fn quote(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.quote)
    }

    /// Direct if the USD leg quotes USD as CCY2; Indirect if USD is CCY1.
    #[getter]
    fn usd_quotation(&self) -> PyFxQuoteConvention {
        PyFxQuoteConvention::from_inner(self.inner.usd_quotation)
    }

    /// Pip size in outright-rate units (`0.01` or `0.0001`).
    #[getter]
    fn pip_size(&self) -> f64 {
        self.inner.pip_size
    }

    /// Standard spot lag in business days (T+1 or T+2).
    #[getter]
    fn spot_lag_days(&self) -> u32 {
        self.inner.spot_lag_days
    }

    fn __repr__(&self) -> String {
        format!(
            "FxPairConvention(base={}, quote={}, usd_quotation={}, pip_size={}, spot_lag_days={})",
            self.inner.base,
            self.inner.quote,
            self.inner.usd_quotation,
            self.inner.pip_size,
            self.inner.spot_lag_days,
        )
    }
}

/// Order two currencies into the market CCY1/CCY2 pair.
///
/// Priority is EUR > GBP > AUD > NZD > USD > other, with an ISO-code tie-break.
///
/// # Arguments
///
/// * `a` - First currency of the unordered pair (`Currency` or ISO code).
/// * `b` - Second currency of the unordered pair (`Currency` or ISO code).
#[pyfunction]
#[pyo3(text_signature = "(a, b)")]
fn fx_market_pair(
    a: &Bound<'_, PyAny>,
    b: &Bound<'_, PyAny>,
) -> PyResult<(PyCurrency, PyCurrency)> {
    let (base, quote) = rust_fx_market_pair(extract_currency(a)?, extract_currency(b)?);
    Ok((PyCurrency::from_inner(base), PyCurrency::from_inner(quote)))
}

/// Market convention for an unordered currency pair.
///
/// Returned ``base`` / ``quote`` are always market CCY1/CCY2.
///
/// # Arguments
///
/// * `base` - One currency of the pair (`Currency` or ISO code). Orientation
///   is ignored.
/// * `quote` - The other currency of the pair (`Currency` or ISO code).
///   Orientation is ignored.
#[pyfunction]
#[pyo3(text_signature = "(base, quote)")]
fn fx_pair_convention(
    base: &Bound<'_, PyAny>,
    quote: &Bound<'_, PyAny>,
) -> PyResult<PyFxPairConvention> {
    Ok(PyFxPairConvention::from_inner(rust_fx_pair_convention(
        extract_currency(base)?,
        extract_currency(quote)?,
    )))
}

/// Pip size in outright-rate units for a currency pair.
///
/// ``0.01`` when either side is JPY, KRW, or HUF; otherwise ``0.0001``.
///
/// # Arguments
///
/// * `base` - One currency of the pair (`Currency` or ISO code). Order is not
///   significant.
/// * `quote` - The other currency of the pair (`Currency` or ISO code). Order
///   is not significant.
#[pyfunction]
#[pyo3(text_signature = "(base, quote)")]
fn fx_pip_size(base: &Bound<'_, PyAny>, quote: &Bound<'_, PyAny>) -> PyResult<f64> {
    Ok(rust_fx_pip_size(
        extract_currency(base)?,
        extract_currency(quote)?,
    ))
}

/// Reciprocal of a strictly positive finite FX rate.
///
/// # Arguments
///
/// * `rate` - Outright FX rate to invert, in quote-per-base units. Must be
///   finite and strictly positive; the reciprocal must also be a valid FX rate.
#[pyfunction]
#[pyo3(text_signature = "(rate)")]
fn invert_fx_rate(rate: f64) -> PyResult<f64> {
    rust_invert_fx_rate(rate).map_err(core_to_py)
}

pub(super) const EXPORTS: &[&str] = &[
    "FxConversionPolicy",
    "FxMatrix",
    "FxPairConvention",
    "FxQuoteConvention",
    "FxRateResult",
    "fx_market_pair",
    "fx_pair_convention",
    "fx_pip_size",
    "invert_fx_rate",
];

/// Register the `finstack_quant.core.market_data.fx` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "fx")?;
    m.setattr(
        "__doc__",
        "FX rate matrix and conversion policy bindings (finstack-quant-core).",
    )?;

    m.add_class::<PyFxConversionPolicy>()?;
    m.add_class::<PyFxRateResult>()?;
    m.add_class::<PyFxMatrix>()?;
    m.add_class::<PyFxQuoteConvention>()?;
    m.add_class::<PyFxPairConvention>()?;
    m.add_function(wrap_pyfunction!(fx_market_pair, &m)?)?;
    m.add_function(wrap_pyfunction!(fx_pair_convention, &m)?)?;
    m.add_function(wrap_pyfunction!(fx_pip_size, &m)?)?;
    m.add_function(wrap_pyfunction!(invert_fx_rate, &m)?)?;

    let all = PyList::new(py, EXPORTS)?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "fx",
        "finstack_quant.core.market_data",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
