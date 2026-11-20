//! XVA inputs and margin valuation adjustment: funding configuration,
//! exposure and initial-margin profiles, decay profiles and MVA.
//!
//! `FundingConfig`, `ExposureProfile`, `ImProfile`, `ImDecayProfile` and
//! `MvaResult` cross the boundary as plain JSON values (the generated
//! TypeScript types of the same names). The functions named after a type are
//! the twins of that Python class's factories and methods.

use super::im::{JsSimmCalculator, JsSimmSensitivities};
use crate::api::core::market_discount::JsDiscountCurve;
use crate::api::core::market_hazard::JsHazardCurve;
use crate::utils::input::{from_js_json, js_f64};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_margin::xva::{mva, types as xva};
use wasm_bindgen::prelude::*;

fn parse_funding(funding: &JsValue, label: &str) -> Result<xva::FundingConfig, JsValue> {
    let funding: xva::FundingConfig = from_js_json(funding, label)?;
    funding.validate().map_err(to_js_err)?;
    Ok(funding)
}

// ---------------------------------------------------------------------------
// FundingConfig
// ---------------------------------------------------------------------------

/// Funding benefit spread applied to negative exposure.
///
/// The configured `funding_benefit_bp`, or the funding spread when no
/// separate benefit spread is set (symmetric funding).
/// @param funding - `FundingConfig` (object or JSON).
/// @returns The benefit spread in basis points.
///
/// # Errors
///
/// Throws if `funding` is malformed, has unknown fields, or fails validation.
#[wasm_bindgen(js_name = fundingConfigEffectiveBenefitBp)]
pub fn funding_config_effective_benefit_bp(
    #[wasm_bindgen(unchecked_param_type = "FundingConfig | string")] funding: JsValue,
) -> Result<f64, JsValue> {
    Ok(parse_funding(&funding, "funding")?.effective_benefit_bp())
}

// ---------------------------------------------------------------------------
// ExposureProfile / ImProfile
// ---------------------------------------------------------------------------

/// Validate an exposure profile: equal-length arrays on a strictly increasing time grid.
/// @param exposureProfile - `ExposureProfile` (object or JSON) with valuation cut, reporting currency, dated grid, signed marks, EPE and ENE, and completeness diagnostics.
///
/// # Errors
///
/// Throws if the profile is malformed, has unknown fields, mismatched array
/// lengths, a non-increasing or non-finite time grid, or invalid exposures.
#[wasm_bindgen(js_name = exposureProfileValidate)]
pub fn exposure_profile_validate(
    #[wasm_bindgen(
        js_name = "exposureProfile",
        unchecked_param_type = "ExposureProfile | string"
    )]
    exposure_profile: JsValue,
) -> Result<(), JsValue> {
    let profile: xva::ExposureProfile = from_js_json(&exposure_profile, "exposureProfile")?;
    profile.validate().map_err(to_js_err)
}

/// Validate an expected initial-margin profile.
/// @param imProfile - `ImProfile` (object or JSON) with currency, scope, valuation date, calendar observations and expected IM.
///
/// # Errors
///
/// Throws for incomplete scope, unanchored or unordered dates and non-finite or negative amounts.
#[wasm_bindgen(js_name = imProfileValidate)]
pub fn im_profile_validate(
    #[wasm_bindgen(js_name = "imProfile", unchecked_param_type = "ImProfile | string")] im_profile: JsValue,
) -> Result<(), JsValue> {
    let profile: mva::ImProfile = from_js_json(&im_profile, "imProfile")?;
    profile.validate().map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// ImDecayProfile
// ---------------------------------------------------------------------------

/// Linear IM decay profile: `factor(t) = max(1 - t/T, 0)`.
/// @param maturityYears - Portfolio maturity `T` in years; positive and finite.
/// @returns The `ImDecayProfile` value `{ linear_to_maturity: { maturity_years } }`.
///
/// # Errors
///
/// Throws if `maturity_years` is non-positive or non-finite.
#[wasm_bindgen(js_name = imDecayProfileLinearToMaturity, unchecked_return_type = "ImDecayProfile")]
pub fn im_decay_profile_linear_to_maturity(
    #[wasm_bindgen(js_name = "maturityYears", unchecked_param_type = "number")]
    maturity_years: JsValue,
) -> Result<JsValue, JsValue> {
    let maturity_years = js_f64(&maturity_years, "maturityYears")?;
    to_js_value(&mva::ImDecayProfile::linear_to_maturity(maturity_years).map_err(to_js_err)?)
}

/// Square-root IM decay profile: `factor(t) = sqrt(max(1 - t/T, 0))`.
/// @param maturityYears - Portfolio maturity `T` in years; positive and finite.
/// @returns The `ImDecayProfile` value `{ sqrt_time: { maturity_years } }`.
///
/// # Errors
///
/// Throws if `maturity_years` is non-positive or non-finite.
#[wasm_bindgen(js_name = imDecayProfileSqrtTime, unchecked_return_type = "ImDecayProfile")]
pub fn im_decay_profile_sqrt_time(
    #[wasm_bindgen(js_name = "maturityYears", unchecked_param_type = "number")]
    maturity_years: JsValue,
) -> Result<JsValue, JsValue> {
    let maturity_years = js_f64(&maturity_years, "maturityYears")?;
    to_js_value(&mva::ImDecayProfile::sqrt_time(maturity_years).map_err(to_js_err)?)
}

/// Decay factor of an IM decay profile at time `t`.
/// @param decay - `ImDecayProfile` (object or JSON), for example `"constant"` or `{ linear_to_maturity: { maturity_years: 5 } }`.
/// @param t - Time from today in years.
/// @returns The factor multiplying today's IM, in `[0, 1]` for `t >= 0`.
///
/// # Errors
///
/// Throws if `decay` is malformed or its maturity is non-positive or non-finite.
#[wasm_bindgen(js_name = imDecayProfileFactor)]
pub fn im_decay_profile_factor(
    #[wasm_bindgen(unchecked_param_type = "ImDecayProfile | string")] decay: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
) -> Result<f64, JsValue> {
    let decay = parse_decay(&decay, "decay")?;
    Ok(decay.factor(js_f64(&t, "t")?))
}

/// A decay profile argument: the plain serde value, or its JSON text.
///
/// The unit variant serializes as the bare label `"constant"`, which is not
/// JSON text, so a JavaScript string is read as that label first.
fn parse_decay(decay: &JsValue, label: &str) -> Result<mva::ImDecayProfile, JsValue> {
    let bare_label = decay
        .as_string()
        .and_then(|text| serde_json::from_value(serde_json::Value::String(text)).ok());
    let decay: mva::ImDecayProfile = match bare_label {
        Some(decay) => decay,
        None => from_js_json(decay, label)?,
    };
    decay.validate().map_err(to_js_err)?;
    Ok(decay)
}

// ---------------------------------------------------------------------------
// IM profile and MVA
// ---------------------------------------------------------------------------

/// Project the opening historical SIMM value over a dated runoff schedule.
/// @param calculator - Elected SIMM parameter version and MPOR.
/// @param sensitivities - Signed risk of the admitted netting set.
/// @param spec - Dated legal scope, risk snapshot, currency and elected runoff.
/// @returns Complete expected IM at each date, including the opening SIMM.
/// @throws Error if the scope, date grid, decay or sensitivities fail validation.
#[wasm_bindgen(js_name = imProfileFromSimm, unchecked_return_type = "ImProfile")]
pub fn im_profile_from_simm(
    calculator: &JsSimmCalculator,
    sensitivities: &JsSimmSensitivities,
    #[wasm_bindgen(unchecked_param_type = "generated.margin.ImProjectionSpec | string")]
    spec: JsValue,
) -> Result<JsValue, JsValue> {
    let spec = from_js_json(&spec, "spec")?;
    let profile = mva::im_profile_from_simm(&calculator.inner, &sensitivities.inner, &spec)
        .map_err(to_js_err)?;
    to_js_value(&profile)
}

fn compute_mva_inner(
    im_profile: &JsValue,
    funding_spread: &JsValue,
    discount_curve: &JsDiscountCurve,
    survival_curve: Option<&JsHazardCurve>,
) -> Result<JsValue, JsValue> {
    let im_profile: mva::ImProfile = from_js_json(im_profile, "imProfile")?;
    let funding_spread: mva::MarginFundingSpread = from_js_json(funding_spread, "fundingSpread")?;
    let result = mva::compute_mva(
        &im_profile,
        &funding_spread,
        &discount_curve.inner,
        survival_curve.map(|curve| curve.inner.as_ref()),
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Margin valuation adjustment over an expected-IM profile, without
/// own-default weighting.
///
/// Sample each curve on its own calendar clock and integrate the full product.
/// @param imProfile - `ImProfile` (object or JSON) of expected IM.
/// @param fundingSpread - Explicit flat or dated basis-point funding spread covering the posted-margin horizon.
/// @param discountCurve - Risk-free discount curve handle.
/// @returns MVA, expected IM weighted only by time, and complete interval evidence.
///
/// # Errors
///
/// Throws if the profile or spread curve is malformed or invalid, or a curve
/// evaluation is non-finite.
#[wasm_bindgen(js_name = computeMvaWithoutSurvival, unchecked_return_type = "MvaResult")]
pub fn compute_mva_without_survival(
    #[wasm_bindgen(js_name = "imProfile", unchecked_param_type = "ImProfile | string")] im_profile: JsValue,
    #[wasm_bindgen(
        js_name = "fundingSpread",
        unchecked_param_type = "generated.margin.MarginFundingSpread | string"
    )]
    funding_spread: JsValue,
    #[wasm_bindgen(js_name = "discountCurve")] discount_curve: &JsDiscountCurve,
) -> Result<JsValue, JsValue> {
    compute_mva_inner(&im_profile, &funding_spread, discount_curve, None)
}

/// Margin valuation adjustment weighted by the bank's own survival probability.
///
/// Sample each curve on its own calendar clock and integrate the full product.
/// @param imProfile - `ImProfile` (object or JSON) of expected IM.
/// @param fundingSpread - Explicit flat or dated basis-point funding spread covering the posted-margin horizon.
/// @param discountCurve - Risk-free discount curve handle.
/// @param survivalCurve - The bank's own hazard curve handle.
/// @returns MVA, expected IM weighted only by time, and complete interval evidence.
///
/// # Errors
///
/// Throws if the profile or spread curve is malformed or invalid, or a curve
/// evaluation is non-finite.
#[wasm_bindgen(js_name = computeMvaWithSurvival, unchecked_return_type = "MvaResult")]
pub fn compute_mva_with_survival(
    #[wasm_bindgen(js_name = "imProfile", unchecked_param_type = "ImProfile | string")] im_profile: JsValue,
    #[wasm_bindgen(
        js_name = "fundingSpread",
        unchecked_param_type = "generated.margin.MarginFundingSpread | string"
    )]
    funding_spread: JsValue,
    #[wasm_bindgen(js_name = "discountCurve")] discount_curve: &JsDiscountCurve,
    #[wasm_bindgen(js_name = "survivalCurve")] survival_curve: &JsHazardCurve,
) -> Result<JsValue, JsValue> {
    compute_mva_inner(
        &im_profile,
        &funding_spread,
        discount_curve,
        Some(survival_curve),
    )
}
