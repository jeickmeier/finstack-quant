//! WASM bindings for the `finstack-quant-margin` crate.
//!
//! Exposes CSA specification loading, variation margin calculation, and
//! bilateral XVA via JSON-based interfaces for JavaScript/TypeScript
//! consumers.

use crate::api::core::market_data::{JsDiscountCurve, JsHazardCurve};
use crate::utils::{parse_iso_date, to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

fn serialize_csa(csa: &finstack_quant_margin::CsaSpec) -> Result<String, JsValue> {
    serde_json::to_string(csa).map_err(to_js_err)
}

/// Create a standard USD regulatory CSA specification as JSON.
///
/// Returns the canonical ISDA-compliant CSA for USD OTC derivatives.
///
/// # Errors
///
/// Rejects if the embedded margin registry cannot be loaded or the resulting
/// CSA cannot be serialized to JSON.
#[wasm_bindgen(js_name = csaUsdRegulatoryJson)]
pub fn csa_usd_regulatory_json() -> Result<String, JsValue> {
    let csa = finstack_quant_margin::CsaSpec::usd_regulatory().map_err(to_js_err)?;
    serialize_csa(&csa)
}

/// Create a standard EUR regulatory CSA specification as JSON.
///
/// # Errors
///
/// Rejects if the embedded margin registry cannot be loaded or the resulting
/// CSA cannot be serialized to JSON.
#[wasm_bindgen(js_name = csaEurRegulatoryJson)]
pub fn csa_eur_regulatory_json() -> Result<String, JsValue> {
    let csa = finstack_quant_margin::CsaSpec::eur_regulatory().map_err(to_js_err)?;
    serialize_csa(&csa)
}

/// Validate a CSA specification JSON string.
///
/// Deserializes and re-serializes the input to verify it conforms
/// to the `CsaSpec` schema. Returns the canonical JSON on success.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, or failure to serialize
/// the decoded CSA specification.
/// @param json - CSA specification JSON to validate and normalize into canonical form.
#[wasm_bindgen(js_name = validateCsaJson)]
pub fn validate_csa_json(json: &str) -> Result<String, JsValue> {
    let csa: finstack_quant_margin::CsaSpec = serde_json::from_str(json).map_err(to_js_err)?;
    serialize_csa(&csa)
}

/// Calculate variation margin given exposure, posted collateral, and CSA JSON.
///
/// Returns a JSON object with delivery_amount, return_amount, net_exposure,
/// and requires_call fields.
///
/// @param csa_json - CSA specification JSON governing thresholds, minimum transfer, and timing.
/// @param exposure - Current mark-to-market exposure in the supplied currency units.
/// @param posted_collateral - Collateral already posted in the supplied currency units.
/// @param currency - ISO-4217 currency code shared by exposure and collateral amounts.
/// @param as_of - ISO-8601 VM calculation date.
/// @returns Variation-margin call amount, currency, and CSA metadata as a plain object.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `csa_json`, an unknown `currency`,
/// non-finite exposure or collateral amounts, an invalid calendar date, a
/// currency mismatch with the CSA, invalid VM parameters, calendar lookup or
/// settlement-date adjustment failures, or failure to serialize the result.
#[wasm_bindgen(js_name = calculateVm)]
pub fn calculate_vm(
    csa_json: &str,
    exposure: f64,
    posted_collateral: f64,
    currency: &str,
    as_of: &str,
) -> Result<JsValue, JsValue> {
    let csa: finstack_quant_margin::CsaSpec = serde_json::from_str(csa_json).map_err(to_js_err)?;
    let ccy: finstack_quant_core::currency::Currency = currency.parse().map_err(to_js_err)?;
    let exp = finstack_quant_core::money::Money::new(exposure, ccy)
        .map_err(|e| to_js_err(format!("invalid exposure: {e}")))?;
    let posted = finstack_quant_core::money::Money::new(posted_collateral, ccy)
        .map_err(|e| to_js_err(format!("invalid posted_collateral: {e}")))?;
    let as_of = parse_iso_date(as_of)?;

    let calc = finstack_quant_margin::VmCalculator::new(csa);
    let result = calc.calculate(exp, posted, as_of).map_err(to_js_err)?;

    let out = serde_json::json!({
        "gross_exposure": result.gross_exposure.amount(),
        "net_exposure": result.net_exposure.amount(),
        "delivery_amount": result.delivery_amount.amount(),
        "return_amount": result.return_amount.amount(),
        "net_margin": result.net_margin().amount(),
        "requires_call": result.requires_call(),
    });
    to_js_value(&out)
}

/// Compute bilateral XVA: CVA, DVA, FVA, MVA, and the all-in adjustment.
///
/// All legs are weighted by joint (first-to-default) survival. MVA is computed
/// only when `fundingJson` carries an `im_profile`; that posted IM also reduces
/// ENE for bilateral DVA.
///
/// The returned object reports the required all-in amount as
/// `total_xva = CVA - DVA + FVA + MVA`. Optional funding legs are absent from
/// the payload when they were not computed.
///
/// @param exposureProfileJson - `ExposureProfile` JSON with `times`,
/// `mtm_values`, `epe`, and `ene` arrays of equal length.
/// @param counterpartyHazardCurve - Hazard curve for the counterparty's credit.
/// @param ownHazardCurve - Hazard curve for the institution's own credit.
/// @param discountCurve - Risk-free discount curve for present-valuing.
/// @param counterpartyRecoveryRate - Recovery on counterparty default, in `[0, 1]`.
/// @param ownRecoveryRate - Recovery on own default, in `[0, 1]`.
/// @param fundingJson - Optional strict `FundingConfig` JSON driving FVA and,
/// when it carries `im_profile`, MVA; unknown fields are rejected. Omit for
/// credit legs only.
/// @returns The `XvaResult` as a plain object.
/// @throws Error - If JSON is malformed or has unknown funding fields, a recovery rate
/// is outside `[0, 1]`, a profile is invalid or has a mismatched IM horizon,
/// or a curve evaluation is non-finite.
///
/// @example
/// ```javascript
/// import init, { core, margin } from "finstack-quant-wasm";
/// await init();
/// const df = new core.DiscountCurve("USD-OIS", "2025-01-01", [0.0, 1.0, 5.0, 1.0], "log_linear");
/// const hz = new core.HazardCurve("CPTY", "2025-01-01", [0.0, 0.02, 30.0, 0.02], 0.4);
/// const result = margin.computeBilateralXva(
///   JSON.stringify({ times: [1, 2], mtm_values: [1e6, 1e6], epe: [1e6, 1e6], ene: [0, 0] }),
///   hz, hz, df, 0.4, 0.4,
///   JSON.stringify({ funding_spread_bp: 50.0 }),
/// );
/// result.total_xva; // CVA - DVA + FVA + MVA
/// ```
#[wasm_bindgen(js_name = computeBilateralXva)]
pub fn compute_bilateral_xva(
    exposure_profile_json: &str,
    counterparty_hazard_curve: &JsHazardCurve,
    own_hazard_curve: &JsHazardCurve,
    discount_curve: &JsDiscountCurve,
    counterparty_recovery_rate: f64,
    own_recovery_rate: f64,
    funding_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let exposure: finstack_quant_margin::xva::types::ExposureProfile =
        serde_json::from_str(exposure_profile_json).map_err(to_js_err)?;
    let funding: Option<finstack_quant_margin::xva::types::FundingConfig> = funding_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(to_js_err)?;

    let result = finstack_quant_margin::xva::cva::compute_bilateral_xva(
        &exposure,
        &counterparty_hazard_curve.inner,
        &own_hazard_curve.inner,
        &discount_curve.inner,
        counterparty_recovery_rate,
        own_recovery_rate,
        funding.as_ref(),
    )
    .map_err(to_js_err)?;

    to_js_value(&result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn assert_csa_json_shape(json: &str, expected_base_currency: &str) {
        let Ok(v) = serde_json::from_str::<Value>(json) else {
            panic!("CSA JSON should parse");
        };
        let Some(obj) = v.as_object() else {
            panic!("CSA JSON should be an object");
        };
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("base_currency"));
        assert!(obj.contains_key("vm_params"));
        assert!(obj.contains_key("eligible_collateral"));
        assert!(obj.contains_key("call_timing"));
        assert!(obj.contains_key("collateral_curve_id"));
        assert_eq!(
            obj.get("base_currency").and_then(Value::as_str),
            Some(expected_base_currency)
        );
    }

    #[test]
    fn csa_usd_regulatory_json_shape() {
        let Ok(json) = csa_usd_regulatory_json() else {
            panic!("csa_usd_regulatory should succeed");
        };
        assert_csa_json_shape(&json, "USD");
    }

    #[test]
    fn csa_eur_regulatory_json_shape() {
        let Ok(json) = csa_eur_regulatory_json() else {
            panic!("csa_eur_regulatory should succeed");
        };
        assert_csa_json_shape(&json, "EUR");
    }

    #[test]
    fn validate_csa_json_round_trips_usd_regulatory() {
        let Ok(original) = csa_usd_regulatory_json() else {
            panic!("csa_usd_regulatory should succeed");
        };
        let Ok(parsed_once) = serde_json::from_str::<finstack_quant_margin::CsaSpec>(&original)
        else {
            panic!("original JSON should deserialize to CsaSpec");
        };
        let Ok(canonical) = validate_csa_json(&original) else {
            panic!("validate_csa_json should succeed on regulatory CSA JSON");
        };
        let Ok(parsed_twice) = serde_json::from_str::<finstack_quant_margin::CsaSpec>(&canonical)
        else {
            panic!("canonical JSON should deserialize to CsaSpec");
        };
        assert_eq!(parsed_once, parsed_twice);
    }
}
