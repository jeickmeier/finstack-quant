//! WASM bindings for `finstack_quant_core::config`: `RoundingMode` and
//! `FinstackConfig`.
//!
//! `ToleranceConfig` is plain data: it crosses the boundary as a plain object
//! typed by the schema-generated `ToleranceConfig` type.

use crate::utils::input::{
    from_js_json, js_currency, js_json_value, js_opt_f64, js_opt_string, js_string, js_uint,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::config::{FinstackConfig, RoundingMode, ToleranceConfig};
use finstack_quant_core::currency::Currency;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

/// Construct validated absolute comparison tolerances using Rust defaults.
///
/// # Arguments
///
/// * `generic_epsilon` - Optional finite, strictly positive absolute tolerance;
///   `null` or omission uses `ToleranceConfig::default()` (currently `1e-10`).
///
/// @returns A `ToleranceConfig` plain object with `generic_epsilon`.
/// @throws `TypeError` (kind `invalid_type`) for a supplied non-number;
/// `FinstackError` (kind `validation`) for zero, negative or non-finite values.
/// @param genericEpsilon - Optional finite, strictly positive absolute tolerance; `null` or omission uses `ToleranceConfig::default()` (currently `1e-10`).
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const tolerance = core.toleranceConfigNew();
/// console.log(tolerance.generic_epsilon); // 1e-10
/// ```
#[wasm_bindgen(js_name = toleranceConfigNew, unchecked_return_type = "ToleranceConfig")]
pub fn tolerance_config_new(
    #[wasm_bindgen(
        js_name = "genericEpsilon",
        unchecked_optional_param_type = "number | null"
    )]
    generic_epsilon: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let epsilon = js_opt_f64(generic_epsilon.as_ref(), "genericEpsilon")?
        .unwrap_or_else(|| ToleranceConfig::default().generic_epsilon);
    to_js_value(&ToleranceConfig::new(epsilon).map_err(to_js_err)?)
}

/// Decimal rounding mode used when amounts are scaled for ingest or output.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// core.RoundingMode.bankers().toString(); // "bankers"
/// core.RoundingMode.fromName("midpoint_away_from_zero").toJson(); // "\"midpoint_away_from_zero\""
/// ```
#[wasm_bindgen(js_name = RoundingMode)]
#[derive(Clone, Copy, Debug)]
pub struct JsRoundingMode {
    pub(crate) inner: RoundingMode,
}

impl JsRoundingMode {
    pub(crate) const fn from_inner(inner: RoundingMode) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = RoundingMode)]
impl JsRoundingMode {
    /// Round half to even (banker's rounding).
    ///
    /// @returns The `bankers` mode (exact midpoint to even).
    #[wasm_bindgen(js_name = bankers)]
    pub fn bankers() -> Self {
        Self::from_inner(RoundingMode::Bankers)
    }

    /// Round an exact midpoint away from zero; other fractions to nearest.
    ///
    /// @returns The `midpoint_away_from_zero` mode.
    #[wasm_bindgen(js_name = midpointAwayFromZero)]
    pub fn midpoint_away_from_zero() -> Self {
        Self::from_inner(RoundingMode::MidpointAwayFromZero)
    }

    /// Truncate toward zero.
    ///
    /// @returns The `toward_zero` mode.
    #[wasm_bindgen(js_name = towardZero)]
    pub fn toward_zero() -> Self {
        Self::from_inner(RoundingMode::TowardZero)
    }

    /// Round toward negative infinity.
    ///
    /// @returns The `floor` mode.
    #[wasm_bindgen(js_name = floor)]
    pub fn floor() -> Self {
        Self::from_inner(RoundingMode::Floor)
    }

    /// Round toward positive infinity.
    ///
    /// @returns The `ceil` mode.
    #[wasm_bindgen(js_name = ceil)]
    pub fn ceil() -> Self {
        Self::from_inner(RoundingMode::Ceil)
    }

    /// Parse a rounding-mode name (Rust `RoundingMode::from_str`).
    ///
    /// # Arguments
    ///
    /// * `name` - Lowercase mode name: `"bankers"`, `"midpoint_away_from_zero"`,
    ///   `"toward_zero"`, `"floor"` or `"ceil"`.
    ///
    /// @returns The matching `RoundingMode`.
    /// @throws `TypeError` (kind `invalid_type`) if `name` is not a string;
    /// `FinstackError` (kind `validation`) if no mode matches.
    /// @param name - Lowercase mode name: `"bankers"`, `"midpoint_away_from_zero"`, `"toward_zero"`, `"floor"` or `"ceil"`.
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(
        #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
    ) -> Result<JsRoundingMode, JsValue> {
        js_string(&name, "name")?
            .parse::<RoundingMode>()
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Serialize to the canonical JSON wire form (a quoted lowercase name).
    ///
    /// @returns JSON text such as `"\"bankers\""`.
    /// @throws If serialization fails (not expected).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form produced by `toJson`.
    ///
    /// # Arguments
    ///
    /// * `json` - JSON text holding the quoted lowercase mode name, such as
    ///   `"\"bankers\""`.
    ///
    /// @returns The parsed `RoundingMode`.
    /// @throws `TypeError` if `json` is not a string; `FinstackError` (kind
    /// `validation`) if it is not a quoted mode name.
    /// @param json - JSON text holding the quoted lowercase mode name, such as `"\"bankers\""`.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "string")] json: JsValue,
    ) -> Result<JsRoundingMode, JsValue> {
        from_js_json(&json, "json").map(Self::from_inner)
    }

    /// Lowercase name of the mode, such as `"bankers"`.
    ///
    /// @returns The name accepted by `fromName`.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Currency-keyed scale overrides as a plain `{ code: scale }` object.
fn scale_overrides(overrides: &BTreeMap<Currency, u32>) -> Result<JsValue, JsValue> {
    let by_code: BTreeMap<String, u32> = overrides
        .iter()
        .map(|(currency, scale)| (currency.to_string(), *scale))
        .collect();
    to_js_value(&by_code)
}

/// Global configuration: rounding mode, per-currency decimal scales, numeric
/// tolerances and versioned extension sections.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const config = new core.FinstackConfig("midpoint_away_from_zero");
/// config.setOutputScale("JPY", 2);
/// config.outputScale("JPY"); // 2
/// config.outputScale("USD"); // 2 (the ISO-4217 minor units)
/// const copy = core.FinstackConfig.fromJson(config.toJson());
/// copy.roundingMode.toString(); // "midpoint_away_from_zero"
/// ```
#[wasm_bindgen(js_name = FinstackConfig)]
#[derive(Clone, Debug)]
pub struct JsFinstackConfig {
    pub(crate) inner: FinstackConfig,
}

#[wasm_bindgen(js_class = FinstackConfig)]
impl JsFinstackConfig {
    /// Create a configuration from the Rust defaults.
    ///
    /// # Arguments
    ///
    /// * `rounding_mode` - Lowercase rounding-mode name (for example
    ///   `"bankers"`, or `mode.toString()` of a `RoundingMode`); omitted selects
    ///   no mode (there is no default financial mode, so an ingest scale
    ///   override then cannot be applied).
    /// * `tolerances` - `ToleranceConfig` object or JSON text with the
    ///   optional absolute tolerance `generic_epsilon` (default `1e-10`),
    ///   finite and positive; omitted uses the default.
    ///
    /// @returns A new `FinstackConfig`.
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
    /// `FinstackError` (kind `validation`) for an unknown rounding mode, an
    /// unknown tolerance field, or a non-positive tolerance.
    /// @param roundingMode - Lowercase rounding-mode name (for example `"bankers"`, or `mode.toString()` of a `RoundingMode`); omitted selects no mode (there is no default financial mode, so an ingest scale override then cannot be applied).
    /// @param tolerances - `ToleranceConfig` object or JSON text with the optional absolute tolerance `generic_epsilon` (default `1e-10`), finite and positive; omitted uses the default.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(
            js_name = "roundingMode",
            unchecked_optional_param_type = "string | null"
        )]
        rounding_mode: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "ToleranceConfig | string | null")]
        tolerances: Option<JsValue>,
    ) -> Result<JsFinstackConfig, JsValue> {
        let mut inner = FinstackConfig::default();
        if let Some(name) = js_opt_string(rounding_mode.as_ref(), "roundingMode")? {
            inner.rounding.mode = Some(name.parse().map_err(to_js_err)?);
        }
        if let Some(value) = tolerances.filter(|v| !(v.is_null() || v.is_undefined())) {
            inner.tolerances = from_js_json::<ToleranceConfig>(&value, "tolerances")?;
        }
        Ok(Self { inner })
    }

    /// Explicitly selected rounding mode as a `RoundingMode`, or `undefined`
    /// when none was selected.
    #[wasm_bindgen(getter, js_name = roundingMode)]
    pub fn rounding_mode(&self) -> Option<JsRoundingMode> {
        self.inner.rounding.mode.map(JsRoundingMode::from_inner)
    }

    /// Numeric tolerances as a plain `ToleranceConfig` object (`generic_epsilon`).
    #[wasm_bindgen(getter, js_name = tolerances, unchecked_return_type = "ToleranceConfig")]
    pub fn tolerances(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.tolerances)
    }

    /// Decimal places used when formatting amounts in a currency (Rust
    /// `FinstackConfig::output_scale`).
    ///
    /// # Arguments
    ///
    /// * `currency` - ISO-4217 alphabetic code, such as `"USD"`.
    ///
    /// @returns The override for that currency, or its ISO-4217 minor units.
    /// @throws `TypeError` if `currency` is not a string; `FinstackError`
    /// (kind `validation`) if it is not a supported currency code.
    /// @param currency - ISO-4217 alphabetic code, such as `"USD"`.
    #[wasm_bindgen(js_name = outputScale)]
    pub fn output_scale(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    ) -> Result<u32, JsValue> {
        Ok(self.inner.output_scale(js_currency(&currency, "currency")?))
    }

    /// Configured ingest decimal places for a currency (Rust
    /// `FinstackConfig::ingest_scale`).
    ///
    /// # Arguments
    ///
    /// * `currency` - ISO-4217 alphabetic code, such as `"USD"`.
    ///
    /// @returns The override for that currency, or `undefined` when none is
    /// set, which means no ingest rounding.
    /// @throws `TypeError` if `currency` is not a string; `FinstackError`
    /// (kind `validation`) if it is not a supported currency code.
    /// @param currency - ISO-4217 alphabetic code, such as `"USD"`.
    #[wasm_bindgen(js_name = ingestScale)]
    pub fn ingest_scale(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    ) -> Result<Option<u32>, JsValue> {
        Ok(self.inner.ingest_scale(js_currency(&currency, "currency")?))
    }

    /// Override the output decimal places of one currency.
    ///
    /// # Arguments
    ///
    /// * `currency` - ISO-4217 alphabetic code, such as `"JPY"`.
    /// * `scale` - Number of decimal places, an integer in `0..=28`.
    ///
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if `currency` is not a supported currency code or `scale`
    /// exceeds 28.
    /// @param currency - ISO-4217 alphabetic code, such as `"JPY"`.
    /// @param scale - Number of decimal places, an integer in `0..=28`.
    #[wasm_bindgen(js_name = setOutputScale)]
    pub fn set_output_scale(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] scale: JsValue,
    ) -> Result<(), JsValue> {
        let currency = js_currency(&currency, "currency")?;
        let scale: u32 = js_uint(&scale, "scale")?;
        self.inner
            .rounding
            .output_scale
            .set_scale(currency, scale)
            .map_err(to_js_err)
    }

    /// Override the ingest decimal places of one currency.
    ///
    /// # Arguments
    ///
    /// * `currency` - ISO-4217 alphabetic code, such as `"JPY"`.
    /// * `scale` - Number of decimal places, an integer in `0..=28`.
    ///
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if `currency` is not a supported currency code or `scale`
    /// exceeds 28.
    /// @param currency - ISO-4217 alphabetic code, such as `"JPY"`.
    /// @param scale - Number of decimal places, an integer in `0..=28`.
    #[wasm_bindgen(js_name = setIngestScale)]
    pub fn set_ingest_scale(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] scale: JsValue,
    ) -> Result<(), JsValue> {
        let currency = js_currency(&currency, "currency")?;
        let scale: u32 = js_uint(&scale, "scale")?;
        self.inner
            .rounding
            .ingest_scale
            .set_scale(currency, scale)
            .map_err(to_js_err)
    }

    /// Per-currency output-scale overrides.
    ///
    /// @returns A plain object mapping ISO-4217 code to decimal places; empty
    /// when no override is set.
    /// @throws If the map cannot be converted (not expected).
    #[wasm_bindgen(js_name = outputScaleOverrides, unchecked_return_type = "Record<string, number>")]
    pub fn output_scale_overrides(&self) -> Result<JsValue, JsValue> {
        scale_overrides(self.inner.rounding.output_scale.get_overrides())
    }

    /// Per-currency ingest-scale overrides.
    ///
    /// @returns A plain object mapping ISO-4217 code to decimal places; empty
    /// when no override is set.
    /// @throws If the map cannot be converted (not expected).
    #[wasm_bindgen(js_name = ingestScaleOverrides, unchecked_return_type = "Record<string, number>")]
    pub fn ingest_scale_overrides(&self) -> Result<JsValue, JsValue> {
        scale_overrides(self.inner.rounding.ingest_scale.get_overrides())
    }

    /// Store a versioned extension section (Rust `ConfigExtensions::insert`).
    ///
    /// # Arguments
    ///
    /// * `key` - Extension key of the form `{crate}.{domain}.v{N}`, such as
    ///   `"core.rating_scales.v1"`: lowercase identifiers and a version suffix.
    /// * `value` - The section itself: any JSON-representable value (plain
    ///   object, array, string, finite number, boolean or `null`). A string is
    ///   stored as a string, not parsed as JSON text.
    ///
    /// @throws `TypeError` (kind `invalid_type`) if `key` is not a string or
    /// `value` is not JSON-representable; `FinstackError` (kind `validation`)
    /// if `key` does not match the required pattern.
    /// @param key - Extension key of the form `{crate}.{domain}.v{N}`, such as `"core.rating_scales.v1"`: lowercase identifiers and a version suffix.
    /// @param value - The section itself: any JSON-representable value (plain object, array, string, finite number, boolean or `null`). A string is stored as a string, not parsed as JSON text.
    #[wasm_bindgen(js_name = setExtension)]
    pub fn set_extension(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] key: JsValue,
        #[wasm_bindgen(unchecked_param_type = "unknown")] value: JsValue,
    ) -> Result<(), JsValue> {
        let key = js_string(&key, "key")?;
        let value = js_json_value(&value, "value")?;
        self.inner
            .extensions
            .insert(key, value)
            .map_err(to_js_err)?;
        Ok(())
    }

    /// Remove an extension section.
    ///
    /// # Arguments
    ///
    /// * `key` - Extension key to remove.
    ///
    /// @returns `true` if a section was stored under `key`.
    /// @throws `TypeError` if `key` is not a string.
    /// @param key - Extension key to remove.
    #[wasm_bindgen(js_name = removeExtension)]
    pub fn remove_extension(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] key: JsValue,
    ) -> Result<bool, JsValue> {
        Ok(self
            .inner
            .extensions
            .remove(&js_string(&key, "key")?)
            .is_some())
    }

    /// Keys of the stored extension sections.
    ///
    /// @returns Extension keys in sorted order.
    #[wasm_bindgen(js_name = extensionKeys)]
    pub fn extension_keys(&self) -> Vec<String> {
        self.inner.extensions.keys().map(str::to_string).collect()
    }

    /// One extension section as a plain value.
    ///
    /// # Arguments
    ///
    /// * `key` - Extension key to read.
    ///
    /// @returns The stored section (plain object, array or primitive), or
    /// `undefined` if none is stored under `key`.
    /// @throws `TypeError` if `key` is not a string.
    /// @param key - Extension key to read.
    #[wasm_bindgen(js_name = getExtension, unchecked_return_type = "unknown")]
    pub fn get_extension(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] key: JsValue,
    ) -> Result<JsValue, JsValue> {
        match self.inner.extensions.get(&js_string(&key, "key")?) {
            Some(value) => to_js_value(value),
            None => Ok(JsValue::UNDEFINED),
        }
    }

    /// Serialize to the canonical JSON wire form shared with Python `FinstackConfig.to_json`.
    ///
    /// @returns Compact JSON text with `rounding`, `tolerances` and any `extensions`.
    /// @throws If serialization fails (not expected for a valid configuration).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form produced by `toJson`.
    ///
    /// # Arguments
    ///
    /// * `json` - FinstackConfig JSON text or plain object; unknown fields
    ///   and malformed extension keys are rejected.
    ///
    /// @returns The parsed `FinstackConfig`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema.
    /// @param json - FinstackConfig JSON text or plain object; unknown fields and malformed extension keys are rejected.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
    ) -> Result<JsFinstackConfig, JsValue> {
        from_js_json::<FinstackConfig>(&json, "json").map(|inner| Self { inner })
    }
}
