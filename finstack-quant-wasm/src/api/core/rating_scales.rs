//! WASM bindings for [`finstack_quant_core::rating_scales`]: the shared
//! scorecard rating-scale registry.
//!
//! `RatingLevel` and `ScorecardScale` are plain data: they cross the boundary
//! as plain objects typed by the schema-generated types of the same names.

use crate::api::core::config::JsFinstackConfig;
use crate::utils::input::{from_js_json, js_f64, js_opt_string, js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::rating_scales::{
    embedded_registry as rust_embedded_registry, registry_from_config as rust_registry_from_config,
    RatingLevel, RatingScaleRegistry, ScorecardScale, UnknownScalePolicy,
    RATING_SCALES_EXTENSION_KEY,
};
use wasm_bindgen::prelude::*;

/// Construct a validated rating threshold row on the 0–100 scorecard scale.
///
/// # Arguments
///
/// * `name` - Nonblank rating label, retained verbatim (for example `"AAA"`).
/// * `score` - Finite representative score in the inclusive range `[0, 100]`.
/// * `min_score` - Finite minimum qualifying score in `[0, 100]`.
///
/// @returns A `RatingLevel` plain object with `name`, `score` and `min_score`.
/// @throws `TypeError` (kind `invalid_type`) for a non-string name or non-number
/// score; `FinstackError` (kind `validation`) for a blank name or invalid score.
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const level = core.ratingLevelTryNew("AAA", 95, 90);
/// console.log(level.min_score); // 90
/// ```
#[wasm_bindgen(js_name = ratingLevelTryNew)]
pub fn rating_level_try_new(
    name: JsValue,
    score: JsValue,
    min_score: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(
        &RatingLevel::try_new(
            js_string(&name, "name")?,
            js_f64(&score, "score")?,
            js_f64(&min_score, "minScore")?,
        )
        .map_err(to_js_err)?,
    )
}

/// Construct a validated best-to-worst scorecard scale.
///
/// # Arguments
///
/// * `scale_name` - Display name stored verbatim and used in validation messages.
/// * `description` - Optional description; pass `null` or `undefined` for none.
/// * `ratings` - Nonempty array (or its JSON text) of `RatingLevel` objects;
///   names must be unique and both score and minimum score must strictly descend.
///
/// @returns A `ScorecardScale` plain object preserving the supplied rating order.
/// @throws `TypeError` (kind `invalid_type`) for mistyped string arguments;
/// `FinstackError` (kind `validation`) for malformed, empty, duplicated, out-of-range
/// or incorrectly ordered rating data. Description precedes ratings as in Rust.
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const scale = core.scorecardScaleTryNew("custom", undefined, [
///   core.ratingLevelTryNew("A", 90, 85),
///   core.ratingLevelTryNew("B", 70, 65),
/// ]);
/// console.log(scale.ratings.length); // 2
/// ```
#[wasm_bindgen(js_name = scorecardScaleTryNew)]
pub fn scorecard_scale_try_new(
    scale_name: JsValue,
    description: JsValue,
    ratings: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(
        &ScorecardScale::try_new(
            js_string(&scale_name, "scaleName")?,
            js_opt_string(Some(&description), "description")?,
            from_js_json::<Vec<RatingLevel>>(&ratings, "ratings")?,
        )
        .map_err(to_js_err)?,
    )
}

/// What a registry does when asked for a rating scale it does not know.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// core.UnknownScalePolicy.fallbackToDefault().toString(); // "fallback_to_default"
/// core.embeddedRegistry().unknownScalePolicy().toString();
/// ```
#[wasm_bindgen(js_name = UnknownScalePolicy)]
#[derive(Clone, Copy, Debug)]
pub struct JsUnknownScalePolicy {
    pub(crate) inner: UnknownScalePolicy,
}

impl JsUnknownScalePolicy {
    const fn from_inner(inner: UnknownScalePolicy) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = UnknownScalePolicy)]
impl JsUnknownScalePolicy {
    /// Reject unknown scale names.
    ///
    /// @returns The `error` policy.
    #[wasm_bindgen(js_name = error)]
    pub fn error() -> Self {
        Self::from_inner(UnknownScalePolicy::Error)
    }

    /// Use the registry's default scale for unknown names.
    ///
    /// @returns The `fallback_to_default` policy.
    #[wasm_bindgen(js_name = fallbackToDefault)]
    pub fn fallback_to_default() -> Self {
        Self::from_inner(UnknownScalePolicy::FallbackToDefault)
    }

    /// Use the default scale for unknown names and let the caller warn.
    ///
    /// @returns The `warn_and_fallback` policy.
    #[wasm_bindgen(js_name = warnAndFallback)]
    pub fn warn_and_fallback() -> Self {
        Self::from_inner(UnknownScalePolicy::WarnAndFallback)
    }

    /// Parse a policy name (the Rust serde label).
    ///
    /// # Arguments
    ///
    /// * `name` - Policy name: `"error"`, `"fallback_to_default"` or
    ///   `"warn_and_fallback"`, in any ASCII case.
    ///
    /// @returns The matching `UnknownScalePolicy`.
    /// @throws `TypeError` (kind `invalid_type`) if `name` is not a string;
    /// `FinstackError` (kind `validation`) if no policy matches.
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(name: JsValue) -> Result<JsUnknownScalePolicy, JsValue> {
        js_string(&name, "name")?
            .parse()
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Serialize to the canonical JSON wire form (a quoted snake_case name).
    ///
    /// @returns JSON text such as `"\"error\""`.
    /// @throws If serialization fails (not expected).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form (the Rust serde representation).
    ///
    /// # Arguments
    ///
    /// * `json` - JSON text holding the quoted policy name, such as
    ///   `"\"warn_and_fallback\""`.
    ///
    /// @returns The parsed `UnknownScalePolicy`.
    /// @throws `TypeError` if `json` is not a string; `FinstackError` (kind
    /// `validation`) if it is not a quoted policy name.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsUnknownScalePolicy, JsValue> {
        from_js_json::<UnknownScalePolicy>(&json, "json").map(Self::from_inner)
    }

    /// Snake_case name of the policy, such as `"fallback_to_default"`.
    ///
    /// @returns The name accepted by `fromName`.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Versioned registry of scorecard rating scales (canonical ids `sp` and `moodys`).
///
/// Get one from `embeddedRegistry()`, `registryFromConfig(config)` or
/// `RatingScaleRegistry.fromJson(json)`.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const registry = core.embeddedRegistry();
/// const scale = registry.ratingScale(registry.defaultScaleId());
/// scale.ratings[0].name; // strongest grade of the default scale
/// ```
#[wasm_bindgen(js_name = RatingScaleRegistry)]
#[derive(Clone, Debug)]
pub struct JsRatingScaleRegistry {
    pub(crate) inner: RatingScaleRegistry,
}

#[wasm_bindgen(js_class = RatingScaleRegistry)]
impl JsRatingScaleRegistry {
    /// Score assigned when a scorecard value falls in a threshold gap.
    ///
    /// @returns The default scorecard score on the 0–100 scale.
    #[wasm_bindgen(js_name = defaultScorecardScore)]
    pub fn default_scorecard_score(&self) -> f64 {
        self.inner.default_scorecard_score()
    }

    /// Identifier of the default rating scale.
    ///
    /// @returns The id used when a caller names no scale or an unknown one
    /// under a fallback policy.
    #[wasm_bindgen(js_name = defaultScaleId)]
    pub fn default_scale_id(&self) -> String {
        self.inner.default_scale_id().to_string()
    }

    /// Canonical normalized id of every registered scale, in registry order.
    ///
    /// @returns Canonical trimmed, ASCII-lowercase scale ids.
    #[wasm_bindgen(js_name = scaleIds)]
    pub fn scale_ids(&self) -> Vec<String> {
        self.inner
            .scale_ids()
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    /// Policy applied when `ratingScale` is asked for an unknown name.
    ///
    /// @returns The registry's `UnknownScalePolicy`.
    #[wasm_bindgen(js_name = unknownScalePolicy)]
    pub fn unknown_scale_policy(&self) -> JsUnknownScalePolicy {
        JsUnknownScalePolicy::from_inner(self.inner.unknown_scale_policy())
    }

    /// Whether a name is a registered canonical id after trimming and ASCII case folding.
    ///
    /// # Arguments
    ///
    /// * `name` - Canonical id (`sp` or `moodys`); surrounding whitespace and ASCII case are ignored.
    ///
    /// @returns `true` when the registry knows the name.
    /// @throws `TypeError` if `name` is not a string.
    #[wasm_bindgen(js_name = isKnownRatingScale)]
    pub fn is_known_rating_scale(&self, name: JsValue) -> Result<bool, JsValue> {
        Ok(self.inner.is_known_rating_scale(&js_string(&name, "name")?))
    }

    /// Resolve a canonical scale id under the unknown-scale policy (Rust
    /// `RatingScaleRegistry::rating_scale`).
    ///
    /// # Arguments
    ///
    /// * `name` - Canonical id, ignoring surrounding whitespace and ASCII case; an unknown name returns the default
    ///   scale under a fallback policy.
    ///
    /// @returns A plain `ScorecardScale` object: `scale_name`, optional
    /// `description` and `ratings` ordered best to worst, each with `name`,
    /// `score` and `min_score` on the 0–100 scale.
    /// @throws `TypeError` if `name` is not a string; `FinstackError` if the
    /// name is unknown and the policy is `error`.
    #[wasm_bindgen(js_name = ratingScale)]
    pub fn rating_scale(&self, name: JsValue) -> Result<JsValue, JsValue> {
        let scale = self
            .inner
            .rating_scale(&js_string(&name, "name")?)
            .map_err(to_js_err)?;
        to_js_value(scale)
    }

    /// Serialize to the canonical JSON wire form shared with Python `RatingScaleRegistry.to_json`.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid registry).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize and validate a registry (Rust `RatingScaleRegistry::from_json`).
    ///
    /// # Arguments
    ///
    /// * `json` - Registry JSON text or plain object, such as `toJson()`
    ///   output. Validation enforces the schema version, unique normalized scale ids, an existing default scale and in-range scores.
    ///
    /// @returns The validated `RatingScaleRegistry`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it is malformed or fails
    /// validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsRatingScaleRegistry, JsValue> {
        RatingScaleRegistry::from_json(&json_text(&json, "json")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }
}

/// The rating-scale registry compiled into the library (Rust `embedded_registry`).
///
/// @returns A copy of the embedded `RatingScaleRegistry`.
/// @throws If the embedded registry fails validation (not expected).
#[wasm_bindgen(js_name = embeddedRegistry)]
pub fn embedded_registry() -> Result<JsRatingScaleRegistry, JsValue> {
    rust_embedded_registry()
        .map(|registry| JsRatingScaleRegistry {
            inner: registry.clone(),
        })
        .map_err(to_js_err)
}

/// Rating-scale registry selected by a configuration (Rust `registry_from_config`).
///
/// # Arguments
///
/// * `config` - Configuration to read; when its extensions hold the
///   `ratingScalesExtensionKey()` section, that section replaces the embedded
///   registry, otherwise the embedded registry is returned.
///
/// @returns The validated `RatingScaleRegistry`.
/// @throws `FinstackError` (kind `validation`) if the extension section is
/// malformed or fails registry validation.
#[wasm_bindgen(js_name = registryFromConfig)]
pub fn registry_from_config(config: &JsFinstackConfig) -> Result<JsRatingScaleRegistry, JsValue> {
    rust_registry_from_config(&config.inner)
        .map(|inner| JsRatingScaleRegistry { inner })
        .map_err(to_js_err)
}

/// Extension key under which a `FinstackConfig` carries a rating-scale
/// registry (Rust `RATING_SCALES_EXTENSION_KEY`).
///
/// @returns The key `"core.rating_scales.v1"`.
#[wasm_bindgen(js_name = ratingScalesExtensionKey)]
pub fn rating_scales_extension_key() -> String {
    RATING_SCALES_EXTENSION_KEY.to_string()
}
