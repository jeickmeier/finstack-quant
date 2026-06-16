//! WASM bindings for the `finstack-quant-margin` crate.
//!
//! Exposes CSA specification loading and variation margin calculation
//! via JSON-based interfaces for JavaScript/TypeScript consumers.

use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

fn serialize_csa(csa: &finstack_quant_margin::CsaSpec) -> Result<String, JsValue> {
    serde_json::to_string(csa).map_err(to_js_err)
}

/// Create a standard USD regulatory CSA specification as JSON.
///
/// Returns the canonical ISDA-compliant CSA for USD OTC derivatives.
#[wasm_bindgen(js_name = csaUsdRegulatory)]
pub fn csa_usd_regulatory() -> Result<String, JsValue> {
    let csa = finstack_quant_margin::CsaSpec::usd_regulatory().map_err(to_js_err)?;
    serialize_csa(&csa)
}

/// Create a standard EUR regulatory CSA specification as JSON.
#[wasm_bindgen(js_name = csaEurRegulatory)]
pub fn csa_eur_regulatory() -> Result<String, JsValue> {
    let csa = finstack_quant_margin::CsaSpec::eur_regulatory().map_err(to_js_err)?;
    serialize_csa(&csa)
}

/// Validate a CSA specification JSON string.
///
/// Deserializes and re-serializes the input to verify it conforms
/// to the `CsaSpec` schema. Returns the canonical JSON on success.
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
/// # Arguments
///
/// * `csa_json` - CSA specification as JSON string
/// * `exposure` - Current mark-to-market exposure amount
/// * `posted_collateral` - Currently posted collateral amount
/// * `currency` - ISO currency code (e.g. "USD")
/// * `year` - Calculation year
/// * `month` - Calculation month (1-12)
/// * `day` - Calculation day
#[wasm_bindgen(js_name = calculateVm)]
pub fn calculate_vm(
    csa_json: &str,
    exposure: f64,
    posted_collateral: f64,
    currency: &str,
    year: i32,
    month: u8,
    day: u8,
) -> Result<JsValue, JsValue> {
    let csa: finstack_quant_margin::CsaSpec = serde_json::from_str(csa_json).map_err(to_js_err)?;
    let ccy: finstack_quant_core::currency::Currency = currency.parse().map_err(to_js_err)?;
    let exp = finstack_quant_core::money::Money::try_new(exposure, ccy)
        .map_err(|e| to_js_err(format!("invalid exposure: {e}")))?;
    let posted = finstack_quant_core::money::Money::try_new(posted_collateral, ccy)
        .map_err(|e| to_js_err(format!("invalid posted_collateral: {e}")))?;
    let m = time::Month::try_from(month).map_err(to_js_err)?;
    let as_of =
        finstack_quant_core::dates::Date::from_calendar_date(year, m, day).map_err(to_js_err)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn assert_csa_json_shape(json: &str, expected_base_ccy: &str) {
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
            Some(expected_base_ccy)
        );
    }

    #[test]
    fn csa_usd_regulatory_json_shape() {
        let Ok(json) = csa_usd_regulatory() else {
            panic!("csa_usd_regulatory should succeed");
        };
        assert_csa_json_shape(&json, "USD");
    }

    #[test]
    fn csa_eur_regulatory_json_shape() {
        let Ok(json) = csa_eur_regulatory() else {
            panic!("csa_eur_regulatory should succeed");
        };
        assert_csa_json_shape(&json, "EUR");
    }

    #[test]
    fn validate_csa_json_round_trips_usd_regulatory() {
        let Ok(original) = csa_usd_regulatory() else {
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
