//! WASM bindings for `finstack_quant_core::credit` liability management.
//!
//! Mirrors `finstack-quant-py/src/bindings/core/credit/liability_management.rs`.
//! Structure labels are passed as strings and parsed with the canonical Rust
//! [`FromStr`](core::str::FromStr) implementations, so JS callers may use the
//! same market shorthand (`"par"`, `"omr"`, `"A&E"`) as Python. Results are
//! returned as plain JS objects with snake_case keys matching the serde
//! representation of the Rust result types.

use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::credit::liability_management::{self as lm, ExchangeType, LmeType};
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
/// @param exchangeType - Offer structure: par_for_par (alias par), discount, uptier, or downtier.
#[wasm_bindgen(js_name = analyzeExchangeOffer)]
pub fn analyze_exchange_offer(
    old_pv: f64,
    new_pv: f64,
    consent_fee: f64,
    equity_sweetener_value: f64,
    exchange_type: &str,
) -> Result<JsValue, JsValue> {
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
/// @param lmeType - Structure of the exercise: open_market (aliases open_market_repurchase, omr), tender_offer (alias tender), amend_and_extend (aliases ae, a&e), or dropdown.
/// @param notional - Outstanding face amount of the target instrument, in the caller's monetary unit; must be positive.
/// @param repurchasePricePct - Price as a fraction of par for repurchases and tenders, the extension fee for amend-and-extend, or the transferred-asset fraction for a dropdown.
/// @param optAcceptancePct - Fraction of holders participating, in [0, 1].
/// @param ebitda - EBITDA in the same unit as notional; a positive value adds the leverage_impact block, null or non-positive omits it.
#[wasm_bindgen(js_name = analyzeLme)]
pub fn analyze_lme(
    lme_type: &str,
    notional: f64,
    repurchase_price_pct: f64,
    opt_acceptance_pct: f64,
    ebitda: Option<f64>,
) -> Result<JsValue, JsValue> {
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
