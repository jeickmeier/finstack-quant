//! Reusable WASM `Portfolio` handle and spec parser.

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Handle to a built Rust `Portfolio` (positions and books) that can be reused
/// across WASM calls without re-parsing and rebuilding from the spec.
///
/// `Portfolio::from_spec` parses positions, builds indices, and validates
/// invariants; for pipelines that call both `valuePortfolio` and
/// `aggregateFullCashflows` on the same portfolio, holding this handle
/// avoids paying that cost twice.
///
/// @example
/// ```typescript
/// import init, { portfolio } from "finstack-quant-wasm";
/// await init();
/// const book = portfolio.Portfolio.builder("BOOK", "USD", "2025-01-02").build();
/// console.log(book.id, book.baseCurrency);
/// book.free();
/// ```
#[wasm_bindgen(js_name = Portfolio)]
pub struct JsPortfolio {
    pub(crate) inner: finstack_quant_portfolio::Portfolio,
}

#[wasm_bindgen(js_class = Portfolio)]
impl JsPortfolio {
    /// Read immutable holdings in insertion order.
    /// @returns An array of position handles for all holdings.
    #[wasm_bindgen(getter)]
    pub fn positions(&self) -> Vec<super::position::JsPosition> {
        self.inner
            .positions()
            .iter()
            .cloned()
            .map(|inner| super::position::JsPosition { inner })
            .collect()
    }

    /// Copy holdings and metadata into a fluent builder.
    /// @returns New builder seeded with this portfolio's data.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> super::builder::JsPortfolioBuilder {
        super::builder::JsPortfolioBuilder {
            inner: self.inner.to_builder(),
        }
    }

    /// Build a portfolio copy with the additional holding.
    /// @param position - Holding with a unique identifier and valid entity and book references.
    /// @returns New validated portfolio including the new holding.
    /// @throws FinstackError on a duplicate ID or unknown entity or book.
    #[wasm_bindgen(js_name = withPosition)]
    pub fn with_position(
        &self,
        position: &super::position::JsPosition,
    ) -> Result<JsPortfolio, JsValue> {
        self.inner
            .with_position(position.inner.clone())
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Start a fluent portfolio builder (Rust `PortfolioBuilder`).
    /// @param id - Portfolio identifier.
    /// @param baseCurrency - ISO-4217 reporting currency used for every base-currency rollup.
    /// @param asOf - ISO-8601 valuation date.
    /// @returns A `PortfolioBuilder`; chain `.entity(...)` / `.position(...)` / `.tag(...)` and finish with `.build()`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) if an argument is not a
    /// string, and a `FinstackError` (kind `validation`) if `baseCurrency` is
    /// not an ISO-4217 code or `asOf` is not an ISO date.
    pub fn builder(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(js_name = "baseCurrency", unchecked_param_type = "string")]
        base_currency: JsValue,
        #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    ) -> Result<super::builder::JsPortfolioBuilder, JsValue> {
        super::builder::JsPortfolioBuilder::start(&id, &base_currency, &as_of)
    }

    /// Build from a JSON-serialised `PortfolioSpec`.
    /// @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `specJson` is malformed or does not match
    /// the portfolio schema, a position has an invalid quantity or instrument
    /// specification, or portfolio validation finds duplicate identifiers or an
    /// unknown entity reference.
    /// @returns A reusable portfolio handle.
    #[wasm_bindgen(js_name = fromSpec)]
    pub fn from_spec(
        #[wasm_bindgen(js_name = "specJson", unchecked_param_type = "JsonInput")]
        spec_json: JsValue,
    ) -> Result<JsPortfolio, JsValue> {
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
    #[wasm_bindgen(getter, unchecked_return_type = "Record<string, string>")]
    pub fn tags(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.tags)
    }

    /// Portfolio-level metadata as a plain JSON-shaped object.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the metadata cannot be converted to a
    /// JavaScript value.
    #[wasm_bindgen(getter, unchecked_return_type = "Record<string, unknown>")]
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
    /// @returns Non-negative position count.
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
    #[wasm_bindgen(js_name = toSpec, unchecked_return_type = "generated.portfolio.PortfolioSpec")]
    pub fn to_spec(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.to_spec())
    }

    /// Serialise the canonical spec (Rust `Portfolio::to_spec`) back to JSON.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the canonical portfolio specification
    /// cannot be serialized to JSON.
    /// @returns Canonical JSON string accepted by `Portfolio.fromSpec`.
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
/// @param jsonStr - Canonical JSON string to validate and re-serialize.
///
/// # Errors
///
/// Throws a JavaScript exception if `jsonStr` is malformed or does not match the
/// `PortfolioSpec` schema, or if the canonical form cannot be serialized.
/// @returns Canonical portfolio-specification JSON after validation.
#[wasm_bindgen(js_name = parsePortfolioSpecJson)]
pub fn parse_portfolio_spec_json(
    #[wasm_bindgen(js_name = "jsonStr", unchecked_param_type = "JsonInput")] json_str: JsValue,
) -> Result<String, JsValue> {
    let spec: finstack_quant_portfolio::PortfolioSpec = from_js_json(&json_str, "jsonStr")?;

    serde_json::to_string(&spec).map_err(to_js_err)
}
