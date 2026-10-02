//! WASM binding for the fluent [`finstack_quant_portfolio::PortfolioBuilder`].

use std::sync::Arc;

use super::JsPortfolio;
use crate::utils::date::parse_iso_date;
use crate::utils::input::{from_js_json, js_string};
use crate::utils::to_js_err;
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::{PortfolioBuilder, Position};
use wasm_bindgen::prelude::*;

/// Fluent builder returned by `Portfolio.builder(id, baseCurrency, asOf)`.
///
/// Mirrors the Rust `PortfolioBuilder`: every setter consumes the handle it is
/// called on and returns the builder, so chain the calls (or reassign the
/// returned handle); `build()` validates the portfolio and returns a reusable
/// `Portfolio`. Positions that name the standalone entity get it created
/// automatically.
#[wasm_bindgen(js_name = PortfolioBuilder)]
pub struct JsPortfolioBuilder {
    inner: PortfolioBuilder,
}

impl JsPortfolioBuilder {
    /// Start a builder with the reporting currency and valuation date set.
    pub(super) fn start(
        id: &JsValue,
        base_currency: &JsValue,
        as_of: &JsValue,
    ) -> Result<Self, JsValue> {
        let id = js_string(id, "id")?;
        let base_currency: finstack_quant_core::currency::Currency =
            js_string(base_currency, "baseCurrency")?
                .parse()
                .map_err(to_js_err)?;
        let as_of = parse_iso_date(&js_string(as_of, "asOf")?)?;
        Ok(Self {
            inner: PortfolioBuilder::new(id)
                .base_currency(base_currency)
                .as_of(as_of),
        })
    }
}

#[wasm_bindgen(js_class = PortfolioBuilder)]
impl JsPortfolioBuilder {
    /// Set the human-readable portfolio name.
    /// @param name - Display name stored alongside the portfolio identifier.
    /// @returns The builder, for chaining.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) if `name` is not a string.
    pub fn name(self, name: JsValue) -> Result<JsPortfolioBuilder, JsValue> {
        let name = js_string(&name, "name")?;
        Ok(Self {
            inner: self.inner.name(name),
        })
    }

    /// Register an entity that positions can reference.
    /// @param entity - `Entity` object or JSON: `{ id, name, tags?, meta? }` (`name` may be `null`).
    /// @returns The builder, for chaining.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) if `entity` is not a JSON
    /// string or plain object, and a `FinstackError` (kind `validation`) if it
    /// does not match the `Entity` schema.
    pub fn entity(self, entity: JsValue) -> Result<JsPortfolioBuilder, JsValue> {
        let entity: Entity = from_js_json(&entity, "entity")?;
        Ok(Self {
            inner: self.inner.entity(entity),
        })
    }

    /// Add a position from its serializable specification.
    /// @param position - `PositionSpec` object or JSON: `position_id`, `entity_id`, `instrument_id`, `instrument_spec` (the tagged instrument JSON), signed `quantity`, `unit`, and optional `book_id`, `attributes` and `meta`.
    /// @returns The builder, for chaining.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) if `position` is not a JSON
    /// string or plain object, and a `FinstackError` (kind `validation`) if it
    /// does not match the `PositionSpec` schema, carries no `instrument_spec`,
    /// the instrument cannot be built, or `quantity` is not finite.
    pub fn position(self, position: JsValue) -> Result<JsPortfolioBuilder, JsValue> {
        let spec = from_js_json(&position, "position")?;
        let position = Position::from_spec(spec).map_err(to_js_err)?;
        Ok(Self {
            inner: self.inner.position(position),
        })
    }

    /// Attach a portfolio-level tag.
    /// @param key - Tag name used for grouping and filtering, such as `desk`; a repeated key replaces the earlier value.
    /// @param value - Text stored under `key`, such as `rates`.
    /// @returns The builder, for chaining.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) if either argument is not a
    /// string.
    pub fn tag(self, key: JsValue, value: JsValue) -> Result<JsPortfolioBuilder, JsValue> {
        let key = js_string(&key, "key")?;
        let value = js_string(&value, "value")?;
        Ok(Self {
            inner: self.inner.tag(key, value),
        })
    }

    /// Attach a JSON-shaped metadata entry.
    /// @param key - Name the metadata entry is stored under; a repeated key replaces the earlier entry.
    /// @param value - Any JSON value (object, array, string, number, boolean or `null`), or a JSON string encoding one.
    /// @returns The builder, for chaining.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) if `key` is not a string or
    /// `value` is not a JSON string or plain object, and a `FinstackError`
    /// (kind `validation`) if a string `value` is not valid JSON.
    pub fn meta(self, key: JsValue, value: JsValue) -> Result<JsPortfolioBuilder, JsValue> {
        let key = js_string(&key, "key")?;
        let value: serde_json::Value = from_js_json(&value, "value")?;
        Ok(Self {
            inner: self.inner.meta(key, value),
        })
    }

    /// Validate and build the portfolio, consuming the builder.
    /// @returns The validated, reusable `Portfolio` handle.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `validation`) if a position references an
    /// unregistered entity, an identifier is duplicated, or another structural
    /// invariant fails.
    pub fn build(self) -> Result<JsPortfolio, JsValue> {
        let portfolio = self.inner.build().map_err(to_js_err)?;
        Ok(JsPortfolio {
            inner: Arc::new(portfolio),
        })
    }
}
