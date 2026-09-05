//! P&L attribution entry points and JSON helpers.

use crate::bindings::attribution::pnl_attribution::{PyPnlAttribution, WIDE_COLUMNS};
use crate::bindings::attribution::return_contribution::{
    extract_return_contribution_spec, PyReturnContributionResult,
};
use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::extract_date;
use crate::bindings::extract::{extract_instrument_json, extract_market_ref};
use crate::bindings::module_utils::py_to_json_value;
use crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema;
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};
use finstack_quant_attribution::{
    AttributionConfig, AttributionEnvelope, AttributionMethod, AttributionSpec,
};
use finstack_quant_core::market_data::context::MarketContextState;
use finstack_quant_valuations::instruments::{InstrumentEnvelope, InstrumentJson};
use pyo3::prelude::*;

/// Parse a typed instrument wrapper or canonical envelope JSON into the
/// instrument payload the attribution spec carries.
fn extract_instrument(obj: &Bound<'_, PyAny>) -> PyResult<InstrumentJson> {
    let json = extract_instrument_json(obj)?;
    let envelope: InstrumentEnvelope = serde_json::from_str(&json)
        .map_err(|e| serde_json_to_py(e, "invalid attribution instrument envelope JSON"))?;
    Ok(envelope.instrument)
}

/// Shared optional inputs of [`attribute_pnl`] / [`attribute_pnl_many`].
struct AttributionOptions<'a, 'py> {
    config: Option<&'a Bound<'py, PyAny>>,
    full_cross_attribution: bool,
    model_params_t0_json: Option<&'a str>,
    credit_factor_model_json: Option<&'a str>,
}

/// Build an [`AttributionSpec`] from typed-or-JSON Python inputs.
///
/// `instrument` is the payload attributed; `attribute_pnl_many` swaps it per
/// instrument after building one template this way.
#[allow(clippy::too_many_arguments)]
fn build_spec(
    py: Python<'_>,
    instrument: InstrumentJson,
    market_t0: &Bound<'_, PyAny>,
    market_t1: &Bound<'_, PyAny>,
    as_of_t0: &Bound<'_, PyAny>,
    as_of_t1: &Bound<'_, PyAny>,
    method: &Bound<'_, PyAny>,
    options: AttributionOptions<'_, '_>,
) -> PyResult<AttributionSpec> {
    let market_t0 = MarketContextState::from(&*extract_market_ref(py, market_t0)?);
    let market_t1 = MarketContextState::from(&*extract_market_ref(py, market_t1)?);
    let as_of_t0 = extract_date(as_of_t0)?;
    let as_of_t1 = extract_date(as_of_t1)?;
    let method: AttributionMethod = serde_json::from_value(py_to_json_value(py, method, "method")?)
        .map_err(|e| serde_json_to_py(e, "invalid attribution method"))?;
    let config: Option<AttributionConfig> = options
        .config
        .map(|value| {
            serde_json::from_value(py_to_json_value(py, value, "config")?)
                .map_err(|e| serde_json_to_py(e, "invalid attribution config"))
        })
        .transpose()?;
    let model_params_t0 = options
        .model_params_t0_json
        .map(|json| {
            serde_json::from_str(json)
                .map_err(|e| serde_json_to_py(e, "invalid attribution model_params_t0 JSON"))
        })
        .transpose()?;
    let credit_factor_model = options
        .credit_factor_model_json
        .map(|json| {
            serde_json::from_str(json)
                .map(Box::new)
                .map_err(|e| serde_json_to_py(e, "invalid attribution credit_factor_model JSON"))
        })
        .transpose()?;
    Ok(AttributionSpec {
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        method,
        model_params_t0,
        config,
        credit_factor_model,
        credit_factor_detail_options: Default::default(),
        full_cross_attribution: options.full_cross_attribution,
    })
}

/// Run P&L attribution for a single instrument.
///
/// This is the main entry point. It accepts the instrument, two market
/// snapshots, valuation dates, and a method descriptor — typed objects or
/// their canonical JSON — and returns a typed ``PnlAttribution``. Call
/// ``.to_json()`` on the result for the canonical JSON form.
///
/// Parameters
/// ----------
/// instrument : Bond | InterestRateSwap | CreditDefaultSwap | ... | str
///     Any typed instrument wrapper from ``finstack_quant.valuations`` or a
///     canonical v1 instrument envelope JSON string
///     (``{"schema": "finstack_quant.instrument/1", "instrument": {...}}``).
/// market_t0 : MarketContext | str
///     Market snapshot at T₀ (typed ``MarketContext`` or its JSON).
/// market_t1 : MarketContext | str
///     Market snapshot at T₁ (typed ``MarketContext`` or its JSON).
/// as_of_t0 : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Valuation date T₀; strings are ISO 8601 calendar dates
///     (``YYYY-MM-DD``). A tz-aware timestamp contributes its wall-clock
///     calendar date.
/// as_of_t1 : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Valuation date T₁, in the same forms as ``as_of_t0``; must not
///     precede ``as_of_t0``.
/// method : str | dict
///     Attribution method. One of:
///
///     * ``"parallel"``
///     * ``{"waterfall": [...]}`` with factor tokens in order, drawn from
///       ``carry``, ``rates_curves``, ``credit_curves``, ``inflation_curves``,
///       ``correlations``, ``fx``, ``volatility``, ``model_parameters``,
///       ``market_scalars`` (``default_waterfall_order()`` gives the full
///       canonical list; the order must start with ``carry``)
///     * ``"metrics_based"``
///     * ``{"taylor": {"include_gamma": True, ...}}``
/// config : dict | str, optional
///     Attribution config overrides (``tolerance_abs``, ``tolerance_pct``,
///     ``metrics``, ``strict_validation``, ``rounding_scale``,
///     ``rate_bump_bp``, ``target_currency``, ``execution_policy``).
/// full_cross_attribution : bool, default False
///     When true, the parallel method evaluates every pairwise cross-factor
///     interaction (full cross matrix) instead of the default seven economic
///     pairs. More repricings, smaller residual.
/// model_params_t0_json : str, optional
///     Serialized opening ``ModelParamsSnapshot``. When omitted, model-
///     parameter P&L is isolated from the instrument's current snapshot.
/// credit_factor_model_json : str, optional
///     Serialized ``CreditFactorModel``. When supplied, credit-factor
///     hierarchy detail is populated on the result.
///
/// Returns
/// -------
/// PnlAttribution
///     Typed attribution result. Use ``.to_json()`` for the wire form and
///     ``.to_dataframe()`` for a pandas view.
///
/// Raises
/// ------
/// ValueError
///     If an input JSON, the method or config cannot be parsed, a date is
///     malformed, or attribution validation / pricing fails.
/// KeyError
///     If a required curve, market item, calendar, or FX leg is missing.
/// RuntimeError
///     If calibration or solver convergence fails, or the attribution
///     engine reports an internal failure (including a contained panic).
///
/// Examples
/// --------
/// >>> from finstack_quant.attribution import attribute_pnl
/// >>> try:
/// ...     attribute_pnl("{}", "{}", "{}", "2025-01-15", "2025-01-16", "parallel")
/// ... except ValueError as exc:
/// ...     "instrument envelope" in str(exc)
/// True
#[pyfunction]
#[pyo3(signature = (instrument, market_t0, market_t1, as_of_t0, as_of_t1, method, config=None, full_cross_attribution=false, model_params_t0_json=None, credit_factor_model_json=None))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn attribute_pnl(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    market_t0: &Bound<'_, PyAny>,
    market_t1: &Bound<'_, PyAny>,
    as_of_t0: &Bound<'_, PyAny>,
    as_of_t1: &Bound<'_, PyAny>,
    method: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
    full_cross_attribution: bool,
    model_params_t0_json: Option<&str>,
    credit_factor_model_json: Option<&str>,
) -> PyResult<PyPnlAttribution> {
    let instrument = extract_instrument(instrument)?;
    let spec = build_spec(
        py,
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        method,
        AttributionOptions {
            config,
            full_cross_attribution,
            model_params_t0_json,
            credit_factor_model_json,
        },
    )?;

    // `execute_contained` turns a Rust panic into `Error::Internal` so it
    // surfaces as a catchable `RuntimeError` rather than a
    // `pyo3_runtime.PanicException` (a `BaseException`).
    let result = py.detach(|| spec.execute_contained()).map_err(core_to_py)?;
    Ok(PyPnlAttribution {
        inner: result.attribution,
    })
}

/// Run one attribution set-up against many instruments and tabulate the
/// results.
///
/// Every instrument shares the same markets, dates, method and config; the
/// batch runs in Rust (``attribute_pnl_many``) in input order and stops at
/// the first failing instrument.
///
/// Parameters
/// ----------
/// instruments : list[Bond | InterestRateSwap | ... | str]
///     Typed instrument wrappers or canonical instrument envelope JSON
///     strings, in the row order wanted.
/// market_t0 : MarketContext | str
///     Market snapshot at T₀.
/// market_t1 : MarketContext | str
///     Market snapshot at T₁.
/// as_of_t0 : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Valuation date T₀.
/// as_of_t1 : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Valuation date T₁.
/// method : str | dict
///     Attribution method, as for ``attribute_pnl``.
/// config : dict | str, optional
///     Attribution config overrides, as for ``attribute_pnl``.
/// full_cross_attribution : bool, default False
///     Evaluate every pairwise cross-factor term (parallel method only).
/// model_params_t0_json : str, optional
///     Serialized opening ``ModelParamsSnapshot`` applied to every instrument.
/// credit_factor_model_json : str, optional
///     Serialized ``CreditFactorModel`` applied to every instrument.
///
/// Returns
/// -------
/// pandas.DataFrame
///     One row per instrument with the same columns as
///     ``PnlAttribution.to_dataframe()`` (``instrument_id``, ``method``,
///     ``t0``, ``t1``, ``currency``, ``total_pnl``, every factor P&L,
///     ``residual``, ``residual_pct``, ``num_repricings``,
///     ``result_invalid``). Empty (schema columns present) for an empty
///     instrument list.
///
/// Raises
/// ------
/// ValueError
///     If any input cannot be parsed or an instrument's attribution fails
///     validation / pricing, or a result mixes currencies across factors.
/// KeyError
///     If a required curve, market item, calendar, or FX leg is missing.
/// RuntimeError
///     If the engine reports an internal failure for any instrument.
///
/// Examples
/// --------
/// >>> from finstack_quant.attribution import attribute_pnl_many
/// >>> try:
/// ...     attribute_pnl_many(["{}"], "{}", "{}", "2025-01-15", "2025-01-16", "parallel")
/// ... except ValueError as exc:
/// ...     "instrument envelope" in str(exc)
/// True
#[pyfunction]
#[pyo3(signature = (instruments, market_t0, market_t1, as_of_t0, as_of_t1, method, config=None, full_cross_attribution=false, model_params_t0_json=None, credit_factor_model_json=None))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn attribute_pnl_many<'py>(
    py: Python<'py>,
    instruments: Vec<Bound<'py, PyAny>>,
    market_t0: &Bound<'py, PyAny>,
    market_t1: &Bound<'py, PyAny>,
    as_of_t0: &Bound<'py, PyAny>,
    as_of_t1: &Bound<'py, PyAny>,
    method: &Bound<'py, PyAny>,
    config: Option<&Bound<'py, PyAny>>,
    full_cross_attribution: bool,
    model_params_t0_json: Option<&str>,
    credit_factor_model_json: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut payloads = instruments
        .iter()
        .map(extract_instrument)
        .collect::<PyResult<Vec<_>>>()?;
    let Some(first) = payloads.first().cloned() else {
        return serde_rows_to_dataframe_with_schema::<serde_json::Value>(py, &[], &WIDE_COLUMNS);
    };
    let template = build_spec(
        py,
        first,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        method,
        AttributionOptions {
            config,
            full_cross_attribution,
            model_params_t0_json,
            credit_factor_model_json,
        },
    )?;
    let payloads = std::mem::take(&mut payloads);
    let attributions = py
        .detach(|| finstack_quant_attribution::attribute_pnl_many(&template, payloads))
        .map_err(core_to_py)?;
    let rows = attributions
        .iter()
        .map(finstack_quant_attribution::pnl_attribution_wide_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(display_to_py)?;
    serde_rows_to_dataframe_with_schema(py, &rows, &WIDE_COLUMNS)
}

/// Headline P&L bridge: ``value(T₁) − value(T₀)`` in one currency.
///
/// The cheapest attribution entry point — two repricings, no factor loop.
/// FX conversion into ``target_currency`` uses ``market_t0`` for the T₀ value
/// and ``market_t1`` for the T₁ value. Use ``attribute_pnl`` when the
/// factor decomposition matters.
///
/// Parameters
/// ----------
/// instrument : Bond | InterestRateSwap | ... | str
///     Typed instrument wrapper or canonical instrument envelope JSON.
/// market_t0 : MarketContext | str
///     Opening market state.
/// market_t1 : MarketContext | str
///     Closing market state.
/// as_of_t0 : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Opening valuation date.
/// as_of_t1 : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Closing valuation date.
/// target_currency : Currency | str
///     ISO-4217 currency the P&L is reported in.
///
/// Returns
/// -------
/// Money
///     ``value(T₁) − value(T₀)`` in ``target_currency``.
///
/// Raises
/// ------
/// ValueError
///     If the instrument JSON, a market, a date, or the currency is
///     malformed, or pricing fails validation.
/// KeyError
///     If a curve or FX rate needed for pricing or conversion is missing.
/// RuntimeError
///     If a pricer's solver fails to converge.
///
/// Examples
/// --------
/// >>> from finstack_quant.attribution import pnl_bridge
/// >>> try:
/// ...     pnl_bridge("{}", "{}", "{}", "2025-01-15", "2025-01-16", "USD")
/// ... except ValueError as exc:
/// ...     "instrument envelope" in str(exc)
/// True
#[pyfunction]
#[pyo3(signature = (instrument, market_t0, market_t1, as_of_t0, as_of_t1, target_currency))]
pub(crate) fn pnl_bridge(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    market_t0: &Bound<'_, PyAny>,
    market_t1: &Bound<'_, PyAny>,
    as_of_t0: &Bound<'_, PyAny>,
    as_of_t1: &Bound<'_, PyAny>,
    target_currency: &Bound<'_, PyAny>,
) -> PyResult<PyMoney> {
    let instrument: std::sync::Arc<dyn finstack_quant_valuations::instruments::Instrument> =
        std::sync::Arc::from(
            extract_instrument(instrument)?
                .into_boxed()
                .map_err(core_to_py)?,
        );
    let market_t0 = extract_market_ref(py, market_t0)?;
    let market_t1 = extract_market_ref(py, market_t1)?;
    let as_of_t0 = extract_date(as_of_t0)?;
    let as_of_t1 = extract_date(as_of_t1)?;
    let currency = extract_currency(target_currency)?;
    let m0: &finstack_quant_core::market_data::context::MarketContext = &market_t0;
    let m1: &finstack_quant_core::market_data::context::MarketContext = &market_t1;
    let pnl = py
        .detach(|| {
            finstack_quant_attribution::pnl_bridge(
                &instrument,
                m0,
                m1,
                as_of_t0,
                as_of_t1,
                currency,
            )
        })
        .map_err(core_to_py)?;
    Ok(PyMoney::from_inner(pnl))
}

/// Run attribution from a full JSON ``AttributionEnvelope`` and return JSON.
///
/// This is the raw JSON round-trip variant. Most users should prefer
/// ``attribute_pnl``, which accepts separate arguments and returns a typed
/// ``PnlAttribution``.
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-serialized ``AttributionEnvelope``.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``AttributionResultEnvelope``.
///
/// Raises
/// ------
/// ValueError
///     If ``spec_json`` is malformed or fails schema validation, or the
///     attribution fails validation / pricing.
/// KeyError
///     If a required curve, market item, calendar, or FX leg is missing.
/// RuntimeError
///     If the engine reports an internal failure.
#[pyfunction]
pub(crate) fn attribute_pnl_envelope_json(py: Python<'_>, spec_json: &str) -> PyResult<String> {
    let envelope: AttributionEnvelope = serde_json::from_str(spec_json)
        .map_err(|e| serde_json_to_py(e, "invalid attribution envelope JSON"))?;
    let result_envelope = py
        .detach(|| envelope.execute_contained())
        .map_err(core_to_py)?;
    serde_json::to_string(&result_envelope).map_err(display_to_py)
}

/// Compute single-period return contribution attribution.
///
/// Parameters
/// ----------
/// spec : dict | str | pandas.DataFrame
///     The return-contribution specification. A ``dict`` or JSON ``str``
///     carries ``as_of``, ``positions``, optional ``factors`` and
///     ``weighting`` exactly as the wire schema (``as_of`` may be a
///     ``datetime.date`` in the dict form). A ``DataFrame`` is one position
///     per row with columns ``id`` (or the index), exactly one of
///     ``market_value`` / ``weight``, ``return``, optional
///     ``benchmark_weight`` / ``benchmark_return``, and any number of
///     ``group:<dimension>`` label columns; missing optional cells may be
///     ``NaN``.
/// as_of : datetime.date | str, optional
///     Attribution date label. Required with a ``DataFrame`` spec; fills a
///     missing ``as_of`` in the dict form.
/// weighting : str, optional
///     ``"gross"`` (default) or ``"net_market_value"`` for market-value
///     positions; ``DataFrame`` form only.
/// factors : list[dict], optional
///     Factor rows ``{"factor", "exposure", "factor_return"}``; ``DataFrame``
///     form only.
///
/// Returns
/// -------
/// ReturnContributionResult
///     Typed result. Use ``.to_json()`` for the wire form,
///     ``.to_dataframe()`` for per-instrument rows, ``.to_group_dataframe()``
///     / ``.to_factor_dataframe()`` for the other blocks, and
///     ``.to_series()`` for contributions indexed by instrument id.
///
/// Raises
/// ------
/// ValueError
///     If the spec is malformed, ``as_of`` is missing for a DataFrame,
///     positions are empty, weighting modes are mixed, or benchmark inputs
///     are incomplete, or a Brinson group has zero net weight but nonzero
///     return contribution. Split offsetting long/short positions into distinct groups.
/// TypeError
///     If ``spec`` is none of ``dict``, ``str``, ``pandas.DataFrame``.
///
/// Examples
/// --------
/// >>> from finstack_quant.attribution import attribute_return_contribution
/// >>> spec = {
/// ...     "as_of": "2026-01-02",
/// ...     "positions": [{"id": "A", "market_value": 100.0, "return": 0.02}],
/// ... }
/// >>> attribute_return_contribution(spec).portfolio_return
/// 0.02
#[pyfunction]
#[pyo3(signature = (spec, as_of=None, weighting=None, factors=None))]
pub(crate) fn attribute_return_contribution(
    py: Python<'_>,
    spec: &Bound<'_, PyAny>,
    as_of: Option<&Bound<'_, PyAny>>,
    weighting: Option<&str>,
    factors: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyReturnContributionResult> {
    let spec = extract_return_contribution_spec(py, spec, as_of, weighting, factors)?;
    let inner =
        finstack_quant_attribution::attribute_return_contribution(&spec).map_err(core_to_py)?;
    Ok(PyReturnContributionResult { inner })
}

/// Validate an attribution specification JSON.
///
/// Deserializes the input against the ``AttributionEnvelope`` schema,
/// checks the ``schema`` version tag (the same gate ``execute`` applies, so
/// a payload that validates here cannot later be rejected at execution),
/// and returns the canonical (re-serialized) JSON.
///
/// Parameters
/// ----------
/// json : str
///     JSON-serialized ``AttributionEnvelope``.
///
/// Returns
/// -------
/// str
///     Canonical compact JSON.
///
/// Raises
/// ------
/// ValueError
///     If ``json`` is malformed or violates the envelope schema.
#[pyfunction]
pub(crate) fn validate_attribution_json(json: &str) -> PyResult<String> {
    finstack_quant_attribution::validate_attribution_json(json).map_err(core_to_py)
}

/// Validate a return contribution specification JSON.
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-serialized return contribution specification.
///
/// Returns
/// -------
/// str
///     Canonical compact JSON.
///
/// Raises
/// ------
/// ValueError
///     If ``spec_json`` is malformed or violates the weighting / benchmark
///     invariants.
#[pyfunction]
pub(crate) fn validate_return_contribution_json(spec_json: &str) -> PyResult<String> {
    finstack_quant_attribution::validate_return_contribution_json(spec_json).map_err(core_to_py)
}

/// Return the default waterfall factor ordering.
///
/// Returns
/// -------
/// list[str]
///     Canonical snake-case factor tokens in the default waterfall order
///     (``carry``, ``rates_curves``, ``credit_curves``, ``inflation_curves``,
///     ``correlations``, ``fx``, ``volatility``, ``model_parameters``,
///     ``market_scalars``); pass a prefix or reordering to
///     ``attribute_pnl(method={"waterfall": [...]})``.
///
/// Examples
/// --------
/// >>> from finstack_quant.attribution import default_waterfall_order
/// >>> default_waterfall_order()[:2]
/// ['carry', 'rates_curves']
#[pyfunction]
pub(crate) fn default_waterfall_order() -> Vec<String> {
    finstack_quant_attribution::default_waterfall_order()
        .into_iter()
        .map(|factor| factor.as_str().to_owned())
        .collect()
}

/// Return the default metric ids used by metrics-based attribution.
///
/// Returns
/// -------
/// list[str]
///     Canonical snake-case metric ids (``theta``, ``dv01``, ``cs01``,
///     ``bucketed_cs01``, ``vega``, ...) — the same tokens accepted by
///     ``config={"metrics": [...]}`` and ``PnlAttribution.required_metrics()``.
///
/// Examples
/// --------
/// >>> from finstack_quant.attribution import default_attribution_metrics
/// >>> default_attribution_metrics()[:3]
/// ['theta', 'dv01', 'cs01']
#[pyfunction]
pub(crate) fn default_attribution_metrics() -> Vec<String> {
    finstack_quant_attribution::default_attribution_metrics()
        .into_iter()
        .map(|m| m.to_string())
        .collect()
}
