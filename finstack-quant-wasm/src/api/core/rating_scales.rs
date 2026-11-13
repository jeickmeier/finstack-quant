//! WASM bindings for [`finstack_quant_core::rating_scales`]: the
//! caller-supplied scorecard rating-scale registry. No scale is embedded and
//! nothing falls back to a default.
//!
//! `RatingLevel` and `ScorecardScale` are plain data: they cross the boundary
//! as plain objects typed by the schema-generated types of the same names;
//! registry entries cross as `{ id, source, scale }` objects.

use crate::api::core::config::JsFinstackConfig;
use crate::utils::input::{from_js_json, js_f64, js_opt_string, js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::rating_scales::{
    RATING_SCALES_EXTENSION_KEY, RatingLevel, RatingScaleEntry, RatingScaleRegistry,
    ScorecardScale, registry_from_config as rust_registry_from_config,
};
use wasm_bindgen::prelude::*;

/// Construct a validated scorecard band on the 0–100 scale.
///
/// # Arguments
///
/// * `name` - Band label (for example `"AAA"`); outer whitespace is trimmed and
///   the label must be nonblank and not an unrated status (`NR`, `NOTRATED`,
///   `NOT RATED`, `UNRATED`).
/// * `score` - Finite representative score in `[0, 100]`, not below `min_score`.
/// * `min_score` - Finite inclusive lower bound of the band in `[0, 100]`.
///
/// @returns A `RatingLevel` plain object with `name`, `score` and `min_score`.
/// @throws `TypeError` (kind `invalid_type`) for a non-string name or non-number
/// score; `FinstackError` (kind `validation`) for a blank or reserved name, an
/// invalid score, or a score below the minimum.
/// @param name - Band label (for example `"AAA"`); outer whitespace is trimmed and the label must be nonblank and not an unrated status (`NR`, `NOTRATED`, `NOT RATED`, `UNRATED`).
/// @param score - Finite representative score in `[0, 100]`, not below `minScore`.
/// @param minScore - Finite inclusive lower bound of the band in `[0, 100]`.
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const level = core.ratingLevelTryNew("AAA", 95, 90);
/// console.log(level.min_score); // 90
/// ```
#[wasm_bindgen(js_name = ratingLevelTryNew, unchecked_return_type = "RatingLevel")]
pub fn rating_level_try_new(
    #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] score: JsValue,
    #[wasm_bindgen(js_name = "minScore", unchecked_param_type = "number")] min_score: JsValue,
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

/// Construct a validated best-to-worst scorecard scale covering `[0, 100]`.
///
/// # Arguments
///
/// * `scale_name` - Scale name, outer whitespace trimmed; must be nonblank.
/// * `description` - Optional description; pass `null` or `undefined` for none.
/// * `ratings` - Nonempty array (or its JSON text) of `RatingLevel` objects
///   ordered best to worst: labels unique after trim and ASCII case folding,
///   score and minimum strictly descending, the worst minimum `0`, and each
///   representative score below the preceding band's minimum.
///
/// @returns A `ScorecardScale` plain object preserving the supplied band order.
/// @throws `TypeError` (kind `invalid_type`) for mistyped string arguments;
/// `FinstackError` (kind `validation`) for a blank name or bands that are
/// empty, colliding, out of order, leave a gap below the worst band, or hold
/// a representative outside its band. Description precedes ratings as in Rust.
/// @param scaleName - Scale name, outer whitespace trimmed; must be nonblank.
/// @param description - Optional description; pass `null` or `undefined` for none.
/// @param ratings - Nonempty array (or its JSON text) of `RatingLevel` objects ordered best to worst: labels unique after trim and ASCII case folding, score and minimum strictly descending, the worst minimum `0`, and each representative score below the preceding band's minimum.
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const scale = core.scorecardScaleTryNew("custom", undefined, [
///   core.ratingLevelTryNew("A", 90, 80),
///   core.ratingLevelTryNew("B", 70, 0),
/// ]);
/// console.log(scale.ratings.length); // 2
/// ```
#[wasm_bindgen(js_name = scorecardScaleTryNew, unchecked_return_type = "ScorecardScale")]
pub fn scorecard_scale_try_new(
    #[wasm_bindgen(js_name = "scaleName", unchecked_param_type = "string")] scale_name: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string | null | undefined")] description: JsValue,
    #[wasm_bindgen(unchecked_param_type = "ReadonlyArray<RatingLevel> | string")] ratings: JsValue,
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

/// Classify a score into its band on a scorecard scale (Rust `ScorecardScale::classify`).
///
/// # Arguments
///
/// * `scale` - `ScorecardScale` plain object (or JSON text); validated again.
/// * `score` - Score on the inclusive 0–100 scale; each boundary belongs to
///   the better band.
///
/// @returns The `RatingLevel` the score falls in.
/// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
/// `FinstackError` (kind `validation`) for an invalid scale or a score that
/// is non-finite or outside `[0, 100]`.
/// @param scale - `ScorecardScale` plain object (or JSON text); validated again.
/// @param score - Score on the inclusive 0–100 scale; each boundary belongs to the better band.
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const scale = core.scorecardScaleTryNew("custom", undefined, [
///   core.ratingLevelTryNew("A", 90, 80),
///   core.ratingLevelTryNew("B", 70, 0),
/// ]);
/// console.log(core.scorecardScaleClassify(scale, 80).name); // "A"
/// ```
#[wasm_bindgen(js_name = scorecardScaleClassify, unchecked_return_type = "RatingLevel")]
pub fn scorecard_scale_classify(
    #[wasm_bindgen(unchecked_param_type = "ScorecardScale | string")] scale: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] score: JsValue,
) -> Result<JsValue, JsValue> {
    let scale = from_js_json::<ScorecardScale>(&scale, "scale")?;
    to_js_value(
        scale
            .classify(js_f64(&score, "score")?)
            .map_err(to_js_err)?,
    )
}

/// Best-to-worst position (`0` is best) of a band label (Rust `ScorecardScale::level_position`).
///
/// # Arguments
///
/// * `scale` - `ScorecardScale` plain object (or JSON text); validated again.
/// * `label` - Band label, matched after outer trim and ASCII case folding.
///
/// @returns The zero-based position of the band.
/// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
/// `FinstackError` (kind `validation`) for an invalid scale or an unknown label.
/// @param scale - `ScorecardScale` plain object (or JSON text); validated again.
/// @param label - Band label, matched after outer trim and ASCII case folding.
#[wasm_bindgen(js_name = scorecardScaleLevelPosition)]
pub fn scorecard_scale_level_position(
    #[wasm_bindgen(unchecked_param_type = "ScorecardScale | string")] scale: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] label: JsValue,
) -> Result<usize, JsValue> {
    from_js_json::<ScorecardScale>(&scale, "scale")?
        .level_position(&js_string(&label, "label")?)
        .map_err(to_js_err)
}

/// Construct a validated registry entry (Rust `RatingScaleEntry::try_new`).
///
/// # Arguments
///
/// * `id` - Scale id; normalized by outer trim and ASCII lowercasing, nonblank.
/// * `source` - Provenance of the scale's numbers (policy, owner, vintage);
///   trimmed and nonblank.
/// * `scale` - `ScorecardScale` plain object (or JSON text).
///
/// @returns A `{ id, source, scale }` plain object with the normalized id.
/// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
/// `FinstackError` (kind `validation`) for a blank id or source or an invalid scale.
/// @param id - Scale id; normalized by outer trim and ASCII lowercasing, nonblank.
/// @param source - Provenance of the scale's numbers (policy, owner, vintage); trimmed and nonblank.
/// @param scale - `ScorecardScale` plain object (or JSON text).
#[wasm_bindgen(js_name = ratingScaleEntryTryNew, unchecked_return_type = "RatingScaleEntry")]
pub fn rating_scale_entry_try_new(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] source: JsValue,
    #[wasm_bindgen(unchecked_param_type = "ScorecardScale | string")] scale: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(
        &RatingScaleEntry::try_new(
            js_string(&id, "id")?,
            js_string(&source, "source")?,
            from_js_json::<ScorecardScale>(&scale, "scale")?,
        )
        .map_err(to_js_err)?,
    )
}

/// Caller-supplied registry of scorecard rating scales.
///
/// Build one with `new RatingScaleRegistry(entries)`,
/// `RatingScaleRegistry.fromJson(json)` or `registryFromConfig(config)`;
/// there is no embedded or default registry.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const scale = core.scorecardScaleTryNew("Internal", undefined, [
///   core.ratingLevelTryNew("A", 90, 80),
///   core.ratingLevelTryNew("B", 70, 0),
/// ]);
/// const registry = new core.RatingScaleRegistry([
///   core.ratingScaleEntryTryNew("internal", "credit policy 2026", scale),
/// ]);
/// registry.ratingScale(" INTERNAL ").scale_name; // "Internal"
/// ```
#[wasm_bindgen(js_name = RatingScaleRegistry)]
#[derive(Clone, Debug)]
pub struct JsRatingScaleRegistry {
    pub(crate) inner: RatingScaleRegistry,
}

#[wasm_bindgen(js_class = RatingScaleRegistry)]
impl JsRatingScaleRegistry {
    /// Build and validate a registry (Rust `RatingScaleRegistry::try_new`).
    ///
    /// # Arguments
    ///
    /// * `rating_scales` - Nonempty array (or its JSON text) of
    ///   `{ id, source, scale }` entries with unique ids, in registry order.
    ///
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
    /// `FinstackError` (kind `validation`) for an empty list, a repeated id or
    /// an invalid entry.
    /// @param ratingScales - Nonempty array (or its JSON text) of `{ id, source, scale }` entries with unique ids, in registry order.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(
            js_name = "ratingScales",
            unchecked_param_type = "ReadonlyArray<RatingScaleEntry> | string"
        )]
        rating_scales: JsValue,
    ) -> Result<JsRatingScaleRegistry, JsValue> {
        RatingScaleRegistry::try_new(from_js_json::<Vec<RatingScaleEntry>>(
            &rating_scales,
            "ratingScales",
        )?)
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Registered entries in registry order.
    ///
    /// @returns `{ id, source, scale }` plain objects.
    /// @throws If serialization fails (not expected for a valid registry).
    #[wasm_bindgen(js_name = ratingScales, unchecked_return_type = "Array<RatingScaleEntry>")]
    pub fn rating_scales(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.get_rating_scales())
    }

    /// Normalized id of every registered scale, in registry order.
    ///
    /// @returns Trimmed, ASCII-lowercase scale ids.
    #[wasm_bindgen(js_name = scaleIds)]
    pub fn scale_ids(&self) -> Vec<String> {
        self.inner
            .scale_ids()
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    /// Whether a scale id is registered after outer trim and ASCII lowercasing.
    ///
    /// # Arguments
    ///
    /// * `id` - Scale id to look up.
    ///
    /// @returns `true` when the registry holds the id.
    /// @throws `TypeError` if `id` is not a string.
    /// @param id - Scale id to look up.
    #[wasm_bindgen(js_name = isKnownRatingScale)]
    pub fn is_known_rating_scale(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    ) -> Result<bool, JsValue> {
        Ok(self.inner.is_known_rating_scale(&js_string(&id, "id")?))
    }

    /// Resolve a registered entry by id (Rust `RatingScaleRegistry::entry`).
    ///
    /// # Arguments
    ///
    /// * `id` - Scale id; outer whitespace and ASCII case are ignored. No
    ///   alias or fallback is applied.
    ///
    /// @returns The `{ id, source, scale }` entry.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `validation`) if the registry has no scale with that id.
    /// @param id - Scale id; outer whitespace and ASCII case are ignored. No alias or fallback is applied.
    #[wasm_bindgen(js_name = entry, unchecked_return_type = "RatingScaleEntry")]
    pub fn entry(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    ) -> Result<JsValue, JsValue> {
        to_js_value(
            self.inner
                .entry(&js_string(&id, "id")?)
                .map_err(to_js_err)?,
        )
    }

    /// Resolve a registered scale by id (Rust `RatingScaleRegistry::rating_scale`).
    ///
    /// # Arguments
    ///
    /// * `id` - Scale id; outer whitespace and ASCII case are ignored. No
    ///   alias or fallback is applied.
    ///
    /// @returns A plain `ScorecardScale` object: `scale_name`, optional
    /// `description` and `ratings` ordered best to worst, each with `name`,
    /// `score` and `min_score` on the 0–100 scale.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `validation`) if the registry has no scale with that id.
    /// @param id - Scale id; outer whitespace and ASCII case are ignored. No alias or fallback is applied.
    #[wasm_bindgen(js_name = ratingScale, unchecked_return_type = "ScorecardScale")]
    pub fn rating_scale(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    ) -> Result<JsValue, JsValue> {
        let scale = self
            .inner
            .rating_scale(&js_string(&id, "id")?)
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
    ///   output. Validation enforces the schema version, a nonempty list,
    ///   unique normalized scale ids, nonblank sources and valid scales.
    ///
    /// @returns The validated `RatingScaleRegistry`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it is malformed or fails
    /// validation.
    /// @param json - Registry JSON text or plain object, such as `toJson()` output. Validation enforces the schema version, a nonempty list, unique normalized scale ids, nonblank sources and valid scales.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
    ) -> Result<JsRatingScaleRegistry, JsValue> {
        RatingScaleRegistry::from_json(&json_text(&json, "json")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }
}

/// Rating-scale registry held in a configuration (Rust `registry_from_config`).
///
/// # Arguments
///
/// * `config` - Configuration whose extensions hold the
///   `ratingScalesExtensionKey()` section; there is no embedded fallback.
///
/// @returns The validated `RatingScaleRegistry`.
/// @throws `FinstackError` (kind `validation`) if the extension section is
/// absent, malformed or fails registry validation.
/// @param config - Configuration whose extensions hold the `ratingScalesExtensionKey()` section; there is no embedded fallback.
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
