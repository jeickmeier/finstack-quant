//! Shared position and lifecycle terms for listed futures.

use crate::utils::input::{from_js_json, js_f64, js_opt_f64};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::ListedFutureTerms;
use wasm_bindgen::prelude::*;

/// Validate listed position and lifecycle terms and emit their standard wire form.
/// @param terms - Listed terms with quantity in contracts, multiplier in settlement currency per point, and marks in contract price points.
/// @returns Validated independent position and settlement terms.
/// @throws `TypeError` for malformed terms or `FinstackError` for invalid quantities, dates, marks or delivery terms.
#[wasm_bindgen(js_name = listedFutureTerms, unchecked_return_type = "generated.valuations.ListedFutureTerms")]
pub fn listed_future_terms(
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.ListedFutureTerms")] terms: JsValue,
) -> Result<JsValue, JsValue> {
    let terms: ListedFutureTerms = from_js_json(&terms, "terms")?;
    terms.validate().map_err(to_js_err)?;
    to_js_value(&terms)
}

/// Derive listed-future terms with a new pair of exchange marks.
/// @param terms - Source listed position and lifecycle terms.
/// @param quotedPrice - Live mark in contract price points, or null to clear it.
/// @param settlementPrice - Official final settlement mark, or null to clear it.
/// @returns Independent terms with both marks replaced.
/// @throws `TypeError` for an invalid input shape or `FinstackError` for invalid terms or nonfinite marks.
#[wasm_bindgen(js_name = listedFutureTermsWithMarks, unchecked_return_type = "generated.valuations.ListedFutureTerms")]
pub fn listed_future_terms_with_marks(
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.ListedFutureTerms")] terms: JsValue,
    #[wasm_bindgen(
        js_name = "quotedPrice",
        unchecked_optional_param_type = "number | null"
    )]
    quoted_price: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "settlementPrice",
        unchecked_optional_param_type = "number | null"
    )]
    settlement_price: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let terms: ListedFutureTerms = from_js_json(&terms, "terms")?;
    let quoted_price = js_opt_f64(quoted_price.as_ref(), "quotedPrice")?;
    let settlement_price = js_opt_f64(settlement_price.as_ref(), "settlementPrice")?;
    to_js_value(
        &terms
            .with_marks(quoted_price, settlement_price)
            .map_err(to_js_err)?,
    )
}

/// Derive listed-future terms with a different final settlement policy.
/// @param terms - Source listed position and lifecycle terms.
/// @param settlement - Cash settlement or physical delivery of a specified asset and quantity per contract.
/// @returns Independent terms with the replacement settlement policy.
/// @throws `TypeError` for an invalid input shape or `FinstackError` for invalid terms or physical delivery details.
#[wasm_bindgen(js_name = listedFutureTermsWithSettlement, unchecked_return_type = "generated.valuations.ListedFutureTerms")]
pub fn listed_future_terms_with_settlement(
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.ListedFutureTerms")] terms: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.ListedFutureSettlement")]
    settlement: JsValue,
) -> Result<JsValue, JsValue> {
    let terms: ListedFutureTerms = from_js_json(&terms, "terms")?;
    let settlement = from_js_json(&settlement, "settlement")?;
    to_js_value(&terms.with_settlement(settlement).map_err(to_js_err)?)
}

/// Set the number of exchange contracts in a derived position.
/// @param terms - Source listed position and lifecycle terms.
/// @param contracts - Positive finite number of contracts. Fractional values are permitted.
/// @returns Independent listed terms with the new contract count.
/// @throws `TypeError` for mistyped input or `FinstackError` for invalid terms or quantities.
#[wasm_bindgen(js_name = listedFutureTermsWithContracts, unchecked_return_type = "generated.valuations.ListedFutureTerms")]
pub fn listed_future_terms_with_contracts(
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.ListedFutureTerms")] terms: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] contracts: JsValue,
) -> Result<JsValue, JsValue> {
    let terms: ListedFutureTerms = from_js_json(&terms, "terms")?;
    let contracts = js_f64(&contracts, "contracts")?;
    to_js_value(&terms.with_contracts(contracts).map_err(to_js_err)?)
}
