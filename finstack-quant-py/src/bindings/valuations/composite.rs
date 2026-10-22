//! Python wrappers for resolved composite instruments and dated history.
//!
//! Runtime ``help()`` text lives on these wrappers. The ``.pyi`` stub remains
//! the IDE surface. Host bindings expose only ``initialize``; Rust
//! ``initialize_fixed`` is not wrapped because that path also resolves
//! ``fixed_quantity`` without historical observations.

use crate::bindings::core::currency::PyCurrency;
use crate::bindings::core::money::PyMoney;
use crate::bindings::extract::{extract_instrument, extract_market};
use crate::bindings::json_bridge::{deserialize_json_with_context, serialize_json_with_context};
use crate::bindings::module_utils::opt_spec_from_py;
use crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema;
use crate::bindings::pandas_utils::{serde_to_py, ColumnSchema};
use crate::bindings::pydoc::pydoc;
use crate::bindings::valuations::instruments::{
    parse_typed_instrument_json, serialize_typed_instrument_json,
};
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::composite::{
    history, history_from_spec, CompositeHistoryRow, CompositeInstrument, CompositeLegSpec,
    CompositeMarketObservation, CompositeRebalanceResult, CompositeSpec, CompositeState,
    RebalanceRule, WeightingMethod,
};
use finstack_quant_valuations::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::composite::CompositeExposureReport;
use pyo3::prelude::*;
use pyo3::types::PyList;

const COMPOSITE_TRADE_COLUMNS: &[ColumnSchema<'static>] = &[
    ("instrument_id", "str"),
    ("instrument_type", "str"),
    ("quantity_delta", "float64"),
];

const COMPOSITE_AGGREGATE_COLUMNS: &[ColumnSchema<'static>] = &[
    ("instrument_id", "str"),
    ("instrument_type", "str"),
    ("net_quantity", "float64"),
    ("gross_quantity", "float64"),
    ("net_value", "float64"),
    ("gross_value", "float64"),
    ("currency", "str"),
];

const COMPOSITE_HISTORY_COLUMNS: &[ColumnSchema<'static>] = &[
    ("date", "str"),
    ("value", "float64"),
    ("cashflows", "float64"),
    ("pnl", "float64"),
    ("currency", "str"),
    ("period_return", "float64"),
    ("return_index", "float64"),
    ("held_state_effective_date", "str"),
    ("next_state_effective_date", "str"),
    ("rebalance_trade_count", "int64"),
];

/// Accept a ``CompositeMarketObservation`` array as a Python list of dicts,
/// a JSON string, or ``None`` (no observations).
fn observations_from_py(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
    what: &str,
) -> PyResult<Vec<CompositeMarketObservation>> {
    Ok(opt_spec_from_py(py, obj, what)?.unwrap_or_default())
}

/// Render the fields of a serde object Python-style (``key='str'``, ``True``,
/// ``None``) for ``__repr__`` implementations of serde-backed enums.
fn py_style_fields(value: &serde_json::Value) -> String {
    fn scalar(v: &serde_json::Value) -> String {
        match v {
            serde_json::Value::Null => "None".to_string(),
            serde_json::Value::Bool(b) => {
                crate::bindings::valuations::convert::bool_repr(*b).to_string()
            }
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("{s:?}").replace('"', "'"),
            other => other.to_string(),
        }
    }
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(k, v)| format!("{k}={}", scalar(v)))
            .collect::<Vec<_>>()
            .join(", "),
        other => scalar(other),
    }
}

#[doc = pydoc!("finstack_quant.valuations.composite", "CompositeLegSpec")]
#[pyclass(
    name = "CompositeLegSpec",
    module = "finstack_quant.valuations.composite",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCompositeLegSpec {
    pub(crate) inner: CompositeLegSpec,
}

#[pymethods]
impl PyCompositeLegSpec {
    #[new]
    #[pyo3(text_signature = "(instrument_id, instrument, score)")]
    fn new(instrument_id: &str, instrument: &Bound<'_, PyAny>, score: f64) -> PyResult<Self> {
        let instrument = extract_instrument(instrument)?
            .into_instrument_json()
            .map_err(core_to_py)?;
        Ok(Self {
            inner: CompositeLegSpec::new(instrument_id, instrument, score),
        })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeLegSpec.from_json")]
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        deserialize_json_with_context(json, "invalid CompositeLegSpec JSON")
            .map(|inner| Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeLegSpec.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json_with_context(&self.inner, "failed to serialize CompositeLegSpec")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeLegSpec.instrument_dict")]
    fn instrument_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(
            py,
            &InstrumentEnvelope::new(self.inner.instrument.as_ref().clone()),
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CompositeLegSpec(instrument_id='{}', instrument_type='{}', score={})",
            self.inner.instrument_id,
            self.inner.instrument.type_tag(),
            self.inner.score
        )
    }
}

crate::bindings::macros::py_getters!(PyCompositeLegSpec |s| {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeLegSpec.instrument_id")]
    instrument_id -> String = s.inner.instrument_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeLegSpec.score")]
    score -> f64 = s.inner.score;
});

#[doc = pydoc!("finstack_quant.valuations.composite", "WeightingMethod")]
#[pyclass(
    name = "WeightingMethod",
    module = "finstack_quant.valuations.composite",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyWeightingMethod {
    pub(crate) inner: WeightingMethod,
}

#[pymethods]
impl PyWeightingMethod {
    #[doc = pydoc!("finstack_quant.valuations.composite", "WeightingMethod.fixed_quantity")]
    #[staticmethod]
    fn fixed_quantity() -> Self {
        Self {
            inner: WeightingMethod::FixedQuantity,
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "WeightingMethod.notional_weighted")]
    #[staticmethod]
    fn notional_weighted(gross_notional: PyRef<'_, PyMoney>) -> Self {
        Self {
            inner: WeightingMethod::NotionalWeighted {
                gross_notional: gross_notional.inner,
            },
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "WeightingMethod.metric_weighted")]
    #[staticmethod]
    #[pyo3(text_signature = "(metric, anchor_leg_id, anchor_quantity, neutralize)")]
    fn metric_weighted(
        metric: &str,
        anchor_leg_id: &str,
        anchor_quantity: f64,
        neutralize: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: WeightingMethod::MetricWeighted {
                metric: metric.parse().map_err(core_to_py)?,
                anchor_leg_id: InstrumentId::new(anchor_leg_id),
                anchor_quantity,
                neutralize,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "WeightingMethod.dv01_neutral")]
    #[staticmethod]
    fn dv01_neutral(anchor_leg_id: &str, anchor_quantity: f64) -> Self {
        Self {
            inner: WeightingMethod::dv01_neutral(anchor_leg_id, anchor_quantity),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "WeightingMethod.delta_neutral")]
    #[staticmethod]
    fn delta_neutral(anchor_leg_id: &str, anchor_quantity: f64) -> Self {
        Self {
            inner: WeightingMethod::delta_neutral(anchor_leg_id, anchor_quantity),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "WeightingMethod.duration_weighted")]
    #[staticmethod]
    fn duration_weighted(anchor_leg_id: &str, anchor_quantity: f64) -> Self {
        Self {
            inner: WeightingMethod::duration_weighted(anchor_leg_id, anchor_quantity),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "WeightingMethod.volatility_weighted")]
    #[staticmethod]
    fn volatility_weighted(
        anchor_leg_id: &str,
        anchor_quantity: f64,
        lookback: usize,
        min_observations: usize,
        annualization_factor: f64,
    ) -> Self {
        Self {
            inner: WeightingMethod::volatility_weighted(
                anchor_leg_id,
                anchor_quantity,
                lookback,
                min_observations,
                annualization_factor,
            ),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "WeightingMethod.from_json")]
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        deserialize_json_with_context(json, "invalid WeightingMethod JSON")
            .map(|inner| Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "WeightingMethod.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json_with_context(&self.inner, "failed to serialize WeightingMethod")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Return ``repr(self)`` (``WeightingMethod(kind='dv01_neutral', anchor_leg_id='A', ...)``).
    fn __repr__(&self) -> String {
        let fields = serde_json::to_value(&self.inner)
            .map(|v| py_style_fields(&v))
            .unwrap_or_default();
        format!("WeightingMethod({fields})")
    }
}

#[doc = pydoc!("finstack_quant.valuations.composite", "RebalanceRule")]
#[pyclass(
    name = "RebalanceRule",
    module = "finstack_quant.valuations.composite",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRebalanceRule {
    pub(crate) inner: RebalanceRule,
}

#[pymethods]
impl PyRebalanceRule {
    #[doc = pydoc!("finstack_quant.valuations.composite", "RebalanceRule.manual")]
    #[staticmethod]
    fn manual() -> Self {
        Self {
            inner: RebalanceRule::Manual,
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "RebalanceRule.dates")]
    #[staticmethod]
    fn dates(dates: Vec<Bound<'_, PyAny>>) -> PyResult<Self> {
        let dates = dates
            .iter()
            .map(crate::bindings::date_utils::py_to_date)
            .collect::<PyResult<Vec<_>>>()?;
        let inner = RebalanceRule::Dates { dates };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "RebalanceRule.calendar")]
    #[staticmethod]
    #[pyo3(signature = (start, frequency, calendar_id, business_day_convention, end=None))]
    fn calendar(
        start: &Bound<'_, PyAny>,
        frequency: &Bound<'_, PyAny>,
        calendar_id: &str,
        business_day_convention: &str,
        end: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let inner = RebalanceRule::Calendar {
            start: crate::bindings::date_utils::py_to_date(start)?,
            end: end
                .filter(|value| !value.is_none())
                .map(crate::bindings::date_utils::py_to_date)
                .transpose()?,
            frequency: crate::bindings::core::dates::tenor::extract_tenor(frequency)?,
            calendar_id: calendar_id.to_string(),
            business_day_convention: super::convert::bdc_from_str(
                business_day_convention,
                "business_day_convention",
            )?,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "RebalanceRule.from_json")]
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: RebalanceRule =
            deserialize_json_with_context(json, "invalid RebalanceRule JSON")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "RebalanceRule.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json_with_context(&self.inner, "failed to serialize RebalanceRule")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Return ``repr(self)`` (``RebalanceRule(kind='manual')``).
    fn __repr__(&self) -> String {
        let fields = serde_json::to_value(&self.inner)
            .map(|v| py_style_fields(&v))
            .unwrap_or_default();
        format!("RebalanceRule({fields})")
    }
}

#[doc = pydoc!("finstack_quant.valuations.composite", "CompositeSpec")]
#[pyclass(
    name = "CompositeSpec",
    module = "finstack_quant.valuations.composite",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCompositeSpec {
    pub(crate) inner: CompositeSpec,
}

#[pymethods]
impl PyCompositeSpec {
    #[new]
    fn new(
        id: &str,
        reporting_currency: PyRef<'_, PyCurrency>,
        capital: PyRef<'_, PyMoney>,
        legs: Vec<PyRef<'_, PyCompositeLegSpec>>,
        weighting_method: PyRef<'_, PyWeightingMethod>,
        rebalance_rule: PyRef<'_, PyRebalanceRule>,
    ) -> PyResult<Self> {
        let inner = CompositeSpec::new(
            id,
            reporting_currency.inner,
            capital.inner,
            legs.into_iter().map(|leg| leg.inner.clone()).collect(),
            weighting_method.inner.clone(),
            rebalance_rule.inner.clone(),
        );
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeSpec.from_json")]
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: CompositeSpec =
            deserialize_json_with_context(json, "invalid CompositeSpec JSON")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeSpec.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json_with_context(&self.inner, "failed to serialize CompositeSpec")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeSpec.legs")]
    #[getter]
    fn legs(&self) -> Vec<PyCompositeLegSpec> {
        self.inner
            .legs
            .iter()
            .map(|leg| PyCompositeLegSpec { inner: leg.clone() })
            .collect()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CompositeSpec(id='{}', reporting_currency='{}', capital={}, legs={}, weighting_method={}, rebalance_rule={})",
            self.inner.id,
            self.inner.reporting_currency,
            self.inner.capital.amount(),
            self.inner.legs.len(),
            PyWeightingMethod {
                inner: self.inner.weighting_method.clone()
            }
            .__repr__(),
            PyRebalanceRule {
                inner: self.inner.rebalance_rule.clone()
            }
            .__repr__(),
        )
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeSpec.initialize")]
    #[pyo3(signature = (market, as_of, history=None))]
    fn initialize(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        history: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyCompositeRebalanceResult> {
        let market = extract_market(py, market)?;
        let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
        let history = observations_from_py(py, history, "composite market history")?;
        self.inner
            .initialize(
                &market,
                as_of,
                &history,
                finstack_quant_calibration::recalibration::pricing_options(),
            )
            .map(PyCompositeRebalanceResult::from_inner)
            .map_err(core_to_py)
    }
}

crate::bindings::macros::py_getters!(PyCompositeSpec |s| {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeSpec.id")]
    id -> String = s.inner.id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeSpec.reporting_currency")]
    reporting_currency -> PyCurrency = PyCurrency::from_inner(s.inner.reporting_currency);

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeSpec.capital")]
    capital -> PyMoney = PyMoney { inner: s.inner.capital };

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeSpec.weighting_method")]
    weighting_method -> PyWeightingMethod =
        PyWeightingMethod { inner: s.inner.weighting_method.clone() };

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeSpec.rebalance_rule")]
    rebalance_rule -> PyRebalanceRule = PyRebalanceRule { inner: s.inner.rebalance_rule.clone() };
});

#[doc = pydoc!("finstack_quant.valuations.composite", "CompositeState")]
#[pyclass(
    name = "CompositeState",
    module = "finstack_quant.valuations.composite",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCompositeState {
    pub(crate) inner: CompositeState,
}

#[pymethods]
impl PyCompositeState {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeState.from_json")]
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        deserialize_json_with_context(json, "invalid CompositeState JSON")
            .map(|inner| Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeState.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json_with_context(&self.inner, "failed to serialize CompositeState")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CompositeState(effective_date='{}', legs={})",
            self.inner.effective_date,
            self.inner.resolved_legs.len()
        )
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeState.resolved_legs")]
    #[getter]
    fn resolved_legs(&self) -> std::collections::BTreeMap<String, f64> {
        self.inner
            .resolved_legs
            .iter()
            .map(|leg| (leg.instrument_id.to_string(), leg.quantity))
            .collect()
    }
}

crate::bindings::macros::py_getters!(PyCompositeState |s| {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeState.effective_date")]
    effective_date -> String = s.inner.effective_date.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.composite", "CompositeInstrument")]
#[pyclass(
    name = "CompositeInstrument",
    module = "finstack_quant.valuations.composite",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCompositeInstrument {
    pub(crate) inner: CompositeInstrument,
}

#[pymethods]
impl PyCompositeInstrument {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeInstrument.from_json")]
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_typed_instrument_json::<CompositeInstrument>(json).map(|inner| Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeInstrument.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(InstrumentJson::from(self.inner.clone()), "composite")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeInstrument.rebalance")]
    #[pyo3(signature = (market, as_of, history=None))]
    fn rebalance(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        history: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyCompositeRebalanceResult> {
        let market = extract_market(py, market)?;
        let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
        let history = observations_from_py(py, history, "composite market history")?;
        self.inner
            .rebalance(
                &market,
                as_of,
                &history,
                finstack_quant_calibration::recalibration::pricing_options(),
            )
            .map(PyCompositeRebalanceResult::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeInstrument.primitive_exposures")]
    #[pyo3(signature = (market, as_of, metrics=None))]
    fn primitive_exposures(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        metrics: Option<Vec<String>>,
    ) -> PyResult<PyCompositeExposureReport> {
        let market = extract_market(py, market)?;
        let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
        let metrics = metrics
            .unwrap_or_default()
            .into_iter()
            .map(MetricId::try_from)
            .collect::<Result<Vec<_>, _>>()
            .map_err(core_to_py)?;
        self.inner
            .primitive_exposures(
                &market,
                as_of,
                &metrics,
                finstack_quant_calibration::recalibration::pricing_options(),
            )
            .map(PyCompositeExposureReport::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeInstrument.execution_trades")]
    #[pyo3(signature = (previous=None))]
    fn execution_trades<'py>(
        &self,
        py: Python<'py>,
        previous: Option<&PyCompositeInstrument>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let trades = self
            .inner
            .execution_trades(previous.map(|value| &value.inner))
            .map_err(core_to_py)?;
        serde_to_py(py, &trades)
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeInstrument.execution_trades_dataframe")]
    #[pyo3(signature = (previous=None))]
    fn execution_trades_dataframe<'py>(
        &self,
        py: Python<'py>,
        previous: Option<&PyCompositeInstrument>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let trades = self
            .inner
            .execution_trades(previous.map(|value| &value.inner))
            .map_err(core_to_py)?;
        serde_rows_to_dataframe_with_schema(py, &trades, COMPOSITE_TRADE_COLUMNS)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CompositeInstrument(id='{}', effective_date='{}', legs={})",
            self.inner.id(),
            self.inner.state.effective_date,
            self.inner.state.resolved_legs.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyCompositeInstrument |s| {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeInstrument.id")]
    id -> String = s.inner.spec.id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeInstrument.spec")]
    spec -> PyCompositeSpec = PyCompositeSpec { inner: s.inner.spec.clone() };

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeInstrument.state")]
    state -> PyCompositeState = PyCompositeState { inner: s.inner.state.clone() };
});

#[doc = pydoc!("finstack_quant.valuations.composite", "CompositeRebalanceResult")]
#[pyclass(
    name = "CompositeRebalanceResult",
    module = "finstack_quant.valuations.composite",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCompositeRebalanceResult {
    pub(crate) inner: CompositeRebalanceResult,
}

impl PyCompositeRebalanceResult {
    fn from_inner(inner: CompositeRebalanceResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCompositeRebalanceResult {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeRebalanceResult.from_json")]
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        deserialize_json_with_context(json, "invalid CompositeRebalanceResult JSON")
            .map(|inner| Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeRebalanceResult.trades")]
    #[getter]
    fn trades<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.trades)
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeRebalanceResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.trades, COMPOSITE_TRADE_COLUMNS)
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeRebalanceResult.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json_with_context(&self.inner, "failed to serialize CompositeRebalanceResult")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "CompositeRebalanceResult(instrument_id={:?}, trades={})",
            self.inner.instrument.id(),
            self.inner.trades.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyCompositeRebalanceResult |s| {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeRebalanceResult.instrument")]
    instrument -> PyCompositeInstrument =
        PyCompositeInstrument { inner: s.inner.instrument.clone() };
});

#[doc = pydoc!("finstack_quant.valuations.composite", "CompositeExposureReport")]
#[pyclass(
    name = "CompositeExposureReport",
    module = "finstack_quant.valuations.composite",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCompositeExposureReport {
    pub(crate) inner: CompositeExposureReport,
}

impl PyCompositeExposureReport {
    fn from_inner(inner: CompositeExposureReport) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCompositeExposureReport {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeExposureReport.from_json")]
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        deserialize_json_with_context(json, "invalid CompositeExposureReport JSON")
            .map(|inner| Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeExposureReport.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = self
            .inner
            .aggregates
            .iter()
            .map(|aggregate| {
                serde_json::json!({
                    "instrument_id": aggregate.instrument_id.as_str(),
                    "instrument_type": aggregate.instrument_type,
                    "net_quantity": aggregate.net_quantity,
                    "gross_quantity": aggregate.gross_quantity,
                    "net_value": aggregate.net_value.amount(),
                    "gross_value": aggregate.gross_value.amount(),
                    "currency": aggregate.net_value.currency().to_string(),
                })
            })
            .collect::<Vec<_>>();
        serde_rows_to_dataframe_with_schema(py, &rows, COMPOSITE_AGGREGATE_COLUMNS)
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeExposureReport.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json_with_context(&self.inner, "failed to serialize CompositeExposureReport")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "CompositeExposureReport(currency={}, paths={}, aggregates={})",
            self.inner.reporting_currency,
            self.inner.paths.len(),
            self.inner.aggregates.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyCompositeExposureReport |s| {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeExposureReport.reporting_currency")]
    reporting_currency -> PyCurrency = PyCurrency::from_inner(s.inner.reporting_currency);

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeExposureReport.path_count")]
    path_count -> usize = s.inner.paths.len();

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeExposureReport.aggregate_count")]
    aggregate_count -> usize = s.inner.aggregates.len();
});

#[doc = pydoc!("finstack_quant.valuations.composite", "CompositeHistoryResult")]
#[pyclass(
    name = "CompositeHistoryResult",
    module = "finstack_quant.valuations.composite",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCompositeHistoryResult {
    pub(crate) inner: Vec<CompositeHistoryRow>,
}

impl PyCompositeHistoryResult {
    /// The zero-based row `index`, or a Python `IndexError` when it is out of range.
    fn history_row(&self, index: usize) -> PyResult<&CompositeHistoryRow> {
        self.inner.get(index).ok_or_else(|| {
            pyo3::exceptions::PyIndexError::new_err(format!(
                "history row index {index} is out of range"
            ))
        })
    }
}

#[pymethods]
impl PyCompositeHistoryResult {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeHistoryResult.from_json")]
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        deserialize_json_with_context(json, "invalid composite history JSON")
            .map(|inner| Self { inner })
    }

    /// Return the number of chronological output rows.
    ///
    /// Returns
    /// -------
    /// int
    ///     Count of dated history rows in chronological order.
    ///
    /// Notes
    /// -----
    /// This accessor does not raise; an empty result has length ``0``.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeHistoryResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = self
            .inner
            .iter()
            .map(|row| {
                serde_json::json!({
                    "date": row.date.to_string(),
                    "value": row.value.amount(),
                    "cashflows": row.cashflows.amount(),
                    "pnl": row.pnl.amount(),
                    "currency": row.value.currency().to_string(),
                    "period_return": row.period_return,
                    "return_index": row.return_index,
                    "held_state_effective_date": row.held_state_effective_date.to_string(),
                    "next_state_effective_date": row.next_state_effective_date.map(|date| date.to_string()),
                    "rebalance_trade_count": row.rebalance_trades.len(),
                })
            })
            .collect::<Vec<_>>();
        serde_rows_to_dataframe_with_schema(py, &rows, COMPOSITE_HISTORY_COLUMNS)
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeHistoryResult.row")]
    fn row<'py>(&self, py: Python<'py>, index: usize) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, self.history_row(index)?)
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeHistoryResult.row_json")]
    fn row_json(&self, index: usize) -> PyResult<String> {
        serialize_json_with_context(
            self.history_row(index)?,
            "failed to serialize CompositeHistoryRow",
        )
    }

    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeHistoryResult.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json_with_context(&self.inner, "failed to serialize composite history")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!("CompositeHistoryResult(rows={})", self.inner.len())
    }
}

crate::bindings::macros::py_getters!(PyCompositeHistoryResult |s| {
    #[doc = pydoc!("finstack_quant.valuations.composite", "CompositeHistoryResult.dates")]
    dates -> Vec<String> = s.inner.iter().map(|row| row.date.to_string()).collect();
});

#[doc = pydoc!("finstack_quant.valuations.composite", "history_from_spec")]
#[pyfunction]
#[pyo3(
        name = "history_from_spec",
        signature = (spec, observations, warmup=None, metrics=None),
        text_signature = "(spec, observations, warmup=None, metrics=None)"
    )]
fn py_history_from_spec(
    py: Python<'_>,
    spec: &PyCompositeSpec,
    observations: &Bound<'_, PyAny>,
    warmup: Option<&Bound<'_, PyAny>>,
    metrics: Option<Vec<String>>,
) -> PyResult<PyCompositeHistoryResult> {
    let observations = observations_from_py(py, Some(observations), "composite observations")?;
    let warmup = observations_from_py(py, warmup, "composite warmup")?;
    let metrics = metrics
        .unwrap_or_default()
        .into_iter()
        .map(MetricId::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(core_to_py)?;
    history_from_spec(
        &spec.inner,
        &warmup,
        &observations,
        &metrics,
        finstack_quant_calibration::recalibration::pricing_options(),
    )
    .map(|inner| PyCompositeHistoryResult { inner })
    .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.valuations.composite", "history")]
#[pyfunction]
#[pyo3(
        name = "history",
        signature = (instrument, observations, metrics=None),
        text_signature = "(instrument, observations, metrics=None)"
    )]
fn py_history(
    py: Python<'_>,
    instrument: &PyCompositeInstrument,
    observations: &Bound<'_, PyAny>,
    metrics: Option<Vec<String>>,
) -> PyResult<PyCompositeHistoryResult> {
    let observations = observations_from_py(py, Some(observations), "composite observations")?;
    let metrics = metrics
        .unwrap_or_default()
        .into_iter()
        .map(MetricId::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(core_to_py)?;
    history(
        &instrument.inner,
        &observations,
        &metrics,
        finstack_quant_calibration::recalibration::pricing_options(),
    )
    .map(|inner| PyCompositeHistoryResult { inner })
    .map_err(core_to_py)
}

pub(crate) const EXPORTS: &[&str] = &[
    "CompositeExposureReport",
    "CompositeHistoryResult",
    "CompositeInstrument",
    "CompositeLegSpec",
    "CompositeRebalanceResult",
    "CompositeSpec",
    "CompositeState",
    "RebalanceRule",
    "WeightingMethod",
    "history",
    "history_from_spec",
];

/// Register the ``finstack_quant.valuations.composite`` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = crate::bindings::module_utils::new_submodule(parent, "composite")?;
    module.setattr(
        "__doc__",
        pydoc!("finstack_quant.valuations.composite", "__module__"),
    )?;
    module.add_class::<PyCompositeLegSpec>()?;
    module.add_class::<PyWeightingMethod>()?;
    module.add_class::<PyRebalanceRule>()?;
    module.add_class::<PyCompositeSpec>()?;
    module.add_class::<PyCompositeState>()?;
    module.add_class::<PyCompositeInstrument>()?;
    module.add_class::<PyCompositeRebalanceResult>()?;
    module.add_class::<PyCompositeExposureReport>()?;
    module.add_class::<PyCompositeHistoryResult>()?;
    module.add_function(wrap_pyfunction!(py_history, &module)?)?;
    module.add_function(wrap_pyfunction!(py_history_from_spec, &module)?)?;
    module.setattr("__all__", PyList::new(py, EXPORTS)?)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &module,
        crate::bindings::module_utils::Exposure::Compiled,
    )
}
