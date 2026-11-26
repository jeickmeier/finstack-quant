//! Instrument pricing pipeline: canonical instrument envelope + market → ValuationResult.
//!
//! Also binds the two typed option payloads `price_instrument` accepts:
//! `MetricPricingOverrides` (metric-time overrides) and `MarketHistory`
//! (historical scenarios for `hvar` / `expected_shortfall`).

use super::PyValuationResult;
use crate::bindings::core::config::PyFinstackConfig;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::extract::{extract_instrument, extract_market};
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::module_utils::{
    opt_spec_from_py, py_to_json_string, py_to_serde, spec_from_py,
};
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::{core_to_py, display_to_py, value_error};
use finstack_quant_valuations::instruments::cashflow_export::InstrumentCashflowEnvelope;
use finstack_quant_valuations::instruments::pricing_overrides::MetricPricingOverrides;
use finstack_quant_valuations::metrics::risk::{MarketHistory, MarketScenario};
use finstack_quant_valuations::pricer::PriceExtras;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MetricPricingOverrides")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MetricPricingOverrides.__init__")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MetricPricingOverrides.bump_config")]
    #[getter]
    fn bump_config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.bump_config)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MetricPricingOverrides.breakeven_config")]
    #[getter]
    fn breakeven_config<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .breakeven_config
            .as_ref()
            .map(|cfg| serde_to_py(py, cfg))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MetricPricingOverrides.bond_risk_basis")]
    #[getter]
    fn bond_risk_basis(&self) -> PyResult<Option<String>> {
        self.inner
            .bond_risk_basis
            .as_ref()
            .map(super::convert::enum_to_py_string)
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MetricPricingOverrides.theta_day_basis")]
    #[getter]
    fn theta_day_basis(&self) -> PyResult<Option<String>> {
        self.inner
            .theta_day_basis
            .as_ref()
            .map(super::convert::enum_to_py_string)
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MetricPricingOverrides.var_config")]
    #[getter]
    fn var_config<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .var_config
            .as_ref()
            .map(|cfg| serde_to_py(py, cfg))
            .transpose()
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MetricPricingOverrides.theta_period")]
    theta_period -> Option<crate::bindings::core::dates::tenor::PyTenor> =
        s.inner.theta_period.map(crate::bindings::core::dates::tenor::PyTenor::from_inner);
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MarketHistory")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MarketHistory.__init__")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MarketHistory.from_dict")]
    #[staticmethod]
    #[pyo3(text_signature = "(data)")]
    fn from_dict(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: py_to_serde(py, data, "MarketHistory")?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MarketHistory.base_date")]
    #[getter]
    fn base_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.base_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MarketHistory.scenarios")]
    #[getter]
    fn scenarios<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.scenarios)
    }

    fn __len__(&self) -> usize {
        self.inner.len()
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

crate::bindings::macros::wire_methods!(
    PyMetricPricingOverrides,
    MetricPricingOverrides,
    "MetricPricingOverrides",
    module = "finstack_quant.valuations.instruments._pricing",
    validate
);

crate::bindings::macros::wire_methods!(
    PyMarketHistory,
    MarketHistory,
    "MarketHistory",
    module = "finstack_quant.valuations.instruments._pricing"
);

crate::bindings::macros::py_getters!(PyMarketHistory |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "MarketHistory.window_days")]
    window_days -> u32 = s.inner.window_days;
});

impl PyMarketHistory {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let table = finstack_quant_valuations::metrics::risk::market_history_to_table(&self.inner)
            .map_err(crate::errors::core_to_py)?;
        PyArrowTable::from_envelope(&table)
    }
}

crate::bindings::macros::tabular_methods!(
    PyMarketHistory,
    "MarketHistory",
    module = "finstack_quant.valuations.instruments._pricing"
);

/// Coerce ``MetricPricingOverrides | dict | str | None`` into the JSON patch
/// ``validate_instrument_json`` merges before validation.
///
/// The pricing entry points read the same patch from the
/// ``metric_pricing_overrides`` key of their ``extras`` object instead
/// (see [`price_extras_from_py`]).
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "PriceExtras")]
#[pyclass(
    name = "PriceExtras",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
struct PyPriceExtras {
    inner: PriceExtras,
}

#[pymethods]
impl PyPriceExtras {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "PriceExtras.__init__")]
    #[new]
    #[pyo3(signature = (*, metric_pricing_overrides=None, instrument_pricing_overrides=None, market_history=None, explain=false, config=None))]
    fn new(
        py: Python<'_>,
        metric_pricing_overrides: Option<&Bound<'_, PyAny>>,
        instrument_pricing_overrides: Option<&Bound<'_, PyDict>>,
        market_history: Option<&PyMarketHistory>,
        explain: bool,
        config: Option<PyRef<'_, PyFinstackConfig>>,
    ) -> PyResult<Self> {
        let metric_pricing_overrides = metric_pricing_overrides
            .map(|value| {
                py_to_serde::<serde_json::Map<String, serde_json::Value>>(
                    py,
                    value,
                    "metric_pricing_overrides",
                )
            })
            .transpose()?
            .map(serde_json::Value::Object);
        let instrument_pricing_overrides = instrument_pricing_overrides
            .map(|value| py_to_serde(py, value.as_any(), "instrument_pricing_overrides"))
            .transpose()?;
        Ok(Self {
            inner: PriceExtras {
                metric_pricing_overrides,
                instrument_pricing_overrides,
                market_history: market_history.map(|history| history.inner.clone()),
                explain,
                config: config.map(|config| config.inner.clone()),
            },
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "PriceExtras.config")]
    #[getter]
    fn config(&self) -> Option<PyFinstackConfig> {
        self.inner
            .config
            .clone()
            .map(|inner| PyFinstackConfig { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "PriceExtras.explain")]
    #[getter]
    fn explain(&self) -> bool {
        self.inner.explain
    }
}

/// Read the optional ``extras`` argument of the pricing entry points.
///
/// Accepts ``None`` (no extras), a typed ``PriceExtras``, a dict or JSON text in the shape of the Rust
/// `PriceExtras`; typed ``MetricPricingOverrides`` / ``MarketHistory``
/// wrappers inside a dict serialize through the shared ``json.dumps`` hook.
///
/// # Arguments
///
/// * `py` - GIL token used for ``json.dumps`` on dict inputs.
/// * `extras` - The caller's ``extras`` argument; unknown keys are rejected
///   with ``ValueError``.
pub(crate) fn price_extras_from_py(
    py: Python<'_>,
    extras: Option<&Bound<'_, PyAny>>,
) -> PyResult<PriceExtras> {
    if let Some(value) = extras
        && let Ok(typed) = value.cast::<PyPriceExtras>()
    {
        return Ok(typed.borrow().inner.clone());
    }
    Ok(opt_spec_from_py(py, extras, "PriceExtras")?.unwrap_or_default())
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "price_instrument")]
#[pyfunction]
#[pyo3(signature = (instrument, market, as_of, model="default", metrics=None, extras=None))]
#[pyo3(text_signature = "(instrument, market, as_of, model='default', metrics=None, extras=None)")]
fn price_instrument(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
    metrics: Option<Vec<String>>,
    extras: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyValuationResult> {
    let extras = price_extras_from_py(py, extras)?;
    let overrides = extras.metric_pricing_overrides_json();
    let instrument = extract_instrument(instrument)?;
    let prepared = py
        .detach(|| {
            instrument.into_parsed(
                overrides.as_deref(),
                extras.instrument_pricing_overrides.as_ref(),
            )
        })
        .map_err(core_to_py)?;
    price_prepared(py, prepared, market, as_of, model, metrics, extras)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "list_standard_metrics")]
#[pyfunction]
fn list_standard_metrics() -> Vec<String> {
    finstack_quant_valuations::pricer::list_standard_metrics()
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "list_standard_metrics_grouped")]
#[pyfunction]
fn list_standard_metrics_grouped() -> std::collections::BTreeMap<String, Vec<String>> {
    finstack_quant_valuations::pricer::list_standard_metrics_grouped()
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "metrics_for_instrument")]
#[pyfunction]
#[pyo3(text_signature = "(instrument_type)")]
fn metrics_for_instrument(
    instrument_type: &str,
) -> PyResult<std::collections::BTreeMap<String, Vec<String>>> {
    finstack_quant_valuations::pricer::metrics_for_instrument(instrument_type).map_err(core_to_py)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "metric_metadata")]
#[pyfunction]
#[pyo3(text_signature = "(keys)")]
fn metric_metadata<'py>(py: Python<'py>, keys: Vec<String>) -> PyResult<Bound<'py, PyAny>> {
    let metadata = finstack_quant_valuations::pricer::metric_metadata(&keys).map_err(core_to_py)?;
    serde_to_py(py, &metadata)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "price_driver_fields")]
#[pyfunction]
fn price_driver_fields() -> Vec<&'static str> {
    finstack_quant_valuations::instruments::pricing_overrides::MarketQuoteOverrides::price_driver_fields()
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "standard_deal_families")]
#[pyfunction]
#[pyo3(text_signature = "()")]
fn standard_deal_families(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let families = finstack_quant_valuations::instruments::fixed_income::structured_credit::standard_deal_families()
        .map_err(core_to_py)?;
    serde_to_py(py, &families)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "tranche_solve_field_units")]
#[pyfunction]
#[pyo3(text_signature = "()")]
fn tranche_solve_field_units(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    serde_to_py(
        py,
        &finstack_quant_valuations::results::tranche_solve_field_units(),
    )
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "tranche_metric_field_units")]
#[pyfunction]
#[pyo3(text_signature = "(currency)")]
fn tranche_metric_field_units<'py>(
    py: Python<'py>,
    currency: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let currency = crate::bindings::core::currency::extract_currency(currency)?;
    serde_to_py(
        py,
        &finstack_quant_valuations::results::tranche_metric_field_units(currency),
    )
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "scenario_table_field_units")]
#[pyfunction]
#[pyo3(text_signature = "(currency)")]
fn scenario_table_field_units<'py>(
    py: Python<'py>,
    currency: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let currency = crate::bindings::core::currency::extract_currency(currency)?;
    serde_to_py(
        py,
        &finstack_quant_valuations::results::scenario_table_field_units(currency),
    )
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "simulation_field_units")]
#[pyfunction]
#[pyo3(text_signature = "(currency)")]
fn simulation_field_units<'py>(
    py: Python<'py>,
    currency: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let currency = crate::bindings::core::currency::extract_currency(currency)?;
    serde_to_py(
        py,
        &finstack_quant_valuations::results::simulation_field_units(currency),
    )
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "equity_metrics_field_units")]
#[pyfunction]
#[pyo3(text_signature = "(currency)")]
fn equity_metrics_field_units<'py>(
    py: Python<'py>,
    currency: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let currency = crate::bindings::core::currency::extract_currency(currency)?;
    serde_to_py(
        py,
        &finstack_quant_valuations::results::equity_metrics_field_units(currency),
    )
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "bond_quote_set_field_units")]
#[pyfunction]
#[pyo3(text_signature = "(currency)")]
fn bond_quote_set_field_units<'py>(
    py: Python<'py>,
    currency: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let currency = crate::bindings::core::currency::extract_currency(currency)?;
    serde_to_py(
        py,
        &finstack_quant_valuations::results::bond_quote_set_field_units(currency),
    )
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "pool_report_field_units")]
#[pyfunction]
#[pyo3(text_signature = "(currency)")]
fn pool_report_field_units<'py>(
    py: Python<'py>,
    currency: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let currency = crate::bindings::core::currency::extract_currency(currency)?;
    serde_to_py(
        py,
        &finstack_quant_valuations::results::pool_report_field_units(currency),
    )
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "list_models")]
#[pyfunction]
fn list_models() -> Vec<String> {
    finstack_quant_valuations::pricer::list_models()
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "list_models_grouped")]
#[pyfunction]
fn list_models_grouped() -> std::collections::BTreeMap<String, Vec<String>> {
    finstack_quant_valuations::pricer::list_models_grouped()
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "instrument_cashflows")]
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
        .detach(move || instrument.into_parsed(None, None))
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope")]
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

/// Documented column order of ``InstrumentCashflowEnvelope`` tabular exports:
/// every ``CashflowRow`` field, in struct order, with its empty-table dtype.
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
    ("principal_date", "str"),
    ("year_fraction", "float64"),
    ("rate", "float64"),
    ("reset_date", "str"),
    ("discount_factor", "float64"),
    ("discount_curve_id", "str"),
    ("survival_probability", "float64"),
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.as_of")]
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.as_of)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.calendar_year_ladder_by_currency")]
    #[pyo3(text_signature = "($self)")]
    fn calendar_year_ladder_by_currency<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let ladders = self
            .inner
            .calendar_year_ladder_by_currency()
            .map_err(crate::errors::core_to_py)?;
        serde_to_py(py, &ladders)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.flows")]
    #[getter]
    fn flows<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.flows)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.to_pandas")]
    #[pyo3(text_signature = "($self)")]
    fn to_pandas<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table()?)?;
        let frame = PyArrowTable::to_pandas(&table, py)?;
        let to_datetime = py.import("pandas")?.getattr("to_datetime")?;
        let parsed = pyo3::types::PyDict::new(py);
        for column in [
            "date",
            "reset_date",
            "accrual_start",
            "accrual_end",
            "principal_date",
        ] {
            parsed.set_item(column, to_datetime.call1((frame.get_item(column)?,))?)?;
        }
        frame.call_method("assign", (), Some(&parsed))
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

crate::bindings::macros::wire_methods!(
    PyInstrumentCashflowEnvelope,
    InstrumentCashflowEnvelope,
    "InstrumentCashflowEnvelope",
    module = "finstack_quant.valuations.instruments._pricing"
);

crate::bindings::macros::py_getters!(PyInstrumentCashflowEnvelope |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.instrument_id")]
    instrument_id -> String = s.inner.instrument_id.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.model")]
    model -> String = s.inner.model.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.credit_curve_id")]
    credit_curve_id -> Option<String> = s.inner.credit_curve_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.recovery_rate")]
    recovery_rate -> Option<f64> = s.inner.recovery_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.scenario_price_shock_decimal")]
    scenario_price_shock_decimal -> Option<f64> = s.inner.scenario_price_shock_decimal;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.total_pv")]
    total_pv -> f64 = s.inner.total_pv;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._pricing", "InstrumentCashflowEnvelope.reconciles_with_base_value")]
    reconciles_with_base_value -> bool = s.inner.reconciles_with_base_value;
});

impl PyInstrumentCashflowEnvelope {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.flows, CASHFLOW_ROW_COLUMNS)
    }
}

crate::bindings::macros::tabular_methods!(
    PyInstrumentCashflowEnvelope,
    "InstrumentCashflowEnvelope",
    module = "finstack_quant.valuations.instruments._pricing",
    custom_pandas
);

/// Register pricing functions on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMetricPricingOverrides>()?;
    m.add_class::<PyMarketHistory>()?;
    m.add_class::<PyPriceExtras>()?;
    m.add_class::<PyInstrumentCashflowEnvelope>()?;
    m.add_function(pyo3::wrap_pyfunction!(instrument_cashflows, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(price_instrument, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_models, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_models_grouped, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_standard_metrics, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_standard_metrics_grouped, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(metric_metadata, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(price_driver_fields, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(pool_report_field_units, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(bond_quote_set_field_units, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(standard_deal_families, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(scenario_table_field_units, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(simulation_field_units, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(equity_metrics_field_units, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(tranche_metric_field_units, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(tranche_solve_field_units, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(metrics_for_instrument, m)?)?;
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
    "PriceExtras",
    "instrument_cashflows",
    "list_models",
    "list_models_grouped",
    "list_standard_metrics",
    "list_standard_metrics_grouped",
    "metric_metadata",
    "metrics_for_instrument",
    "price_driver_fields",
    "price_instrument",
    "pool_report_field_units",
    "bond_quote_set_field_units",
    "standard_deal_families",
    "scenario_table_field_units",
    "simulation_field_units",
    "equity_metrics_field_units",
    "tranche_metric_field_units",
    "tranche_solve_field_units",
];

/// Price a prepared instrument after host market/date conversion.
///
/// `extras` supplies the market history and the `explain` flag; its
/// metric-pricing override patch was already merged into `instrument`.
pub(crate) fn price_prepared(
    py: Python<'_>,
    instrument: finstack_quant_valuations::pricer::ParsedInstrument,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
    metrics: Option<Vec<String>>,
    extras: PriceExtras,
) -> PyResult<PyValuationResult> {
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    let metrics = metrics.unwrap_or_default();
    let inner = py
        .detach(move || {
            finstack_quant_valuations::pricer::price_instrument(
                &instrument,
                &market,
                &as_of,
                &model,
                &metrics,
                None,
                extras
                    .pricing_options(finstack_quant_calibration::recalibration::pricing_options()),
            )
        })
        .map_err(core_to_py)?;
    Ok(PyValuationResult { inner })
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
    extras: PriceExtras,
) -> PyResult<Bound<'py, PyDict>> {
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    let pairs = py
        .detach(move || {
            let instrument = finstack_quant_valuations::pricer::ParsedInstrument::from_instrument(
                instrument,
                extras.metric_pricing_overrides_json().as_deref(),
                extras.instrument_pricing_overrides.as_ref(),
            )?;
            finstack_quant_valuations::pricer::present_standard_option_greeks(
                &instrument,
                &market,
                &as_of,
                &model,
                extras
                    .pricing_options(finstack_quant_calibration::recalibration::pricing_options()),
            )
        })
        .map_err(core_to_py)?;
    let out = PyDict::new(py);
    for (metric, value) in pairs {
        out.set_item(metric, value)?;
    }
    Ok(out)
}
