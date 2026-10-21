//! Python bindings for `Portfolio`, `PortfolioBuilder` and `PortfolioSpec` JSON.
//!
//! Mirrors the Rust `portfolio` and `builder` modules; named `container` because
//! `portfolio::portfolio` trips clippy `module_inception`.

use crate::bindings::core::currency::extract_currency;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::extract::extract_instrument;
use crate::bindings::extract::InstrumentArg;
use crate::bindings::module_utils::py_to_json_value;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::core_to_py;
use crate::errors::display_to_py;
use crate::errors::portfolio_to_py;
use crate::errors::value_error;
use finstack_quant_portfolio::position::Position;
use finstack_quant_portfolio::position::PositionUnit;
use finstack_quant_portfolio::types::AttributeValue;
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::types::DUMMY_ENTITY_ID;
use finstack_quant_portfolio::PortfolioBuilder;
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::types::PyDict;
use pyo3::types::PyModule;
use std::collections::HashMap;
use std::sync::Arc;

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
        let date = py_to_date(as_of)?;
        Ok(PyPortfolioBuilder {
            inner: Some(
                finstack_quant_portfolio::Portfolio::builder(id)
                    .base_currency(ccy)
                    .as_of(date),
            ),
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
        let spec: finstack_quant_portfolio::PortfolioSpec = py
            .detach(move || serde_json::from_str(&spec_json))
            .map_err(display_to_py)?;
        let inner = py
            .detach(move || finstack_quant_portfolio::Portfolio::from_spec(spec))
            .map_err(portfolio_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through ``to_json`` / ``from_spec`` — the same
    /// strict serde round-trip as the wire format — so an unpickled portfolio
    /// is rebuilt (positions materialized, indices rebuilt) exactly as if it
    /// had been loaded from its canonical ``PortfolioSpec``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let payload = self.to_json(py)?;
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
        Ok(self.to_json(py)? == other.to_json(py)?)
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

    /// Canonical ``PortfolioSpec`` of the portfolio as a plain dict (Rust
    /// ``Portfolio::to_spec``), the input accepted by ``from_spec``.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.to_spec())
    }

    /// Serialize the canonical ``PortfolioSpec`` (Rust ``Portfolio::to_spec``)
    /// to a JSON string accepted by ``from_spec``.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
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

/// Build a validated runtime instrument from a typed instrument wrapper or
/// canonical envelope JSON (`dict` or `str`).
///
/// Envelope text keeps the plain serde decode and its error text; a typed
/// instance is validated and boxed directly.
pub(crate) fn extract_position_instrument(
    instrument: &Bound<'_, PyAny>,
) -> PyResult<Box<dyn finstack_quant_valuations::instruments::Instrument>> {
    let payload = match extract_instrument(instrument)? {
        InstrumentArg::Typed(payload) => *payload,
        InstrumentArg::Envelope(json) => {
            let envelope: finstack_quant_valuations::instruments::json_loader::InstrumentEnvelope =
                serde_json::from_str(&json).map_err(display_to_py)?;
            envelope.instrument
        }
    };
    payload.into_boxed().map_err(core_to_py)
}

/// Parse a position unit from ``None`` (units), a serde name string
/// (``"units"``, ``"face_value"``, ``"percentage"``, parsed by Rust
/// ``PositionUnit::from_str``) or a JSON-shaped dict (``{"notional": "USD"}``
/// / ``{"notional": None}``).
pub(crate) fn extract_position_unit(
    py: Python<'_>,
    unit: Option<&Bound<'_, PyAny>>,
) -> PyResult<PositionUnit> {
    match unit {
        None => Ok(PositionUnit::Units),
        Some(obj) => match obj.extract::<String>() {
            Ok(s) => s.parse().map_err(crate::errors::portfolio_to_py),
            Err(_) => py_to_serde(py, obj, "position unit"),
        },
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
    ///     ``"percentage"``, or the mapping ``{"notional": "USD"}`` /
    ///     ``{"notional": None}`` for a (currency-tagged) lot multiplier. The
    ///     bare string ``"notional"`` is rejected, as in the JSON spec.
    /// attributes : dict[str, str | float] | None
    ///     Position attributes (rating, sector, scores) used by grouping and
    ///     optimization filters.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``instrument`` is not a typed instrument, an envelope dict or a JSON string.
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
        let boxed = extract_position_instrument(instrument)?;
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

/// Parse a portfolio specification from JSON and return the canonical form.
#[pyfunction]
pub fn parse_portfolio_spec_json(py: Python<'_>, json_str: &str) -> PyResult<String> {
    let json_str = json_str.to_owned();
    py.detach(move || {
        let spec: finstack_quant_portfolio::PortfolioSpec = serde_json::from_str(&json_str)?;
        serde_json::to_string(&spec)
    })
    .map_err(display_to_py)
}

/// Register the container bindings on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPortfolio>()?;
    m.add_class::<PyPortfolioBuilder>()?;
    m.add_function(pyo3::wrap_pyfunction!(parse_portfolio_spec_json, m)?)?;
    Ok(())
}
