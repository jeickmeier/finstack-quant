//! Python wrappers for scenario engine application.

use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::extract::{extract_market, extract_model_ref};
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    serde_to_py,
};
use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::errors::{display_to_py, scenarios_to_py};
use finstack_quant_scenarios::engine::{ApplicationEnvelope, ApplicationReport};
use finstack_quant_valuations::instruments::Instrument;
use pyo3::prelude::*;
use pyo3::types::PyAny;

use super::extract::{extract_config, extract_instruments};

/// Report describing what a scenario application changed.
///
/// Returned as the ``report`` attribute of an ``ApplicationResult`` from
/// ``apply_scenario`` and as
/// ``HorizonResult.scenario_report``.
///
/// ``warnings`` is a list of structured dicts, each carrying a ``kind``
/// discriminator (``"equity_not_found"``, ``"discount_curve_heuristic"``, ...)
/// plus variant-specific fields.
#[pyclass(
    name = "ApplicationReport",
    module = "finstack_quant.scenarios",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyApplicationReport {
    pub(crate) inner: ApplicationReport,
}

impl PyApplicationReport {
    pub(crate) fn from_inner(inner: ApplicationReport) -> Self {
        Self { inner }
    }
}

const REPORT_COLUMNS: [&str; 6] = [
    "operations_applied",
    "user_operations",
    "expanded_operations",
    "warning_count",
    "as_of_changed",
    "all_dirty",
];

#[pymethods]
impl PyApplicationReport {
    /// Number of effects successfully applied to the execution context.
    ///
    /// One user-level operation can produce zero, one, or many effects. Inspect
    /// ``changes`` and ``warnings`` rather than treating this as coverage.
    #[getter]
    fn operations_applied(&self) -> usize {
        self.inner.operations_applied
    }

    /// Number of user-provided operations before hierarchy expansion.
    #[getter]
    fn user_operations(&self) -> usize {
        self.inner.user_operations
    }

    /// Number of operations the engine attempted after hierarchy expansion.
    #[getter]
    fn expanded_operations(&self) -> usize {
        self.inner.expanded_operations
    }

    /// Non-fatal warnings raised while applying the scenario, as structured
    /// dicts with a ``kind`` discriminator plus variant-specific fields.
    #[getter]
    fn warnings<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.warnings)
    }

    /// Number of warnings raised while applying the scenario.
    #[getter]
    fn warning_count(&self) -> usize {
        self.inner.warnings.len()
    }

    /// Audit stamp: numeric mode, rounding context, and FX policy in force.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .meta
            .as_ref()
            .map(|meta| serde_to_py(py, meta))
            .transpose()
    }

    /// Metadata describing exactly which market state the effects changed.
    #[getter]
    fn changes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.changes)
    }

    /// Roll-forward report, present only when the scenario contained a
    /// ``time_roll_forward`` operation.
    #[getter]
    fn time_roll<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .time_roll
            .as_ref()
            .map(|roll| serde_to_py(py, roll))
            .transpose()
    }

    /// Export the report counters as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``operations_applied``, ``user_operations``,
    /// ``expanded_operations``, ``warning_count``, ``as_of_changed``,
    /// ``all_dirty``. Use ``changes_to_dataframe()`` for the per-target
    /// change manifest and ``carry_to_dataframe()`` for time-roll carry.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "operations_applied": self.inner.operations_applied,
            "user_operations": self.inner.user_operations,
            "expanded_operations": self.inner.expanded_operations,
            "warning_count": self.inner.warnings.len(),
            "as_of_changed": self.inner.changes.as_of_changed,
            "all_dirty": self.inner.changes.all_dirty,
        });
        serde_object_to_single_row_dataframe_with_schema(py, &row, &REPORT_COLUMNS)
    }

    /// Export the market targets the scenario actually changed, one row per
    /// target.
    ///
    /// Columns: ``kind`` (``curve``, ``volatility_index``,
    /// ``base_correlation``, ``vol_surface``, ``equity_price``, ``fx``),
    /// ``id`` (curve / surface / price identifier, or ``BASE/QUOTE`` for FX)
    /// and ``curve_kind`` (curve family for ``curve`` rows, else ``None``).
    /// Empty when nothing changed.
    fn changes_to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // `kind` and `curve_kind` are the target's own serde fields; only the
        // identifier needs the Rust display form (FX pairs have two fields).
        let rows = self
            .inner
            .changes
            .market_targets
            .iter()
            .map(|target| {
                let wire = serde_json::to_value(target).map_err(display_to_py)?;
                Ok(serde_json::json!({
                    "kind": wire["kind"],
                    "id": target.id_label(),
                    "curve_kind": wire["curve_kind"],
                }))
            })
            .collect::<PyResult<Vec<serde_json::Value>>>()?;
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[("kind", "str"), ("id", "str"), ("curve_kind", "str")],
        )
    }

    /// Export per-instrument carry from the time roll, one row per
    /// instrument and currency.
    ///
    /// Columns: ``instrument_id``, ``amount`` (carry P&L as a float),
    /// ``currency``. Empty when the scenario had no ``time_roll_forward`` or
    /// no instruments were supplied.
    fn carry_to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut rows = Vec::new();
        if let Some(roll) = &self.inner.time_roll {
            for (instrument_id, by_currency) in &roll.instrument_carry {
                for (currency, money) in by_currency {
                    rows.push(serde_json::json!({
                        "instrument_id": instrument_id,
                        "amount": money.amount(),
                        "currency": currency.to_string(),
                    }));
                }
            }
        }
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[
                ("instrument_id", "str"),
                ("amount", "float64"),
                ("currency", "str"),
            ],
        )
    }

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: ApplicationReport = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "ApplicationReport(operations_applied={}, user_operations={}, expanded_operations={}, warnings={})",
            self.inner.operations_applied,
            self.inner.user_operations,
            self.inner.expanded_operations,
            self.inner.warnings.len(),
        )
    }
}

/// Result of applying a scenario: the mutated market, the mutated model (when
/// one was supplied), and the application report.
///
/// The input objects are copied. ``instruments`` returns the shocked copies as
/// canonical envelope JSON strings, accepted by subsequent scenario calls and
/// instrument ``from_json`` constructors.
#[pyclass(
    name = "ApplicationResult",
    module = "finstack_quant.scenarios",
    frozen,
    skip_from_py_object
)]
pub struct PyApplicationResult {
    market: finstack_quant_core::market_data::context::MarketContext,
    model: Option<finstack_quant_statements::FinancialModelSpec>,
    report: ApplicationReport,
    instruments: Option<Vec<Box<dyn Instrument>>>,
}

#[pymethods]
impl PyApplicationResult {
    /// The mutated market context.
    #[getter]
    fn market(&self) -> PyMarketContext {
        PyMarketContext::from_inner(self.market.clone())
    }

    /// The mutated financial model, or ``None`` when no model was supplied.
    #[getter]
    fn model(&self) -> Option<PyFinancialModelSpec> {
        self.model.as_ref().map(|inner| PyFinancialModelSpec {
            inner: inner.clone(),
        })
    }

    /// Shocked instrument copies as canonical envelope JSON strings in input
    /// order, or ``None`` when no inventory was supplied. Raises ``ValueError``
    /// if an instrument cannot be serialized.
    #[getter]
    fn instruments(&self) -> PyResult<Option<Vec<String>>> {
        self.instruments
            .as_ref()
            .map(|inventory| {
                finstack_quant_scenarios::engine::instrument_envelopes(inventory)
                    .map_err(display_to_py)?
                    .iter()
                    .map(|envelope| serde_json::to_string(envelope).map_err(display_to_py))
                    .collect()
            })
            .transpose()
    }

    /// What the scenario changed.
    #[getter]
    fn report(&self) -> PyApplicationReport {
        PyApplicationReport::from_inner(self.report.clone())
    }

    /// Export the application report counters as a single-row pandas
    /// ``DataFrame`` (same columns as ``ApplicationReport.to_dataframe``).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        PyApplicationReport::from_inner(self.report.clone()).to_dataframe(py)
    }

    /// Serialize to a compact JSON string.
    ///
    /// Emits the canonical ``ApplicationEnvelope`` shape, with ``market`` and
    /// ``model`` and the canonical ``report`` as nested objects.
    fn to_json(&self) -> PyResult<String> {
        let envelope = ApplicationEnvelope::from_contexts(
            self.report.clone(),
            &self.market,
            self.model.as_ref(),
            self.instruments.as_deref(),
        )
        .map_err(display_to_py)?;
        serde_json::to_string(&envelope).map_err(display_to_py)
    }

    /// Deserialize from a JSON string produced by ``to_json``.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let envelope: ApplicationEnvelope = serde_json::from_str(json).map_err(display_to_py)?;
        let ApplicationEnvelope {
            market,
            model,
            instruments,
            report,
        } = envelope;
        let instruments = instruments
            .map(|items| {
                items
                    .into_iter()
                    .map(|envelope| envelope.into_boxed().map_err(crate::errors::core_to_py))
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;
        let market: finstack_quant_core::market_data::context::MarketContext =
            serde_json::from_value(market).map_err(display_to_py)?;
        let model = model
            .map(finstack_quant_statements::FinancialModelSpec::from_value)
            .transpose()
            .map_err(crate::errors::statements_to_py)?;
        Ok(Self {
            market,
            model,
            report,
            instruments,
        })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "ApplicationResult(operations_applied={}, user_operations={}, expanded_operations={}, warnings={}, model={})",
            self.report.operations_applied,
            self.report.user_operations,
            self.report.expanded_operations,
            self.report.warnings.len(),
            if self.model.is_some() { "True" } else { "False" },
        )
    }
}

/// Apply a scenario to copied market data and an optional financial model.
///
/// Parameters
/// ----------
/// scenario : ScenarioSpec | str
///     Typed scenario or JSON-serialized ``ScenarioSpec``.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string. Never mutated; the result
///     carries a modified copy.
/// model : FinancialModelSpec | str | None, default None
///     A ``FinancialModelSpec`` object or JSON string; omit for market-only execution.
/// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Valuation date (ISO 8601 accepted).
/// instruments : list[Instrument | str] | None, default None
///     Typed instruments (``Bond``, ``CreditDefaultSwap``, ...) or canonical
///     instrument-envelope JSON strings. Required when the scenario contains
///     instrument-scoped operations; also used for carry when the scenario
///     contains ``time_roll_forward``. Shocked copies are returned in ``ApplicationResult.instruments``.
/// config : FinstackConfig | str | None, default None
///     Library configuration (rounding policy stamped into ``report.meta``).
///     ``None`` uses the library default.
///
/// Returns
/// -------
/// ApplicationResult
///     Typed result exposing ``market``, ``model``, ``instruments`` and ``report``.
///
/// Raises
/// ------
/// TypeError
///     If scenario is neither a ScenarioSpec nor canonical JSON.
/// ValueError
///     If any input fails to parse or validate, or the scenario contains
///     instrument-scoped operations and ``instruments`` is ``None`` (the Rust
///     engine rejects it after validating the scenario).
/// KeyError
///     If the scenario references market data, statement nodes, tenors or
///     instruments that do not exist.
/// RuntimeError
///     If the engine fails internally.
#[pyfunction]
#[pyo3(signature = (scenario, market, as_of, model=None, instruments=None, config=None))]
fn apply_scenario(
    py: Python<'_>,
    scenario: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: Option<&Bound<'_, PyAny>>,
    instruments: Option<Vec<Bound<'_, PyAny>>>,
    config: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyApplicationResult> {
    let spec = crate::bindings::extract::extract_scenario_spec(py, scenario)?;
    let mut market = extract_market(py, market)?;
    let mut model = model
        .map(|value| extract_model_ref(value).map(|model| model.into_owned()))
        .transpose()?;
    let date = crate::bindings::date_utils::py_to_date(as_of)?;
    let mut instruments = extract_instruments(instruments)?;
    let config = extract_config(config)?;

    // Release the GIL for scenario application: shifts + re-pricing can run for seconds.
    let report = py.detach(|| {
        let mut ctx = finstack_quant_scenarios::ExecutionContext {
            market: &mut market,
            model: model.as_mut(),
            instruments: instruments.as_mut(),
            rate_bindings: None,
            calendar: None,
            as_of: date,
        };
        finstack_quant_scenarios::ScenarioEngine::with_config(config).apply(&spec, &mut ctx)
    });
    let report = report.map_err(scenarios_to_py)?;

    Ok(PyApplicationResult {
        market,
        model,
        report,
        instruments,
    })
}

/// Register engine functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyApplicationReport>()?;
    m.add_class::<PyApplicationResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(apply_scenario, m)?)?;
    Ok(())
}
