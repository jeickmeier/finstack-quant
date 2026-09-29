//! WASM bindings for `finstack_quant_models::credit` liability management.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/liability_management.rs`.
//! Structure labels are passed as strings and parsed with the canonical Rust
//! [`FromStr`](core::str::FromStr) implementations, so JS callers may use the
//! same market shorthand (`"par"`, `"omr"`, `"A&E"`) as Python. Results are
//! returned as plain JS objects with snake_case keys matching the serde
//! representation of the Rust result types.

use crate::utils::input::js_string;
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
    old_pv: f64,
    new_pv: f64,
    consent_fee: f64,
    equity_sweetener_value: f64,
    exchange_type: JsValue,
) -> Result<JsValue, JsValue> {
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
    notional: f64,
    repurchase_price_pct: f64,
    opt_acceptance_pct: f64,
    ebitda: Option<f64>,
) -> Result<JsValue, JsValue> {
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
