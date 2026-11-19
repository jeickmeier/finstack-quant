//! Position handles own a snapshot of the holding and share its instrument.

use crate::utils::input::{js_f64, js_opt_string, js_string, json_text, opt_from_js_json};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::DUMMY_ENTITY_ID;
use finstack_quant_valuations::pricer::parse_boxed_instrument_from_json;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Immutable holding with a signed quantity and a shared instrument.
///
/// @example
/// ```typescript
/// const deposit = valuations.instruments.Deposit.example();
/// const holding = new portfolio.Position("DEP-LOT", deposit.toJson(), 2);
/// console.log(holding.positionId, holding.quantity);
/// holding.free();
/// deposit.free();
/// ```
#[wasm_bindgen(js_name = Position)]
pub struct JsPosition {
    pub(super) inner: Position,
}

#[wasm_bindgen(js_class = Position)]
impl JsPosition {
    /// Create a signed holding of an instrument for one owner.
    /// @param positionId - Identifier unique within a portfolio.
    /// @param instrument - Tagged instrument envelope object or JSON text.
    /// @param quantity - Finite signed quantity in the given unit; percentage is bounded by ±100.
    /// @param entityId - Optional owning entity identifier; omitted selects the standalone entity.
    /// @param unit - Optional quantity convention, defaulting to units.
    /// @param attributes - Optional labels and numerical attributes used in grouping.
    /// @returns Validated position handle.
    /// @throws TypeError if any input has an incompatible type; a FinstackError if the instrument, quantity or unit is invalid.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "positionId", unchecked_param_type = "string")]
        position_id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "generated.valuations.InstrumentEnvelope | string")]
        instrument: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] quantity: JsValue,
        #[wasm_bindgen(js_name = "entityId", unchecked_optional_param_type = "string | null")]
        entity_id: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "generated.portfolio.PositionUnit | null")]
        unit: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "Record<string, string | number> | null")]
        attributes: Option<JsValue>,
    ) -> Result<JsPosition, JsValue> {
        let position_id = js_string(&position_id, "positionId")?;
        let parsed =
            parse_boxed_instrument_from_json(&json_text(&instrument, "instrument")?, None, None)
                .map_err(to_js_err)?;
        let instrument_id = parsed.as_instrument().id().to_string();
        let instrument = parsed.into_boxed();
        let unit = if let Some(label) = unit.as_ref().and_then(JsValue::as_string) {
            label.parse::<PositionUnit>().map_err(to_js_err)?
        } else {
            opt_from_js_json(unit.as_ref(), "unit")?.unwrap_or(PositionUnit::Units)
        };
        let entity_id = js_opt_string(entity_id.as_ref(), "entityId")?;
        let mut inner = Position::new(
            position_id,
            entity_id.as_deref().unwrap_or(DUMMY_ENTITY_ID),
            instrument_id,
            Arc::from(instrument),
            js_f64(&quantity, "quantity")?,
            unit,
        )
        .map_err(to_js_err)?;
        inner.attributes = opt_from_js_json(attributes.as_ref(), "attributes")?.unwrap_or_default();
        Ok(Self { inner })
    }

    /// Read one holding from its canonical position specification.
    /// @param json - JSON specifying the position, instrument, quantity and owner.
    /// @returns Reusable position handle.
    /// @throws TypeError for a mistyped input or a FinstackError for invalid JSON, instrument, unit or quantity.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "string")] json: JsValue,
    ) -> Result<JsPosition, JsValue> {
        let json = js_string(&json, "json")?;
        let spec = serde_json::from_str(&json).map_err(to_js_err)?;
        Position::from_spec(spec)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Serialize this holding as a canonical PositionSpec document.
    /// @returns JSON with the embedded instrument and position metadata.
    /// @throws FinstackError if serialization fails.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner.to_spec()).map_err(to_js_err)
    }

    /// Read the position identifier.
    /// @returns Identifier unique within the originating portfolio.
    #[wasm_bindgen(getter, js_name = positionId)]
    pub fn position_id(&self) -> String {
        self.inner.position_id.to_string()
    }

    /// Read the instrument identifier.
    /// @returns Identifier of the holding's instrument.
    #[wasm_bindgen(getter, js_name = instrumentId)]
    pub fn instrument_id(&self) -> String {
        self.inner.instrument_id.clone()
    }

    /// Read the owning entity.
    /// @returns Identifier of the owning entity.
    #[wasm_bindgen(getter, js_name = entityId)]
    pub fn entity_id(&self) -> String {
        self.inner.entity_id.to_string()
    }

    /// Read the assigned book.
    /// @returns Book identifier, or undefined when unassigned.
    #[wasm_bindgen(getter, js_name = bookId)]
    pub fn book_id(&self) -> Option<String> {
        self.inner.book_id.as_ref().map(ToString::to_string)
    }

    /// Read the signed holding.
    /// @returns Signed quantity in the selected unit.
    #[wasm_bindgen(getter)]
    pub fn quantity(&self) -> f64 {
        self.inner.quantity
    }

    /// Read the quantity unit.
    /// @returns Position quantity convention.
    /// @throws FinstackError if the unit cannot be encoded.
    #[wasm_bindgen(getter, unchecked_return_type = "generated.portfolio.PositionUnit")]
    pub fn unit(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.unit)
    }

    /// Read holding-level attributes.
    /// @returns Object mapping labels to text or numeric values.
    /// @throws FinstackError if attributes cannot be encoded.
    #[wasm_bindgen(getter, unchecked_return_type = "Record<string, string | number>")]
    pub fn attributes(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.attributes)
    }
}
