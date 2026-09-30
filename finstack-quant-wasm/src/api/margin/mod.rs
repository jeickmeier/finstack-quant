//! WASM bindings for the `finstack-quant-margin` crate.
//!
//! Exposes CSA specification loading, variation margin calculation, and
//! bilateral XVA via JSON-based interfaces for JavaScript/TypeScript
//! consumers.

use crate::api::core::market_data::{JsDiscountCurve, JsHazardCurve};
use crate::utils::input::{js_f64, js_string, json_text, opt_json_text};
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
/// Checks the JSON schema and canonical CSA semantics, including currencies,
/// monetary bounds and calendar lookup. Returns canonical JSON on success.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, or failure to serialize
/// the decoded CSA specification; also rejects invalid CSA terms or calendar identifiers.
/// @param json - CSA specification JSON to validate and normalize into canonical form.
#[wasm_bindgen(js_name = validateCsaJson)]
pub fn validate_csa_json(json: JsValue) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    let csa: finstack_quant_margin::CsaSpec = serde_json::from_str(json).map_err(to_js_err)?;
    csa.validate().map_err(to_js_err)?;
    serialize_csa(&csa)
}

/// Calculate variation margin given exposure, posted collateral, and CSA JSON.
///
/// Returns the Rust `VmResult` in its canonical serde form (the same wire
/// Python `VmResult.to_json()` emits): `date`, `gross_exposure`,
/// `net_exposure`, `post_amount`, `collect_amount` (each a Money object
/// `{amount, currency}` with a decimal-string amount) and `settlement_date`.
///
/// @param csa_json - CSA specification JSON governing thresholds, minimum transfer, and timing.
/// @param exposure - Signed mark-to-market in the supplied currency: positive means the counterparty owes the desk.
/// @param posted_collateral - Signed collateral balance: positive held, negative posted, including pending agreed calls.
/// @param currency - ISO-4217 currency code shared by exposure and collateral amounts.
/// @param as_of - ISO-8601 VM calculation date.
/// @returns The canonical `VmResult` as a plain object.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `csa_json`, an unknown `currency`,
/// non-finite exposure or collateral amounts, an invalid calendar date, a
/// currency mismatch with the CSA, invalid VM parameters, calendar lookup or
/// settlement-date adjustment failures, or failure to serialize the result.
#[wasm_bindgen(js_name = calculateVm)]
pub fn calculate_vm(
    csa_json: JsValue,
    exposure: JsValue,
    posted_collateral: JsValue,
    currency: JsValue,
    as_of: JsValue,
) -> Result<JsValue, JsValue> {
    let exposure = js_f64(&exposure, "exposure")?;
    let posted_collateral = js_f64(&posted_collateral, "postedCollateral")?;
    let csa_json: &str = &json_text(&csa_json, "csaJson")?;
    let currency: &str = &js_string(&currency, "currency")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let csa: finstack_quant_margin::CsaSpec = serde_json::from_str(csa_json).map_err(to_js_err)?;
    let ccy: finstack_quant_core::currency::Currency = currency.parse().map_err(to_js_err)?;
    let exp = finstack_quant_core::money::Money::new(exposure, ccy).map_err(to_js_err)?;
    let posted =
        finstack_quant_core::money::Money::new(posted_collateral, ccy).map_err(to_js_err)?;
    let as_of = parse_iso_date(as_of)?;

    let calc = finstack_quant_margin::VmCalculator::new(csa);
    let result = calc.calculate(exp, posted, as_of).map_err(to_js_err)?;
    to_js_value(&result)
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
/// @param exposureProfileJson - Strict `ExposureProfile` JSON with `times`,
/// `mtm_values`, `epe`, and `ene` arrays of equal length and an optional
/// `diagnostics` object (`market_roll_failures`, `valuation_failures`,
/// `total_time_points`); unknown fields are rejected.
/// @param counterpartyHazardCurve - Hazard curve for the counterparty's credit.
/// @param ownHazardCurve - Hazard curve for the institution's own credit.
/// @param discountCurve - Risk-free discount curve for present-valuing.
/// @param counterpartyRecoveryRate - Recovery on counterparty default, in `[0, 1]`.
/// @param ownRecoveryRate - Recovery on own default, in `[0, 1]`.
/// @param fundingJson - Optional strict `FundingConfig` JSON driving FVA and,
/// when it carries `im_profile`, MVA; unknown fields are rejected. Omit for
/// credit legs only.
/// @returns The `XvaResult` as a plain object.
/// @throws Error - If JSON is malformed or has unknown profile or funding fields, a recovery rate
/// is outside `[0, 1]`, a profile is invalid or has a mismatched IM horizon,
/// or a curve evaluation is non-finite.
///
/// @example
/// ```javascript
/// import init, { core, margin } from "finstack-quant-wasm";
/// await init();
/// const df = new core.DiscountCurve({
///   id: "USD-OIS",
///   baseDate: "2025-01-01",
///   knots: [0.0, 1.0, 5.0, 1.0],
///   interp: "log_linear",
/// });
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
    exposure_profile_json: JsValue,
    counterparty_hazard_curve: &JsHazardCurve,
    own_hazard_curve: &JsHazardCurve,
    discount_curve: &JsDiscountCurve,
    counterparty_recovery_rate: JsValue,
    own_recovery_rate: JsValue,
    funding_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let counterparty_recovery_rate =
        js_f64(&counterparty_recovery_rate, "counterpartyRecoveryRate")?;
    let own_recovery_rate = js_f64(&own_recovery_rate, "ownRecoveryRate")?;
    let exposure_profile_json: &str = &json_text(&exposure_profile_json, "exposureProfileJson")?;
    let funding_json = opt_json_text(funding_json.as_ref(), "fundingJson")?;
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
}
