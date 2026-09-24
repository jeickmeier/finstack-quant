//! Structured-credit tranche analytics: DM, break-even CDR, OAS, metrics,
//! scenario table.
//!
//! These take a tranche id, so they are not reachable through
//! `price_instrument`. Signatures mirror the WASM facade. The
//! OAS, metrics, and scenario-table entry points return the typed
//! `OasResult` / `TrancheMetrics` / `ScenarioTable` wrapper classes defined
//! in this module; each wrapper's `to_json` still yields the wire payload,
//! and `from_json` decodes it back.

use crate::bindings::extract::{extract_instrument_json, extract_market};
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    ColumnSchema,
};
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_tranche_breakeven_cdr, calculate_tranche_discount_margin, calculate_tranche_metrics,
    calculate_tranche_oas, scenario_table, OasConfig, OasResult, ScenarioGrid, ScenarioTable,
    StructuredCredit, TrancheMetrics,
};
use finstack_quant_valuations::instruments::InstrumentJson;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::collections::BTreeMap;

/// Column schema of [`PyScenarioTable::to_dataframe`], kept so a grid that
/// evaluated no cells still exports the documented columns.
const SCENARIO_CELL_COLUMNS: &[ColumnSchema<'static>] = &[
    ("tranche_id", "str"),
    ("cpr", "float64"),
    ("cdr", "float64"),
    ("severity", "float64"),
    ("price", "float64"),
    ("wal", "float64"),
    ("writedown", "float64"),
];

fn extract_structured_credit(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
) -> PyResult<StructuredCredit> {
    let instrument_json = extract_instrument_json(instrument)?;
    py.detach(move || {
        match finstack_quant_valuations::pricer::json::parse_instrument_from_json(&instrument_json)?
        {
            InstrumentJson::StructuredCredit(deal) => Ok(*deal),
            other => Err(finstack_quant_core::Error::Validation(format!(
                "expected a structured_credit instrument, got {}",
                other.type_tag()
            ))),
        }
    })
    .map_err(core_to_py)
}

/// Solve a z-spread-equivalent discount margin for a floating-rate tranche.
///
/// Contractual cashflows are projected without changing coupon projection, then
/// a constant additive spread is applied to the discount curve. The result is
/// zero at model PV, negative for a richer (higher) target PV, and positive for
/// a cheaper (lower) target PV; it is not the contractual quoted margin.
///
/// Parameters
/// ----------
/// instrument : StructuredCredit | str
///     Tagged JSON for a ``StructuredCredit`` deal, or a typed
///     ``StructuredCredit`` instance.
/// tranche_id : str
///     Identifier of the floating-rate tranche whose contractual cashflows are
///     spread-discounted.
/// market : MarketContext
///     Market context supplying the discount curve and any forward curves or
///     historical fixings required for cashflow projection.
/// as_of : datetime.date | str
///     Valuation date used for projection and discounting, either a date-like
///     object (``datetime.date``, ``pandas.Timestamp``) or an ISO 8601 string.
/// target_pv : float
///     Positive dirty settlement value in the tranche's currency, including
///     accrued interest once. The deal's ``quote_settlement_date`` defaults
///     to valuation; payments on or before settlement belong to the seller. Values above model PV
///     produce a negative result; values below model PV produce a positive
///     result.
///
/// Returns
/// -------
/// float
///     Z-spread-equivalent discount margin in decimal (``0.015`` = 150 bp).
///
/// Raises
/// ------
/// ValueError
///     If the JSON or date is malformed, the deal is invalid, the tranche is
///     missing or fixed-rate, ``target_pv`` is not finite, required market data
///     is unavailable, or the spread solve fails or exceeds ±5000 bp.
#[pyfunction]
#[pyo3(
    name = "structured_credit_tranche_discount_margin",
    text_signature = "(instrument, tranche_id, market, as_of, target_pv)"
)]
fn structured_credit_tranche_discount_margin(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    target_pv: f64,
) -> PyResult<f64> {
    let deal = extract_structured_credit(py, instrument)?;
    let market = extract_market(py, market)?;
    let tranche_id = tranche_id.to_owned();
    let as_of = crate::bindings::date_utils::extract_date(as_of)?;
    py.detach(move || {
        let target_pv = Money::new(target_pv, deal.tranches.currency())?;
        calculate_tranche_discount_margin(&deal, &tranche_id, &market, as_of, target_pv)
    })
    .map_err(core_to_py)
}

/// Solve the constant default rate at which a tranche first takes a writedown.
///
/// Parameters
/// ----------
/// instrument : StructuredCredit | str
///     Tagged JSON for a ``StructuredCredit`` deal, or a typed
///     ``StructuredCredit`` instance.
/// tranche_id : str
///     Identifier of the tranche.
/// market : MarketContext
///     Market context supplying curves and fixings.
/// as_of : datetime.date | str
///     Valuation date, either a date-like object (``datetime.date``,
///     ``pandas.Timestamp``) or an ISO 8601 string.
///
/// Returns
/// -------
/// float
///     Break-even annual CDR in decimal.
#[pyfunction]
#[pyo3(
    name = "structured_credit_tranche_breakeven_cdr",
    text_signature = "(instrument, tranche_id, market, as_of)"
)]
fn structured_credit_tranche_breakeven_cdr(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<f64> {
    let deal = extract_structured_credit(py, instrument)?;
    let market = extract_market(py, market)?;
    let tranche_id = tranche_id.to_owned();
    let as_of = crate::bindings::date_utils::extract_date(as_of)?;
    py.detach(move || calculate_tranche_breakeven_cdr(&deal, &tranche_id, &market, as_of))
        .map_err(core_to_py)
}

/// Compute the option-adjusted spread for a tranche at a market price.
///
/// Parameters
/// ----------
/// instrument : StructuredCredit | str
///     Tagged JSON for a ``StructuredCredit`` deal, or a typed
///     ``StructuredCredit`` instance.
/// tranche_id : str
///     Identifier of the tranche.
/// market_price_pct : float
///     Clean settlement price as a percentage of current balance (100.0 = par).
///     Accrued interest is added once at the deal's ``quote_settlement_date``
///     (valuation date when omitted).
/// market : MarketContext
///     Market context supplying curves and fixings.
/// as_of : datetime.date | str
///     Valuation date, either a date-like object (``datetime.date``,
///     ``pandas.Timestamp``) or an ISO 8601 string.
/// config : dict | str, optional
///     ``OasConfig`` as a dict or JSON string. All fields are currently
///     required when supplied.
///
/// Returns
/// -------
/// OasResult
///     Typed OAS result. Call ``to_json`` on it for the wire payload.
#[pyfunction]
#[pyo3(
    name = "structured_credit_tranche_oas",
    signature = (instrument, tranche_id, market_price_pct, market, as_of, config=None),
    text_signature = "(instrument, tranche_id, market_price_pct, market, as_of, config=None)"
)]
fn structured_credit_tranche_oas(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    tranche_id: &str,
    market_price_pct: f64,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyOasResult> {
    let deal = extract_structured_credit(py, instrument)?;
    let market = extract_market(py, market)?;
    let tranche_id = tranche_id.to_owned();
    let as_of = crate::bindings::date_utils::extract_date(as_of)?;
    let config: OasConfig = match config.filter(|value| !value.is_none()) {
        Some(value) => {
            let json = crate::bindings::module_utils::py_to_json_string(py, value, "OasConfig")?;
            serde_json::from_str(&json)
                .map_err(|e| serde_json_to_py(e, "invalid OAS config JSON"))?
        }
        None => OasConfig::default(),
    };
    let inner = py
        .detach(move || {
            calculate_tranche_oas(
                &deal,
                &tranche_id,
                market_price_pct,
                &market,
                as_of,
                &config,
            )
        })
        .map_err(core_to_py)?;
    Ok(PyOasResult { inner })
}

/// Compute the summary risk/pricing metrics for a tranche.
///
/// Parameters
/// ----------
/// instrument : StructuredCredit | str
///     Tagged JSON for a ``StructuredCredit`` deal, or a typed
///     ``StructuredCredit`` instance.
/// tranche_id : str
///     Identifier of the tranche.
/// market : MarketContext
///     Market context supplying curves and fixings.
/// as_of : datetime.date | str
///     Valuation date, either a date-like object (``datetime.date``,
///     ``pandas.Timestamp``) or an ISO 8601 string.
/// market_price_pct : float, optional
///     Clean settlement price as a percentage of current balance. When omitted,
///     the deal's market quote is used, or its model clean settlement price
///     if no quote is supplied. PV remains measured at valuation.
///
/// Returns
/// -------
/// TrancheMetrics
///     Typed metrics bundle. Call ``to_json`` on it for the wire payload.
#[pyfunction]
#[pyo3(
    name = "structured_credit_tranche_metrics",
    signature = (instrument, tranche_id, market, as_of, market_price_pct=None),
    text_signature = "(instrument, tranche_id, market, as_of, market_price_pct=None)"
)]
fn structured_credit_tranche_metrics(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    market_price_pct: Option<f64>,
) -> PyResult<PyTrancheMetrics> {
    let deal = extract_structured_credit(py, instrument)?;
    let market = extract_market(py, market)?;
    let tranche_id = tranche_id.to_owned();
    let as_of = crate::bindings::date_utils::extract_date(as_of)?;
    let inner = py
        .detach(move || {
            calculate_tranche_metrics(&deal, &tranche_id, &market, as_of, market_price_pct)
        })
        .map_err(core_to_py)?;
    Ok(PyTrancheMetrics { inner })
}

/// Price a tranche across a CPR x CDR x severity scenario grid.
///
/// Parameters
/// ----------
/// instrument : StructuredCredit | str
///     Tagged JSON for a ``StructuredCredit`` deal, or a typed
///     ``StructuredCredit`` instance.
/// tranche_id : str
///     Identifier of the tranche.
/// market : MarketContext
///     Market context supplying curves and fixings.
/// as_of : datetime.date | str
///     Valuation date, either a date-like object (``datetime.date``,
///     ``pandas.Timestamp``) or an ISO 8601 string.
/// grid : dict | str
///     ``ScenarioGrid`` as a dict or JSON string. The grid is capped at 10,000
///     cells because each cell reprices the entire deal.
///
/// Returns
/// -------
/// ScenarioTable
///     Typed scenario table. Call ``to_json`` on it for the wire payload.
#[pyfunction]
#[pyo3(
    name = "structured_credit_tranche_scenario_table",
    text_signature = "(instrument, tranche_id, market, as_of, grid)"
)]
fn structured_credit_tranche_scenario_table(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    grid: &Bound<'_, PyAny>,
) -> PyResult<PyScenarioTable> {
    let deal = extract_structured_credit(py, instrument)?;
    let market = extract_market(py, market)?;
    let tranche_id = tranche_id.to_owned();
    let as_of = crate::bindings::date_utils::extract_date(as_of)?;
    let grid_json = crate::bindings::module_utils::py_to_json_string(py, grid, "ScenarioGrid")?;
    let grid: ScenarioGrid = serde_json::from_str(&grid_json)
        .map_err(|e| serde_json_to_py(e, "invalid scenario grid JSON"))?;
    let inner = py
        .detach(move || scenario_table(&deal, &tranche_id, &market, as_of, &grid))
        .map_err(core_to_py)?;
    Ok(PyScenarioTable { inner })
}

/// Result of an option-adjusted-spread calculation for a structured-credit
/// tranche ([`structured_credit_tranche_oas`]'s return value).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "OasResult",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyOasResult {
    pub(crate) inner: OasResult,
}

#[pymethods]
impl PyOasResult {
    /// Jupyter rich display: delegates to ``to_dataframe()``.
    fn _repr_html_<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_dataframe(py)?.call_method0("_repr_html_")
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON-encoded ``OasResult``.
    ///
    /// Returns
    /// -------
    /// OasResult
    ///     The decoded result.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not valid JSON for the ``OasResult`` shape.
    ///
    /// Examples
    /// --------
    /// >>> import json
    /// >>> from finstack_quant.valuations.instruments import OasResult
    /// >>> payload = json.dumps({
    /// ...     "oas": 0.0125, "model_price": 99.5, "market_price": 98.75,
    /// ...     "num_paths": 256, "price_std_error": 0.05,
    /// ... })
    /// >>> result = OasResult.from_json(payload)
    /// >>> result.oas
    /// 0.0125
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: OasResult = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize back to the same JSON shape ``from_json`` accepts.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON-encoded ``OasResult``.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Option-adjusted spread, as an annual decimal (``0.01`` = 100 bp).
    #[getter]
    fn oas(&self) -> f64 {
        self.inner.oas
    }

    /// Model price at the solved OAS, as a percentage of current balance.
    #[getter]
    fn model_price(&self) -> f64 {
        self.inner.model_price
    }

    /// Target market price, as a percentage of current balance.
    #[getter]
    fn market_price(&self) -> f64 {
        self.inner.market_price
    }

    /// Number of Monte-Carlo scenarios used.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    /// Monte-Carlo standard error of the mean price, as a percentage of
    /// current balance.
    #[getter]
    fn price_std_error(&self) -> f64 {
        self.inner.price_std_error
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``oas`` (annual decimal), ``model_price`` and ``market_price``
    /// (percentage of current balance), ``num_paths``, ``price_std_error``
    /// (percentage of current balance).
    ///
    /// One row, so a book of tranches stacks with
    /// ``pd.concat([r.to_dataframe() for r in results])``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &[
                "oas",
                "model_price",
                "market_price",
                "num_paths",
                "price_std_error",
            ],
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "OasResult(oas={}, model_price={}, market_price={})",
            self.inner.oas, self.inner.model_price, self.inner.market_price
        )
    }
}

/// Summary risk/pricing metrics for a structured-credit tranche
/// ([`structured_credit_tranche_metrics`]'s return value).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TrancheMetrics",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTrancheMetrics {
    pub(crate) inner: TrancheMetrics,
}

#[pymethods]
impl PyTrancheMetrics {
    /// Jupyter rich display: delegates to ``to_dataframe()``.
    fn _repr_html_<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_dataframe(py)?.call_method0("_repr_html_")
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON-encoded ``TrancheMetrics``.
    ///
    /// Returns
    /// -------
    /// TrancheMetrics
    ///     The decoded metrics bundle.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not valid JSON for the ``TrancheMetrics`` shape.
    ///
    /// Examples
    /// --------
    /// >>> import json
    /// >>> from finstack_quant.valuations.instruments import TrancheMetrics
    /// >>> payload = json.dumps({
    /// ...     "tranche_id": "A", "currency": "USD", "pv": 1000.0,
    /// ...     "price_pct": 100.0, "factor": 1.0, "wal": 3.0, "z_spread_bp": 0.0,
    /// ...     "cs01": -1.0, "spread_duration": 3.0, "spread_convexity": 12.0,
    /// ...     "modified_duration": 3.0, "convexity": 12.0, "target_price_pct": 100.0,
    /// ... })
    /// >>> metrics = TrancheMetrics.from_json(payload)
    /// >>> metrics.tranche_id
    /// 'A'
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: TrancheMetrics = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize back to the same JSON shape ``from_json`` accepts.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON-encoded ``TrancheMetrics``.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Identifier of the tranche.
    #[getter]
    fn tranche_id(&self) -> &str {
        &self.inner.tranche_id
    }

    /// ISO-4217 code of the currency ``pv`` and ``cs01`` are denominated in.
    #[getter]
    fn currency(&self) -> &str {
        &self.inner.currency
    }

    /// Present value of the tranche, in ``currency`` units.
    #[getter]
    fn pv(&self) -> f64 {
        self.inner.pv
    }

    /// Model clean price, as a percentage of the tranche's current balance
    /// (the factor-adjusted secondary-market quote basis).
    #[getter]
    fn price_pct(&self) -> f64 {
        self.inner.price_pct
    }

    /// Pool factor of the note: current balance over original balance, so a
    /// price on original face is ``price_pct * factor``.
    #[getter]
    fn factor(&self) -> f64 {
        self.inner.factor
    }

    /// Weighted-average life, in years.
    #[getter]
    fn wal(&self) -> f64 {
        self.inner.wal
    }

    /// Z-spread to ``target_price_pct``, in basis points.
    #[getter]
    fn z_spread_bp(&self) -> f64 {
        self.inner.z_spread_bp
    }

    /// Credit-spread DV01 — currency change for a +1 bp z-spread shock,
    /// in ``currency`` units. Negative for a long tranche.
    #[getter]
    fn cs01(&self) -> f64 {
        self.inner.cs01
    }

    /// Spread duration, in years (``-cs01 / (pv * 1bp)``).
    #[getter]
    fn spread_duration(&self) -> f64 {
        self.inner.spread_duration
    }

    /// Spread convexity at the solved z-spread, in years squared.
    #[getter]
    fn spread_convexity(&self) -> f64 {
        self.inner.spread_convexity
    }

    /// Effective (rate) duration, in years: the cashflows are re-projected
    /// with every rate curve bumped ±1 bp, so a floater's coupon resets move
    /// with the curve and its duration is short.
    #[getter]
    fn modified_duration(&self) -> f64 {
        self.inner.modified_duration
    }

    /// Effective convexity from the same ±1 bp re-projection, in years
    /// squared.
    #[getter]
    fn convexity(&self) -> f64 {
        self.inner.convexity
    }

    /// Discount margin to maturity at ``target_price_pct`` for a
    /// floating-rate tranche, in basis points; ``None`` for fixed-rate
    /// tranches.
    #[getter]
    fn dm_bp(&self) -> Option<f64> {
        self.inner.dm_bp
    }

    /// Price the z-spread/CS01 were solved against, as a percentage of
    /// current balance.
    #[getter]
    fn target_price_pct(&self) -> f64 {
        self.inner.target_price_pct
    }

    /// Weighted-average life to the assumed call, in years, or ``None`` when
    /// the deal carries no call covering this tranche.
    #[getter]
    fn wal_to_call(&self) -> Option<f64> {
        self.inner.wal_to_call
    }

    /// Z-spread of the to-call flows to ``target_price_pct``, in basis
    /// points, or ``None`` without a call.
    #[getter]
    fn z_spread_to_call_bp(&self) -> Option<f64> {
        self.inner.z_spread_to_call_bp
    }

    /// Discount margin to call for a floating-rate tranche, in basis points,
    /// or ``None`` for fixed-rate tranches or without a call.
    #[getter]
    fn dm_to_call_bp(&self) -> Option<f64> {
        self.inner.dm_to_call_bp
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``tranche_id``, ``currency``, ``pv``, ``price_pct``,
    /// ``factor``, ``wal``, ``z_spread_bp``, ``cs01``, ``spread_duration``,
    /// ``spread_convexity``, ``modified_duration``, ``convexity``,
    /// ``target_price_pct``, ``wal_to_call``, ``z_spread_to_call_bp``,
    /// ``dm_to_call_bp``, ``dm_bp`` — the same fields and units as the
    /// getters of the same name (the ``*_to_call`` columns are ``NaN``
    /// without a call, ``dm_bp`` for fixed-rate tranches).
    ///
    /// One row per tranche, so a capital structure stacks with
    /// ``pd.concat([m.to_dataframe() for m in metrics])``. ``pv`` and ``cs01``
    /// are in ``currency`` units and are only additive across tranches sharing
    /// one currency.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &[
                "tranche_id",
                "currency",
                "pv",
                "price_pct",
                "factor",
                "wal",
                "z_spread_bp",
                "cs01",
                "spread_duration",
                "spread_convexity",
                "modified_duration",
                "convexity",
                "target_price_pct",
                "wal_to_call",
                "z_spread_to_call_bp",
                "dm_to_call_bp",
                "dm_bp",
            ],
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "TrancheMetrics(tranche_id={:?}, pv={}, price_pct={})",
            self.inner.tranche_id, self.inner.pv, self.inner.price_pct
        )
    }
}

/// Scenario/yield table for a single structured-credit tranche
/// ([`structured_credit_tranche_scenario_table`]'s return value).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "ScenarioTable",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyScenarioTable {
    pub(crate) inner: ScenarioTable,
}

#[pymethods]
impl PyScenarioTable {
    /// Jupyter rich display: delegates to ``to_dataframe()``.
    fn _repr_html_<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_dataframe(py)?.call_method0("_repr_html_")
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON-encoded ``ScenarioTable``.
    ///
    /// Returns
    /// -------
    /// ScenarioTable
    ///     The decoded scenario table.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not valid JSON for the ``ScenarioTable`` shape.
    ///
    /// Examples
    /// --------
    /// >>> import json
    /// >>> from finstack_quant.valuations.instruments import ScenarioTable
    /// >>> payload = json.dumps({
    /// ...     "tranche_id": "A",
    /// ...     "cells": [{"cpr": 0.06, "cdr": 0.02, "severity": 0.6,
    /// ...                "price": 98.2, "wal": 4.1, "writedown": 0.0}],
    /// ... })
    /// >>> table = ScenarioTable.from_json(payload)
    /// >>> table.tranche_id
    /// 'A'
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: ScenarioTable = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize back to the same JSON shape ``from_json`` accepts.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON-encoded ``ScenarioTable``.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Identifier of the tranche evaluated.
    #[getter]
    fn tranche_id(&self) -> &str {
        &self.inner.tranche_id
    }

    /// Evaluated cells, in CPR-major, then CDR, then severity order.
    ///
    /// Each cell is a dict with keys ``cpr`` (annual decimal), ``cdr``
    /// (annual decimal), ``severity`` (decimal), ``price`` (percentage of
    /// original balance), ``wal`` (years), and ``writedown`` (currency
    /// units).
    ///
    /// Returns
    /// -------
    /// list[dict[str, float]]
    ///     One dict per evaluated scenario cell.
    #[pyo3(text_signature = "($self)")]
    fn cells(&self) -> Vec<BTreeMap<String, f64>> {
        self.inner
            .cells
            .iter()
            .map(|cell| {
                let mut map = BTreeMap::new();
                map.insert("cpr".to_string(), cell.cpr);
                map.insert("cdr".to_string(), cell.cdr);
                map.insert("severity".to_string(), cell.severity);
                map.insert("price".to_string(), cell.price);
                map.insert("wal".to_string(), cell.wal);
                map.insert("writedown".to_string(), cell.writedown);
                map
            })
            .collect()
    }

    /// Export the evaluated cells as a pandas ``DataFrame``.
    ///
    /// Columns: ``tranche_id``, ``cpr``, ``cdr``, ``severity`` (all annual
    /// decimals except ``severity``, which is a plain decimal), ``price``
    /// (percentage of current balance), ``wal`` (years), ``writedown``
    /// (currency units).
    ///
    /// One row per cell of the grid, in CPR-major then CDR then severity
    /// order — the same cells and order as ``cells``. ``tranche_id`` repeats on
    /// every row so tables for several tranches concatenate and pivot. A grid
    /// that evaluated no cells yields a zero-row frame that still carries the
    /// columns above.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .cells
            .iter()
            .map(|cell| {
                serde_json::json!({
                    "tranche_id": self.inner.tranche_id,
                    "cpr": cell.cpr,
                    "cdr": cell.cdr,
                    "severity": cell.severity,
                    "price": cell.price,
                    "wal": cell.wal,
                    "writedown": cell.writedown,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, SCENARIO_CELL_COLUMNS)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ScenarioTable(tranche_id={:?}, cells={})",
            self.inner.tranche_id,
            self.inner.cells.len()
        )
    }
}

/// Names registered by this module, in the order they appear in `__all__`.
pub(crate) const EXPORTS: &[&str] = &[
    "OasResult",
    "ScenarioTable",
    "TrancheMetrics",
    "structured_credit_tranche_breakeven_cdr",
    "structured_credit_tranche_discount_margin",
    "structured_credit_tranche_metrics",
    "structured_credit_tranche_oas",
    "structured_credit_tranche_scenario_table",
];

/// Register the structured-credit tranche analytics on the instruments module.
pub(crate) fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    parent.add_function(wrap_pyfunction!(
        structured_credit_tranche_discount_margin,
        parent
    )?)?;
    parent.add_function(wrap_pyfunction!(
        structured_credit_tranche_breakeven_cdr,
        parent
    )?)?;
    parent.add_function(wrap_pyfunction!(structured_credit_tranche_oas, parent)?)?;
    parent.add_function(wrap_pyfunction!(structured_credit_tranche_metrics, parent)?)?;
    parent.add_function(wrap_pyfunction!(
        structured_credit_tranche_scenario_table,
        parent
    )?)?;
    parent.add_class::<PyOasResult>()?;
    parent.add_class::<PyTrancheMetrics>()?;
    parent.add_class::<PyScenarioTable>()?;
    for &name in EXPORTS {
        parent
            .getattr(name)?
            .setattr("__module__", "finstack_quant.valuations.instruments")?;
    }
    Ok(())
}
