//! WASM bindings for `finstack_quant_models::credit` liability management.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/liability_management.rs`.
//! Structure labels are the canonical snake_case wire values (`par_for_par`,
//! `discount`, `uptier`, `downtier`; `open_market_repurchase`, `tender_offer`,
//! `amend_and_extend`, `dropdown`), parsed with the Rust
//! [`FromStr`](core::str::FromStr) implementations; any other label throws.
//! Results are
//! returned as plain JS objects with snake_case keys matching the serde
//! representation of the Rust result types.

use crate::utils::input::{js_f64, js_opt_f64, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::credit::liability_management::{self as lm, ExchangeType, LmeType};
use wasm_bindgen::prelude::*;

/// Compare hold-versus-tender economics for a distressed exchange offer.
///
/// Returns an object with `exchange_type`, `old_npv`, `new_npv`,
/// `consent_fee`, `equity_sweetener_value`, `tender_total`, `delta_npv`,
/// `breakeven_recovery` and `tender_recommended`. Tendering is recommended
/// only when the total consideration exceeds the hold-out value by more than
/// 2%.
/// @param oldPv - Present value of the existing claim if it is not tendered, in the caller's monetary unit.
/// @param newPv - Present value of the new instrument received on tendering, in the same unit as oldPv.
/// @param consentFee - Cash consent or early-tender fee paid to participating holders, in the same unit as oldPv.
/// @param equitySweetenerValue - Estimated value of equity or warrants attached to the new instrument, in the same unit as oldPv.
/// @param exchangeType - Canonical offer structure: par_for_par, discount, uptier, or downtier.
///
/// # Errors
///
/// Throws a JavaScript exception if `exchangeType` is unrecognized, any monetary
/// input is negative or non-finite, or the result cannot be converted to a
/// JavaScript object.
#[wasm_bindgen(js_name = analyzeExchangeOffer)]
pub fn analyze_exchange_offer(
    old_pv: JsValue,
    new_pv: JsValue,
    consent_fee: JsValue,
    equity_sweetener_value: JsValue,
    exchange_type: JsValue,
) -> Result<JsValue, JsValue> {
    let old_pv = js_f64(&old_pv, "oldPv")?;
    let new_pv = js_f64(&new_pv, "newPv")?;
    let consent_fee = js_f64(&consent_fee, "consentFee")?;
    let equity_sweetener_value = js_f64(&equity_sweetener_value, "equitySweetenerValue")?;
    let exchange_type: &str = &js_string(&exchange_type, "exchangeType")?;
    let exchange_type: ExchangeType = exchange_type.parse().map_err(to_js_err)?;
    let analysis = lm::analyze_exchange_offer(
        old_pv,
        new_pv,
        consent_fee,
        equity_sweetener_value,
        exchange_type,
    )
    .map_err(to_js_err)?;
    to_js_value(&analysis)
}

/// Compute discount capture and leverage impact for an LME transaction.
///
/// Returns an object with `lme_type`, `cost`, `notional_reduction`,
/// `discount_capture`, `discount_capture_pct`, `remaining_holder_impact_pct`
/// and `leverage_impact` (null unless a positive EBITDA is supplied).
/// @param lmeType - Canonical structure: open_market_repurchase, tender_offer, amend_and_extend, or dropdown.
/// @param notional - Outstanding face amount of the target instrument, in the caller's monetary unit; must be positive.
/// @param repurchasePricePct - Price as a fraction of par for repurchases and tenders, the extension fee for amend-and-extend, or the transferred-asset fraction for a dropdown.
/// @param optAcceptancePct - Fraction of holders participating, in [0, 1].
/// @param ebitda - EBITDA in the same unit as notional; a positive value adds the leverage_impact block, null or non-positive omits it.
///
/// # Errors
///
/// Throws a JavaScript exception if `lmeType` is unrecognized, `notional` is
/// non-positive or non-finite, `optAcceptancePct` is outside `[0, 1]`, or
/// `repurchasePricePct` is outside the range accepted for the selected LME type:
/// `(0, 1.5]` for repurchases and tenders, `[0, 0.1]` for amend-and-extend, and
/// `[0, 1]` for dropdowns. It also throws if the result cannot be converted to a
/// JavaScript object.
#[wasm_bindgen(js_name = analyzeLme)]
pub fn analyze_lme(
    lme_type: JsValue,
    notional: JsValue,
    repurchase_price_pct: JsValue,
    opt_acceptance_pct: JsValue,
    ebitda: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let notional = js_f64(&notional, "notional")?;
    let repurchase_price_pct = js_f64(&repurchase_price_pct, "repurchasePricePct")?;
    let opt_acceptance_pct = js_f64(&opt_acceptance_pct, "optAcceptancePct")?;
    let ebitda = js_opt_f64(ebitda.as_ref(), "ebitda")?;
    let lme_type: &str = &js_string(&lme_type, "lmeType")?;
    let lme_type: LmeType = lme_type.parse().map_err(to_js_err)?;
    let analysis = lm::analyze_lme(
        lme_type,
        notional,
        repurchase_price_pct,
        opt_acceptance_pct,
        ebitda,
    )
    .map_err(to_js_err)?;
    to_js_value(&analysis)
}

/// Minimum NPV ratio at which a tender is recommended. Twin of the Rust and
/// Python constant `TENDER_RECOMMENDATION_HURDLE`: a holder is advised to
/// tender when the tender NPV is at least `old_npv` times this hurdle.
/// @returns The dimensionless hurdle multiple.
#[wasm_bindgen(js_name = tenderRecommendationHurdle)]
pub fn tender_recommendation_hurdle() -> f64 {
    finstack_quant_models::credit::liability_management::TENDER_RECOMMENDATION_HURDLE
}
