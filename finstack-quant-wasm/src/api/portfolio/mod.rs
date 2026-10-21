//! WASM bindings for the `finstack-quant-portfolio` crate.
//!
//! Exposes portfolio spec parsing, validation, and result extraction for
//! JavaScript/TypeScript consumption.
//!
//! # Return conventions
//!
//! Two return shapes are used, and the name says which:
//!
//! - **Computation results** — valuations, aggregations, attributions,
//!   optimizations, decompositions and P&L — return **plain structured
//!   JavaScript objects** built with
//!   [`crate::utils::to_js_value`]. Read properties directly; call
//!   `JSON.stringify` when a canonical JSON string is needed (for example to
//!   chain into an entry point that still takes a `*Json` argument).
//! - **Wire / spec / validator surfaces** — anything whose purpose is to echo a
//!   canonical document for re-ingest — return a **JSON string** and their
//!   names end in `Json` (`Portfolio.toJson`, `parsePortfolioSpecJson`).
//!
//! The materialization entry points (`Portfolio.fromMaterialization`,
//! `Portfolio.validateMaterialization`) return plain structured objects
//! (`{ portfolio, report }`, and `MaterializationReport | ValidationReport`
//! respectively); validation diagnostics are returned, not thrown.
//!
//! A Rust method on a result type is a free function that takes the plain
//! result object first (`portfolioAttributionExplainText`,
//! `portfolioValuationGetPositionValue`, `portfolioMetricsGetTotal`, ...); see
//! the `results` module.
//!
//! For repeated calls against the same portfolio (scenario sweeps,
//! interactive dashboards), reuse the `Portfolio` and `MarketContext` handles
//! accepted by the calculation entry points.

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

pub mod attribution;
pub mod brinson;
pub mod builder;
pub mod cashflows;
pub mod excess_return;
pub mod factor_brinson;
pub mod factor_model;
pub mod fi_attribution;
pub mod grid_attribution;
pub mod margin;
pub mod materialization;
pub mod metrics;
pub mod optimization;
pub mod performance;
pub mod primitive;
pub mod replay;
pub mod results;
pub mod scenarios;
pub mod sensitivity;
pub mod valuation;

/// Handle to a built [`finstack_quant_portfolio::Portfolio`] that can be reused
/// across WASM calls without re-parsing and rebuilding from the spec.
///
/// `Portfolio::from_spec` parses positions, builds indices, and validates
/// invariants; for pipelines that call both `valuePortfolio` and
/// `aggregateFullCashflows` on the same portfolio, holding this handle
/// avoids paying that cost twice.
#[wasm_bindgen(js_name = Portfolio)]
pub struct JsPortfolio {
    pub(crate) inner: finstack_quant_portfolio::Portfolio,
}

#[wasm_bindgen(js_class = Portfolio)]
impl JsPortfolio {
    /// Start a fluent portfolio builder (Rust `PortfolioBuilder`).
    /// @param id - Portfolio identifier.
    /// @param base_currency - ISO-4217 reporting currency used for every base-currency rollup.
    /// @param as_of - ISO-8601 valuation date.
    /// @returns A `PortfolioBuilder`; chain `.entity(...)` / `.position(...)` / `.tag(...)` and finish with `.build()`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) if an argument is not a
    /// string, and a `FinstackError` (kind `validation`) if `baseCurrency` is
    /// not an ISO-4217 code or `asOf` is not an ISO date.
    pub fn builder(
        id: JsValue,
        base_currency: JsValue,
        as_of: JsValue,
    ) -> Result<builder::JsPortfolioBuilder, JsValue> {
        builder::JsPortfolioBuilder::start(&id, &base_currency, &as_of)
    }

    /// Build from a JSON-serialised `PortfolioSpec`.
    /// @param spec_json - Canonical portfolio specification JSON defining positions, quantities, and base currency.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `specJson` is malformed or does not match
    /// the portfolio schema, a position has an invalid quantity or instrument
    /// specification, or portfolio validation finds duplicate identifiers or an
    /// unknown entity reference.
    #[wasm_bindgen(js_name = fromSpec)]
    pub fn from_spec(spec_json: JsValue) -> Result<JsPortfolio, JsValue> {
        let spec: finstack_quant_portfolio::PortfolioSpec = from_js_json(&spec_json, "specJson")?;
        let portfolio = finstack_quant_portfolio::Portfolio::from_spec(spec).map_err(to_js_err)?;
        Ok(Self { inner: portfolio })
    }

    /// Portfolio identifier.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Valuation date (ISO 8601).
    #[wasm_bindgen(getter, js_name = asOf)]
    pub fn as_of(&self) -> String {
        self.inner.as_of.to_string()
    }

    /// Base currency code.
    #[wasm_bindgen(getter, js_name = baseCurrency)]
    pub fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    /// Human-readable portfolio name, or `null` when unset.
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> Option<String> {
        self.inner.name.clone()
    }

    /// Portfolio-level tags as a plain `{ key: value }` object.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the tags cannot be converted to a
    /// JavaScript value.
    #[wasm_bindgen(getter)]
    pub fn tags(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.tags)
    }

    /// Portfolio-level metadata as a plain JSON-shaped object.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the metadata cannot be converted to a
    /// JavaScript value.
    #[wasm_bindgen(getter)]
    pub fn meta(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.meta)
    }

    /// Entity identifiers in registration order (includes the auto-created
    /// standalone entity when any position uses it).
    #[wasm_bindgen(getter, js_name = entityIds)]
    pub fn entity_ids(&self) -> Vec<String> {
        self.inner
            .entities
            .keys()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    /// Position identifiers in portfolio order.
    #[wasm_bindgen(getter, js_name = positionIds)]
    pub fn position_ids(&self) -> Vec<String> {
        self.inner
            .positions()
            .iter()
            .map(|p| p.position_id.as_str().to_owned())
            .collect()
    }

    /// Number of positions in the portfolio.
    #[wasm_bindgen(js_name = numPositions)]
    pub fn num_positions(&self) -> usize {
        self.inner.positions().len()
    }

    /// Canonical `PortfolioSpec` of the portfolio (Rust `Portfolio::to_spec`).
    ///
    /// @returns Plain `PortfolioSpec` object accepted by `Portfolio.fromSpec`.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the specification cannot be converted
    /// to a JavaScript value.
    #[wasm_bindgen(js_name = toSpec)]
    pub fn to_spec(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.to_spec())
    }

    /// Serialise the canonical spec (Rust `Portfolio::to_spec`) back to JSON.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the canonical portfolio specification
    /// cannot be serialized to JSON.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        let spec = self.inner.to_spec();
        serde_json::to_string(&spec).map_err(to_js_err)
    }
}

/// Parse and validate a portfolio specification from JSON.
///
/// Wire/validator surface: returns the re-serialized canonical JSON **string**,
/// suitable for storage or re-ingest by `Portfolio.fromSpec`.
/// @param json_str - Canonical JSON string to validate and re-serialize.
///
/// # Errors
///
/// Throws a JavaScript exception if `jsonStr` is malformed or does not match the
/// `PortfolioSpec` schema, or if the canonical form cannot be serialized.
#[wasm_bindgen(js_name = parsePortfolioSpecJson)]
pub fn parse_portfolio_spec_json(json_str: JsValue) -> Result<String, JsValue> {
    let spec: finstack_quant_portfolio::PortfolioSpec = from_js_json(&json_str, "jsonStr")?;

    serde_json::to_string(&spec).map_err(to_js_err)
}
