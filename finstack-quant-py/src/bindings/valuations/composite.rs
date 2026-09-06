//! Python wrappers for resolved composite instruments and dated history.
//!
//! Runtime ``help()`` text lives on these wrappers. The ``.pyi`` stub remains
//! the IDE surface. Host bindings expose only ``initialize``; Rust
//! ``initialize_fixed`` is not wrapped because that path also resolves
//! ``fixed_quantity`` without historical observations.

use crate::bindings::core::currency::PyCurrency;
use crate::bindings::core::money::PyMoney;
use crate::bindings::extract::{extract_instrument_json, extract_market};
use crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema;
use crate::bindings::pandas_utils::{serde_to_py, ColumnSchema};
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::composite::{
    CompositeExposureReport, CompositeHistoryEngine, CompositeHistoryRow, CompositeInstrument,
    CompositeLegSpec, CompositeMarketObservation, CompositeRebalanceResult, CompositeSpec,
    CompositeState, RebalanceFrequency, RebalanceRule, WeightingMethod,
};
use finstack_quant_valuations::instruments::{Instrument, InstrumentEnvelope, InstrumentJson};
use finstack_quant_valuations::metrics::MetricId;
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

fn parse_json<T: serde::de::DeserializeOwned>(json: &str, what: &str) -> PyResult<T> {
    serde_json::from_str(json)
        .map_err(|err| crate::errors::serde_json_to_py(err, &format!("invalid {what} JSON")))
}

fn to_json<T: serde::Serialize>(value: &T, what: &str) -> PyResult<String> {
    serde_json::to_string(value)
        .map_err(|err| crate::errors::serde_json_to_py(err, &format!("failed to serialize {what}")))
}

fn parse_composite_envelope(json: &str) -> PyResult<CompositeInstrument> {
    match finstack_quant_valuations::pricer::json::parse_instrument_from_json(json)
        .map_err(core_to_py)?
    {
        InstrumentJson::Composite(instrument) => Ok(*instrument),
        other => Err(crate::errors::value_error(format!(
            "expected composite instrument envelope, found '{}'",
            other.type_tag()
        ))),
    }
}

fn composite_envelope_json(instrument: &CompositeInstrument) -> PyResult<String> {
    to_json(
        &InstrumentEnvelope::new(InstrumentJson::Composite(Box::new(instrument.clone()))),
        "composite instrument envelope",
    )
}

fn parse_observations(json: &str, what: &str) -> PyResult<Vec<CompositeMarketObservation>> {
    parse_json(json, what)
}

/// Accept a ``CompositeMarketObservation`` array as a Python list of dicts,
/// a JSON string, or ``None`` (no observations).
fn observations_from_py(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
    what: &str,
) -> PyResult<Vec<CompositeMarketObservation>> {
    match obj {
        None => Ok(Vec::new()),
        Some(obj) if obj.is_none() => Ok(Vec::new()),
        Some(obj) => {
            let json = crate::bindings::module_utils::py_to_json_string(py, obj, what)?;
            parse_observations(&json, what)
        }
    }
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

/// One self-contained composite leg definition.
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
    /// Build a signed leg from an existing typed instrument or canonical envelope.
    ///
    /// Parameters
    /// ----------
    /// instrument_id : str
    ///     Identifier that must equal the embedded instrument's identifier.
    /// instrument : object | str
    ///     Typed Python instrument or canonical ``finstack_quant.instrument/1`` JSON.
    /// weight : float
    ///     Non-zero signed quantity or relative weighting score.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the instrument payload is malformed.
    #[new]
    #[pyo3(text_signature = "(instrument_id, instrument, weight)")]
    fn new(instrument_id: &str, instrument: &Bound<'_, PyAny>, weight: f64) -> PyResult<Self> {
        let envelope = extract_instrument_json(instrument)?;
        let instrument =
            finstack_quant_valuations::pricer::json::parse_instrument_from_json(&envelope)
                .map_err(core_to_py)?;
        Ok(Self {
            inner: CompositeLegSpec::new(instrument_id, instrument, weight),
        })
    }

    /// Deserialize a bare canonical leg object.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON produced by ``to_json`` for exactly one leg.
    ///
    /// Returns
    /// -------
    /// CompositeLegSpec
    ///     Parsed leg retaining the embedded instrument definition.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or does not match the strict leg schema.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_json(json, "CompositeLegSpec").map(|inner| Self { inner })
    }

    /// Serialize this leg as a bare canonical JSON object.
    ///
    /// Returns
    /// -------
    /// str
    ///     Strict ``CompositeLegSpec`` JSON including its embedded instrument.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the canonical Rust value cannot be serialized.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "CompositeLegSpec")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Return the declared embedded-instrument identifier.
    ///
    /// Returns
    /// -------
    /// str
    ///     Stable identifier that matches the embedded instrument.
    ///
    /// Notes
    /// -----
    /// This accessor does not raise; it returns the stored identifier.
    #[getter]
    fn instrument_id(&self) -> String {
        self.inner.instrument_id.to_string()
    }

    /// Return the signed fixed quantity or dynamic weighting score.
    ///
    /// Returns
    /// -------
    /// float
    ///     Finite non-zero signed leg input.
    ///
    /// Notes
    /// -----
    /// This accessor does not raise; it returns the validated stored value.
    #[getter]
    fn weight(&self) -> f64 {
        self.inner.weight
    }

    /// Return the embedded instrument as a canonical v1 envelope.
    ///
    /// Returns
    /// -------
    /// str
    ///     ``finstack_quant.instrument/1`` JSON for the embedded instrument.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If canonical JSON serialization fails.
    #[getter]
    fn instrument_json(&self) -> PyResult<String> {
        to_json(
            &InstrumentEnvelope::new(self.inner.instrument.as_ref().clone()),
            "embedded instrument envelope",
        )
    }

    /// Return the embedded instrument as a plain ``dict`` (canonical serde shape).
    ///
    /// Returns
    /// -------
    /// dict
    ///     Tagged instrument object identical to ``json.loads(self.instrument_json)``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If canonical serialization fails.
    fn instrument_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(
            py,
            &InstrumentEnvelope::new(self.inner.instrument.as_ref().clone()),
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CompositeLegSpec(instrument_id='{}', instrument_type='{}', weight={})",
            self.inner.instrument_id,
            self.inner.instrument.type_tag(),
            self.inner.weight
        )
    }
}

/// Serializable policy for resolving signed composite quantities.
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
    /// Use signed leg weights directly as quantities without market data.
    ///
    /// Returns
    /// -------
    /// WeightingMethod
    ///     Fixed-quantity policy.
    ///
    /// Notes
    /// -----
    /// This factory does not raise; validation occurs when a specification is built.
    #[staticmethod]
    fn fixed_quantity() -> Self {
        Self {
            inner: WeightingMethod::FixedQuantity,
        }
    }

    /// Normalize absolute scores to a target gross reporting-currency notional.
    ///
    /// Parameters
    /// ----------
    /// gross_notional : Money
    ///     Positive gross allocation denominated in the composite reporting currency.
    ///
    /// Returns
    /// -------
    /// WeightingMethod
    ///     Gross-notional weighting policy preserving score signs.
    ///
    /// Notes
    /// -----
    /// This factory does not raise; currency and positivity are validated by ``CompositeSpec``.
    #[staticmethod]
    fn notional_weighted(gross_notional: PyRef<'_, PyMoney>) -> Self {
        Self {
            inner: WeightingMethod::NotionalWeighted {
                gross_notional: gross_notional.inner,
            },
        }
    }

    /// Resolve quantities from unit metric contributions and an anchor scale.
    ///
    /// Parameters
    /// ----------
    /// metric : str
    ///     Canonical unit metric identifier such as ``dv01`` or ``delta``.
    /// anchor_leg_id : str
    ///     Existing leg whose signed quantity fixes overall scale.
    /// anchor_quantity : float
    ///     Finite non-zero signed quantity assigned to the anchor leg.
    /// neutralize : bool
    ///     Whether positive and negative score groups normalize separately.
    ///
    /// Returns
    /// -------
    /// WeightingMethod
    ///     Anchored metric-weighting policy.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the metric key uses noncanonical composite encoding. Anchors and
    ///     quantities are validated by ``CompositeSpec``.
    #[staticmethod]
    #[pyo3(signature = (metric, anchor_leg_id, anchor_quantity, neutralize=false))]
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

    /// Construct parallel-DV01-neutral weighting.
    ///
    /// Parameters
    /// ----------
    /// anchor_leg_id : str
    ///     Existing rates leg that fixes quantity scale.
    /// anchor_quantity : float
    ///     Signed non-zero quantity assigned to the anchor.
    ///
    /// Returns
    /// -------
    /// WeightingMethod
    ///     Neutral metric policy using ``dv01``.
    ///
    /// Notes
    /// -----
    /// This factory does not raise; the anchor is validated by ``CompositeSpec``.
    #[staticmethod]
    fn dv01_neutral(anchor_leg_id: &str, anchor_quantity: f64) -> Self {
        Self {
            inner: WeightingMethod::dv01_neutral(anchor_leg_id, anchor_quantity),
        }
    }

    /// Construct delta-neutral weighting for cross-asset hedges.
    ///
    /// Parameters
    /// ----------
    /// anchor_leg_id : str
    ///     Existing delta-bearing leg that fixes quantity scale.
    /// anchor_quantity : float
    ///     Signed non-zero quantity assigned to the anchor.
    ///
    /// Returns
    /// -------
    /// WeightingMethod
    ///     Neutral metric policy using ``delta``.
    ///
    /// Notes
    /// -----
    /// This factory does not raise; the anchor is validated by ``CompositeSpec``.
    #[staticmethod]
    fn delta_neutral(anchor_leg_id: &str, anchor_quantity: f64) -> Self {
        Self {
            inner: WeightingMethod::delta_neutral(anchor_leg_id, anchor_quantity),
        }
    }

    /// Construct modified-duration weighting without sign-group neutrality.
    ///
    /// Parameters
    /// ----------
    /// anchor_leg_id : str
    ///     Existing duration-bearing leg that fixes quantity scale.
    /// anchor_quantity : float
    ///     Signed non-zero quantity assigned to the anchor.
    ///
    /// Returns
    /// -------
    /// WeightingMethod
    ///     Anchored policy using modified duration.
    ///
    /// Notes
    /// -----
    /// This factory does not raise; the anchor is validated by ``CompositeSpec``.
    #[staticmethod]
    fn duration_weighted(anchor_leg_id: &str, anchor_quantity: f64) -> Self {
        Self {
            inner: WeightingMethod::duration_weighted(anchor_leg_id, anchor_quantity),
        }
    }

    /// Construct inverse annualized unit-P&L-volatility weighting.
    ///
    /// Parameters
    /// ----------
    /// anchor_leg_id : str
    ///     Existing leg whose quantity fixes overall scale.
    /// anchor_quantity : float
    ///     Signed non-zero quantity assigned to the anchor.
    /// lookback : int
    ///     Maximum number of most-recent P&L observations used.
    /// min_observations : int
    ///     Minimum finite P&L observations required for every leg.
    /// annualization_factor : float
    ///     Positive periods-per-year multiplier, such as ``252`` for daily data.
    ///
    /// Returns
    /// -------
    /// WeightingMethod
    ///     Inverse-volatility policy using one-unit total P&L.
    ///
    /// Notes
    /// -----
    /// This factory does not raise; window and anchor validation occurs in ``CompositeSpec``.
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

    /// Deserialize any canonical weighting policy, including expressions.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Strict weighting-method JSON using its ``kind`` discriminator.
    ///
    /// Returns
    /// -------
    /// WeightingMethod
    ///     Parsed canonical weighting policy.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If JSON is malformed or carries an unknown field or variant.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_json(json, "WeightingMethod").map(|inner| Self { inner })
    }

    /// Serialize the canonical weighting policy.
    ///
    /// Returns
    /// -------
    /// str
    ///     Strict tagged weighting-method JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization of the canonical Rust policy fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "WeightingMethod")
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

/// Explicit or calendar-based composite rebalance schedule.
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
    /// Require callers to invoke rebalance explicitly.
    ///
    /// Returns
    /// -------
    /// RebalanceRule
    ///     Manual rule with no scheduled dates.
    ///
    /// Notes
    /// -----
    /// This factory does not raise; it returns a fixed manual policy.
    #[staticmethod]
    fn manual() -> Self {
        Self {
            inner: RebalanceRule::Manual,
        }
    }

    /// Schedule rebalances on strictly increasing dates.
    ///
    /// Parameters
    /// ----------
    /// dates : list[datetime.date | datetime.datetime | pandas.Timestamp | str]
    ///     Rebalance dates (ISO-8601 strings or date-like objects); duplicates
    ///     and descending dates are rejected.
    ///
    /// Returns
    /// -------
    /// RebalanceRule
    ///     Validated explicit-date schedule.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a date is invalid or the sequence is not strictly increasing.
    #[staticmethod]
    fn dates(dates: Vec<Bound<'_, PyAny>>) -> PyResult<Self> {
        let dates = dates
            .iter()
            .map(crate::bindings::date_utils::extract_date)
            .collect::<PyResult<Vec<_>>>()?;
        let inner = RebalanceRule::Dates { dates };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Build a calendar-adjusted daily, weekly, monthly, or quarterly cadence.
    ///
    /// Parameters
    /// ----------
    /// start : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Unadjusted schedule start date (date-like or ISO-8601 string).
    /// frequency : str
    ///     One of ``daily``, ``weekly``, ``monthly``, or ``quarterly``.
    /// calendar_id : str
    ///     Registered calendar identifier such as ``weekends_only``.
    /// business_day_convention : str
    ///     Canonical convention such as ``following`` or ``modified_following``.
    /// end : datetime.date | datetime.datetime | pandas.Timestamp | str | None
    ///     Optional final date; omit for an open-ended cadence.
    ///
    /// Returns
    /// -------
    /// RebalanceRule
    ///     Validated calendar-aware schedule.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If dates, enums, bounds, or the calendar identifier are invalid.
    #[staticmethod]
    #[pyo3(signature = (start, frequency, calendar_id, business_day_convention, end=None))]
    fn calendar(
        start: &Bound<'_, PyAny>,
        frequency: &str,
        calendar_id: &str,
        business_day_convention: &str,
        end: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let inner = RebalanceRule::Calendar {
            start: crate::bindings::date_utils::extract_date(start)?,
            end: end
                .filter(|value| !value.is_none())
                .map(crate::bindings::date_utils::extract_date)
                .transpose()?,
            frequency: super::instruments::enum_from_str::<RebalanceFrequency>(
                frequency,
                "rebalance frequency",
            )?,
            calendar_id: calendar_id.to_string(),
            business_day_convention: super::instruments::enum_from_str(
                business_day_convention,
                "business-day convention",
            )?,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Deserialize and validate a canonical rebalance rule.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Strict tagged rebalance-rule JSON.
    ///
    /// Returns
    /// -------
    /// RebalanceRule
    ///     Parsed and validated scheduling policy.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If JSON, dates, schedule ordering, or calendar lookup is invalid.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: RebalanceRule = parse_json(json, "RebalanceRule")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize the canonical tagged rebalance rule.
    ///
    /// Returns
    /// -------
    /// str
    ///     Strict rebalance-rule JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization of the canonical Rust rule fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "RebalanceRule")
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

/// Unresolved composite economic definition and future policy.
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
    /// Construct and validate a self-contained composite specification.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Stable composite identifier used for pricing and serialization.
    /// reporting_currency : Currency
    ///     Currency used for capital, values, risk, P&L, and return reporting.
    /// capital : Money
    ///     Positive return denominator in exactly ``reporting_currency``.
    /// legs : list[CompositeLegSpec]
    ///     At least two unique signed legs with matching embedded identifiers.
    /// weighting_method : WeightingMethod
    ///     Policy used only during initialization or explicit rebalance.
    /// rebalance_rule : RebalanceRule
    ///     Manual or scheduled rule controlling state transitions.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If any specification invariant or embedded definition is invalid.
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

    /// Deserialize and validate a bare composite specification.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Bare strict ``CompositeSpec`` JSON produced by ``to_json``.
    ///
    /// Returns
    /// -------
    /// CompositeSpec
    ///     Parsed unresolved economic definition.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If JSON or any nested specification invariant is invalid.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: CompositeSpec = parse_json(json, "CompositeSpec")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize this unresolved definition as bare JSON.
    ///
    /// Returns
    /// -------
    /// str
    ///     Strict ``CompositeSpec`` JSON with embedded instruments.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If canonical serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "CompositeSpec")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Return the stable composite identifier.
    ///
    /// Returns
    /// -------
    /// str
    ///     Identifier stored on the unresolved specification.
    ///
    /// Notes
    /// -----
    /// This accessor does not raise; it returns the stored identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
    }

    /// Return the ISO code used for values, risk, P&L, and returns.
    ///
    /// Returns
    /// -------
    /// str
    ///     Three-letter reporting-currency code.
    ///
    /// Notes
    /// -----
    /// This accessor does not raise; it returns the validated stored currency.
    #[getter]
    fn reporting_currency(&self) -> String {
        self.inner.reporting_currency.to_string()
    }

    /// Return the capital denominator (``Money`` in the reporting currency).
    #[getter]
    fn capital(&self) -> PyMoney {
        PyMoney {
            inner: self.inner.capital,
        }
    }

    /// Return the signed leg definitions in specification order.
    #[getter]
    fn legs(&self) -> Vec<PyCompositeLegSpec> {
        self.inner
            .legs
            .iter()
            .map(|leg| PyCompositeLegSpec { inner: leg.clone() })
            .collect()
    }

    /// Return the weighting policy.
    #[getter]
    fn weighting_method(&self) -> PyWeightingMethod {
        PyWeightingMethod {
            inner: self.inner.weighting_method.clone(),
        }
    }

    /// Return the rebalance rule.
    #[getter]
    fn rebalance_rule(&self) -> PyRebalanceRule {
        PyRebalanceRule {
            inner: self.inner.rebalance_rule.clone(),
        }
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

    /// Resolve immutable quantities from information available through a date.
    ///
    /// There is no separate ``initialize_fixed`` binding. ``fixed_quantity``
    /// specs resolve through this method and do not require historical
    /// observations. Volatility weighting requires ``history`` to end on
    /// ``as_of``.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Complete current market object or canonical market JSON.
    /// as_of : datetime.date | str
    ///     Effective date as a date-like value or ISO-8601 string.
    /// history : list[dict] | str | None
    ///     Strict chronological ``CompositeMarketObservation`` array (list of
    ///     dicts or JSON string). ``None`` means no history.
    ///
    /// Returns
    /// -------
    /// CompositeRebalanceResult
    ///     New priceable instrument and primitive establishment trades.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If validation, history, metric, notional, FX, or quantity resolution fails.
    #[pyo3(signature = (market, as_of, history=None))]
    fn initialize(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        history: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyCompositeRebalanceResult> {
        let market = extract_market(py, market)?;
        let as_of = crate::bindings::date_utils::extract_date(as_of)?;
        let history = observations_from_py(py, history, "composite market history")?;
        self.inner
            .initialize(&market, as_of, &history)
            .map(PyCompositeRebalanceResult::from_inner)
            .map_err(core_to_py)
    }
}

/// Frozen resolved quantities and their effective date.
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
    /// Deserialize a bare resolved-state object.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Strict state JSON produced by ``to_json``.
    ///
    /// Returns
    /// -------
    /// CompositeState
    ///     Parsed immutable state data.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If JSON does not match the strict state schema.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_json(json, "CompositeState").map(|inner| Self { inner })
    }

    /// Serialize the frozen state as canonical JSON.
    ///
    /// Returns
    /// -------
    /// str
    ///     State effective date, resolved legs, and finite weighting inputs.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If canonical serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "CompositeState")
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Return the ISO date from which these quantities are held.
    ///
    /// Returns
    /// -------
    /// str
    ///     Effective date formatted as ``YYYY-MM-DD``.
    ///
    /// Notes
    /// -----
    /// This accessor does not raise; it returns the stored state date.
    #[getter]
    fn effective_date(&self) -> String {
        self.inner.effective_date.to_string()
    }

    /// Return signed top-level quantities keyed by leg identifier.
    ///
    /// Returns
    /// -------
    /// dict[str, float]
    ///     New mapping from top-level leg IDs to frozen signed quantities.
    ///
    /// Notes
    /// -----
    /// This accessor does not raise; it copies the validated resolved legs.
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CompositeState(effective_date='{}', legs={})",
            self.inner.effective_date,
            self.inner.resolved_legs.len()
        )
    }

    #[getter]
    fn resolved_quantities(&self) -> std::collections::BTreeMap<String, f64> {
        self.inner
            .resolved_legs
            .iter()
            .map(|leg| (leg.instrument_id.to_string(), leg.quantity))
            .collect()
    }
}

/// Priceable composite with an immutable resolved state.
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

impl PyCompositeInstrument {
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        composite_envelope_json(&self.inner)
    }
}

#[pymethods]
impl PyCompositeInstrument {
    /// Deserialize and validate a canonical composite instrument envelope.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Required ``finstack_quant.instrument/1`` composite envelope.
    ///
    /// Returns
    /// -------
    /// CompositeInstrument
    ///     Parsed priceable resolved composite.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If JSON is malformed, non-composite, unresolved, or internally inconsistent.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_composite_envelope(json).map(|inner| Self { inner })
    }

    /// Serialize as the canonical instrument envelope accepted by pricing APIs.
    ///
    /// Returns
    /// -------
    /// str
    ///     Validated ``finstack_quant.instrument/1`` composite JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If canonical serialization fails.
    fn to_json(&self) -> PyResult<String> {
        self.envelope_json()
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Return the stable composite identifier.
    ///
    /// Returns
    /// -------
    /// str
    ///     Identifier stored on the composite specification.
    ///
    /// Notes
    /// -----
    /// This accessor does not raise; it returns the stored identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.spec.id.to_string()
    }

    /// Return a clone of the unresolved economic definition.
    ///
    /// Returns
    /// -------
    /// CompositeSpec
    ///     Independent wrapper around a cloned specification.
    ///
    /// Notes
    /// -----
    /// This accessor does not raise; cloning preserves the immutable definition.
    #[getter]
    fn spec(&self) -> PyCompositeSpec {
        PyCompositeSpec {
            inner: self.inner.spec.clone(),
        }
    }

    /// Return a clone of the immutable resolved holdings state.
    ///
    /// Returns
    /// -------
    /// CompositeState
    ///     Independent wrapper around the frozen effective-date state.
    ///
    /// Notes
    /// -----
    /// This accessor does not raise; cloning cannot rebalance the instrument.
    #[getter]
    fn state(&self) -> PyCompositeState {
        PyCompositeState {
            inner: self.inner.state.clone(),
        }
    }

    /// Explicitly return a distinct resolved state and primitive trade deltas.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Complete rebalance-date market object or canonical JSON.
    /// as_of : datetime.date | str
    ///     Effective date for the new state.
    /// history : list[dict] | str | None
    ///     Strict chronological observation array (list of dicts or JSON
    ///     string) available through ``as_of``; ``None`` means no history.
    ///
    /// Returns
    /// -------
    /// CompositeRebalanceResult
    ///     New immutable instrument plus net primitive quantity deltas.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If market/history inputs or quantity resolution are invalid.
    #[pyo3(signature = (market, as_of, history=None))]
    fn rebalance(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        history: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyCompositeRebalanceResult> {
        let market = extract_market(py, market)?;
        let as_of = crate::bindings::date_utils::extract_date(as_of)?;
        let history = observations_from_py(py, history, "composite market history")?;
        self.inner
            .rebalance(&market, as_of, &history)
            .map(PyCompositeRebalanceResult::from_inner)
            .map_err(core_to_py)
    }

    /// Price recursive primitive paths and report net/gross value and risk.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Complete valuation and FX market context.
    /// as_of : datetime.date | str
    ///     Valuation date used for prices, metrics, and FX conversion.
    /// metrics : list[str] | None
    ///     Additive metric IDs; normalized non-additive measures are rejected.
    ///
    /// Returns
    /// -------
    /// CompositeExposureReport
    ///     Path-level and primitive net/gross concentration report.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If state, metrics, market data, FX, or primitive pricing are invalid.
    #[pyo3(signature = (market, as_of, metrics=None))]
    fn primitive_exposures(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        metrics: Option<Vec<String>>,
    ) -> PyResult<PyCompositeExposureReport> {
        let market = extract_market(py, market)?;
        let as_of = crate::bindings::date_utils::extract_date(as_of)?;
        let metrics = metrics
            .unwrap_or_default()
            .into_iter()
            .map(MetricId::try_from)
            .collect::<Result<Vec<_>, _>>()
            .map_err(core_to_py)?;
        self.inner
            .primitive_exposure_report(&market, as_of, &metrics)
            .map(PyCompositeExposureReport::from_inner)
            .map_err(core_to_py)
    }

    /// Flatten target holdings or a transition into primitive quantity deltas.
    ///
    /// Parameters
    /// ----------
    /// previous : CompositeInstrument | None
    ///     Prior resolved state, or ``None`` for establishment trades.
    ///
    /// Returns
    /// -------
    /// list[dict]
    ///     One ``{"instrument_id", "instrument_type", "quantity_delta"}`` dict
    ///     per primitive, with signed quantity deltas.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If either state is invalid or primitive definitions conflict.
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

    /// JSON twin of ``execution_trades``.
    ///
    /// Parameters
    /// ----------
    /// previous : CompositeInstrument | None
    ///     Prior resolved state, or ``None`` for establishment trades.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON array of primitive identifiers, types, and signed quantity deltas.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If either state is invalid or primitive definitions conflict.
    #[pyo3(signature = (previous=None))]
    fn execution_trades_json(&self, previous: Option<&PyCompositeInstrument>) -> PyResult<String> {
        let trades = self
            .inner
            .execution_trades(previous.map(|value| &value.inner))
            .map_err(core_to_py)?;
        to_json(&trades, "composite execution trades")
    }

    /// ``execution_trades`` as a pandas ``DataFrame``.
    ///
    /// Parameters
    /// ----------
    /// previous : CompositeInstrument | None
    ///     Prior resolved state, or ``None`` for establishment trades.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     Columns ``instrument_id``, ``instrument_type``, ``quantity_delta``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If either state is invalid or primitive definitions conflict.
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

/// New resolved instrument plus flattened primitive trades.
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
    /// Deserialize a complete resolved instrument and primitive trade list.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Strict JSON produced by ``to_json``.
    ///
    /// Returns
    /// -------
    /// CompositeRebalanceResult
    ///     Parsed immutable instrument and its primitive execution deltas.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If JSON is malformed or the embedded composite state is invalid.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: CompositeRebalanceResult = parse_json(json, "CompositeRebalanceResult")?;
        inner.instrument.validate_invariants().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Return the newly resolved priceable composite instrument.
    ///
    /// Returns
    /// -------
    /// CompositeInstrument
    ///     Independent wrapper around the new immutable resolved state.
    ///
    /// Notes
    /// -----
    /// This accessor does not raise; it clones the stored result instrument.
    #[getter]
    fn instrument(&self) -> PyCompositeInstrument {
        PyCompositeInstrument {
            inner: self.inner.instrument.clone(),
        }
    }

    /// Return net primitive quantity deltas as a JSON array.
    ///
    /// Returns
    /// -------
    /// str
    ///     Primitive IDs, type tags, and signed quantity deltas.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If canonical JSON serialization fails.
    #[getter]
    fn trades_json(&self) -> PyResult<String> {
        to_json(&self.inner.trades, "composite rebalance trades")
    }

    /// Return net primitive quantity deltas as a list of dicts.
    ///
    /// Returns
    /// -------
    /// list[dict]
    ///     ``{"instrument_id", "instrument_type", "quantity_delta"}`` per primitive.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If canonical serialization fails.
    #[getter]
    fn trades<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.trades)
    }

    /// Export primitive execution deltas as a pandas ``DataFrame``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.trades, COMPOSITE_TRADE_COLUMNS)
    }

    /// Serialize the complete rebalance result.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON containing the resolved instrument data and primitive trades.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If canonical serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "CompositeRebalanceResult")
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

/// Primitive path, net, and gross exposure report.
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
    /// Deserialize primitive paths and net/gross aggregate exposures.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Strict JSON produced by ``to_json``.
    ///
    /// Returns
    /// -------
    /// CompositeExposureReport
    ///     Parsed report in its declared reporting currency.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If JSON is malformed or does not match the report contract.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_json(json, "CompositeExposureReport").map(|inner| Self { inner })
    }

    /// Reporting currency shared by all aggregate values.
    #[getter]
    fn reporting_currency(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.reporting_currency)
    }

    /// Number of primitive paths before overlap netting.
    #[getter]
    fn path_count(&self) -> usize {
        self.inner.paths.len()
    }

    /// Number of net/gross primitive aggregates.
    #[getter]
    fn aggregate_count(&self) -> usize {
        self.inner.aggregates.len()
    }

    /// Export net and gross primitive aggregates as a pandas ``DataFrame``.
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

    /// Serialize paths and aggregate quantity, value, and additive risk.
    ///
    /// Returns
    /// -------
    /// str
    ///     Canonical exposure-report JSON in the composite reporting currency.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If canonical serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "CompositeExposureReport")
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

/// Chronological composite history rows.
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

#[pymethods]
impl PyCompositeHistoryResult {
    /// Deserialize a chronological array of composite history rows.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Strict history-row array JSON produced by ``to_json``.
    ///
    /// Returns
    /// -------
    /// CompositeHistoryResult
    ///     Parsed immutable row collection.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If JSON is malformed or a row violates its serialized contract.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_json(json, "composite history").map(|inner| Self { inner })
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

    /// Observation dates in chronological order.
    #[getter]
    fn dates(&self) -> Vec<String> {
        self.inner.iter().map(|row| row.date.to_string()).collect()
    }

    /// Export chronological value, cashflow, P&L, return, and state metadata.
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

    /// Serialize one zero-based dated history row.
    ///
    /// Parameters
    /// ----------
    /// index : int
    ///     Zero-based row index in chronological order.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON for the selected value, P&L, return, exposure, and trade row.
    ///
    /// Raises
    /// ------
    /// IndexError
    ///     If ``index`` is outside the result bounds.
    /// ValueError
    ///     If the selected row cannot be serialized.
    fn row_json(&self, index: usize) -> PyResult<String> {
        let row = self.inner.get(index).ok_or_else(|| {
            pyo3::exceptions::PyIndexError::new_err(format!(
                "history row index {index} is out of range"
            ))
        })?;
        to_json(row, "CompositeHistoryRow")
    }

    /// Serialize every dated history row as a JSON array.
    ///
    /// Returns
    /// -------
    /// str
    ///     Chronological array containing values, cashflows, P&L, returns, indices, exposures, and trades.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If canonical serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "composite history")
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

/// Dated-market engine for composite P&L, returns, exposures, and rebalances.
#[pyclass(
    name = "CompositeHistoryEngine",
    module = "finstack_quant.valuations.composite",
    frozen,
    skip_from_py_object
)]
pub struct PyCompositeHistoryEngine;

#[pymethods]
impl PyCompositeHistoryEngine {
    /// Initialize at the first observation and calculate chronological rows.
    ///
    /// Warmup observations feed dynamic weighting only. The first output row
    /// has ``return_index = 100`` and zero P&L. Scheduled rebalances are
    /// close-effective.
    ///
    /// Parameters
    /// ----------
    /// spec : CompositeSpec
    ///     Unresolved definition initialized using only available warmup and first-date information.
    /// observations : list[dict] | str
    ///     Non-empty strictly increasing complete market-observation array
    ///     (list of dicts or JSON string).
    /// warmup : list[dict] | str | None
    ///     Optional strictly earlier complete observations used for weighting only.
    /// metrics : list[str] | None
    ///     Optional canonical additive metric keys included on every output row.
    ///
    /// Returns
    /// -------
    /// CompositeHistoryResult
    ///     Dated value, cashflow, P&L, return, index, exposure, state, and trade rows.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If metrics, observations, warmup, initialization, pricing, FX, or rebalancing fail.
    #[staticmethod]
    #[pyo3(signature = (spec, observations, warmup=None, metrics=None))]
    fn run_from_spec(
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
        CompositeHistoryEngine::run_from_spec(&spec.inner, &warmup, &observations, &metrics)
            .map(|inner| PyCompositeHistoryResult { inner })
            .map_err(core_to_py)
    }

    /// Calculate chronological rows from an already-resolved initial state.
    ///
    /// Period return is ``pnl / capital``. The initial effective date must be
    /// on or before the first observation.
    ///
    /// Parameters
    /// ----------
    /// instrument : CompositeInstrument
    ///     Immutable resolved state held from the first supplied observation.
    /// observations : list[dict] | str
    ///     Non-empty strictly increasing complete market-observation array
    ///     (list of dicts or JSON string).
    /// metrics : list[str] | None
    ///     Optional canonical additive metric keys included on every output row.
    ///
    /// Returns
    /// -------
    /// CompositeHistoryResult
    ///     Dated total-return rows with close-effective rebalance transitions.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If metrics, state, observations, market inputs, pricing, FX, or rebalancing fail.
    #[staticmethod]
    #[pyo3(signature = (instrument, observations, metrics=None))]
    fn run(
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
        CompositeHistoryEngine::run(&instrument.inner, &observations, &metrics)
            .map(|inner| PyCompositeHistoryResult { inner })
            .map_err(core_to_py)
    }
}

pub(crate) const EXPORTS: &[&str] = &[
    "CompositeExposureReport",
    "CompositeHistoryEngine",
    "CompositeHistoryResult",
    "CompositeInstrument",
    "CompositeLegSpec",
    "CompositeRebalanceResult",
    "CompositeSpec",
    "CompositeState",
    "RebalanceRule",
    "WeightingMethod",
];

/// Register the ``finstack_quant.valuations.composite`` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "composite")?;
    let qual = crate::bindings::module_utils::set_submodule_package_by_package(
        parent,
        &module,
        "composite",
        "finstack_quant.valuations",
    )?;
    module.setattr(
        "__doc__",
        "Resolved cross-asset composite instruments, primitive exposures, and dated history. Pricing uses frozen quantities; initialize covers fixed_quantity without a separate initialize_fixed binding.",
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
    module.add_class::<PyCompositeHistoryEngine>()?;
    module.setattr("__all__", PyList::new(py, EXPORTS)?)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &module, &qual)
}
