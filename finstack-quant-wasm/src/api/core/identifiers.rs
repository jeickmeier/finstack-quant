//! WASM bindings for the credit-rating scale and the identifier and
//! attribute types of `finstack_quant_core::types`.

use crate::utils::input::{from_js_json, js_string};
use crate::utils::to_js_err;
use finstack_quant_core::types::{Attributes, CreditRating};
use wasm_bindgen::prelude::*;

/// Agency credit rating on the 23-step S&P/Fitch scale (`AAA` … `D`, plus `NR`).
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const rating = new core.CreditRating("Baa3"); // Moody's spelling
/// rating.toString(); // "BBB-"
/// rating.isInvestmentGrade(); // true
/// core.CreditRating.bbb().notchesTo("BB"); // 3
/// ```
#[wasm_bindgen(js_name = CreditRating)]
#[derive(Clone, Copy, Debug)]
pub struct JsCreditRating {
    pub(crate) inner: CreditRating,
}

impl JsCreditRating {
    const fn from_inner(inner: CreditRating) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = CreditRating)]
impl JsCreditRating {
    /// Parse a rating (Rust `CreditRating::from_str`).
    ///
    /// # Arguments
    ///
    /// * `name` - Rating text in S&P/Fitch (`"BBB-"`) or Moody's (`"Baa3"`)
    ///   spelling; case and spaces are ignored, and `"NR"`, `"Not Rated"` and
    ///   `"Unrated"` all mean not rated.
    ///
    /// @returns The parsed `CreditRating`.
    /// @throws `TypeError` (kind `invalid_type`) if `name` is not a string;
    /// `FinstackError` (kind `validation`) if it is not a rating.
    #[wasm_bindgen(constructor)]
    pub fn new(name: JsValue) -> Result<JsCreditRating, JsValue> {
        js_string(&name, "name")?
            .parse()
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// S&P/Fitch `AAA`, Moody's `Aaa`.
    ///
    /// @returns The `AAA` rating.
    #[wasm_bindgen(js_name = aaa)]
    pub fn aaa() -> Self {
        Self::from_inner(CreditRating::AAA)
    }

    /// S&P/Fitch `AA+`, Moody's `Aa1`.
    ///
    /// @returns The `AA+` rating.
    #[wasm_bindgen(js_name = aaPlus)]
    pub fn aa_plus() -> Self {
        Self::from_inner(CreditRating::AAPlus)
    }

    /// S&P/Fitch `AA`, Moody's `Aa2`.
    ///
    /// @returns The `AA` rating.
    #[wasm_bindgen(js_name = aa)]
    pub fn aa() -> Self {
        Self::from_inner(CreditRating::AA)
    }

    /// S&P/Fitch `AA-`, Moody's `Aa3`.
    ///
    /// @returns The `AA-` rating.
    #[wasm_bindgen(js_name = aaMinus)]
    pub fn aa_minus() -> Self {
        Self::from_inner(CreditRating::AAMinus)
    }

    /// S&P/Fitch `A+`, Moody's `A1`.
    ///
    /// @returns The `A+` rating.
    #[wasm_bindgen(js_name = aPlus)]
    pub fn a_plus() -> Self {
        Self::from_inner(CreditRating::APlus)
    }

    /// S&P/Fitch `A`, Moody's `A2`.
    ///
    /// @returns The `A` rating.
    #[wasm_bindgen(js_name = a)]
    pub fn a() -> Self {
        Self::from_inner(CreditRating::A)
    }

    /// S&P/Fitch `A-`, Moody's `A3`.
    ///
    /// @returns The `A-` rating.
    #[wasm_bindgen(js_name = aMinus)]
    pub fn a_minus() -> Self {
        Self::from_inner(CreditRating::AMinus)
    }

    /// S&P/Fitch `BBB+`, Moody's `Baa1`.
    ///
    /// @returns The `BBB+` rating.
    #[wasm_bindgen(js_name = bbbPlus)]
    pub fn bbb_plus() -> Self {
        Self::from_inner(CreditRating::BBBPlus)
    }

    /// S&P/Fitch `BBB`, Moody's `Baa2`.
    ///
    /// @returns The `BBB` rating.
    #[wasm_bindgen(js_name = bbb)]
    pub fn bbb() -> Self {
        Self::from_inner(CreditRating::BBB)
    }

    /// S&P/Fitch `BBB-`, Moody's `Baa3`.
    ///
    /// @returns The `BBB-` rating.
    #[wasm_bindgen(js_name = bbbMinus)]
    pub fn bbb_minus() -> Self {
        Self::from_inner(CreditRating::BBBMinus)
    }

    /// S&P/Fitch `BB+`, Moody's `Ba1`.
    ///
    /// @returns The `BB+` rating.
    #[wasm_bindgen(js_name = bbPlus)]
    pub fn bb_plus() -> Self {
        Self::from_inner(CreditRating::BBPlus)
    }

    /// S&P/Fitch `BB`, Moody's `Ba2`.
    ///
    /// @returns The `BB` rating.
    #[wasm_bindgen(js_name = bb)]
    pub fn bb() -> Self {
        Self::from_inner(CreditRating::BB)
    }

    /// S&P/Fitch `BB-`, Moody's `Ba3`.
    ///
    /// @returns The `BB-` rating.
    #[wasm_bindgen(js_name = bbMinus)]
    pub fn bb_minus() -> Self {
        Self::from_inner(CreditRating::BBMinus)
    }

    /// S&P/Fitch `B+`, Moody's `B1`.
    ///
    /// @returns The `B+` rating.
    #[wasm_bindgen(js_name = bPlus)]
    pub fn b_plus() -> Self {
        Self::from_inner(CreditRating::BPlus)
    }

    /// S&P/Fitch `B`, Moody's `B2`.
    ///
    /// @returns The `B` rating.
    #[wasm_bindgen(js_name = b)]
    pub fn b() -> Self {
        Self::from_inner(CreditRating::B)
    }

    /// S&P/Fitch `B-`, Moody's `B3`.
    ///
    /// @returns The `B-` rating.
    #[wasm_bindgen(js_name = bMinus)]
    pub fn b_minus() -> Self {
        Self::from_inner(CreditRating::BMinus)
    }

    /// S&P/Fitch `CCC+`, Moody's `Caa1`.
    ///
    /// @returns The `CCC+` rating.
    #[wasm_bindgen(js_name = cccPlus)]
    pub fn ccc_plus() -> Self {
        Self::from_inner(CreditRating::CCCPlus)
    }

    /// S&P/Fitch `CCC`, Moody's `Caa2`.
    ///
    /// @returns The `CCC` rating.
    #[wasm_bindgen(js_name = ccc)]
    pub fn ccc() -> Self {
        Self::from_inner(CreditRating::CCC)
    }

    /// S&P/Fitch `CCC-`, Moody's `Caa3`.
    ///
    /// @returns The `CCC-` rating.
    #[wasm_bindgen(js_name = cccMinus)]
    pub fn ccc_minus() -> Self {
        Self::from_inner(CreditRating::CCCMinus)
    }

    /// S&P/Fitch `CC`, Moody's `Ca`.
    ///
    /// @returns The `CC` rating.
    #[wasm_bindgen(js_name = cc)]
    pub fn cc() -> Self {
        Self::from_inner(CreditRating::CC)
    }

    /// S&P/Fitch `C`, Moody's `C`.
    ///
    /// @returns The `C` rating.
    #[wasm_bindgen(js_name = c)]
    pub fn c() -> Self {
        Self::from_inner(CreditRating::C)
    }

    /// Default (`D`): the obligor has failed to pay.
    ///
    /// @returns The `D` rating.
    #[wasm_bindgen(js_name = d)]
    pub fn d() -> Self {
        Self::from_inner(CreditRating::D)
    }

    /// Not rated (`NR`): no agency rating is assigned.
    ///
    /// @returns The `NR` rating.
    #[wasm_bindgen(js_name = nr)]
    pub fn nr() -> Self {
        Self::from_inner(CreditRating::NR)
    }

    /// Whether the rating is investment grade (`BBB-` or better).
    ///
    /// @returns `true` for `AAA` through `BBB-`.
    #[wasm_bindgen(js_name = isInvestmentGrade)]
    pub fn is_investment_grade(&self) -> bool {
        self.inner.is_investment_grade()
    }

    /// Whether the rating is speculative grade (below `BBB-`).
    ///
    /// @returns `true` for `BB+` through `D`; `false` for investment grade and `NR`.
    #[wasm_bindgen(js_name = isSpeculativeGrade)]
    pub fn is_speculative_grade(&self) -> bool {
        self.inner.is_speculative_grade()
    }

    /// Whether the rating is the default state `D`.
    ///
    /// @returns `true` only for `D`.
    #[wasm_bindgen(js_name = isDefault)]
    pub fn is_default(&self) -> bool {
        self.inner.is_default()
    }

    /// Moody's spelling of the rating, such as `"Baa3"` for `BBB-`.
    ///
    /// @returns The Moody's rating text.
    #[wasm_bindgen(js_name = toMoodysString)]
    pub fn to_moodys_string(&self) -> String {
        self.inner.to_moodys_string().to_string()
    }

    /// Signed notch distance to another rating (Rust `CreditRating::notches_to`).
    ///
    /// # Arguments
    ///
    /// * `other` - Rating text to measure against (S&P/Fitch or Moody's
    ///   spelling; use `rating.toString()` for a `CreditRating`).
    ///
    /// @returns Notches from this rating to `other`: positive when `other` is
    /// weaker, negative when stronger. `NR` sits between `C` and `D`.
    /// @throws `TypeError` if `other` is not a string; `FinstackError` (kind
    /// `validation`) if it is not a rating.
    #[wasm_bindgen(js_name = notchesTo)]
    pub fn notches_to(&self, other: JsValue) -> Result<i32, JsValue> {
        let other = js_string(&other, "other")?.parse().map_err(to_js_err)?;
        Ok(self.inner.notches_to(other))
    }

    /// Serialize to the canonical JSON wire form (the quoted S&P/Fitch spelling).
    ///
    /// @returns JSON text such as `"\"BBB-\""`.
    /// @throws If serialization fails (not expected).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form produced by `toJson`.
    ///
    /// # Arguments
    ///
    /// * `json` - JSON text holding the quoted S&P/Fitch spelling, such as
    ///   `"\"BBB-\""`.
    ///
    /// @returns The parsed `CreditRating`.
    /// @throws `TypeError` if `json` is not a string; `FinstackError` (kind
    /// `validation`) if it is not a quoted rating.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsCreditRating, JsValue> {
        from_js_json(&json, "json").map(Self::from_inner)
    }

    /// S&P/Fitch spelling of the rating, such as `"BBB-"`.
    ///
    /// @returns The rating text accepted by `new CreditRating(name)`.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Tags and key/value metadata attached to an instrument or position, used
/// by scenario and reporting selectors.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const attributes = new core.Attributes();
/// attributes.addTag("energy");
/// attributes.setMeta("sector", "utilities");
/// attributes.hasTag("energy"); // true
/// attributes.getMeta("sector"); // "utilities"
/// ```
#[wasm_bindgen(js_name = Attributes)]
#[derive(Clone, Debug, Default)]
pub struct JsAttributes {
    pub(crate) inner: Attributes,
}

#[wasm_bindgen(js_class = Attributes)]
impl JsAttributes {
    /// Create an empty attribute set (no tags, no metadata).
    ///
    /// @returns A new `Attributes`.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsAttributes {
        Self::default()
    }

    /// Tags in sorted order.
    #[wasm_bindgen(getter, js_name = tags)]
    pub fn tags(&self) -> Vec<String> {
        self.inner.tags.iter().cloned().collect()
    }

    /// Add a tag; adding an existing tag has no effect.
    ///
    /// # Arguments
    ///
    /// * `tag` - Tag text, stored exactly as given.
    ///
    /// @throws `TypeError` if `tag` is not a string.
    #[wasm_bindgen(js_name = addTag)]
    pub fn add_tag(&mut self, tag: JsValue) -> Result<(), JsValue> {
        self.inner.add_tag(js_string(&tag, "tag")?);
        Ok(())
    }

    /// Whether a tag is present (Rust `Attributes::has_tag`).
    ///
    /// # Arguments
    ///
    /// * `tag` - Tag text to look for (exact match).
    ///
    /// @returns `true` when the tag is present.
    /// @throws `TypeError` if `tag` is not a string.
    #[wasm_bindgen(js_name = hasTag)]
    pub fn has_tag(&self, tag: JsValue) -> Result<bool, JsValue> {
        Ok(self.inner.has_tag(&js_string(&tag, "tag")?))
    }

    /// Whether the attributes match a selector (Rust `Attributes::matches_selector`).
    ///
    /// # Arguments
    ///
    /// * `selector` - `"tag:<name>"` matches a tag, `"meta:<key>=<value>"`
    ///   matches a metadata entry, and `"*"` matches everything.
    ///
    /// @returns `true` when the selector matches; `false` for an unrecognised
    /// selector.
    /// @throws `TypeError` if `selector` is not a string.
    #[wasm_bindgen(js_name = matchesSelector)]
    pub fn matches_selector(&self, selector: JsValue) -> Result<bool, JsValue> {
        Ok(self
            .inner
            .matches_selector(&js_string(&selector, "selector")?))
    }

    /// Read a metadata value (Rust `Attributes::get_meta`).
    ///
    /// # Arguments
    ///
    /// * `key` - Metadata key (exact match).
    ///
    /// @returns The stored text, or `undefined` when the key is absent.
    /// @throws `TypeError` if `key` is not a string.
    #[wasm_bindgen(js_name = getMeta)]
    pub fn get_meta(&self, key: JsValue) -> Result<Option<String>, JsValue> {
        Ok(self
            .inner
            .get_meta(&js_string(&key, "key")?)
            .map(str::to_string))
    }

    /// Store a metadata value (Rust `Attributes::set_meta`), replacing any
    /// existing value for the key.
    ///
    /// # Arguments
    ///
    /// * `key` - Metadata key to set; an existing entry under it is replaced.
    /// * `value` - Metadata text; convert numbers with `String(value)`.
    ///
    /// @throws `TypeError` if `key` or `value` is not a string.
    #[wasm_bindgen(js_name = setMeta)]
    pub fn set_meta(&mut self, key: JsValue, value: JsValue) -> Result<(), JsValue> {
        self.inner
            .set_meta(js_string(&key, "key")?, js_string(&value, "value")?);
        Ok(())
    }

    /// Whether a metadata key is present (Rust `Attributes::contains_meta_key`).
    ///
    /// # Arguments
    ///
    /// * `key` - Metadata key (exact match).
    ///
    /// @returns `true` when the key is present.
    /// @throws `TypeError` if `key` is not a string.
    #[wasm_bindgen(js_name = containsMetaKey)]
    pub fn contains_meta_key(&self, key: JsValue) -> Result<bool, JsValue> {
        Ok(self.inner.contains_meta_key(&js_string(&key, "key")?))
    }

    /// Metadata keys in sorted order.
    ///
    /// @returns The keys of every metadata entry.
    #[wasm_bindgen(js_name = keys)]
    pub fn keys(&self) -> Vec<String> {
        self.inner.meta.keys().cloned().collect()
    }

    /// Metadata entries in key order.
    ///
    /// @returns An array of `[key, value]` pairs.
    #[wasm_bindgen(js_name = items)]
    pub fn items(&self) -> js_sys::Array {
        self.inner
            .meta
            .iter()
            .map(|(key, value)| {
                js_sys::Array::of2(&JsValue::from(key.as_str()), &JsValue::from(value.as_str()))
            })
            .collect()
    }

    /// Serialize to the canonical JSON wire form shared with Python `Attributes.to_json`.
    ///
    /// @returns Compact JSON text with `tags` and `meta`.
    /// @throws If serialization fails (not expected).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form produced by `toJson`.
    ///
    /// # Arguments
    ///
    /// * `json` - Attributes JSON text or plain object with `tags` (array of
    ///   strings) and `meta` (string-to-string map); unknown fields are
    ///   rejected.
    ///
    /// @returns The parsed `Attributes`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsAttributes, JsValue> {
        from_js_json::<Attributes>(&json, "json").map(|inner| Self { inner })
    }
}
