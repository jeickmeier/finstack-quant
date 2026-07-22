//! Shared polymorphic extraction helpers for PyO3 bindings.
//!
//! Each helper accepts a `&Bound<'_, PyAny>` and tries two paths:
//!
//! 1. **Typed fast path** — cast to the corresponding `#[pyclass]` wrapper
//!    and borrow the inner Rust type (no clone, no JSON parse).
//! 2. **JSON fallback** — extract a Python `str`, then `serde_json::from_str`.
//!    This keeps backward compatibility with callers that pass pre-serialized
//!    JSON strings.
//!
//! The `*Access` enums wrap both paths behind a `Deref<Target = T>` impl so
//! pipeline functions can accept `T | str` without branching.

use pyo3::prelude::*;

use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::portfolio::types::{PyPortfolio, PyPortfolioResult, PyPortfolioValuation};
use crate::bindings::statements::evaluator::PyStatementResult;
use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::bindings::valuations::instruments::{PyBond, PyTermLoan};
use crate::errors::{display_to_py as to_py, portfolio_to_py};

// ---------------------------------------------------------------------------
// Instrument — typed-or-JSON extraction to tagged instrument JSON
// ---------------------------------------------------------------------------

/// Extract tagged instrument JSON from a typed `Bond` / `TermLoan` object
/// (fast path) or a pre-serialized JSON string (fallback).
///
/// Typed instances serialize through the same tagged union
/// (`InstrumentJson`) the JSON loader parses, so downstream pricing observes
/// identical payloads for both input forms.
pub fn extract_instrument_json(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(bond) = obj.cast::<PyBond>() {
        return bond.borrow().tagged_json();
    }
    if let Ok(loan) = obj.cast::<PyTermLoan>() {
        return loan.borrow().tagged_json();
    }
    obj.extract::<String>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "expected tagged instrument JSON (str) or a typed Bond / TermLoan instance",
        )
    })
}

// ---------------------------------------------------------------------------
// Zero-clone access types (available for callers that only need &T)
// ---------------------------------------------------------------------------

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
    let inner: finstack_quant_statements::FinancialModelSpec =
        serde_json::from_str(&json).map_err(to_py)?;
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

// ---------------------------------------------------------------------------
// Owned extraction (for callers that need mutable or owned values)
// ---------------------------------------------------------------------------

/// Extract a [`FinancialModelSpec`] — always produces an owned value.
///
/// Prefer [`extract_model_ref`] when only a reference is needed.
pub fn extract_model(
    obj: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_statements::FinancialModelSpec> {
    if let Ok(spec) = obj.cast::<PyFinancialModelSpec>() {
        return Ok(spec.borrow().inner.clone());
    }
    let json: String = obj.extract()?;
    serde_json::from_str(&json).map_err(to_py)
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

// ---------------------------------------------------------------------------
// MarketContext — borrow-preferring access
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Portfolio — borrow-preferring access
// ---------------------------------------------------------------------------

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
    let spec: finstack_quant_portfolio::portfolio::PortfolioSpec = py
        .detach(move || serde_json::from_str(&json))
        .map_err(to_py)?;
    let portfolio = py
        .detach(move || finstack_quant_portfolio::Portfolio::from_spec(spec))
        .map_err(portfolio_to_py)?;
    Ok(PortfolioAccess::Owned(Box::new(portfolio)))
}

// ---------------------------------------------------------------------------
// PortfolioValuation — borrow-preferring access
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// PortfolioResult — borrow-preferring access
// ---------------------------------------------------------------------------

/// Access to a [`PortfolioResult`] without re-parsing JSON when a typed
/// Python object is passed.
pub enum PortfolioResultAccess<'py> {
    Borrowed(PyRef<'py, PyPortfolioResult>),
    Owned(Box<finstack_quant_portfolio::results::PortfolioResult>),
}

impl std::ops::Deref for PortfolioResultAccess<'_> {
    type Target = finstack_quant_portfolio::results::PortfolioResult;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => &r.inner,
            Self::Owned(r) => r.as_ref(),
        }
    }
}

/// Extract a [`PortfolioResult`] from a typed Python object or a JSON string.
pub fn extract_portfolio_result_ref<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<PortfolioResultAccess<'py>> {
    if let Ok(r) = obj.cast::<PyPortfolioResult>() {
        return Ok(PortfolioResultAccess::Borrowed(r.borrow()));
    }
    let json: String = obj.extract()?;
    let inner: finstack_quant_portfolio::results::PortfolioResult = py
        .detach(move || serde_json::from_str(&json))
        .map_err(to_py)?;
    Ok(PortfolioResultAccess::Owned(Box::new(inner)))
}
