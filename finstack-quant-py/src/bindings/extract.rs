//! Shared polymorphic extraction helpers for PyO3 bindings.
//!
//! Each helper accepts a `&Bound<'_, PyAny>` and tries two paths:
//!
//! 1. **Typed fast path** — cast to the corresponding `#[pyclass]` wrapper
//!    and use the inner Rust value directly: the `*Access` enums borrow it
//!    (no clone), and [`extract_instrument`] clones it (no JSON).
//! 2. **Canonical JSON path** — extract a Python `str` (or envelope `dict`),
//!    then deserialize the same serde contract used by Rust.
//!
//! The `*Access` enums wrap both paths behind a `Deref<Target = T>` impl so
//! pipeline functions can accept `T | str` without branching.
//! [`extract_instrument_json`] is the exception: it serializes a typed
//! instrument to envelope text, for Rust APIs that take JSON.

use pyo3::prelude::*;

use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::portfolio::types::{PyPortfolio, PyPortfolioValuation};
use crate::bindings::statements::evaluator::PyStatementResult;
use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::bindings::valuations::composite::PyCompositeInstrument;
use crate::bindings::valuations::instruments::{
    serialize_typed_instrument_json, PyBond, PyTermLoan,
};
use crate::bindings::valuations::typed_asset_backed_facility::PyAssetBackedFacility;
use crate::bindings::valuations::typed_credit::{
    PyCdsIndex, PyCdsOption, PyCdsTranche, PyConvertibleBond, PyCreditDefaultSwap,
};
use crate::bindings::valuations::typed_equity::PyEquityOption;
use crate::bindings::valuations::typed_fx::{PyFxForward, PyFxOption};
use crate::bindings::valuations::typed_rates::{PyCapFloor, PyInterestRateSwap, PySwaption};
use crate::bindings::valuations::typed_revolving_credit::PyRevolvingCredit;
use crate::bindings::valuations::typed_structured_credit::PyStructuredCredit;
use crate::errors::{display_to_py as to_py, portfolio_to_py};
use finstack_quant_valuations::instruments::InstrumentJson;
use finstack_quant_valuations::pricer::ParsedInstrument;

// Instrument — typed-or-JSON extraction

/// A polymorphic instrument argument: a typed instance's native payload, or
/// canonical envelope JSON text from a `dict` or `str`.
///
/// The typed arm skips the JSON round trip; the envelope arm parses through
/// the Rust envelope loader. Both prepare instruments identically: overrides
/// merge before validation, and validation runs once.
pub enum InstrumentArg {
    /// Clone of a typed instance's inner Rust instrument, not yet validated.
    Typed(Box<InstrumentJson>),
    /// Canonical `finstack_quant.instrument/1` envelope JSON text.
    Envelope(String),
}

impl InstrumentArg {
    /// Merge optional metric pricing overrides, validate, and box the
    /// instrument for pricing dispatch.
    ///
    /// # Arguments
    ///
    /// * `metric_pricing_overrides` - Optional `MetricPricingOverrides` JSON
    ///   patch merged before validation; `None` keeps stored controls.
    pub fn into_parsed(
        self,
        metric_pricing_overrides: Option<&str>,
    ) -> finstack_quant_core::Result<ParsedInstrument> {
        match self {
            Self::Typed(instrument) => {
                ParsedInstrument::from_instrument_json(*instrument, metric_pricing_overrides)
            }
            Self::Envelope(json) => {
                finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(
                    &json,
                    metric_pricing_overrides,
                )
            }
        }
    }

    /// Validate the instrument and return its tagged payload.
    pub fn into_instrument_json(self) -> finstack_quant_core::Result<InstrumentJson> {
        match self {
            Self::Typed(instrument) => {
                instrument.validate_for_pricing()?;
                Ok(*instrument)
            }
            Self::Envelope(json) => {
                finstack_quant_valuations::pricer::json::parse_instrument_from_json(&json)
            }
        }
    }
}

/// Extract an instrument argument from a typed instrument object, an envelope
/// `dict`, or a pre-serialized envelope string.
///
/// # Extending this function
///
/// Every typed instrument class is wired in here by adding exactly **one**
/// cast arm that clones its `inner` into the matching [`InstrumentJson`]
/// variant, ordered before the final envelope fallback. Update the fallback
/// error message below to name each newly landed class.
///
/// Currently wired: `Bond`, `TermLoan`, `InterestRateSwap`, `Swaption`,
/// `CapFloor`, `CreditDefaultSwap`, `CdsIndex`, `FxForward`, `FxOption`,
/// `CdsTranche`, `CdsOption`, `ConvertibleBond`, `EquityOption`, `StructuredCredit`,
/// `RevolvingCredit`, `AssetBackedFacility`, `CompositeInstrument`.
pub fn extract_instrument(obj: &Bound<'_, PyAny>) -> PyResult<InstrumentArg> {
    macro_rules! typed {
        ($($py:ty => $wrap:expr),+ $(,)?) => {
            $(
                if let Ok(typed) = obj.cast::<$py>() {
                    let inner = typed.borrow().inner.clone();
                    return Ok(InstrumentArg::Typed(Box::new($wrap(inner))));
                }
            )+
        };
    }
    typed!(
        PyCompositeInstrument => InstrumentJson::from,
        PyBond => InstrumentJson::Bond,
        PyTermLoan => InstrumentJson::TermLoan,
        PyRevolvingCredit => InstrumentJson::RevolvingCredit,
        PyAssetBackedFacility => |inner| InstrumentJson::AssetBackedFacility(Box::new(inner)),
        PyInterestRateSwap => InstrumentJson::InterestRateSwap,
        PySwaption => InstrumentJson::Swaption,
        PyCapFloor => InstrumentJson::CapFloor,
        PyCreditDefaultSwap => InstrumentJson::CreditDefaultSwap,
        PyCdsIndex => InstrumentJson::CdsIndex,
        PyFxForward => InstrumentJson::FxForward,
        PyFxOption => InstrumentJson::FxOption,
        PyCdsOption => InstrumentJson::CdsOption,
        PyCdsTranche => InstrumentJson::CdsTranche,
        PyConvertibleBond => InstrumentJson::ConvertibleBond,
        PyEquityOption => InstrumentJson::EquityOption,
        PyStructuredCredit => |inner| InstrumentJson::StructuredCredit(Box::new(inner)),
    );
    envelope_json_text(obj).map(InstrumentArg::Envelope).map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "expected a canonical instrument envelope (dict or JSON string) or a typed instrument instance",
        )
    })
}

/// Extract canonical instrument-envelope JSON text from a typed instrument
/// object, an envelope `dict`, or a pre-serialized envelope string.
///
/// Typed instances are serialized to the same `InstrumentEnvelope` the JSON
/// loader parses. Use this only where the Rust API takes envelope text;
/// callers that need the instrument itself use [`extract_instrument`], which
/// skips the serialize-and-parse round trip.
pub fn extract_instrument_json(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    match extract_instrument(obj)? {
        InstrumentArg::Typed(instrument) => {
            let what = instrument.type_tag();
            serialize_typed_instrument_json(*instrument, what)
        }
        InstrumentArg::Envelope(json) => Ok(json),
    }
}

/// Canonical instrument-envelope JSON text from an envelope `dict` (as
/// returned by `instrument_envelope_from_spec`) or pre-serialized JSON `str`.
///
/// A `str` is passed through unchanged so the Rust parser reports its own
/// errors; a `dict` is serialized with `json.dumps`.
///
/// # Errors
///
/// Raises `TypeError` for any other Python type and `ValueError` when a
/// `dict` is not JSON-serializable.
pub fn envelope_json_text(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(text) = obj.extract::<String>() {
        return Ok(text);
    }
    if obj.is_instance_of::<pyo3::types::PyDict>() {
        return crate::bindings::module_utils::py_to_json_string(obj.py(), obj, "instrument");
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected a canonical instrument envelope as a dict or JSON string",
    ))
}

// Zero-clone access types (available for callers that only need &T)

/// Access to a [`FinancialModelSpec`] without cloning on the typed fast path.
///
/// When the caller passes a `FinancialModelSpec` Python object, the
/// `Borrowed` variant holds a `PyRef` guard — no clone occurs.  When the
/// caller passes a JSON string, the `Owned` variant holds the parsed value.
///
/// Use `Deref` (i.e. `&model`) for read-only access.  Call `.into_owned()`
/// only when ownership is truly needed (e.g. `goal_seek` which mutates).
pub enum ModelAccess<'py> {
    Borrowed(PyRef<'py, PyFinancialModelSpec>),
    Owned(Box<finstack_quant_statements::FinancialModelSpec>),
}

impl std::ops::Deref for ModelAccess<'_> {
    type Target = finstack_quant_statements::FinancialModelSpec;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => &r.inner,
            Self::Owned(m) => m.as_ref(),
        }
    }
}

impl ModelAccess<'_> {
    /// Consume this access and produce an owned value, cloning only if
    /// the data was borrowed from a Python object.
    pub fn into_owned(self) -> finstack_quant_statements::FinancialModelSpec {
        match self {
            Self::Borrowed(r) => r.inner.clone(),
            Self::Owned(m) => *m,
        }
    }
}

/// Access to a [`StatementResult`] without cloning on the typed fast path.
pub enum ResultAccess<'py> {
    Borrowed(PyRef<'py, PyStatementResult>),
    Owned(Box<finstack_quant_statements::evaluator::StatementResult>),
}

impl std::ops::Deref for ResultAccess<'_> {
    type Target = finstack_quant_statements::evaluator::StatementResult;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => &r.inner,
            Self::Owned(r) => r.as_ref(),
        }
    }
}

impl ResultAccess<'_> {
    pub fn into_owned(self) -> finstack_quant_statements::evaluator::StatementResult {
        match self {
            Self::Borrowed(r) => r.inner.clone(),
            Self::Owned(r) => *r,
        }
    }
}

/// Extract a [`FinancialModelSpec`] without cloning when a typed Python
/// object is passed.  Returns [`ModelAccess`] which dereferences to
/// `&FinancialModelSpec`.
pub fn extract_model_ref<'py>(obj: &Bound<'py, PyAny>) -> PyResult<ModelAccess<'py>> {
    if let Ok(spec) = obj.cast::<PyFinancialModelSpec>() {
        return Ok(ModelAccess::Borrowed(spec.borrow()));
    }
    let json: String = obj.extract()?;
    let inner = finstack_quant_statements::FinancialModelSpec::from_json(&json)
        .map_err(crate::errors::statements_to_py)?;
    Ok(ModelAccess::Owned(Box::new(inner)))
}

/// Extract a [`StatementResult`] without cloning when a typed Python
/// object is passed.
pub fn extract_results_ref<'py>(obj: &Bound<'py, PyAny>) -> PyResult<ResultAccess<'py>> {
    if let Ok(result) = obj.cast::<PyStatementResult>() {
        return Ok(ResultAccess::Borrowed(result.borrow()));
    }
    let json: String = obj.extract()?;
    let inner: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(&json).map_err(to_py)?;
    Ok(ResultAccess::Owned(Box::new(inner)))
}

/// Extract a [`MarketContext`] from a `MarketContext` Python object
/// (fast path) or a JSON string (fallback).
///
/// Always produces an owned value — prefer [`extract_market_ref`] when only
/// a reference is needed.
pub fn extract_market(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_core::market_data::context::MarketContext> {
    if let Ok(ctx) = obj.cast::<PyMarketContext>() {
        return Ok(ctx.borrow().inner.clone());
    }
    let json: String = obj.extract()?;
    py.detach(move || serde_json::from_str(&json))
        .map_err(to_py)
}

/// Extract an optional [`MarketContext`] from `Option<&Bound<'_, PyAny>>`.
///
/// Returns `Ok(None)` when `obj` is `None`.
pub fn extract_market_opt(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<finstack_quant_core::market_data::context::MarketContext>> {
    match obj {
        Some(o) => extract_market(py, o).map(Some),
        None => Ok(None),
    }
}

// MarketContext — borrow-preferring access

/// Access to a [`MarketContext`] without cloning on the typed fast path.
///
/// `MarketContext` holds `HashMap`s of `Arc`s; its `Clone` reallocates the
/// backing storage and bumps every `Arc` refcount. In tight pipelines
/// (replay, chained valuation), avoiding that per-call allocation is
/// measurable.
pub enum MarketAccess<'py> {
    Borrowed(PyRef<'py, PyMarketContext>),
    Owned(Box<finstack_quant_core::market_data::context::MarketContext>),
}

impl std::ops::Deref for MarketAccess<'_> {
    type Target = finstack_quant_core::market_data::context::MarketContext;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => &r.inner,
            Self::Owned(m) => m.as_ref(),
        }
    }
}

/// Borrow a [`MarketContext`] from a typed Python object, or parse from JSON
/// while releasing the GIL.
pub fn extract_market_ref<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<MarketAccess<'py>> {
    if let Ok(ctx) = obj.cast::<PyMarketContext>() {
        return Ok(MarketAccess::Borrowed(ctx.borrow()));
    }
    let json: String = obj.extract()?;
    let inner: finstack_quant_core::market_data::context::MarketContext = py
        .detach(move || serde_json::from_str(&json))
        .map_err(to_py)?;
    Ok(MarketAccess::Owned(Box::new(inner)))
}

// Portfolio — borrow-preferring access

/// Access to a [`Portfolio`] without rebuilding from spec on the typed path.
///
/// Portfolio construction parses positions and rebuilds the position index +
/// dependency index; doing it once and reusing the typed object across
/// pipeline calls (value, cashflows, metrics, scenario) is a major win.
pub enum PortfolioAccess<'py> {
    Borrowed(PyRef<'py, PyPortfolio>),
    Owned(Box<finstack_quant_portfolio::Portfolio>),
}

impl std::ops::Deref for PortfolioAccess<'_> {
    type Target = finstack_quant_portfolio::Portfolio;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => r.inner.as_ref(),
            Self::Owned(p) => p.as_ref(),
        }
    }
}

/// Extract a [`Portfolio`] from a `Portfolio` Python object (fast path) or
/// build one from a JSON spec string (fallback). The JSON path pays the full
/// `Portfolio::from_spec` cost, which includes position materialization,
/// index construction, and validation; both stages release the GIL.
pub fn extract_portfolio_ref<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<PortfolioAccess<'py>> {
    if let Ok(p) = obj.cast::<PyPortfolio>() {
        return Ok(PortfolioAccess::Borrowed(p.borrow()));
    }
    let json: String = obj.extract()?;
    let spec: finstack_quant_portfolio::PortfolioSpec = py
        .detach(move || serde_json::from_str(&json))
        .map_err(to_py)?;
    let portfolio = py
        .detach(move || finstack_quant_portfolio::Portfolio::from_spec(spec))
        .map_err(portfolio_to_py)?;
    Ok(PortfolioAccess::Owned(Box::new(portfolio)))
}

// PortfolioValuation — borrow-preferring access

/// Access to a [`PortfolioValuation`] without re-parsing JSON when a typed
/// Python object is passed.
pub enum ValuationAccess<'py> {
    Borrowed(PyRef<'py, PyPortfolioValuation>),
    Owned(Box<finstack_quant_portfolio::valuation::PortfolioValuation>),
}

impl std::ops::Deref for ValuationAccess<'_> {
    type Target = finstack_quant_portfolio::valuation::PortfolioValuation;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => &r.inner,
            Self::Owned(v) => v.as_ref(),
        }
    }
}

/// Extract a [`PortfolioValuation`] from a typed Python object or a JSON string.
pub fn extract_valuation_ref<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<ValuationAccess<'py>> {
    if let Ok(v) = obj.cast::<PyPortfolioValuation>() {
        return Ok(ValuationAccess::Borrowed(v.borrow()));
    }
    let json: String = obj.extract()?;
    let inner: finstack_quant_portfolio::valuation::PortfolioValuation = py
        .detach(move || serde_json::from_str(&json))
        .map_err(to_py)?;
    Ok(ValuationAccess::Owned(Box::new(inner)))
}

// Rate / spread — typed-or-float unit extraction

/// Extract a decimal rate from a `float | int | Rate | Bps`.
///
/// A `Rate` wrapper contributes `Rate::as_decimal()`; `Bps` is converted
/// through `as_decimal()`. A bare number is taken as an already-decimal rate
/// (`0.05` for 5%). No percent/bp coercion is applied to bare numbers — a
/// caller holding basis points should pass a `Bps`. Percent quotes should be
/// converted with `Rate::from_percent` before extraction.
///
/// # Errors
///
/// Returns `TypeError` when `obj` is neither numeric nor a `Rate`/`Bps`.
pub fn extract_rate_decimal(obj: &Bound<'_, PyAny>) -> PyResult<f64> {
    if let Ok(rate) = obj.cast::<crate::bindings::core::types::PyRate>() {
        return Ok(rate.borrow().inner.as_decimal());
    }
    if let Ok(bps) = obj.cast::<crate::bindings::core::types::PyBps>() {
        return Ok(bps.borrow().inner.as_decimal());
    }
    obj.extract::<f64>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "expected a decimal rate (float, e.g. 0.05 for 5%) or a Rate/Bps \
             instance, got {}",
            obj.get_type()
                .name()
                .map(|n| n.to_string())
                .unwrap_or_default()
        ))
    })
}

/// Extract a basis-point spread from a `float | int | Bps | Rate`.
///
/// A `Bps` wrapper contributes its integer basis points as `f64`; a bare
/// number is taken as already in basis points (`25.0` for 25 bp). A `Rate`
/// reports its fractional basis points through the Rust `as_bp_f64` accessor,
/// so a caller holding a decimal rate never has to
/// rescale by hand.
///
/// # Errors
///
/// Returns `TypeError` when `obj` is neither numeric nor a `Bps`/`Rate`.
pub fn extract_basis_points(obj: &Bound<'_, PyAny>) -> PyResult<f64> {
    if let Ok(bps) = obj.cast::<crate::bindings::core::types::PyBps>() {
        return Ok(f64::from(bps.borrow().inner.as_bp()));
    }
    if let Ok(rate) = obj.cast::<crate::bindings::core::types::PyRate>() {
        return Ok(rate.borrow().inner.as_bp_f64());
    }
    obj.extract::<f64>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "expected a spread in basis points (float, e.g. 25.0) or a Bps/Rate \
             instance, got {}",
            obj.get_type()
                .name()
                .map(|n| n.to_string())
                .unwrap_or_default()
        ))
    })
}

/// Extract a canonical `CreditRating` from a `CreditRating` wrapper or a
/// rating string (`"BBB-"`, `"Baa3"`, `"bbb-"`; agency notation is normalised
/// by the core parser).
///
/// # Errors
///
/// Returns `ValueError` when the string is not a recognised rating and
/// `TypeError` when `obj` is neither a string nor a `CreditRating`.
pub fn extract_credit_rating(
    obj: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_core::types::CreditRating> {
    if let Ok(rating) = obj.cast::<crate::bindings::core::types::PyCreditRating>() {
        return Ok(rating.borrow().inner);
    }
    if let Ok(text) = obj.extract::<String>() {
        return text
            .parse::<finstack_quant_core::types::CreditRating>()
            .map_err(crate::errors::core_to_py);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "expected a rating string (e.g. 'BBB-') or a core.types.CreditRating instance, got {}",
        obj.get_type()
            .name()
            .map(|n| n.to_string())
            .unwrap_or_default()
    )))
}

// ScenarioSpec — typed-or-JSON extraction

/// Extract a [`finstack_quant_scenarios::ScenarioSpec`] from a typed
/// `ScenarioSpec` Python object (fast path, clone) or a canonical
/// `ScenarioSpec` JSON string (parsed while the GIL is released).
///
/// # Errors
///
/// Returns `TypeError` when `obj` is neither, and `ValueError` when the
/// JSON string does not deserialize as a `ScenarioSpec` or fails
/// `ScenarioSpec::validate` (both through `ScenarioSpec::from_json`).
pub fn extract_scenario_spec(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_scenarios::ScenarioSpec> {
    if let Ok(spec) = obj.cast::<crate::bindings::scenarios::spec::PyScenarioSpec>() {
        return Ok(spec.borrow().inner.clone());
    }
    let json: String = obj.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "expected a ScenarioSpec instance or a canonical ScenarioSpec JSON string",
        )
    })?;
    py.detach(move || finstack_quant_scenarios::ScenarioSpec::from_json(&json))
        .map_err(crate::errors::scenarios_to_py)
}

/// Extract an ordered batch of scenario specs from either a JSON array string
/// or a Python sequence whose items are each `ScenarioSpec | str` (see
/// [`extract_scenario_spec`]).
///
/// # Errors
///
/// Returns `TypeError` for an item that is neither form and `ValueError` for
/// malformed JSON or a spec that fails `ScenarioSpec::validate`.
pub fn extract_scenario_specs(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<Vec<finstack_quant_scenarios::ScenarioSpec>> {
    if let Ok(json) = obj.extract::<String>() {
        let specs: Vec<finstack_quant_scenarios::ScenarioSpec> = py
            .detach(move || serde_json::from_str(&json))
            .map_err(to_py)?;
        for spec in &specs {
            spec.validate().map_err(crate::errors::scenarios_to_py)?;
        }
        return Ok(specs);
    }
    let mut specs = Vec::new();
    for item in obj.try_iter()? {
        specs.push(extract_scenario_spec(py, &item?)?);
    }
    Ok(specs)
}

// PortfolioCashflows — borrow-preferring access

/// Access to a [`finstack_quant_portfolio::cashflows::PortfolioCashflows`]
/// ladder without re-parsing JSON when a typed Python object is passed.
pub enum CashflowsAccess<'py> {
    Borrowed(PyRef<'py, crate::bindings::portfolio::types::PyPortfolioCashflows>),
    Owned(Box<finstack_quant_portfolio::cashflows::PortfolioCashflows>),
}

impl std::ops::Deref for CashflowsAccess<'_> {
    type Target = finstack_quant_portfolio::cashflows::PortfolioCashflows;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => &r.inner,
            Self::Owned(c) => c.as_ref(),
        }
    }
}

/// Extract a `PortfolioCashflows` ladder from a typed Python object or a
/// full cashflow-ladder JSON string.
pub fn extract_cashflows_ref<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<CashflowsAccess<'py>> {
    if let Ok(c) = obj.cast::<crate::bindings::portfolio::types::PyPortfolioCashflows>() {
        return Ok(CashflowsAccess::Borrowed(c.borrow()));
    }
    let json: String = obj.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "expected a PortfolioCashflows instance or a cashflow-ladder JSON string",
        )
    })?;
    let inner: finstack_quant_portfolio::cashflows::PortfolioCashflows = py
        .detach(move || serde_json::from_str(&json))
        .map_err(to_py)?;
    Ok(CashflowsAccess::Owned(Box::new(inner)))
}

// Records — JSON string | dict | list | pandas.DataFrame → JSON string

/// Turn a JSON-shaped Python input into its compact JSON string.
///
/// Accepts a pre-serialized JSON `str` (passed through unchanged), any
/// `json.dumps`-able object (`dict`, `list`, tuples of scalars), typed
/// result/spec wrappers standalone or nested in lists and dicts (encoded
/// through their `to_dict` / `to_json`), or a `pandas.DataFrame`, which is
/// converted through `to_dict("records")` so each row becomes one JSON object.
///
/// # Errors
///
/// Returns `ValueError` when the object cannot be JSON-encoded.
pub fn extract_records_json(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    label: &str,
) -> PyResult<String> {
    if let Ok(json) = obj.extract::<String>() {
        return Ok(json);
    }
    if obj.hasattr("to_dict")? && obj.hasattr("columns")? {
        let records = obj.call_method1("to_dict", ("records",))?;
        return crate::bindings::module_utils::py_to_json_string(py, &records, label);
    }
    crate::bindings::module_utils::py_to_json_string(py, obj, label)
}
