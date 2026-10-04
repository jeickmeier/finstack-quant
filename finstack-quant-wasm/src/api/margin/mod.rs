//! WASM bindings for the `finstack-quant-margin` crate.
//!
//! Data types (CSA specifications, schedules, profiles, results) cross the
//! boundary as plain JSON values typed by the generated TypeScript; engines,
//! calculators and the sensitivity builders are classes. The submodules hold
//! one family each.

mod calculators;
mod frtb;
mod im;
mod metrics;
mod sa_ccr;
mod types;
mod xva;

#[cfg(all(test, target_arch = "wasm32"))]
mod tests;

use crate::api::core::market_data::{JsDiscountCurve, JsHazardCurve};
use crate::utils::input::{js_f64, js_string, json_text, opt_json_text};
use crate::utils::{parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use wasm_bindgen::prelude::*;

/// An ISO-4217 currency argument.
fn js_currency(value: &JsValue, label: &str) -> Result<Currency, JsValue> {
    js_string(value, label)?.parse().map_err(to_js_err)
}

/// A numeric amount argument in major units of `currency`.
fn js_money(value: &JsValue, label: &str, currency: Currency) -> Result<Money, JsValue> {
    Money::new(js_f64(value, label)?, currency).map_err(to_js_err)
}

/// An amount in major units of a CSA's base currency.
fn base_money(csa: &finstack_quant_margin::CsaSpec, amount: f64) -> Result<Money, JsValue> {
    Money::new(amount, csa.base_currency).map_err(to_js_err)
}

/// An ISO-8601 date argument.
fn js_date(value: &JsValue, label: &str) -> Result<Date, JsValue> {
    parse_iso_date(&js_string(value, label)?)
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
/// const hz = core.HazardCurve.flat("CPTY", "2025-01-01", 0.02, 0.4);
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
