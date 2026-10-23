//! Instrument pricing pipeline: canonical instrument envelope + market → ValuationResult.
//!
//! Also binds the two typed option payloads `price_instrument` accepts:
//! `MetricPricingOverrides` (metric-time overrides) and `MarketHistory`
//! (historical scenarios for `hvar` / `expected_shortfall`).

use super::PyValuationResult;
use crate::bindings::extract::{extract_instrument, extract_market};
use crate::bindings::module_utils::{
    opt_spec_from_py, py_to_json_string, py_to_serde, spec_from_py,
};
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, serde_to_py};
use crate::errors::{core_to_py, display_to_py, value_error};
use finstack_quant_valuations::instruments::cashflow_export::InstrumentCashflowEnvelope;
use finstack_quant_valuations::instruments::pricing_overrides::MetricPricingOverrides;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::risk::{MarketHistory, MarketScenario};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MetricPricingOverrides")]
#[pyclass(
    name = "MetricPricingOverrides",
    module = "finstack_quant.valuations.instruments",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyMetricPricingOverrides {
    pub(crate) inner: MetricPricingOverrides,
}

#[pymethods]
impl PyMetricPricingOverrides {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MetricPricingOverrides.__init__")]
    #[new]
    #[pyo3(signature = (*, bump_config=None, theta_period=None, breakeven_config=None, bond_risk_basis=None, theta_day_basis=None, var_config=None))]
    #[pyo3(
        text_signature = "(*, bump_config=None, theta_period=None, breakeven_config=None, bond_risk_basis=None, theta_day_basis=None, var_config=None)"
    )]
    fn new(
        py: Python<'_>,
        bump_config: Option<&Bound<'_, PyAny>>,
        theta_period: Option<&Bound<'_, PyAny>>,
        breakeven_config: Option<&Bound<'_, PyAny>>,
        bond_risk_basis: Option<&str>,
        theta_day_basis: Option<&str>,
        var_config: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let theta_period = match theta_period {
            Some(value) if !value.is_none() => {
                Some(crate::bindings::core::dates::tenor::extract_tenor(value)?)
            }
            _ => None,
        };
        let inner = MetricPricingOverrides {
            bump_config: opt_spec_from_py(py, bump_config, "bump_config")?.unwrap_or_default(),
            theta_period,
            breakeven_config: opt_spec_from_py(py, breakeven_config, "breakeven_config")?,
            bond_risk_basis: bond_risk_basis
                .map(|name| {
                    serde_json::from_value(serde_json::Value::String(name.to_string())).map_err(
                        |_| {
                            value_error(format!(
                                "bond_risk_basis: expected 'bullet_discountable' or 'callable_oas', got '{name}'"
                            ))
                        },
                    )
                })
                .transpose()?,
            theta_day_basis: theta_day_basis
                .map(|name| {
                    serde_json::from_value(serde_json::Value::String(name.to_string())).map_err(
                        |_| {
                            value_error(format!(
                                "theta_day_basis: expected 'calendar_365' or 'trading_252', got '{name}'"
                            ))
                        },
                    )
                })
                .transpose()?,
            var_config: opt_spec_from_py(py, var_config, "var_config")?,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MetricPricingOverrides.bump_config")]
    #[getter]
    fn bump_config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.bump_config)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MetricPricingOverrides.breakeven_config")]
    #[getter]
    fn breakeven_config<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .breakeven_config
            .as_ref()
            .map(|cfg| serde_to_py(py, cfg))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MetricPricingOverrides.bond_risk_basis")]
    #[getter]
    fn bond_risk_basis(&self) -> PyResult<Option<String>> {
        self.inner
            .bond_risk_basis
            .as_ref()
            .map(super::convert::enum_to_py_string)
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MetricPricingOverrides.theta_day_basis")]
    #[getter]
    fn theta_day_basis(&self) -> PyResult<Option<String>> {
        self.inner
            .theta_day_basis
            .as_ref()
            .map(super::convert::enum_to_py_string)
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MetricPricingOverrides.var_config")]
    #[getter]
    fn var_config<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .var_config
            .as_ref()
            .map(|cfg| serde_to_py(py, cfg))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MetricPricingOverrides.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: MetricPricingOverrides = serde_json::from_str(json).map_err(|e| {
            crate::errors::serde_json_to_py(e, "invalid MetricPricingOverrides JSON")
        })?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MetricPricingOverrides.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MetricPricingOverrides.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> PyResult<String> {
        let quoted = |value: &Option<String>| match value {
            Some(s) => format!("'{s}'"),
            None => "None".to_string(),
        };
        let json_or_none = |value: Option<serde_json::Value>| match value {
            Some(v) => v.to_string(),
            None => "None".to_string(),
        };
        let bump = if self.inner.bump_config.is_empty() {
            "None".to_string()
        } else {
            serde_json::to_value(&self.inner.bump_config)
                .map_err(display_to_py)?
                .to_string()
        };
        Ok(format!(
            "MetricPricingOverrides(bump_config={}, theta_period={}, breakeven_config={}, bond_risk_basis={}, theta_day_basis={}, var_config={})",
            bump,
            quoted(&self.inner.theta_period.map(|period| period.to_string())),
            json_or_none(
                self.inner
                    .breakeven_config
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(display_to_py)?
            ),
            quoted(&self.bond_risk_basis()?),
            quoted(&self.theta_day_basis()?),
            json_or_none(
                self.inner
                    .var_config
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(display_to_py)?
            ),
        ))
    }
}

crate::bindings::macros::py_getters!(PyMetricPricingOverrides |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MetricPricingOverrides.theta_period")]
    theta_period -> Option<crate::bindings::core::dates::tenor::PyTenor> =
        s.inner.theta_period.map(crate::bindings::core::dates::tenor::PyTenor::from_inner);
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MarketHistory")]
#[pyclass(
    name = "MarketHistory",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMarketHistory {
    pub(crate) inner: MarketHistory,
}

#[pymethods]
impl PyMarketHistory {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MarketHistory.__init__")]
    #[new]
    #[pyo3(text_signature = "(base_date, window_days, scenarios)")]
    fn new(
        py: Python<'_>,
        base_date: &Bound<'_, PyAny>,
        window_days: u32,
        scenarios: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let base_date = crate::bindings::date_utils::py_to_date(base_date)?;
        let scenarios: Vec<MarketScenario> = py_to_serde(py, scenarios, "scenarios")?;
        Ok(Self {
            inner: MarketHistory::new(base_date, window_days, scenarios),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MarketHistory.from_dict")]
    #[staticmethod]
    #[pyo3(text_signature = "(data)")]
    fn from_dict(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: py_to_serde(py, data, "MarketHistory")?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MarketHistory.base_date")]
    #[getter]
    fn base_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.base_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MarketHistory.scenarios")]
    #[getter]
    fn scenarios<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.scenarios)
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MarketHistory.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::table::envelope_to_pandas(
            py,
            &self.inner.to_table().map_err(crate::errors::core_to_py)?,
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MarketHistory.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid MarketHistory JSON"))?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MarketHistory.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MarketHistory.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "MarketHistory(base_date={}, window_days={}, scenarios=<{} items>)",
            self.inner.base_date,
            self.inner.window_days,
            self.inner.scenarios.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyMarketHistory |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MarketHistory.window_days")]
    window_days -> u32 = s.inner.window_days;
});

/// Coerce ``MetricPricingOverrides | dict | str | None`` into the JSON the
/// Rust pricing entry points accept.
///
/// The single conversion path for the ``metric_pricing_overrides`` keyword of
/// ``price_instrument``, every typed ``.price()`` and
/// ``validate_instrument_json``.
///
/// # Arguments
///
/// * `py` - GIL token used for ``json.dumps`` on dict inputs.
/// * `obj` - ``None``, a typed ``MetricPricingOverrides``, a JSON string, or a
///   dict of override fields.
pub(crate) fn metric_pricing_overrides_json(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<String>> {
    let Some(obj) = obj else {
        return Ok(None);
    };
    if obj.is_none() {
        return Ok(None);
    }
    if let Ok(typed) = obj.cast::<PyMetricPricingOverrides>() {
        return typed.borrow().to_json().map(Some);
    }
    if let Ok(text) = obj.cast::<PyString>() {
        return Ok(Some(text.to_str()?.to_owned()));
    }
    py_to_json_string(py, obj, "metric_pricing_overrides").map(Some)
}

/// Coerce ``MetricPricingOverrides | dict | str`` into the Rust struct an
/// instrument builder stores.
///
/// # Arguments
///
/// * `py` - GIL token used for ``json.dumps`` on dict inputs.
/// * `obj` - A typed ``MetricPricingOverrides``, a JSON string, or a dict of
///   override fields; unknown fields are rejected.
pub(crate) fn metric_pricing_overrides_from_py(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<MetricPricingOverrides> {
    if let Ok(typed) = obj.cast::<PyMetricPricingOverrides>() {
        return Ok(typed.borrow().inner.clone());
    }
    spec_from_py(py, obj, "metric_pricing_overrides")
}

/// Coerce ``dict | str | MarketHistory | None`` into the JSON the Rust
/// pricing entry point accepts.
pub(crate) fn market_history_json(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<String>> {
    let Some(obj) = obj else {
        return Ok(None);
    };
    if obj.is_none() {
        return Ok(None);
    }
    if let Ok(typed) = obj.cast::<PyMarketHistory>() {
        return typed.borrow().to_json().map(Some);
    }
    if let Ok(text) = obj.cast::<PyString>() {
        return Ok(Some(text.to_str()?.to_owned()));
    }
    py_to_json_string(py, obj, "market_history").map(Some)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "price_instrument")]
#[pyfunction]
#[pyo3(signature = (instrument, market, as_of, model="default", metrics=None, metric_pricing_overrides=None, market_history=None, explain=false))]
#[pyo3(
    text_signature = "(instrument, market, as_of, model='default', metrics=None, metric_pricing_overrides=None, market_history=None, explain=False)"
)]
// PyO3 binding: the argument list mirrors the Python keyword-argument API, so
// it cannot be collapsed into a parameter struct without changing that API.
#[allow(clippy::too_many_arguments)]
fn price_instrument(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
    metrics: Option<Vec<String>>,
    metric_pricing_overrides: Option<&Bound<'_, PyAny>>,
    market_history: Option<&Bound<'_, PyAny>>,
    explain: bool,
) -> PyResult<PyValuationResult> {
    let overrides = metric_pricing_overrides_json(py, metric_pricing_overrides)?;
    let instrument = extract_instrument(instrument)?;
    let prepared = py
        .detach(move || instrument.into_parsed(overrides.as_deref()))
        .map_err(core_to_py)?;
    price_prepared(
        py,
        prepared,
        market,
        as_of,
        model,
        metrics,
        market_history,
        explain,
    )
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "list_standard_metrics")]
#[pyfunction]
fn list_standard_metrics() -> Vec<String> {
    finstack_quant_valuations::pricer::list_standard_metrics()
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "list_standard_metrics_grouped")]
#[pyfunction]
fn list_standard_metrics_grouped() -> std::collections::BTreeMap<String, Vec<String>> {
    finstack_quant_valuations::pricer::list_standard_metrics_grouped()
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "metric_metadata")]
#[pyfunction]
#[pyo3(text_signature = "(keys)")]
fn metric_metadata<'py>(py: Python<'py>, keys: Vec<String>) -> PyResult<Bound<'py, PyAny>> {
    let metadata = finstack_quant_valuations::pricer::metric_metadata(&keys).map_err(core_to_py)?;
    serde_to_py(py, &metadata)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "list_models")]
#[pyfunction]
fn list_models() -> Vec<String> {
    finstack_quant_valuations::pricer::list_models()
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "list_models_grouped")]
#[pyfunction]
fn list_models_grouped() -> std::collections::BTreeMap<String, Vec<String>> {
    finstack_quant_valuations::pricer::list_models_grouped()
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "instrument_cashflows")]
#[pyfunction]
#[pyo3(text_signature = "(instrument, market, as_of, model)")]
fn instrument_cashflows(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
) -> PyResult<PyInstrumentCashflowEnvelope> {
    // Parse the instrument before the market so instrument errors surface first.
    let instrument = extract_instrument(instrument)?;
    let instrument = py
        .detach(move || instrument.into_parsed(None))
        .map_err(core_to_py)?;
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    py.detach(move || {
        finstack_quant_valuations::instruments::cashflow_export::instrument_cashflows(
            instrument.as_instrument(),
            &market,
            &as_of,
            &model,
        )
        .map_err(core_to_py)
    })
    .map(|inner| PyInstrumentCashflowEnvelope { inner })
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope")]
#[pyclass(
    name = "InstrumentCashflowEnvelope",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyInstrumentCashflowEnvelope {
    pub(crate) inner: InstrumentCashflowEnvelope,
}

/// Documented column order of ``InstrumentCashflowEnvelope.to_dataframe``:
/// every ``CashflowRow`` field, in struct order, with its empty-frame dtype.
const CASHFLOW_ROW_COLUMNS: &[(&str, &str)] = &[
    ("date", "str"),
    ("amount", "float64"),
    ("currency", "str"),
    ("kind", "str"),
    ("accrual_factor", "float64"),
    ("accrual_start", "str"),
    ("accrual_end", "str"),
    ("accrual_day_count", "str"),
    ("accrual_notional", "float64"),
    ("index_rate", "float64"),
    ("principal_delta", "float64"),
    ("year_fraction", "float64"),
    ("rate", "float64"),
    ("reset_date", "str"),
    ("discount_factor", "float64"),
    ("discount_curve_id", "str"),
    ("survival_probability", "float64"),
    ("conditional_default_prob", "float64"),
    ("inflation_index_ratio", "float64"),
    ("prepayment_smm", "float64"),
    ("beginning_balance", "float64"),
    ("ending_balance", "float64"),
    ("native_pv", "float64"),
    ("fx_rate", "float64"),
    ("pv", "float64"),
];

#[pymethods]
impl PyInstrumentCashflowEnvelope {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.as_of")]
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.as_of)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.flows")]
    #[getter]
    fn flows<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.flows)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let frame =
            serde_rows_to_dataframe_with_schema(py, &self.inner.flows, CASHFLOW_ROW_COLUMNS)?;
        let to_datetime = py.import("pandas")?.getattr("to_datetime")?;
        let parsed = pyo3::types::PyDict::new(py);
        for column in ["date", "reset_date", "accrual_start", "accrual_end"] {
            parsed.set_item(column, to_datetime.call1((frame.get_item(column)?,))?)?;
        }
        frame.call_method("assign", (), Some(&parsed))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json).map_err(|e| {
                crate::errors::serde_json_to_py(e, "invalid InstrumentCashflowEnvelope JSON")
            })?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "InstrumentCashflowEnvelope(instrument_id={:?}, model={:?}, flows=<{} rows>, total_pv={})",
            self.inner.instrument_id,
            self.inner.model,
            self.inner.flows.len(),
            self.inner.total_pv
        )
    }
}

crate::bindings::macros::py_getters!(PyInstrumentCashflowEnvelope |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.instrument_id")]
    instrument_id -> String = s.inner.instrument_id.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.model")]
    model -> String = s.inner.model.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.credit_curve_id")]
    credit_curve_id -> Option<String> = s.inner.credit_curve_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.recovery_rate")]
    recovery_rate -> Option<f64> = s.inner.recovery_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.scenario_price_shock_decimal")]
    scenario_price_shock_decimal -> Option<f64> = s.inner.scenario_price_shock_decimal;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.total_pv")]
    total_pv -> f64 = s.inner.total_pv;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "InstrumentCashflowEnvelope.reconciles_with_base_value")]
    reconciles_with_base_value -> bool = s.inner.reconciles_with_base_value;
});

/// Register pricing functions on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMetricPricingOverrides>()?;
    m.add_class::<PyMarketHistory>()?;
    m.add_class::<PyInstrumentCashflowEnvelope>()?;
    m.add_function(pyo3::wrap_pyfunction!(instrument_cashflows, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(price_instrument, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_models, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_models_grouped, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_standard_metrics, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_standard_metrics_grouped, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(metric_metadata, m)?)?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &[
    "InstrumentCashflowEnvelope",
    "MarketHistory",
    "MetricPricingOverrides",
    "instrument_cashflows",
    "list_models",
    "list_models_grouped",
    "list_standard_metrics",
    "list_standard_metrics_grouped",
    "metric_metadata",
    "price_instrument",
];

/// Price an instrument envelope through the canonical Rust pricer.
///
/// Shared by `price_instrument` and every typed wrapper's `.price()`, so all
/// Python pricing entry points coerce their options identically.
///
/// # Arguments
///
/// * `py` - GIL token; parsing and pricing run with the GIL released.
/// * `instrument` - Owned native Rust instrument, validated before pricing.
/// * `market` - `MarketContext` object or market-context JSON string.
/// * `as_of` - Valuation date (date-like or ISO 8601 string).
/// * `model` - Model key (`"default"` selects the instrument-native model).
/// * `metrics` - Metric identifiers to compute; `None` means valuation only.
/// * `metric_pricing_overrides` - Optional `MetricPricingOverrides | dict |
///   str` merged into `instrument.spec.metric_pricing_overrides` before
///   validation; `None` keeps the instrument's own overrides.
/// * `market_history` - Optional `MarketHistory | dict | str` scenarios for
///   the historical risk metrics (`hvar`, `expected_shortfall`).
// PyO3 binding helper: mirrors the keyword surface of `price_instrument`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn price_typed(
    py: Python<'_>,
    instrument: Box<dyn Instrument>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
    metrics: Option<Vec<String>>,
    metric_pricing_overrides: Option<&Bound<'_, PyAny>>,
    market_history: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyValuationResult> {
    let overrides = metric_pricing_overrides_json(py, metric_pricing_overrides)?;
    let instrument = py.detach(move || {
        finstack_quant_valuations::pricer::ParsedInstrument::from_instrument(
            instrument,
            overrides.as_deref(),
        )
        .map_err(core_to_py)
    })?;
    price_prepared(
        py,
        instrument,
        market,
        as_of,
        model,
        metrics,
        market_history,
        false,
    )
}
/// Pricing options shared by the host entry points: the recalibration
/// provider, plus a step trace on the result when `explain` is set.
fn pricing_options(explain: bool) -> finstack_quant_valuations::instruments::PricingOptions {
    let options = finstack_quant_calibration::recalibration::pricing_options();
    if explain {
        options.with_explain(finstack_quant_core::explain::ExplainOpts::enabled())
    } else {
        options
    }
}

/// Price a prepared instrument after host market/date conversion.
#[allow(clippy::too_many_arguments)]
pub(crate) fn price_prepared(
    py: Python<'_>,
    instrument: finstack_quant_valuations::pricer::ParsedInstrument,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
    metrics: Option<Vec<String>>,
    market_history: Option<&Bound<'_, PyAny>>,
    explain: bool,
) -> PyResult<PyValuationResult> {
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    let metrics = metrics.unwrap_or_default();
    let market_history = market_history_json(py, market_history)?;
    let inner = py
        .detach(move || {
            finstack_quant_valuations::pricer::price_instrument(
                &instrument,
                &market,
                &as_of,
                &model,
                &metrics,
                market_history.as_deref(),
                pricing_options(explain),
            )
        })
        .map_err(core_to_py)?;
    Ok(PyValuationResult { inner })
}

/// Compute one scalar metric for a native Rust instrument.
pub(crate) fn metric_typed(
    py: Python<'_>,
    instrument: Box<dyn Instrument>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    metric_id: &str,
    model: &str,
) -> PyResult<f64> {
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    let metric_id = metric_id.to_owned();
    py.detach(move || {
        let instrument =
            finstack_quant_valuations::pricer::ParsedInstrument::from_instrument(instrument, None)?;
        finstack_quant_valuations::pricer::metric_value(
            &instrument,
            &market,
            &as_of,
            &model,
            &metric_id,
            finstack_quant_calibration::recalibration::pricing_options(),
        )
    })
    .map_err(core_to_py)
}

/// Compute the standard option Greek set for a native Rust instrument.
///
/// Mirrors the WASM `greeks` method: non-finite Greeks are rejected rather
/// than returned, so both hosts fail identically instead of one silently
/// yielding `NaN`.
///
/// # Arguments
///
/// * `py` - GIL token; the pricer runs with the GIL released.
/// * `instrument` - Owned native Rust instrument, validated before pricing.
/// * `market` - `MarketContext` object or market-context JSON string.
/// * `as_of` - Valuation date (date-like or ISO string).
/// * `model` - Model key (`"default"` selects the instrument-native model).
pub(crate) fn typed_option_greeks<'py>(
    py: Python<'py>,
    instrument: Box<dyn finstack_quant_valuations::instruments::Instrument>,
    market: &Bound<'py, PyAny>,
    as_of: &Bound<'py, PyAny>,
    model: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    let pairs = py
        .detach(move || {
            let instrument = finstack_quant_valuations::pricer::ParsedInstrument::from_instrument(
                instrument, None,
            )?;
            finstack_quant_valuations::pricer::present_standard_option_greeks(
                &instrument,
                &market,
                &as_of,
                &model,
                finstack_quant_calibration::recalibration::pricing_options(),
            )
        })
        .map_err(core_to_py)?;
    let out = PyDict::new(py);
    for (metric, value) in pairs {
        out.set_item(metric, value)?;
    }
    Ok(out)
}
