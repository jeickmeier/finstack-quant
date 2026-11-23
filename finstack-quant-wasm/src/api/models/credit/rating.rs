//! Moody's WARF factor and rating-factor table WASM bindings.

use crate::utils::input::js_string;
use crate::utils::to_js_err;
use crate::utils::wire::js_parse;
use finstack_quant_core::types::CreditRating;
use finstack_quant_models::credit::{self as credit, RatingFactorTable};
use wasm_bindgen::prelude::*;

/// Moody's WARF rating factor for one credit rating.
/// @param rating - Rating label such as `"Baa3"`, `"BBB-"` or `"B2"`; agency notches are normalized by the Rust parser.
/// @returns The Moody's weighted-average rating factor for that rating (for example 610 for Baa3).
///
/// # Errors
///
/// Throws a `TypeError` if `rating` is not a string, and a `validation` error
/// if it is not a recognized rating or has no factor in the Moody's table.
#[wasm_bindgen(js_name = moodysWarfFactor)]
pub fn moodys_warf_factor(
    #[wasm_bindgen(unchecked_param_type = "string")] rating: JsValue,
) -> Result<f64, JsValue> {
    credit::moodys_warf_factor(js_parse::<CreditRating>(&rating, "rating")?).map_err(to_js_err)
}

/// Rating-to-factor table used for WARF-style portfolio credit quality.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const table = models.credit.RatingFactorTable.defaultCalibration();
/// console.log(table.agency, table.getFactor("B2"));
/// table.free();
/// ```
#[wasm_bindgen(js_name = RatingFactorTable)]
pub struct JsRatingFactorTable {
    pub(crate) inner: RatingFactorTable,
}

json_round_trip!(JsRatingFactorTable, RatingFactorTable);

#[wasm_bindgen(js_class = RatingFactorTable)]
impl JsRatingFactorTable {
    /// Load the calibration selected by the embedded registry's default ID.
    /// @returns The registry-selected calibration handle.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the registry or its configured default is malformed.
    #[wasm_bindgen(js_name = defaultCalibration)]
    pub fn default_calibration() -> Result<JsRatingFactorTable, JsValue> {
        RatingFactorTable::default_calibration()
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Rating-factor table registered under an explicit registry identifier.
    /// @param id - Registry identifier of the table, such as `"moodys_standard"`.
    /// @returns The rating-factor table registered under `id`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `id` is not a string, and a `not_found` error
    /// if no table is registered under it.
    #[wasm_bindgen(js_name = fromRegistryId)]
    pub fn from_registry_id(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    ) -> Result<JsRatingFactorTable, JsValue> {
        let id = js_string(&id, "id")?;
        RatingFactorTable::from_registry_id(&id)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Factor assigned to one rating; unlisted ratings fall back to `defaultFactor`.
    /// @param rating - Rating label such as `"Baa3"`, `"BBB-"` or `"B2"`.
    /// @returns The dimensionless rating factor for that rating.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `rating` is not a string, and a `validation`
    /// error if it is not a recognized rating.
    #[wasm_bindgen(js_name = getFactor)]
    pub fn get_factor(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] rating: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .get_factor(js_parse::<CreditRating>(&rating, "rating")?)
            .map_err(to_js_err)
    }
}

wasm_getters!(JsRatingFactorTable, RatingFactorTable |s| {
    /// Rating agency the table belongs to, such as `"Moodys"`.
    agency -> String = s.inner.agency().to_string();
    /// Methodology label of the table, such as the CLO methodology it was taken from.
    methodology -> String = s.inner.methodology().to_string();
    /// Factor applied to ratings the table does not list.
    default_factor as defaultFactor -> f64 = s.inner.default_factor();
});
