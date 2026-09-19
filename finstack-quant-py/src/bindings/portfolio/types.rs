//! Typed `#[pyclass]` wrappers for portfolio runtime objects.
//!
//! These wrappers let callers hold a built `Portfolio`, `PortfolioValuation`,
//! or `PortfolioResult` in Python and pass it back into pipeline functions
//! without paying the JSON round-trip cost on every call. Pipeline functions
//! accept either the typed object or a JSON string via the `*Access` helpers
//! in [`crate::bindings::extract`].

use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_instrument_json;
use crate::bindings::module_utils::{py_to_json_value, py_to_serde};
use crate::bindings::pandas_utils::{dict_to_dataframe, serde_to_py, table_to_dataframe};
use crate::errors::{core_to_py, display_to_py, portfolio_to_py, value_error};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::CurveId;
use finstack_quant_portfolio::builder::PortfolioBuilder;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::{AttributeValue, Entity, DUMMY_ENTITY_ID};

/// Built runtime portfolio: entities, positions with live instruments, and
/// the derived lookup indices.
///
/// Cheap to clone (wraps an ``Arc``). Build it once — with
/// ``Portfolio.builder(...)`` for typed construction or ``Portfolio.from_spec``
/// for a canonical ``PortfolioSpec`` JSON document — and reuse it across
/// ``value_portfolio``, ``aggregate_full_cashflows``, ``aggregate_metrics``
/// and ``scenario_pnl`` to avoid repeating position materialization and index
/// construction.
///
/// Examples
/// --------
/// >>> import datetime as dt
/// >>> from finstack_quant.portfolio import Portfolio
/// >>> pf = Portfolio.builder("book", "USD", dt.date(2025, 1, 1)).build()
/// >>> (pf.id, pf.base_currency, pf.as_of, len(pf))
/// ('book', 'USD', datetime.date(2025, 1, 1), 0)
#[pyclass(
    name = "Portfolio",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPortfolio {
    pub(crate) inner: Arc<finstack_quant_portfolio::Portfolio>,
}

impl PyPortfolio {
    pub(crate) fn from_inner(inner: finstack_quant_portfolio::Portfolio) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }
}

#[pymethods]
impl PyPortfolio {
    /// Start a typed portfolio builder.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Portfolio identifier.
    /// base_currency : Currency | str
    ///     Reporting currency (ISO-4217 code or ``Currency``) used for every
    ///     base-currency rollup.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date; ISO 8601 strings are accepted.
    ///
    /// Returns
    /// -------
    /// PortfolioBuilder
    ///     Fluent builder; call ``.position(...)`` / ``.entity(...)`` /
    ///     ``.tag(...)`` and finish with ``.build()``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``base_currency`` is not a valid ISO-4217 code or ``as_of`` is
    ///     not a date.
    #[staticmethod]
    #[pyo3(text_signature = "(id, base_currency, as_of)")]
    fn builder(
        id: &str,
        base_currency: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyPortfolioBuilder> {
        let ccy = extract_currency(base_currency)?;
        let date = extract_date(as_of)?;
        Ok(PyPortfolioBuilder {
            inner: Some(PortfolioBuilder::new(id).base_currency(ccy).as_of(date)),
        })
    }

    /// Build a portfolio from a JSON ``PortfolioSpec``.
    ///
    /// This performs position materialization, rebuilds the position and
    /// dependency indices, and validates the result. Hold the returned
    /// object and pass it directly to pipeline functions to avoid repeating
    /// this work.
    #[staticmethod]
    #[pyo3(text_signature = "(spec_json)")]
    fn from_spec(py: Python<'_>, spec_json: &str) -> PyResult<Self> {
        let spec_json = spec_json.to_owned();
        let spec: finstack_quant_portfolio::portfolio::PortfolioSpec = py
            .detach(move || serde_json::from_str(&spec_json))
            .map_err(display_to_py)?;
        let inner = py
            .detach(move || finstack_quant_portfolio::Portfolio::from_spec(spec))
            .map_err(portfolio_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through ``to_spec_json`` / ``from_spec`` — the same
    /// strict serde round-trip as the wire format — so an unpickled portfolio
    /// is rebuilt (positions materialized, indices rebuilt) exactly as if it
    /// had been loaded from its canonical ``PortfolioSpec``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let payload = self.to_spec_json(py)?;
        let from_spec = py.get_type::<Self>().getattr("from_spec")?;
        crate::bindings::pickle_support::reduce_via_json(from_spec, payload)
    }

    /// Structural equality: two portfolios are equal when their canonical
    /// ``PortfolioSpec`` JSON documents are identical.
    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.cast::<Self>() else {
            return Ok(false);
        };
        let other = other.borrow();
        if Arc::ptr_eq(&self.inner, &other.inner) {
            return Ok(true);
        }
        Ok(self.to_spec_json(py)? == other.to_spec_json(py)?)
    }

    /// Portfolio identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Human-readable portfolio name, or ``None`` when unset.
    #[getter]
    fn name(&self) -> Option<String> {
        self.inner.name.clone()
    }

    /// Valuation date as a ``datetime.date``.
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    /// Base currency code.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    /// Portfolio-level tags as a ``dict[str, str]`` in insertion order.
    #[getter]
    fn tags<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (key, value) in &self.inner.tags {
            out.set_item(key, value)?;
        }
        Ok(out)
    }

    /// Portfolio-level metadata as a JSON-shaped ``dict``.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Entity identifiers in registration order (includes the auto-created
    /// standalone entity when any position uses it).
    #[getter]
    fn entity_ids(&self) -> Vec<String> {
        self.inner
            .entities
            .keys()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    /// Position identifiers in portfolio order.
    #[getter]
    fn position_ids(&self) -> Vec<String> {
        self.inner
            .positions()
            .iter()
            .map(|p| p.position_id.as_str().to_owned())
            .collect()
    }

    /// Number of positions in the portfolio.
    fn __len__(&self) -> usize {
        self.inner.positions().len()
    }

    /// Round-trip the portfolio back to its JSON spec form.
    #[pyo3(text_signature = "(self)")]
    fn to_spec_json(&self, py: Python<'_>) -> PyResult<String> {
        let portfolio = self.inner.as_ref();
        py.detach(|| serde_json::to_string(&portfolio.to_spec()))
            .map_err(display_to_py)
    }

    /// Export the position table as a pandas ``DataFrame``.
    ///
    /// One row per position in portfolio order. Columns: ``position_id``,
    /// ``entity_id``, ``instrument_id``, ``instrument_type``, ``quantity``,
    /// ``unit`` (serde name, e.g. ``"units"``, ``"face_value"``), ``book_id``
    /// (``None`` when unassigned).
    #[pyo3(text_signature = "(self)")]
    fn positions_to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let positions = self.inner.positions();
        let position_ids: Vec<&str> = positions.iter().map(|p| p.position_id.as_str()).collect();
        let entity_ids: Vec<&str> = positions.iter().map(|p| p.entity_id.as_str()).collect();
        let instrument_ids: Vec<&str> =
            positions.iter().map(|p| p.instrument_id.as_str()).collect();
        let instrument_types: Vec<String> = positions
            .iter()
            .map(|p| p.instrument.key().to_string())
            .collect();
        let quantities: Vec<f64> = positions.iter().map(|p| p.quantity).collect();
        let units: Vec<String> = positions
            .iter()
            .map(|p| unit_label(&p.unit))
            .collect::<PyResult<_>>()?;
        let book_ids: Vec<Option<String>> = positions
            .iter()
            .map(|p| p.book_id.as_ref().map(|b| b.to_string()))
            .collect();
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("entity_id", entity_ids)?;
        data.set_item("instrument_id", instrument_ids)?;
        data.set_item("instrument_type", instrument_types)?;
        data.set_item("quantity", quantities)?;
        data.set_item("unit", units)?;
        data.set_item("book_id", book_ids)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "Portfolio(id=\"{}\", as_of={}, base_currency={}, positions={})",
            self.inner.id,
            self.inner.as_of,
            self.inner.base_currency,
            self.inner.positions().len()
        )
    }
}

/// Serde label for a position unit (``"units"``, ``"face_value"``,
/// ``"percentage"`` or ``{"notional": ...}`` rendered as JSON).
fn unit_label(unit: &PositionUnit) -> PyResult<String> {
    let value = serde_json::to_value(unit).map_err(display_to_py)?;
    Ok(match value {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    })
}

/// Parse a position unit from ``None`` (units), a serde name string
/// (``"units"``, ``"face_value"``, ``"percentage"``) or a JSON-shaped dict
/// (``{"notional": "USD"}`` / ``{"notional": None}``).
fn extract_position_unit(
    py: Python<'_>,
    unit: Option<&Bound<'_, PyAny>>,
) -> PyResult<PositionUnit> {
    match unit {
        None => Ok(PositionUnit::Units),
        Some(obj) => {
            if let Ok(s) = obj.extract::<String>() {
                if s == "notional" {
                    return Ok(PositionUnit::Notional(None));
                }
                return serde_json::from_value(serde_json::Value::String(s.clone())).map_err(
                    |_| {
                        value_error(format!(
                            "unknown position unit {s:?}; expected one of units, notional, \
                         face_value, percentage or {{\"notional\": <currency>}}"
                        ))
                    },
                );
            }
            py_to_serde(py, obj, "position unit")
        }
    }
}

/// Fluent builder returned by ``Portfolio.builder(id, base_currency, as_of)``.
///
/// Every setter returns the same builder so calls chain; ``build()`` consumes
/// the builder, validates the portfolio (entity references, quantities) and
/// returns a reusable ``Portfolio``. Positions whose ``entity_id`` is omitted
/// are assigned to the standalone entity, which is created automatically.
///
/// Examples
/// --------
/// >>> import datetime as dt
/// >>> from finstack_quant.portfolio import Portfolio
/// >>> builder = Portfolio.builder("book", "USD", dt.date(2025, 1, 1))
/// >>> pf = builder.name("Desk book").entity("ACME").tag("desk", "rates").build()
/// >>> (pf.name, pf.entity_ids, pf.tags)
/// ('Desk book', ['ACME'], {'desk': 'rates'})
#[pyclass(
    name = "PortfolioBuilder",
    module = "finstack_quant.portfolio",
    skip_from_py_object
)]
pub struct PyPortfolioBuilder {
    inner: Option<PortfolioBuilder>,
}

impl PyPortfolioBuilder {
    fn take(&mut self) -> PyResult<PortfolioBuilder> {
        self.inner
            .take()
            .ok_or_else(|| value_error("PortfolioBuilder has already been consumed by build()"))
    }
}

#[pymethods]
impl PyPortfolioBuilder {
    /// Set the human-readable portfolio name.
    #[pyo3(text_signature = "(self, name)")]
    fn name<'py>(mut slf: PyRefMut<'py, Self>, name: &str) -> PyResult<PyRefMut<'py, Self>> {
        let builder = slf.take()?;
        slf.inner = Some(builder.name(name));
        Ok(slf)
    }

    /// Register an entity that positions can reference.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Entity identifier used by ``position(..., entity_id=id)``.
    /// name : str | None
    ///     Optional display name.
    /// tags : dict[str, str] | None
    ///     Optional entity tags for grouping and filtering.
    #[pyo3(signature = (id, name=None, tags=None), text_signature = "(self, id, name=None, tags=None)")]
    fn entity<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: &str,
        name: Option<&str>,
        tags: Option<HashMap<String, String>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let mut entity = Entity::new(id);
        if let Some(name) = name {
            entity = entity.with_name(name);
        }
        if let Some(tags) = tags {
            entity = entity.with_tags(tags);
        }
        let builder = slf.take()?;
        slf.inner = Some(builder.entity(entity));
        Ok(slf)
    }

    /// Add a position holding a typed instrument.
    ///
    /// Parameters
    /// ----------
    /// position_id : str
    ///     Unique position identifier.
    /// instrument : Bond | InterestRateSwap | ... | str
    ///     Any typed instrument wrapper, or a canonical instrument-envelope
    ///     JSON string. The instrument's own ``id`` becomes ``instrument_id``.
    /// quantity : float
    ///     Signed holding; its meaning follows ``unit``. Must be finite.
    /// entity_id : str | None
    ///     Owning entity registered via ``entity(...)``; ``None`` assigns the
    ///     position to the auto-created standalone entity.
    /// unit : str | dict | None
    ///     Position unit: ``"units"`` (default), ``"face_value"``,
    ///     ``"percentage"``, ``"notional"`` or ``{"notional": "USD"}`` for a
    ///     currency-tagged lot multiplier.
    /// attributes : dict[str, str | float] | None
    ///     Position attributes (rating, sector, scores) used by grouping and
    ///     optimization filters.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``instrument`` is neither a typed instrument nor a JSON string.
    /// ValueError
    ///     If the instrument payload is invalid, ``quantity`` is not finite,
    ///     or ``unit`` is unknown.
    #[pyo3(
        signature = (position_id, instrument, quantity, entity_id=None, unit=None, attributes=None),
        text_signature = "(self, position_id, instrument, quantity, entity_id=None, unit=None, attributes=None)"
    )]
    fn position<'py>(
        mut slf: PyRefMut<'py, Self>,
        position_id: &str,
        instrument: &Bound<'py, PyAny>,
        quantity: f64,
        entity_id: Option<&str>,
        unit: Option<&Bound<'py, PyAny>>,
        attributes: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = slf.py();
        let envelope_json = extract_instrument_json(instrument)?;
        let envelope: finstack_quant_valuations::instruments::InstrumentEnvelope =
            serde_json::from_str(&envelope_json).map_err(display_to_py)?;
        let boxed = envelope.into_boxed().map_err(display_to_py)?;
        let instrument_id = boxed.id().to_owned();
        let unit = extract_position_unit(py, unit)?;
        let attributes: IndexMap<String, AttributeValue> = match attributes {
            Some(dict) => py_to_serde(py, dict.as_any(), "position attributes")?,
            None => IndexMap::new(),
        };
        let mut position = Position::new(
            position_id,
            entity_id.unwrap_or(DUMMY_ENTITY_ID),
            instrument_id,
            Arc::from(boxed),
            quantity,
            unit,
        )
        .map_err(portfolio_to_py)?;
        position.attributes = attributes;
        let builder = slf.take()?;
        slf.inner = Some(builder.position(position));
        Ok(slf)
    }

    /// Attach a portfolio-level tag.
    #[pyo3(text_signature = "(self, key, value)")]
    fn tag<'py>(
        mut slf: PyRefMut<'py, Self>,
        key: &str,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let builder = slf.take()?;
        slf.inner = Some(builder.tag(key, value));
        Ok(slf)
    }

    /// Attach a JSON-shaped metadata entry (any ``json.dumps``-able value).
    #[pyo3(text_signature = "(self, key, value)")]
    fn meta<'py>(
        mut slf: PyRefMut<'py, Self>,
        key: &str,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = slf.py();
        let value = py_to_json_value(py, value, "portfolio metadata")?;
        let builder = slf.take()?;
        slf.inner = Some(builder.meta(key, value));
        Ok(slf)
    }

    /// Validate and build the portfolio, consuming the builder.
    ///
    /// Raises
    /// ------
    /// PortfolioError
    ///     If a position references an unregistered entity or another
    ///     structural invariant fails.
    /// ValueError
    ///     If the builder was already consumed.
    #[pyo3(text_signature = "(self)")]
    fn build(&mut self, py: Python<'_>) -> PyResult<PyPortfolio> {
        let builder = self.take()?;
        let inner = py
            .detach(move || builder.build())
            .map_err(portfolio_to_py)?;
        Ok(PyPortfolio::from_inner(inner))
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            Some(_) => "PortfolioBuilder(...)".to_owned(),
            None => "PortfolioBuilder(<consumed>)".to_owned(),
        }
    }
}

/// Valuation of a single position inside a ``PortfolioValuation``.
///
/// Carries the native- and base-currency values, the linear scale applied to
/// summable risk metrics, and the risk-completeness diagnostics recorded when
/// a position fell back to PV-only valuation.
#[pyclass(
    name = "PositionValue",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPositionValue {
    pub(crate) inner: finstack_quant_portfolio::valuation::PositionValue,
}

#[pymethods]
impl PyPositionValue {
    /// Support `pickle` via the same serde round-trip as ``to_json``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse a position value from canonical JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to canonical JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Position identifier.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Owning entity identifier.
    #[getter]
    fn entity_id(&self) -> String {
        self.inner.entity_id.as_str().to_owned()
    }

    /// Value in the instrument's native currency.
    #[getter]
    fn value_native(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.value_native)
    }

    /// Value converted to the portfolio base currency.
    #[getter]
    fn value_base(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.value_base)
    }

    /// Linear scale applied to summable risk metrics (position size and sign).
    #[getter]
    fn metric_scale(&self) -> f64 {
        self.inner.metric_scale
    }

    /// Whether every risk metric requested *of this position* was computed.
    ///
    /// Metrics narrowed away as inapplicable were never requested, so they do
    /// not clear this flag; see ``inapplicable_metrics``.
    #[getter]
    fn risk_metrics_complete(&self) -> bool {
        self.inner.risk_metrics_complete
    }

    /// Original metric-failure message when the valuation fell back to
    /// PV-only, otherwise ``None``.
    #[getter]
    fn risk_error(&self) -> Option<String> {
        self.inner.risk_error.clone()
    }

    /// Requested metrics this position's instrument type has no calculator
    /// for, in request order.
    #[getter]
    fn inapplicable_metrics(&self) -> Vec<String> {
        self.inner
            .inapplicable_metrics
            .iter()
            .map(|metric| metric.as_str().to_owned())
            .collect()
    }

    /// Full instrument ``ValuationResult`` as a JSON-shaped ``dict`` (metrics
    /// included), or ``None`` when it was not retained.
    #[getter]
    fn valuation_result<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .valuation_result
            .as_ref()
            .map(|r| serde_to_py(py, r))
            .transpose()
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionValue(position_id={:?}, value_base={} {}, risk_metrics_complete={})",
            self.inner.position_id.as_str(),
            self.inner.value_base.amount(),
            self.inner.value_base.currency(),
            if self.inner.risk_metrics_complete {
                "True"
            } else {
                "False"
            },
        )
    }
}

/// Complete portfolio valuation: per-position values, entity rollups and the
/// base-currency total, plus the FX policy stamp and degraded-risk
/// diagnostics.
///
/// Avoids re-parsing the (potentially large) valuation JSON every time a
/// downstream function (``aggregate_metrics``, ``PortfolioResult``) needs to
/// read from it.
#[pyclass(
    name = "PortfolioValuation",
    module = "finstack_quant.portfolio",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPortfolioValuation {
    pub(crate) inner: finstack_quant_portfolio::valuation::PortfolioValuation,
}

impl PyPortfolioValuation {
    pub(crate) fn from_inner(
        inner: finstack_quant_portfolio::valuation::PortfolioValuation,
    ) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioValuation {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let payload = self.to_json(py)?;
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, payload)
    }

    /// Parse a valuation from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(valuation_json)")]
    fn from_json(py: Python<'_>, valuation_json: &str) -> PyResult<Self> {
        let valuation_json = valuation_json.to_owned();
        let inner: finstack_quant_portfolio::valuation::PortfolioValuation = py
            .detach(move || serde_json::from_str(&valuation_json))
            .map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize back to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        let valuation = &self.inner;
        py.detach(|| serde_json::to_string(valuation))
            .map_err(display_to_py)
    }

    /// Total portfolio value in the base currency (amount).
    #[getter]
    fn total_value(&self) -> f64 {
        self.inner.total_base_currency.amount()
    }

    /// Base currency of the total.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.total_base_currency.currency().to_string()
    }

    /// Valuation date as a ``datetime.date``.
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    /// Per-position valuations keyed by position id, in valuation order.
    #[getter]
    fn position_values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (id, value) in &self.inner.position_values {
            out.set_item(
                id.as_str(),
                PyPositionValue {
                    inner: value.clone(),
                },
            )?;
        }
        Ok(out)
    }

    /// Base-currency totals by entity id, in valuation order.
    #[getter]
    fn by_entity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (id, money) in &self.inner.by_entity {
            out.set_item(id.as_str(), PyMoney::from_inner(*money))?;
        }
        Ok(out)
    }

    /// Positions whose valuation fell back to PV-only because a requested
    /// risk metric could not be computed.
    #[getter]
    fn degraded_positions(&self) -> Vec<String> {
        self.inner
            .degraded_positions
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    /// FX conversion policy stamped on the base-currency rollups (serde
    /// name, e.g. ``"cashflow_date"``).
    #[getter]
    fn fx_collapse_policy(&self) -> PyResult<String> {
        match serde_json::to_value(self.inner.fx_collapse_policy).map_err(display_to_py)? {
            serde_json::Value::String(s) => Ok(s),
            other => Ok(other.to_string()),
        }
    }

    /// Whether any position carries incomplete risk metrics.
    #[getter]
    fn has_degraded_risk(&self) -> bool {
        self.inner.has_degraded_risk()
    }

    /// Look up one position's valuation.
    ///
    /// Raises ``KeyError`` when ``position_id`` is not in the valuation.
    #[pyo3(text_signature = "(self, position_id)")]
    fn get_position_value(&self, position_id: &str) -> PyResult<PyPositionValue> {
        self.inner
            .get_position_value(position_id)
            .map(|value| PyPositionValue {
                inner: value.clone(),
            })
            .ok_or_else(|| PyKeyError::new_err(format!("position '{position_id}' not valued")))
    }

    /// Look up one entity's base-currency total.
    ///
    /// Raises ``KeyError`` when ``entity_id`` has no positions.
    #[pyo3(text_signature = "(self, entity_id)")]
    fn get_entity_value(&self, entity_id: &str) -> PyResult<PyMoney> {
        self.inner
            .get_entity_value(entity_id)
            .map(|money| PyMoney::from_inner(*money))
            .ok_or_else(|| PyKeyError::new_err(format!("entity '{entity_id}' not valued")))
    }

    /// Export per-position values via Arrow (zero-copy for consumers).
    ///
    /// Columns: ``position_id``, ``entity_id``, ``value_native``,
    /// ``value_base``, ``currency_native``, ``currency_base`` (see
    /// ``finstack_quant_portfolio::positions_to_table``). Returns an
    /// :class:`finstack_quant.core.table.ArrowTable`.
    #[pyo3(text_signature = "($self)")]
    fn to_arrow_positions(&self) -> PyResult<crate::bindings::core::table::PyArrowTable> {
        let table = finstack_quant_portfolio::positions_to_table(&self.inner)
            .map_err(crate::errors::core_to_py)?;
        crate::bindings::core::table::PyArrowTable::from_envelope(&table)
    }

    /// Export per-position values as a pandas ``DataFrame``.
    ///
    /// One row per entry of ``position_values``. Built from the same
    /// ``positions_to_table`` envelope that backs :meth:`to_arrow_positions`
    /// plus the per-position risk diagnostics.
    ///
    /// Columns: ``position_id``, ``entity_id``, ``value_native``,
    /// ``value_base``, ``currency_native``, ``currency_base``,
    /// ``risk_metrics_complete`` (bool), ``risk_error`` (``None`` unless the
    /// position degraded to PV-only). Values are floats and the currency codes
    /// are strings; use :meth:`to_arrow_positions` when a zero-copy handoff
    /// matters more than pandas ergonomics.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let table =
            finstack_quant_portfolio::positions_to_table(&self.inner).map_err(core_to_py)?;
        let frame = table_to_dataframe(py, &table)?;
        let complete: Vec<bool> = self
            .inner
            .position_values
            .values()
            .map(|v| v.risk_metrics_complete)
            .collect();
        let errors: Vec<Option<String>> = self
            .inner
            .position_values
            .values()
            .map(|v| v.risk_error.clone())
            .collect();
        // `assign` returns a new frame. Appending with `set_item` instead trips
        // pandas' copy-on-write heuristic (PyO3 holds an extra reference, so the
        // refcount check reads as chained assignment) and warns on every call.
        let columns = PyDict::new(py);
        columns.set_item("risk_metrics_complete", complete)?;
        columns.set_item("risk_error", errors)?;
        frame.call_method("assign", (), Some(&columns))
    }

    /// Number of position valuations in the result.
    fn __len__(&self) -> usize {
        self.inner.position_values.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioValuation(as_of={}, total={} {}, positions={}, degraded={})",
            self.inner.as_of,
            self.inner.total_base_currency.amount(),
            self.inner.total_base_currency.currency(),
            self.inner.position_values.len(),
            self.inner.degraded_positions.len(),
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

/// Combined portfolio result envelope: a ``PortfolioValuation``, the
/// aggregated ``PortfolioMetrics`` and the calculation metadata stamp.
///
/// Exposes cheap scalar accessors (``total_value``, ``get_metric``) that
/// avoid the full JSON re-parse previously required by the JSON-only API.
#[pyclass(
    name = "PortfolioResult",
    module = "finstack_quant.portfolio",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPortfolioResult {
    pub(crate) inner: finstack_quant_portfolio::results::PortfolioResult,
}

impl PyPortfolioResult {
    pub(crate) fn from_inner(inner: finstack_quant_portfolio::results::PortfolioResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioResult {
    /// Assemble a result envelope from a valuation and its aggregated metrics.
    ///
    /// Parameters
    /// ----------
    /// valuation : PortfolioValuation
    ///     Output of ``value_portfolio``.
    /// metrics : PortfolioMetrics
    ///     Output of ``aggregate_metrics`` for the same valuation.
    ///
    /// The ``meta`` stamp (numeric mode, rounding context, library version)
    /// is taken from the default ``FinstackConfig``.
    #[new]
    #[pyo3(text_signature = "(valuation, metrics)")]
    fn new(valuation: &PyPortfolioValuation, metrics: &PyPortfolioMetrics) -> Self {
        let config = finstack_quant_core::config::FinstackConfig::default();
        let meta = finstack_quant_core::config::results_meta_now(&config);
        Self::from_inner(finstack_quant_portfolio::results::PortfolioResult::new(
            valuation.inner.clone(),
            metrics.inner.clone(),
            meta,
        ))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let payload = self.to_json(py)?;
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, payload)
    }

    /// Parse a result from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(result_json)")]
    fn from_json(py: Python<'_>, result_json: &str) -> PyResult<Self> {
        let result_json = result_json.to_owned();
        let inner: finstack_quant_portfolio::results::PortfolioResult = py
            .detach(move || serde_json::from_str(&result_json))
            .map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize back to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        let result = &self.inner;
        py.detach(|| serde_json::to_string(result))
            .map_err(display_to_py)
    }

    /// The valuation component.
    #[getter]
    fn valuation(&self) -> PyPortfolioValuation {
        PyPortfolioValuation::from_inner(self.inner.valuation.clone())
    }

    /// The aggregated-metrics component.
    #[getter]
    fn metrics(&self) -> PyPortfolioMetrics {
        PyPortfolioMetrics::from_inner(self.inner.metrics.clone())
    }

    /// Calculation metadata stamp (numeric mode, rounding context, version)
    /// as a JSON-shaped ``dict``.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Total portfolio value in base currency.
    #[getter]
    fn total_value(&self) -> f64 {
        self.inner.total_value().amount()
    }

    /// Retrieve an aggregated metric by id. Returns ``None`` if absent.
    #[pyo3(text_signature = "(self, metric_id)")]
    fn get_metric(&self, metric_id: &str) -> Option<f64> {
        self.inner.get_metric(metric_id)
    }

    /// Retrieve a metric and raise ``KeyError`` if it is missing.
    #[pyo3(text_signature = "(self, metric_id)")]
    fn require_metric(&self, metric_id: &str) -> PyResult<f64> {
        self.inner
            .get_metric(metric_id)
            .ok_or_else(|| PyKeyError::new_err(format!("metric '{metric_id}' not present")))
    }

    fn __repr__(&self) -> String {
        let total = self.inner.total_value();
        format!(
            "PortfolioResult(total={} {})",
            total.amount(),
            total.currency(),
        )
    }
}

type PyMetricSeriesEntry = (Vec<String>, f64, Py<PyDict>);

/// Aggregated portfolio metrics: portfolio-wide totals by metric id, the raw
/// per-position values they were summed from, and the diagnostics recording
/// what was skipped or not summable.
#[pyclass(
    name = "PortfolioMetrics",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPortfolioMetrics {
    pub(crate) inner: finstack_quant_portfolio::metrics::PortfolioMetrics,
}

impl PyPortfolioMetrics {
    pub(crate) fn from_inner(inner: finstack_quant_portfolio::metrics::PortfolioMetrics) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioMetrics {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let payload = self.to_json(py)?;
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, payload)
    }

    /// Parse aggregate portfolio metrics from canonical JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(metrics_json)")]
    fn from_json(py: Python<'_>, metrics_json: &str) -> PyResult<Self> {
        let metrics_json = metrics_json.to_owned();
        let inner = py
            .detach(move || serde_json::from_str(&metrics_json))
            .map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize aggregate portfolio metrics to canonical JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        let metrics = &self.inner;
        py.detach(|| serde_json::to_string(metrics))
            .map_err(display_to_py)
    }

    /// Portfolio-wide aggregated metrics keyed by metric id.
    ///
    /// Each value carries ``metric_id``, ``total`` and the ``by_entity``
    /// breakdown, in canonical Rust ``IndexMap`` insertion order. Only summable
    /// metrics are aggregated.
    #[getter]
    fn aggregated<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.aggregated)
    }

    /// Raw per-position metric values keyed by position id.
    ///
    /// Each value carries the position's native ``currency`` — non-summable
    /// metrics are quoted in it — and its ``metrics`` mapping.
    #[getter]
    fn by_position<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.by_position)
    }

    /// One aggregated metric (``{"metric_id", "total", "by_entity"}``) or
    /// ``None`` when ``metric_id`` was not aggregated.
    #[pyo3(text_signature = "(self, metric_id)")]
    fn get_metric<'py>(
        &self,
        py: Python<'py>,
        metric_id: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .get_metric(metric_id)
            .map(|metric| serde_to_py(py, metric))
            .transpose()
    }

    /// One position's raw metrics (``{"currency", "metrics"}``) or ``None``
    /// when ``position_id`` is absent.
    #[pyo3(text_signature = "(self, position_id)")]
    fn get_position_metrics<'py>(
        &self,
        py: Python<'py>,
        position_id: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .get_position_metrics(position_id)
            .map(|metrics| serde_to_py(py, metrics))
            .transpose()
    }

    /// Portfolio total of one aggregated metric, or ``None`` when absent.
    #[pyo3(text_signature = "(self, metric_id)")]
    fn get_total(&self, metric_id: &str) -> Option<f64> {
        self.inner.get_total(metric_id)
    }

    /// Metric values excluded from aggregation because they were non-finite.
    ///
    /// A non-empty list means aggregation is incomplete; each entry records the
    /// ``position_id``, ``metric_id`` and the offending ``value``.
    #[getter]
    fn skipped_metrics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.skipped_metrics)
    }

    /// Positions that carried no risk measures because their valuation fell
    /// back to PV-only.
    ///
    /// Such positions contribute zero to every total without producing a
    /// ``skipped_metrics`` entry, so a non-empty list is the only signal that
    /// the aggregate is partial.
    #[getter]
    fn degraded_positions(&self) -> Vec<String> {
        self.inner
            .degraded_positions
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    /// Metric identifiers present in ``by_position`` that were not aggregated
    /// into a portfolio total because they are not summable across positions
    /// (for example ``ytm`` or ``duration``). Sorted and de-duplicated.
    #[getter]
    fn unaggregated_metrics(&self) -> Vec<String> {
        self.inner.unaggregated_metrics.clone()
    }

    /// Decoded components, total, and ordered entity breakdown by base metric.
    ///
    /// Despite the ``_series`` suffix (which mirrors the Rust name) this is a
    /// plain ``list`` of tuples, not a :class:`pandas.Series`. Use
    /// :meth:`to_dataframe` for the tabular view.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the base metric key is not canonically encoded.
    #[pyo3(text_signature = "(self, base)")]
    fn metric_series(&self, py: Python<'_>, base: &str) -> PyResult<Vec<PyMetricSeriesEntry>> {
        let base = base.parse().map_err(core_to_py)?;
        self.inner
            .metric_series(&base)
            .into_iter()
            .map(|(components, aggregate)| {
                let by_entity = PyDict::new(py);
                for (entity, value) in &aggregate.by_entity {
                    by_entity.set_item(entity.to_string(), value)?;
                }
                Ok((components, aggregate.total, by_entity.unbind()))
            })
            .collect()
    }

    /// Primary :class:`pandas.DataFrame` view: the aggregated metrics table.
    ///
    /// Delegates to :meth:`to_aggregated_dataframe`; the per-position long
    /// table remains available from :meth:`to_position_dataframe`.
    ///
    /// Columns: ``metric_id``, ``total``.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_aggregated_dataframe(py)
    }

    /// Export the portfolio-wide aggregated metrics as a pandas ``DataFrame``.
    ///
    /// One row per entry of the ``aggregated`` map, in canonical Rust
    /// ``IndexMap`` insertion order. Built from the canonical
    /// ``finstack_quant_portfolio::aggregated_metrics_to_table`` envelope so
    /// the frame cannot drift from the Rust table export. The per-entity
    /// breakdown is not flattened here — reach it through
    /// :meth:`metric_series`.
    ///
    /// Columns: ``metric_id``, ``total`` (sum across positions; only summable
    /// metrics are aggregated).
    #[pyo3(text_signature = "(self)")]
    fn to_aggregated_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let table = finstack_quant_portfolio::aggregated_metrics_to_table(&self.inner)
            .map_err(core_to_py)?;
        table_to_dataframe(py, &table)
    }

    /// Export the raw per-position metric values as a long-format pandas
    /// ``DataFrame``.
    ///
    /// One row per ``(position, metric)`` pair — the row count is the total
    /// number of metric values across positions, not the number of positions.
    /// Pivot with ``df.pivot(index="position_id", columns="metric_id",
    /// values="value")`` for a wide view. Built from the canonical
    /// ``finstack_quant_portfolio::metrics_to_table`` envelope so the frame
    /// cannot drift from the Rust table export.
    ///
    /// Columns: ``metric_id``, ``position_id``, ``currency`` (the position's
    /// native currency; non-summable metrics are quoted in it), ``value``.
    #[pyo3(text_signature = "(self)")]
    fn to_position_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let table = finstack_quant_portfolio::metrics_to_table(&self.inner).map_err(core_to_py)?;
        table_to_dataframe(py, &table)
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioMetrics(aggregated={}, positions={})",
            self.inner.aggregated.len(),
            self.inner.by_position.len(),
        )
    }

    /// Render as an HTML table in Jupyter notebooks (delegates to
    /// :meth:`to_dataframe`; ``None`` when the frame cannot be built).
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Classified portfolio cashflow ladder.
///
/// Holds the flat position-scaled ``events`` (sorted by payment date), the
/// per-position drill-down ``by_position``, the ``(date -> currency -> kind)``
/// totals ``by_date``, and the extraction ``issues`` recorded for positions
/// that could not produce a schedule. Returned by
/// :func:`aggregate_full_cashflows`; typed getters return JSON-shaped Python
/// objects and the ``*_json`` twins return the same payload as compact JSON.
#[pyclass(
    name = "PortfolioCashflows",
    module = "finstack_quant.portfolio",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPortfolioCashflows {
    pub(crate) inner: finstack_quant_portfolio::cashflows::PortfolioCashflows,
}

impl PyPortfolioCashflows {
    pub(crate) fn from_inner(
        inner: finstack_quant_portfolio::cashflows::PortfolioCashflows,
    ) -> Self {
        Self { inner }
    }

    fn collapse(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        base_currency: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        discount_curves: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<
        IndexMap<
            finstack_quant_core::dates::Date,
            IndexMap<finstack_quant_core::cashflow::CFKind, finstack_quant_core::money::Money>,
        >,
    > {
        let market = crate::bindings::extract::extract_market_ref(py, market)?;
        let ccy = extract_currency(base_currency)?;
        let as_of_date = extract_date(as_of)?;
        let curves = extract_discount_curve_map(discount_curves)?;
        let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
        let cashflows = &self.inner;
        py.detach(|| {
            cashflows.collapse_to_base_by_date_kind(market_ref, ccy, as_of_date, curves.as_ref())
        })
        .map_err(portfolio_to_py)
    }
}

#[pymethods]
impl PyPortfolioCashflows {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let payload = self.to_json(py)?;
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, payload)
    }

    /// Parse a cashflow ladder from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(cashflows_json)")]
    fn from_json(py: Python<'_>, cashflows_json: &str) -> PyResult<Self> {
        let cashflows_json = cashflows_json.to_owned();
        let inner: finstack_quant_portfolio::cashflows::PortfolioCashflows = py
            .detach(move || serde_json::from_str(&cashflows_json))
            .map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize the full ladder back to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        let cashflows = &self.inner;
        py.detach(|| serde_json::to_string(cashflows))
            .map_err(display_to_py)
    }

    /// Number of dated cashflow events.
    fn __len__(&self) -> usize {
        self.inner.events.len()
    }

    /// Number of positions represented in the ladder (contributing events or
    /// recorded as issues).
    #[getter]
    fn num_positions(&self) -> usize {
        self.inner.by_position.len()
    }

    /// Number of extraction issues recorded during aggregation.
    #[getter]
    fn num_issues(&self) -> usize {
        self.inner.issues.len()
    }

    /// Flat position-scaled events as a list of JSON-shaped dicts (payment
    /// date order). Each carries ``position_id``, ``instrument_id``,
    /// ``instrument_type``, ``date``, ``amount`` (``{"amount", "currency"}``),
    /// ``kind``, ``reset_date``, ``accrual_factor`` and ``rate``.
    #[getter]
    fn events<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.events)
    }

    /// Per-position event drill-down: ``dict[position_id, list[event]]``.
    #[getter]
    fn by_position<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.by_position)
    }

    /// Totals by payment date, then ISO currency, then cashflow kind:
    /// ``dict[date_iso, dict[currency, dict[kind, {"amount", "currency"}]]]``.
    #[getter]
    fn by_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.by_date)
    }

    /// Extraction issues as a list of JSON-shaped dicts (``position_id``,
    /// ``instrument_id``, ``instrument_type``, ``kind``, ``message``).
    #[getter]
    fn issues<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.issues)
    }

    /// FX policy stamped for :meth:`collapse_to_base_by_date_kind` (serde
    /// name).
    #[getter]
    fn fx_collapse_policy(&self) -> PyResult<String> {
        match serde_json::to_value(self.inner.fx_collapse_policy).map_err(display_to_py)? {
            serde_json::Value::String(s) => Ok(s),
            other => Ok(other.to_string()),
        }
    }

    /// JSON for the flat ``events`` vector only.
    #[pyo3(text_signature = "(self)")]
    fn events_json(&self, py: Python<'_>) -> PyResult<String> {
        let events = &self.inner.events;
        py.detach(|| serde_json::to_string(events))
            .map_err(display_to_py)
    }

    /// JSON for the ``by_date`` currency/kind totals only.
    #[pyo3(text_signature = "(self)")]
    fn by_date_json(&self, py: Python<'_>) -> PyResult<String> {
        let by_date = &self.inner.by_date;
        py.detach(|| serde_json::to_string(by_date))
            .map_err(display_to_py)
    }

    /// Export the flat ``events`` ladder as a pandas ``DataFrame``.
    ///
    /// One row per dated cashflow event, in the ladder's canonical
    /// payment-date order. ``amount`` is flattened to a float column plus a
    /// ``currency`` column rather than a nested dict, so a multi-currency
    /// ladder stays currency-safe: group by ``currency`` before summing.
    ///
    /// Columns: ``position_id``, ``instrument_id``, ``instrument_type``,
    /// ``date`` (ISO 8601 string), ``amount`` (position-scaled), ``currency``,
    /// ``kind`` (``"fixed"``, ``"float_reset"``, ``"notional"``, ...),
    /// ``reset_date`` (ISO 8601 string, ``None`` outside floating coupons),
    /// ``accrual_factor``, ``rate`` (``None`` when the event carries no rate).
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let events = &self.inner.events;
        let position_ids: Vec<&str> = events.iter().map(|e| e.position_id.as_str()).collect();
        let instrument_ids: Vec<&str> = events.iter().map(|e| e.instrument_id.as_str()).collect();
        let instrument_types: Vec<String> = events
            .iter()
            .map(|e| e.instrument_type.to_string())
            .collect();
        let dates: Vec<String> = events.iter().map(|e| e.date.to_string()).collect();
        let amounts: Vec<f64> = events.iter().map(|e| e.amount.amount()).collect();
        let currencies: Vec<String> = events
            .iter()
            .map(|e| e.amount.currency().to_string())
            .collect();
        let kinds: Vec<String> = events.iter().map(|e| e.kind.to_string()).collect();
        let reset_dates: Vec<Option<String>> = events
            .iter()
            .map(|e| e.reset_date.map(|d| d.to_string()))
            .collect();
        let accrual_factors: Vec<f64> = events.iter().map(|e| e.accrual_factor).collect();
        let rates: Vec<Option<f64>> = events.iter().map(|e| e.rate).collect();

        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("instrument_id", instrument_ids)?;
        data.set_item("instrument_type", instrument_types)?;
        data.set_item("date", dates)?;
        data.set_item("amount", amounts)?;
        data.set_item("currency", currencies)?;
        data.set_item("kind", kinds)?;
        data.set_item("reset_date", reset_dates)?;
        data.set_item("accrual_factor", accrual_factors)?;
        data.set_item("rate", rates)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Export the extraction ``issues`` as a pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``instrument_id``, ``instrument_type``,
    /// ``kind`` (issue category, e.g. ``"build_failed"``), ``message``.
    #[pyo3(text_signature = "(self)")]
    fn to_issues_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let issues = &self.inner.issues;
        let position_ids: Vec<&str> = issues.iter().map(|i| i.position_id.as_str()).collect();
        let instrument_ids: Vec<&str> = issues.iter().map(|i| i.instrument_id.as_str()).collect();
        let instrument_types: Vec<String> = issues
            .iter()
            .map(|i| i.instrument_type.to_string())
            .collect();
        let kinds: Vec<String> = issues
            .iter()
            .map(|i| {
                serde_json::to_value(i.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default()
            })
            .collect();
        let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("instrument_id", instrument_ids)?;
        data.set_item("instrument_type", instrument_types)?;
        data.set_item("kind", kinds)?;
        data.set_item("message", messages)?;
        dict_to_dataframe(py, &data, None)
    }

    /// JSON for the ``issues`` vector only.
    #[pyo3(text_signature = "(self)")]
    fn issues_json(&self, py: Python<'_>) -> PyResult<String> {
        let issues = &self.inner.issues;
        py.detach(|| serde_json::to_string(issues))
            .map_err(display_to_py)
    }

    /// Net same-currency cashflow amounts across kinds for each payment date.
    ///
    /// Parameters
    /// ----------
    /// currency : Currency | str
    ///     ISO-4217 currency whose per-date kind buckets are summed.
    ///
    /// Returns
    /// -------
    /// list[tuple[str, float]]
    ///     ``(ISO date, net amount)`` pairs in ladder date order; dates with
    ///     no flows in ``currency`` are omitted. Non-finite amounts are
    ///     skipped and totals use compensated summation.
    #[pyo3(text_signature = "(self, currency)")]
    fn net_in_currency_by_date(&self, currency: &Bound<'_, PyAny>) -> PyResult<Vec<(String, f64)>> {
        let ccy = extract_currency(currency)?;
        Ok(self
            .inner
            .net_in_currency_by_date(ccy)
            .into_iter()
            .map(|(date, amount)| (date.to_string(), amount))
            .collect())
    }

    /// Collapse multi-currency flows into a single base-currency ladder
    /// bucketed by ``(date, kind)`` and return it as a pandas ``DataFrame``.
    ///
    /// Payments on or before ``as_of`` use spot FX at ``as_of``. Later
    /// payments use the CIP forward ``F(T) = S × DF_from(T) / DF_base(T)``.
    /// ``discount_curves`` maps ISO currency codes to discount-curve ids;
    /// omitted currencies fall back to ``market.get_discount(currency)``.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market supplying FX spot and discount curves.
    /// base_currency : Currency | str
    ///     Target currency of the collapsed ladder.
    /// as_of : datetime.date | str
    ///     Spot/forward split date (date-like or ISO 8601 string).
    /// discount_curves : dict[str, str] | None
    ///     Optional ``{currency_code: curve_id}`` overrides.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     Columns ``date`` (ISO 8601 string), ``kind``, ``amount`` (float in
    ///     ``base_currency``), ``currency``; one row per ``(date, kind)``.
    ///     Use :meth:`collapse_to_base_by_date_kind_json` for the nested wire
    ///     form.
    ///
    /// Raises
    /// ------
    /// PortfolioError
    ///     If an FX rate or discount factor needed for a conversion is missing
    ///     or invalid.
    #[pyo3(
        signature = (market, base_currency, as_of, discount_curves=None),
        text_signature = "(self, market, base_currency, as_of, discount_curves=None)"
    )]
    fn collapse_to_base_by_date_kind<'py>(
        &self,
        py: Python<'py>,
        market: &Bound<'py, PyAny>,
        base_currency: &Bound<'py, PyAny>,
        as_of: &Bound<'py, PyAny>,
        discount_curves: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let collapsed = self.collapse(py, market, base_currency, as_of, discount_curves)?;
        let mut dates = Vec::new();
        let mut kinds = Vec::new();
        let mut amounts = Vec::new();
        let mut currencies = Vec::new();
        for (date, per_kind) in &collapsed {
            for (kind, money) in per_kind {
                dates.push(date.to_string());
                kinds.push(kind.to_string());
                amounts.push(money.amount());
                currencies.push(money.currency().to_string());
            }
        }
        let data = PyDict::new(py);
        data.set_item("date", dates)?;
        data.set_item("kind", kinds)?;
        data.set_item("amount", amounts)?;
        data.set_item("currency", currencies)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Wire twin of :meth:`collapse_to_base_by_date_kind`: same inputs, the
    /// nested ``{date: {kind: {"amount", "currency"}}}`` ladder as JSON.
    #[pyo3(
        signature = (market, base_currency, as_of, discount_curves=None),
        text_signature = "(self, market, base_currency, as_of, discount_curves=None)"
    )]
    fn collapse_to_base_by_date_kind_json(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        base_currency: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        discount_curves: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<String> {
        let collapsed = self.collapse(py, market, base_currency, as_of, discount_curves)?;
        py.detach(move || serde_json::to_string(&collapsed))
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioCashflows(events={}, positions={}, issues={})",
            self.inner.events.len(),
            self.inner.by_position.len(),
            self.inner.issues.len(),
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

fn extract_discount_curve_map(
    discount_curves: Option<&Bound<'_, PyDict>>,
) -> PyResult<Option<HashMap<Currency, CurveId>>> {
    let Some(dict) = discount_curves else {
        return Ok(None);
    };
    let mut map = HashMap::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        let currency = extract_currency(&key)?;
        let curve_id: String = value.extract()?;
        map.insert(currency, CurveId::new(curve_id));
    }
    Ok(Some(map))
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPortfolio>()?;
    m.add_class::<PyPortfolioBuilder>()?;
    m.add_class::<PyPositionValue>()?;
    m.add_class::<PyPortfolioValuation>()?;
    m.add_class::<PyPortfolioResult>()?;
    m.add_class::<PyPortfolioMetrics>()?;
    m.add_class::<PyPortfolioCashflows>()?;
    m.add_class::<super::scenario_pnl::PyScenarioPnl>()?;
    m.add_class::<super::scenario_pnl::PyScenarioPnlBatchItem>()?;
    Ok(())
}
