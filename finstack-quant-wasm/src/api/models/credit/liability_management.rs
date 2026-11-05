//! WASM bindings for `finstack_quant_models::credit` liability management.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/liability_management.rs`.
//! Inputs are the canonical `ExchangeOffer` / `LmeTerms` objects, whose
//! structure labels are the snake_case wire values (`par_for_par`,
//! `discount`, `uptier`, `downtier`; `open_market_repurchase`, `tender_offer`,
//! `amend_and_extend`, `dropdown`); any other label throws. Results are
//! returned as plain JS objects with snake_case keys matching the serde
//! representation of the Rust result types.

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::credit::liability_management as lm;
use wasm_bindgen::prelude::*;

/// Compare hold-versus-tender economics for a distressed exchange offer.
///
/// Returns an object with `exchange_type`, `old_npv`, `new_npv`,
/// `consent_fee`, `equity_sweetener_value`, `tender_total`, `delta_npv`,
/// `breakeven_recovery` and `tender_recommended`. Tendering is recommended
/// only when the total consideration exceeds the hold-out value by more than
/// 2%.
/// @param offer - `ExchangeOffer` object or JSON text: `old_pv`, `new_pv`, `consent_fee` and `equity_sweetener_value` (non-negative, one monetary unit) and `exchange_type` (`par_for_par`, `discount`, `uptier` or `downtier`).
/// @returns Tender total, NPV pickup, breakeven recovery, and recommendation.
///
/// # Errors
///
/// Throws a JavaScript exception if the offer is malformed, has an unknown
/// field or structure, or any monetary input is negative or non-finite.
#[wasm_bindgen(js_name = analyzeExchangeOffer)]
pub fn analyze_exchange_offer(offer: JsValue) -> Result<JsValue, JsValue> {
    let offer: lm::ExchangeOffer = from_js_json(&offer, "offer")?;
    to_js_value(&lm::analyze_exchange_offer(&offer).map_err(to_js_err)?)
}

/// Compute discount capture and leverage impact for an LME transaction.
///
/// Returns an object with `lme_type`, `cost`, `notional_reduction`,
/// `discount_capture`, `discount_capture_pct`, `remaining_holder_impact_pct`
/// and `leverage_impact` (null unless a positive EBITDA is supplied).
/// @param terms - `LmeTerms` object or JSON text: `lme_type` (`open_market_repurchase`, `tender_offer`, `amend_and_extend` or `dropdown`), `notional` (positive), `repurchase_price_pct` (price as a fraction of par for repurchases and tenders, the extension fee for amend-and-extend, or the transferred-asset fraction for a dropdown), `opt_acceptance_pct` (in [0, 1]) and optional `ebitda` (a positive value adds the leverage_impact block).
/// @returns Cash cost, par retired, discount captured, holder impact, and optional leverage analysis.
///
/// # Errors
///
/// Throws a JavaScript exception if the terms are malformed or have an unknown
/// field or structure, `notional` is non-positive or non-finite,
/// `opt_acceptance_pct` is outside `[0, 1]`, or `repurchase_price_pct` is
/// outside the range accepted for the selected LME type: `(0, 1.5]` for
/// repurchases and tenders, `[0, 0.1]` for amend-and-extend, and `[0, 1]` for
/// dropdowns.
#[wasm_bindgen(js_name = analyzeLme)]
pub fn analyze_lme(terms: JsValue) -> Result<JsValue, JsValue> {
    let terms: lm::LmeTerms = from_js_json(&terms, "terms")?;
    to_js_value(&lm::analyze_lme(&terms).map_err(to_js_err)?)
}

/// Minimum NPV ratio at which a tender is recommended. Twin of the Rust and
/// Python constant `TENDER_RECOMMENDATION_HURDLE`: a holder is advised to
/// tender when the tender NPV exceeds `old_npv` times this hurdle.
/// @returns The dimensionless hurdle multiple.
#[wasm_bindgen(js_name = tenderRecommendationHurdle)]
pub fn tender_recommendation_hurdle() -> f64 {
    finstack_quant_models::credit::liability_management::TENDER_RECOMMENDATION_HURDLE
}
