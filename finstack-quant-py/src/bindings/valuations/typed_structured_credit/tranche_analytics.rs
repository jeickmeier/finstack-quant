//! Structured-credit tranche analytics: DM, break-even CDR, OAS, metrics,
//! scenario table.
//!
//! These take a tranche id, so they are not reachable through
//! `price_instrument`. Signatures mirror the WASM facade. The
//! OAS, metrics, and scenario-table entry points return the typed
//! `OasResult` / `TrancheMetrics` / `ScenarioTable` wrapper classes defined
//! in this module; each wrapper's `to_json` still yields the wire payload,
//! and `from_json` decodes it back.

use crate::bindings::extract::{extract_instrument, extract_market};
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    ColumnSchema,
};
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    self as sc, OasConfig, OasResult, ScenarioGrid, ScenarioTable, StructuredCredit, TrancheMetrics,
};
use finstack_quant_valuations::metrics::MetricId;
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
    let instrument = extract_instrument(instrument)?;
    py.detach(move || StructuredCredit::try_from(instrument.into_instrument_json()?))
        .map_err(core_to_py)
}

/// Solve a z-spread-equivalent discount margin for a floating-rate tranche.
///
/// Contractual cashflows are projected without changing coupon projection, then
/// a constant additive spread is applied to the discount curve. The result is
/// zero at the model price, negative for a richer (higher) price, and positive
/// for a cheaper (lower) price; it is not the contractual quoted margin.
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
/// market_price_pct : float
///     Clean settlement price as a percentage of the tranche's current
///     balance (100.0 = par). Accrued interest is added once at the deal's
///     ``quote_settlement_date`` (valuation date when omitted); payments on
///     or before settlement belong to the seller.
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
///     missing or fixed-rate, ``market_price_pct`` is not finite and positive, required market data
///     is unavailable, or the spread solve fails or exceeds ±5000 bp.
#[pyfunction]
#[pyo3(
    name = "tranche_discount_margin",
    text_signature = "(instrument, tranche_id, market, as_of, market_price_pct)"
)]
fn tranche_discount_margin(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    market_price_pct: f64,
) -> PyResult<f64> {
    let deal = extract_structured_credit(py, instrument)?;
    let market = extract_market(py, market)?;
    let tranche_id = tranche_id.to_owned();
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
    py.detach(move || {
        sc::tranche_discount_margin(&deal, &tranche_id, &market, as_of, market_price_pct)
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
    name = "tranche_breakeven_cdr",
    text_signature = "(instrument, tranche_id, market, as_of)"
)]
fn tranche_breakeven_cdr(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<f64> {
    let deal = extract_structured_credit(py, instrument)?;
    let market = extract_market(py, market)?;
    let tranche_id = tranche_id.to_owned();
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
    py.detach(move || sc::tranche_breakeven_cdr(&deal, &tranche_id, &market, as_of))
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
/// market : MarketContext
///     Market context supplying curves and fixings.
/// as_of : datetime.date | str
///     Valuation date, either a date-like object (``datetime.date``,
///     ``pandas.Timestamp``) or an ISO 8601 string.
/// market_price_pct : float
///     Clean settlement price as a percentage of current balance (100.0 = par).
///     Accrued interest is added once at the deal's ``quote_settlement_date``
///     (valuation date when omitted).
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
    name = "tranche_oas",
    signature = (instrument, tranche_id, market, as_of, market_price_pct, config=None),
    text_signature = "(instrument, tranche_id, market, as_of, market_price_pct, config=None)"
)]
fn tranche_oas(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    market_price_pct: f64,
    config: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyOasResult> {
    let deal = extract_structured_credit(py, instrument)?;
    let market = extract_market(py, market)?;
    let tranche_id = tranche_id.to_owned();
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
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
            sc::tranche_oas(
                &deal,
                &tranche_id,
                &market,
                as_of,
                market_price_pct,
                &config,
            )
        })
        .map_err(core_to_py)?;
    Ok(PyOasResult { inner })
}

/// Compute the per-tranche valuation, yield, spread and risk metrics.
///
/// The one per-note analytics entry point: prices, accrued, yield, spreads
/// and durations come from the tranche's own to-maturity cashflows, and
/// ``metrics`` adds registry metrics (DV01, Default01, …) valued on the note
/// so tranche sensitivities sum to the deal's.
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
/// metrics : list[str], optional
///     Registry metric ids (for example ``"dv01"``, ``"default01"``) to value
///     on this note; returned in ``TrancheMetrics.metrics``. Omitted or empty
///     computes none.
///
/// Returns
/// -------
/// TrancheMetrics
///     Typed metrics bundle. Call ``to_json`` on it for the wire payload.
///
/// Raises
/// ------
/// ValueError
///     If a metric id is unknown, the deal fails validation, the tranche is
///     missing, or a solve fails.
/// KeyError
///     If a required curve is missing from ``market``.
#[pyfunction]
#[pyo3(
    name = "tranche_metrics",
    signature = (instrument, tranche_id, market, as_of, market_price_pct=None, metrics=None),
    text_signature = "(instrument, tranche_id, market, as_of, market_price_pct=None, metrics=None)"
)]
fn tranche_metrics(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    tranche_id: &str,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    market_price_pct: Option<f64>,
    metrics: Option<Vec<String>>,
) -> PyResult<PyTrancheMetrics> {
    let deal = extract_structured_credit(py, instrument)?;
    let market = extract_market(py, market)?;
    let tranche_id = tranche_id.to_owned();
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
    let metric_ids = metrics
        .unwrap_or_default()
        .iter()
        .map(|id| MetricId::parse_strict(id))
        .collect::<finstack_quant_core::Result<Vec<_>>>()
        .map_err(core_to_py)?;
    let inner = py
        .detach(move || {
            sc::tranche_metrics(
                &deal,
                &tranche_id,
                &market,
                as_of,
                market_price_pct,
                &metric_ids,
            )
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
    name = "tranche_scenario_table",
    text_signature = "(instrument, tranche_id, market, as_of, grid)"
)]
fn tranche_scenario_table(
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
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
    let grid_json = crate::bindings::module_utils::py_to_json_string(py, grid, "ScenarioGrid")?;
    let grid: ScenarioGrid = serde_json::from_str(&grid_json)
        .map_err(|e| serde_json_to_py(e, "invalid scenario grid JSON"))?;
    let inner = py
        .detach(move || sc::tranche_scenario_table(&deal, &tranche_id, &market, as_of, &grid))
        .map_err(core_to_py)?;
    Ok(PyScenarioTable { inner })
}

/// Result of an option-adjusted-spread calculation for a structured-credit
/// tranche ([`tranche_oas`]'s return value).
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

crate::bindings::macros::py_getters!(PyOasResult |s| {
    /// Option-adjusted spread, as an annual decimal (``0.01`` = 100 bp).
    oas -> f64 = s.inner.oas;

    /// Model price at the solved OAS, as a percentage of current balance.
    model_price -> f64 = s.inner.model_price;

    /// Target market price, as a percentage of current balance.
    market_price -> f64 = s.inner.market_price;

    /// Number of Monte-Carlo scenarios used.
    num_paths -> usize = s.inner.num_paths;

    /// Monte-Carlo standard error of the mean price, as a percentage of
    /// current balance.
    price_std_error -> f64 = s.inner.price_std_error;
});

/// Summary risk/pricing metrics for a structured-credit tranche
/// ([`tranche_metrics`]'s return value).
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
    /// ...     "clean_price_pct": 100.0, "dirty_price_pct": 100.0, "accrued": 0.0,
    /// ...     "factor": 1.0, "wal": 3.0, "ytm": 0.05, "z_spread_bp": 0.0,
    /// ...     "cs01": -1.0, "spread_duration": 3.0, "spread_convexity": 12.0,
    /// ...     "effective_duration": 3.0, "effective_convexity": 12.0,
    /// ...     "target_price_pct": 100.0,
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

    /// Requested registry metrics valued on this note, keyed by metric id
    /// (empty when none were requested); units follow each metric.
    #[getter]
    fn metrics(&self) -> BTreeMap<String, f64> {
        self.inner
            .metrics
            .iter()
            .map(|(id, value)| (id.to_string(), *value))
            .collect()
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``tranche_id``, ``currency``, ``pv``, ``clean_price_pct``,
    /// ``dirty_price_pct``, ``accrued``, ``factor``, ``wal``, ``ytm``,
    /// ``z_spread_bp``, ``cs01``, ``spread_duration``, ``spread_convexity``,
    /// ``effective_duration``, ``effective_convexity``, ``target_price_pct``,
    /// ``wal_to_call``, ``z_spread_to_call_bp``, ``dm_to_call_bp``, ``dm_bp``
    /// — the same fields and units as the getters of the same name (``ytm``
    /// is ``NaN`` for an impaired note, the ``*_to_call`` columns without a
    /// call, ``dm_bp`` for fixed-rate tranches). Requested registry metrics
    /// stay in the ``metrics`` getter.
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
                "clean_price_pct",
                "dirty_price_pct",
                "accrued",
                "factor",
                "wal",
                "ytm",
                "z_spread_bp",
                "cs01",
                "spread_duration",
                "spread_convexity",
                "effective_duration",
                "effective_convexity",
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
            "TrancheMetrics(tranche_id={:?}, pv={}, clean_price_pct={})",
            self.inner.tranche_id, self.inner.pv, self.inner.clean_price_pct
        )
    }
}

crate::bindings::macros::py_getters!(PyTrancheMetrics |s| {
    /// Identifier of the tranche.
    tranche_id -> &str = &s.inner.tranche_id;

    /// ISO-4217 code of the currency ``pv``, ``accrued`` and ``cs01`` are
    /// denominated in.
    currency -> String = s.inner.currency.to_string();

    /// Present value of the tranche, in ``currency`` units.
    pv -> f64 = s.inner.pv;

    /// Model clean settlement price, as a percentage of the tranche's current
    /// balance (the factor-adjusted secondary-market quote basis).
    clean_price_pct -> f64 = s.inner.clean_price_pct;

    /// Model dirty settlement price (clean plus accrued), as a percentage of
    /// the tranche's current balance.
    dirty_price_pct -> f64 = s.inner.dirty_price_pct;

    /// Accrued coupon at buyer settlement, in ``currency`` units.
    accrued -> f64 = s.inner.accrued;

    /// Pool factor of the note: current balance over original balance, so a
    /// price on original face is ``clean_price_pct * factor``.
    factor -> f64 = s.inner.factor;

    /// Weighted-average life, in years.
    wal -> f64 = s.inner.wal;

    /// Decimal yield reproducing the ``target_price_pct`` dirty value,
    /// compounded at the note's coupon frequency in its own day count;
    /// ``None`` for an impaired note.
    ytm -> Option<f64> = s.inner.ytm;

    /// Z-spread to ``target_price_pct``, in basis points.
    z_spread_bp -> f64 = s.inner.z_spread_bp;

    /// Credit-spread DV01 — currency change for a +1 bp z-spread shock,
    /// in ``currency`` units. Negative for a long tranche.
    cs01 -> f64 = s.inner.cs01;

    /// Spread duration, in years (``-cs01 / (pv * 1bp)``).
    spread_duration -> f64 = s.inner.spread_duration;

    /// Spread convexity at the solved z-spread, in years squared.
    spread_convexity -> f64 = s.inner.spread_convexity;

    /// Effective (rate) duration, in years: the cashflows are re-projected
    /// with every rate curve bumped ±1 bp, so a floater's coupon resets move
    /// with the curve and its duration is short.
    effective_duration -> f64 = s.inner.effective_duration;

    /// Effective convexity from the same ±1 bp re-projection, in years
    /// squared.
    effective_convexity -> f64 = s.inner.effective_convexity;

    /// Discount margin to maturity at ``target_price_pct`` for a
    /// floating-rate tranche, in basis points; ``None`` for fixed-rate
    /// tranches.
    dm_bp -> Option<f64> = s.inner.dm_bp;

    /// Price the yield, z-spread and CS01 were solved against, as a
    /// percentage of current balance.
    target_price_pct -> f64 = s.inner.target_price_pct;

    /// Weighted-average life to the assumed call, in years, or ``None`` when
    /// the deal carries no call covering this tranche.
    wal_to_call -> Option<f64> = s.inner.wal_to_call;

    /// Z-spread of the to-call flows to ``target_price_pct``, in basis
    /// points, or ``None`` without a call.
    z_spread_to_call_bp -> Option<f64> = s.inner.z_spread_to_call_bp;

    /// Discount margin to call for a floating-rate tranche, in basis points,
    /// or ``None`` for fixed-rate tranches or without a call.
    dm_to_call_bp -> Option<f64> = s.inner.dm_to_call_bp;
});

/// Scenario/yield table for a single structured-credit tranche
/// ([`tranche_scenario_table`]'s return value).
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
    ///     A copy of each stored scenario cell; mutating the returned list does
    ///     not change this read-only property.
    #[getter]
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

crate::bindings::macros::py_getters!(PyScenarioTable |s| {
    /// Identifier of the tranche evaluated.
    tranche_id -> &str = &s.inner.tranche_id;
});

/// Names registered by this module, in the order they appear in `__all__`.
pub(crate) const EXPORTS: &[&str] = &[
    "OasResult",
    "ScenarioTable",
    "TrancheMetrics",
    "tranche_breakeven_cdr",
    "tranche_discount_margin",
    "tranche_metrics",
    "tranche_oas",
    "tranche_scenario_table",
];

/// Register the structured-credit tranche analytics on the instruments module.
pub(crate) fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    parent.add_function(wrap_pyfunction!(tranche_discount_margin, parent)?)?;
    parent.add_function(wrap_pyfunction!(tranche_breakeven_cdr, parent)?)?;
    parent.add_function(wrap_pyfunction!(tranche_oas, parent)?)?;
    parent.add_function(wrap_pyfunction!(tranche_metrics, parent)?)?;
    parent.add_function(wrap_pyfunction!(tranche_scenario_table, parent)?)?;
    parent.add_class::<PyOasResult>()?;
    parent.add_class::<PyTrancheMetrics>()?;
    parent.add_class::<PyScenarioTable>()?;
    Ok(())
}
