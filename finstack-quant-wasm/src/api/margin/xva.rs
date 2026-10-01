//! XVA inputs and margin valuation adjustment: funding configuration,
//! exposure and initial-margin profiles, decay profiles and MVA.
//!
//! `FundingConfig`, `ExposureProfile`, `ImProfile`, `ImDecayProfile` and
//! `MvaResult` cross the boundary as plain JSON values (the generated
//! TypeScript types of the same names). The functions named after a type are
//! the twins of that Python class's factories and methods.

use super::im::{JsSimmCalculator, JsSimmSensitivities};
use super::js_currency;
use crate::api::core::market_data::{JsDiscountCurve, JsHazardCurve};
use crate::utils::input::{from_js_json, js_f64, js_f64_seq};
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
pub fn funding_config_effective_benefit_bp(funding: JsValue) -> Result<f64, JsValue> {
    Ok(parse_funding(&funding, "funding")?.effective_benefit_bp())
}

/// Funding spread applied to posted initial margin (the MVA spread).
///
/// The configured `margin_funding_spread_bp`, or the funding spread when no
/// separate margin spread is set.
/// @param funding - `FundingConfig` (object or JSON).
/// @returns The margin funding spread in basis points.
///
/// # Errors
///
/// Throws if `funding` is malformed, has unknown fields, or fails validation.
#[wasm_bindgen(js_name = fundingConfigEffectiveMarginSpreadBp)]
pub fn funding_config_effective_margin_spread_bp(funding: JsValue) -> Result<f64, JsValue> {
    Ok(parse_funding(&funding, "funding")?.effective_margin_spread_bp())
}

// ---------------------------------------------------------------------------
// ExposureProfile / ImProfile
// ---------------------------------------------------------------------------

/// Validate an exposure profile: equal-length arrays on a strictly increasing time grid.
/// @param exposure_profile - `ExposureProfile` (object or JSON) with `times`, `mtm_values`, `epe`, `ene` and optional `diagnostics`.
///
/// # Errors
///
/// Throws if the profile is malformed, has unknown fields, mismatched array
/// lengths, a non-increasing or non-finite time grid, or invalid exposures.
#[wasm_bindgen(js_name = exposureProfileValidate)]
pub fn exposure_profile_validate(exposure_profile: JsValue) -> Result<(), JsValue> {
    let profile: xva::ExposureProfile = from_js_json(&exposure_profile, "exposureProfile")?;
    profile.validate().map_err(to_js_err)
}

/// Validate an expected initial-margin profile.
/// @param im_profile - `ImProfile` (object or JSON) with `times` (years) and `im_values`.
///
/// # Errors
///
/// Throws if the profile is malformed, empty, has mismatched lengths, a
/// non-positive or non-increasing time, or a negative or non-finite IM value.
#[wasm_bindgen(js_name = imProfileValidate)]
pub fn im_profile_validate(im_profile: JsValue) -> Result<(), JsValue> {
    let profile: mva::ImProfile = from_js_json(&im_profile, "imProfile")?;
    profile.validate().map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// ImDecayProfile
// ---------------------------------------------------------------------------

/// Constant IM decay profile: IM stays at today's level for the whole horizon.
/// @returns The `ImDecayProfile` value `"constant"`.
///
/// # Errors
///
/// Throws if the value cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = imDecayProfileConstant)]
pub fn im_decay_profile_constant() -> Result<JsValue, JsValue> {
    to_js_value(&mva::ImDecayProfile::Constant)
}

/// Linear IM decay profile: `factor(t) = max(1 - t/T, 0)`.
/// @param maturity_years - Portfolio maturity `T` in years; positive and finite.
/// @returns The `ImDecayProfile` value `{ linear_to_maturity: { maturity_years } }`.
///
/// # Errors
///
/// Throws if `maturity_years` is non-positive or non-finite.
#[wasm_bindgen(js_name = imDecayProfileLinearToMaturity)]
pub fn im_decay_profile_linear_to_maturity(maturity_years: JsValue) -> Result<JsValue, JsValue> {
    let maturity_years = js_f64(&maturity_years, "maturityYears")?;
    to_js_value(&mva::ImDecayProfile::linear_to_maturity(maturity_years).map_err(to_js_err)?)
}

/// Square-root IM decay profile: `factor(t) = sqrt(max(1 - t/T, 0))`.
/// @param maturity_years - Portfolio maturity `T` in years; positive and finite.
/// @returns The `ImDecayProfile` value `{ sqrt_time: { maturity_years } }`.
///
/// # Errors
///
/// Throws if `maturity_years` is non-positive or non-finite.
#[wasm_bindgen(js_name = imDecayProfileSqrtTime)]
pub fn im_decay_profile_sqrt_time(maturity_years: JsValue) -> Result<JsValue, JsValue> {
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
pub fn im_decay_profile_factor(decay: JsValue, t: JsValue) -> Result<f64, JsValue> {
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

/// Expected initial-margin profile from today's SIMM margin and a decay profile.
///
/// `IM(t) = SIMM(sensitivities) * decay.factor(t)` on `time_grid`.
/// @param calculator - SIMM calculator handle.
/// @param sensitivities - SIMM sensitivities handle.
/// @param currency - ISO-4217 currency the IM is reported in.
/// @param decay - `ImDecayProfile` (object or JSON) shaping IM over time.
/// @param time_grid - Strictly increasing positive times in years.
/// @returns The `ImProfile` (`times`, `im_values`) as a plain object.
///
/// # Errors
///
/// Throws for an unknown currency, an invalid decay profile or time grid, or
/// sensitivities the SIMM calculator rejects.
#[wasm_bindgen(js_name = imProfileFromSimm)]
pub fn im_profile_from_simm(
    calculator: &JsSimmCalculator,
    sensitivities: &JsSimmSensitivities,
    currency: JsValue,
    decay: JsValue,
    time_grid: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = js_currency(&currency, "currency")?;
    let decay = parse_decay(&decay, "decay")?;
    let time_grid = js_f64_seq(&time_grid, "timeGrid")?;
    let profile = mva::im_profile_from_simm(
        &calculator.inner,
        &sensitivities.inner,
        currency,
        &decay,
        &time_grid,
    )
    .map_err(to_js_err)?;
    to_js_value(&profile)
}

fn compute_mva_inner(
    im_profile: &JsValue,
    funding_spread_curve: &JsValue,
    discount_curve: &JsDiscountCurve,
    survival_curve: Option<&JsHazardCurve>,
) -> Result<JsValue, JsValue> {
    let im_profile: mva::ImProfile = from_js_json(im_profile, "imProfile")?;
    let funding_spread_curve: Vec<(f64, f64)> =
        from_js_json(funding_spread_curve, "fundingSpreadCurve")?;
    let result = mva::compute_mva(
        &im_profile,
        &funding_spread_curve,
        &discount_curve.inner,
        survival_curve.map(|curve| curve.inner.as_ref()),
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Margin valuation adjustment over an expected-IM profile, without
/// own-default weighting.
///
/// `MVA = sum_i s(t_i) * IM(t_i) * DF(t_i) * dt_i` on the profile grid. The
/// published `margin.computeMva` calls this when no survival curve is given.
/// @param im_profile - `ImProfile` (object or JSON) of expected IM.
/// @param funding_spread_curve - Array of `[timeYears, spreadBp]` pairs, linearly interpolated with flat extrapolation; one pair is a flat spread.
/// @param discount_curve - Risk-free discount curve handle.
/// @returns The `MvaResult` (`mva`, `average_im`, `im_profile`) as a plain object.
///
/// # Errors
///
/// Throws if the profile or spread curve is malformed or invalid, or a curve
/// evaluation is non-finite.
#[wasm_bindgen(js_name = computeMvaWithoutSurvival)]
pub fn compute_mva_without_survival(
    im_profile: JsValue,
    funding_spread_curve: JsValue,
    discount_curve: &JsDiscountCurve,
) -> Result<JsValue, JsValue> {
    compute_mva_inner(&im_profile, &funding_spread_curve, discount_curve, None)
}

/// Margin valuation adjustment weighted by the bank's own survival probability.
///
/// `MVA = sum_i s(t_i) * IM(t_i) * DF(t_i) * S(t_i) * dt_i` on the profile
/// grid. The published `margin.computeMva` calls this when a survival curve
/// is given.
/// @param im_profile - `ImProfile` (object or JSON) of expected IM.
/// @param funding_spread_curve - Array of `[timeYears, spreadBp]` pairs, linearly interpolated with flat extrapolation; one pair is a flat spread.
/// @param discount_curve - Risk-free discount curve handle.
/// @param survival_curve - The bank's own hazard curve handle.
/// @returns The `MvaResult` (`mva`, `average_im`, `im_profile`) as a plain object.
///
/// # Errors
///
/// Throws if the profile or spread curve is malformed or invalid, or a curve
/// evaluation is non-finite.
#[wasm_bindgen(js_name = computeMvaWithSurvival)]
pub fn compute_mva_with_survival(
    im_profile: JsValue,
    funding_spread_curve: JsValue,
    discount_curve: &JsDiscountCurve,
    survival_curve: &JsHazardCurve,
) -> Result<JsValue, JsValue> {
    compute_mva_inner(
        &im_profile,
        &funding_spread_curve,
        discount_curve,
        Some(survival_curve),
    )
}
