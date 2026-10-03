//! Portfolio historical VaR / expected shortfall by full revaluation.
//!
//! Binds `finstack_quant_valuations::metrics::risk::calculate_var_with_pricing`
//! and its typed `VarResult`. The per-instrument `hvar` metric reprices one
//! instrument; this entry point reprices a whole list under every scenario and
//! takes the quantile of the summed P&L, so diversification is preserved.

use super::pricing::PyMarketHistory;
use crate::bindings::extract::{extract_instrument_json, extract_market};
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_valuations::metrics::risk::{
    calculate_var_with_pricing as rust_calculate_var_with_pricing, MarketHistory, VarConfig,
    VarResult,
};
use finstack_quant_valuations::pricer::PricingDispatch;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};

/// Coerce ``MarketHistory | dict | str`` into the Rust history.
fn extract_history(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<MarketHistory> {
    if let Ok(typed) = obj.cast::<PyMarketHistory>() {
        return Ok(typed.borrow().inner.clone());
    }
    if let Ok(text) = obj.cast::<PyString>() {
        return serde_json::from_str(text.to_str()?)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid MarketHistory JSON"));
    }
    py_to_serde(py, obj, "MarketHistory")
}

/// Coerce ``dict | str | None`` into a `VarConfig`; ``None`` is Rust's default.
fn extract_config(py: Python<'_>, obj: Option<&Bound<'_, PyAny>>) -> PyResult<VarConfig> {
    match obj {
        None => Ok(VarConfig::default()),
        Some(obj) if obj.is_none() => Ok(VarConfig::default()),
        Some(obj) => {
            if let Ok(text) = obj.cast::<PyString>() {
                return serde_json::from_str(text.to_str()?)
                    .map_err(|e| crate::errors::serde_json_to_py(e, "invalid VarConfig JSON"));
            }
            py_to_serde(py, obj, "VarConfig")
        }
    }
}

/// Historical VaR and expected shortfall of a list of instruments.
///
/// Mirrors Rust ``metrics::risk::calculate_var_with_pricing``: every
/// instrument is repriced under every ``history`` scenario, the per-scenario
/// P&Ls are summed across instruments, and VaR / ES are read off that single
/// portfolio distribution (R type-7 linear-interpolated quantile). Unlike
/// summing the per-instrument ``hvar`` metric, offsetting positions diversify.
/// Quote-recalibrated shocks (credit spreads) use the same recalibration
/// provider as ``price_instrument``.
///
/// Parameters
/// ----------
/// instruments : list[Bond | InterestRateSwap | ... | str]
///     Typed instruments or ``finstack_quant.instrument/1`` envelopes. An
///     empty list returns zero VaR and ES.
/// market : MarketContext | str
///     Unshocked base market every scenario perturbs.
/// history : MarketHistory | dict | str
///     Historical risk-factor shifts; a non-empty portfolio needs at least
///     one scenario.
/// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Valuation date for the base and every scenario revaluation.
/// config : dict | str | None, optional
///     Rust ``VarConfig``: ``confidence_level`` (decimal in ``(0, 1)``),
///     ``method`` (``"full_revaluation"`` or ``"taylor_approximation"``) and
///     ``reporting_currency`` (required for mixed-currency portfolios).
///     ``None`` uses Rust ``VarConfig::default()``: 95%, full revaluation,
///     natural currency.
/// model : str, optional
///     ``"default"`` (each instrument's canonical pricing path) or one model
///     key from ``list_models()`` applied to every instrument.
///
/// Returns
/// -------
/// VarResult
///     VaR, expected shortfall and the sorted P&L distribution. Losses are
///     negative.
///
/// Raises
/// ------
/// ValueError
///     If an instrument, ``config``, ``history`` or ``model`` is invalid, the
///     confidence level is outside ``(0, 1)``, a non-empty portfolio has no
///     scenarios, or a mixed-currency portfolio has no ``reporting_currency``.
/// KeyError
///     If a curve, surface or price an instrument needs is missing.
/// RuntimeError
///     If a scenario revaluation fails numerically.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.currency import Currency
/// >>> from finstack_quant.core.dates import StubKind
/// >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.core.types import Rate
/// >>> from finstack_quant.valuations.instruments import (
/// ...     Bond, MarketHistory, calculate_var_with_pricing)
/// >>> as_of = datetime.date(2024, 1, 2)
/// >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
/// >>> def bond(id):
/// ...     return Bond.fixed(id, Money(1e6, Currency("USD")), Rate(0.05), as_of,
/// ...                       datetime.date(2029, 1, 2), StubKind.NONE, "USD-OIS")
/// >>> shift = lambda s: [{"factor": {"type": "discount_rate", "curve_id": "USD-OIS",
/// ...                                "tenor_years": 5.0}, "shift": s}]
/// >>> history = MarketHistory(as_of, 2, [
/// ...     {"date": "2023-12-29", "shifts": shift(0.0010)},
/// ...     {"date": "2023-12-28", "shifts": shift(-0.0005)}])
/// >>> one = calculate_var_with_pricing([bond("A")], market, history, as_of)
/// >>> two = calculate_var_with_pricing([bond("A"), bond("B")], market, history, as_of)
/// >>> (two.num_scenarios, one.var < 0, abs(two.var - 2 * one.var) < 1e-6)
/// (2, True, True)
#[pyfunction]
#[pyo3(signature = (instruments, market, history, as_of, config=None, model="default"))]
#[pyo3(text_signature = "(instruments, market, history, as_of, config=None, model='default')")]
fn calculate_var_with_pricing(
    py: Python<'_>,
    instruments: Vec<Bound<'_, PyAny>>,
    market: &Bound<'_, PyAny>,
    history: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
    model: &str,
) -> PyResult<PyVarResult> {
    let envelopes = instruments
        .iter()
        .map(extract_instrument_json)
        .collect::<PyResult<Vec<String>>>()?;
    let history = extract_history(py, history)?;
    let config = extract_config(py, config)?;
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date(as_of)?;
    let model = model.to_owned();
    py.detach(move || {
        let parsed = envelopes
            .iter()
            .map(|json| {
                finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(json, None)
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;
        let refs: Vec<_> = parsed.iter().map(|p| p.as_instrument()).collect();
        rust_calculate_var_with_pricing(
            &refs,
            &market,
            &history,
            as_of,
            &config,
            PricingDispatch::from_model(&model)?,
            finstack_quant_calibration::recalibration::pricing_options().recalibration_provider,
        )
    })
    .map(|inner| PyVarResult { inner })
    .map_err(core_to_py)
}

/// Historical VaR / expected shortfall with its P&L distribution.
///
/// Typed wrapper of the Rust ``VarResult`` returned by
/// :func:`calculate_var_with_pricing`. VaR and ES follow the P&L sign: losses
/// are negative, and ``expected_shortfall <= var``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import VarResult
/// >>> result = VarResult.from_json(
/// ...     '{"var": -10.0, "expected_shortfall": -12.0, "pnl_distribution": [-12.0, 3.0],'
/// ...     ' "num_scenarios": 2, "confidence_level": 0.95, "skipped_fx": false,'
/// ...     ' "skipped_vol": false}')
/// >>> (result.var, list(result.to_dataframe()["pnl"]))
/// (-10.0, [-12.0, 3.0])
#[pyclass(
    name = "VarResult",
    module = "finstack_quant.valuations.instruments",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyVarResult {
    pub(crate) inner: VarResult,
}

#[pymethods]
impl PyVarResult {
    /// Value-at-Risk: the signed P&L at the ``1 - confidence_level`` quantile
    /// (negative for a loss, ``0.0`` for an all-gain distribution), in the
    /// portfolio or reporting currency.
    #[getter]
    fn var(&self) -> f64 {
        self.inner.var
    }

    /// Expected shortfall: mean signed P&L over the worst
    /// ``1 - confidence_level`` tail; same sign convention as ``var``.
    #[getter]
    fn expected_shortfall(&self) -> f64 {
        self.inner.expected_shortfall
    }

    /// Portfolio P&L per scenario, sorted ascending (worst first).
    #[getter]
    fn pnl_distribution(&self) -> Vec<f64> {
        self.inner.pnl_distribution.clone()
    }

    /// Number of scenarios in the distribution.
    #[getter]
    fn num_scenarios(&self) -> usize {
        self.inner.num_scenarios
    }

    /// Confidence level used, as a decimal (``0.95`` = 95%).
    #[getter]
    fn confidence_level(&self) -> f64 {
        self.inner.confidence_level
    }

    /// ``True`` when the Taylor method skipped FX-spot shocks (VaR understates FX risk).
    #[getter]
    fn skipped_fx(&self) -> bool {
        self.inner.skipped_fx
    }

    /// ``True`` when the Taylor method skipped implied-vol point shocks.
    #[getter]
    fn skipped_vol(&self) -> bool {
        self.inner.skipped_vol
    }

    /// The P&L distribution as a one-column pandas ``DataFrame``.
    ///
    /// Column ``pnl`` (``float64``), one row per scenario in the stored
    /// worst-first order.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        let series = py.import("pandas")?.getattr("Series")?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("dtype", "float64")?;
        data.set_item(
            "pnl",
            series.call((self.inner.pnl_distribution.clone(),), Some(&kwargs))?,
        )?;
        dict_to_dataframe(py, &data, None)
    }

    /// Deserialize a result from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Serialized ``VarResult``.
    ///
    /// Returns
    /// -------
    /// VarResult
    ///     The parsed result.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed, misses a field or carries an unknown one.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid VarResult JSON"))?,
        })
    }

    /// Serialize to compact JSON.
    ///
    /// Returns
    /// -------
    /// str
    ///     The result JSON (the WASM ``calculateVarWithPricing`` object).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the result cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` (and therefore ``multiprocessing``, ``joblib``, ``dask``).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "VarResult(var={}, expected_shortfall={}, num_scenarios={}, confidence_level={})",
            self.inner.var,
            self.inner.expected_shortfall,
            self.inner.num_scenarios,
            self.inner.confidence_level
        )
    }
}

/// Register the VaR entry point and result type on the instruments submodule.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyVarResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(calculate_var_with_pricing, m)?)?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["VarResult", "calculate_var_with_pricing"];
