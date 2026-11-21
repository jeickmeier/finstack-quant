//! Structured-credit tranche analytics: DM, break-even CDR, OAS, metrics,
//! scenario table.
//!
//! These take a tranche id, so they are not reachable through
//! `price_instrument`. Signatures mirror the WASM facade. The
//! OAS, metrics, and scenario-table entry points return the typed
//! `OasResult` / `TrancheMetrics` / `ScenarioTable` wrapper classes defined
//! in this module; each wrapper's `to_json` still yields the wire payload,
//! and `from_json` decodes it back.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::extract::{extract_instrument, extract_market};
use crate::bindings::table_builders::{TableBuilder, serde_rows_to_arrow_table};
use crate::errors::{core_to_py, display_to_py, serde_json_to_py, value_error};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    self as sc, OasConfig, OasResult, ScenarioGrid, ScenarioTable, StructuredCredit, TrancheMetrics,
};
use finstack_quant_valuations::metrics::MetricId;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::collections::BTreeMap;

/// Column schema of [`PyScenarioTable`] tabular exports, kept so a grid that
/// evaluated no cells still exports the documented columns.
const SCENARIO_CELL_COLUMNS: &[(&str, &str)] = &[
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "tranche_discount_margin")]
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "tranche_breakeven_cdr")]
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "tranche_oas")]
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "tranche_metrics")]
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "tranche_scenario_table")]
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "OasResult")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "OasResult.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "OasResult.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: OasResult = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "OasResult.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "OasResult(oas={}, model_price={}, market_price={})",
            self.inner.oas, self.inner.model_price, self.inner.market_price
        )
    }
}

impl PyOasResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let num_paths = i64::try_from(self.inner.num_paths)
            .map_err(|_| value_error("num_paths overflows i64"))?;
        TableBuilder::new()
            .f64("oas", vec![self.inner.oas])?
            .f64("model_price", vec![self.inner.model_price])?
            .f64("market_price", vec![self.inner.market_price])?
            .i64("num_paths", vec![num_paths])?
            .f64("price_std_error", vec![self.inner.price_std_error])?
            .build()
    }
}

crate::bindings::macros::tabular_methods!(
    PyOasResult,
    "OasResult",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics"
);

crate::bindings::macros::py_getters!(PyOasResult |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "OasResult.oas")]
    oas -> f64 = s.inner.oas;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "OasResult.model_price")]
    model_price -> f64 = s.inner.model_price;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "OasResult.market_price")]
    market_price -> f64 = s.inner.market_price;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "OasResult.num_paths")]
    num_paths -> usize = s.inner.num_paths;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "OasResult.price_std_error")]
    price_std_error -> f64 = s.inner.price_std_error;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: TrancheMetrics = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.metrics")]
    #[getter]
    fn metrics(&self) -> BTreeMap<String, f64> {
        self.inner
            .metrics
            .iter()
            .map(|(id, value)| (id.to_string(), *value))
            .collect()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "TrancheMetrics(tranche_id={:?}, pv={}, clean_price_pct={})",
            self.inner.tranche_id, self.inner.pv, self.inner.clean_price_pct
        )
    }
}

impl PyTrancheMetrics {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .utf8("tranche_id", vec![self.inner.tranche_id.clone()])?
            .utf8("currency", vec![self.inner.currency.to_string()])?
            .f64("pv", vec![self.inner.pv])?
            .f64("clean_price_pct", vec![self.inner.clean_price_pct])?
            .f64("dirty_price_pct", vec![self.inner.dirty_price_pct])?
            .f64("accrued", vec![self.inner.accrued])?
            .f64("factor", vec![self.inner.factor])?
            .f64("wal", vec![self.inner.wal])?
            .nullable_f64("ytm", vec![self.inner.ytm])?
            .f64("z_spread_bp", vec![self.inner.z_spread_bp])?
            .f64("cs01", vec![self.inner.cs01])?
            .f64("spread_duration", vec![self.inner.spread_duration])?
            .f64("spread_convexity", vec![self.inner.spread_convexity])?
            .f64("effective_duration", vec![self.inner.effective_duration])?
            .f64("effective_convexity", vec![self.inner.effective_convexity])?
            .f64("target_price_pct", vec![self.inner.target_price_pct])?
            .nullable_f64("wal_to_call", vec![self.inner.wal_to_call])?
            .nullable_f64("z_spread_to_call_bp", vec![self.inner.z_spread_to_call_bp])?
            .nullable_f64("dm_to_call_bp", vec![self.inner.dm_to_call_bp])?
            .nullable_f64("dm_bp", vec![self.inner.dm_bp])?
            .build()
    }
}

crate::bindings::macros::tabular_methods!(
    PyTrancheMetrics,
    "TrancheMetrics",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics"
);

crate::bindings::macros::py_getters!(PyTrancheMetrics |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.tranche_id")]
    tranche_id -> &str = &s.inner.tranche_id;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.pv")]
    pv -> f64 = s.inner.pv;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.clean_price_pct")]
    clean_price_pct -> f64 = s.inner.clean_price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.dirty_price_pct")]
    dirty_price_pct -> f64 = s.inner.dirty_price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.accrued")]
    accrued -> f64 = s.inner.accrued;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.factor")]
    factor -> f64 = s.inner.factor;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.wal")]
    wal -> f64 = s.inner.wal;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.ytm")]
    ytm -> Option<f64> = s.inner.ytm;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.z_spread_bp")]
    z_spread_bp -> f64 = s.inner.z_spread_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.cs01")]
    cs01 -> f64 = s.inner.cs01;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.spread_duration")]
    spread_duration -> f64 = s.inner.spread_duration;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.spread_convexity")]
    spread_convexity -> f64 = s.inner.spread_convexity;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.effective_duration")]
    effective_duration -> f64 = s.inner.effective_duration;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.effective_convexity")]
    effective_convexity -> f64 = s.inner.effective_convexity;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.dm_bp")]
    dm_bp -> Option<f64> = s.inner.dm_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.target_price_pct")]
    target_price_pct -> f64 = s.inner.target_price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.wal_to_call")]
    wal_to_call -> Option<f64> = s.inner.wal_to_call;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.z_spread_to_call_bp")]
    z_spread_to_call_bp -> Option<f64> = s.inner.z_spread_to_call_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "TrancheMetrics.dm_to_call_bp")]
    dm_to_call_bp -> Option<f64> = s.inner.dm_to_call_bp;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "ScenarioTable")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "ScenarioTable.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "ScenarioTable.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: ScenarioTable = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "ScenarioTable.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "ScenarioTable.cells")]
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

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ScenarioTable(tranche_id={:?}, cells={})",
            self.inner.tranche_id,
            self.inner.cells.len()
        )
    }
}

impl PyScenarioTable {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
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
        serde_rows_to_arrow_table(&rows, SCENARIO_CELL_COLUMNS)
    }
}

crate::bindings::macros::tabular_methods!(
    PyScenarioTable,
    "ScenarioTable",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics"
);

crate::bindings::macros::py_getters!(PyScenarioTable |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "ScenarioTable.tranche_id")]
    tranche_id -> &str = &s.inner.tranche_id;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics", "ScenarioTable.currency")]
    currency -> String = s.inner.currency.to_string();
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
