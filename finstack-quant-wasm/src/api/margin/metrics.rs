//! Margin metric value objects: utilization, excess collateral, funding cost
//! and haircut sensitivity.
//!
//! Each metric crosses the boundary as its plain serde value (the generated
//! `MarginUtilization`, `ExcessCollateral`, `MarginFundingCost` and
//! `Haircut01` TypeScript types). The lower-camel-case function named after
//! the type is the twin of the Python constructor (the Rust `new` /
//! `calculate` factory); the remaining functions are the twins of the Python
//! methods and take the plain value.

use super::{js_currency, js_money};
use crate::utils::input::{from_js_json, js_f64};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_margin::metrics;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// MarginUtilization
// ---------------------------------------------------------------------------

/// Margin utilization: posted margin against required margin.
///
/// The wire carries the two amounts; the ratio is derived from them (see
/// `marginUtilizationRatio`).
/// @param postedAmount - Margin posted, in major units of `currency`; finite and non-negative.
/// @param requiredAmount - Margin required, in major units of `currency`; finite and non-negative.
/// @param currency - ISO-4217 currency of both amounts.
/// @returns The `MarginUtilization` (`posted`, `required`) as a plain object.
///
/// # Errors
///
/// Throws for an unknown currency or a non-finite or negative amount.
#[wasm_bindgen(js_name = marginUtilization, unchecked_return_type = "MarginUtilization")]
pub fn margin_utilization(
    #[wasm_bindgen(js_name = "postedAmount", unchecked_param_type = "number")]
    posted_amount: JsValue,
    #[wasm_bindgen(js_name = "requiredAmount", unchecked_param_type = "number")]
    required_amount: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = js_currency(&currency, "currency")?;
    let posted = js_money(&posted_amount, "postedAmount", currency)?;
    let required = js_money(&required_amount, "requiredAmount", currency)?;
    to_js_value(&metrics::MarginUtilization::new(posted, required).map_err(to_js_err)?)
}

/// Utilization ratio `posted / required`.
/// @param utilization - `MarginUtilization` (object or JSON).
/// @returns The ratio as a decimal; with nothing required it is `Infinity` when margin is posted and `1` otherwise.
///
/// # Errors
///
/// Throws if `utilization` is malformed, mixes currencies, or has a non-finite or negative amount.
#[wasm_bindgen(js_name = marginUtilizationRatio)]
pub fn margin_utilization_ratio(
    #[wasm_bindgen(unchecked_param_type = "MarginUtilization | string")] utilization: JsValue,
) -> Result<f64, JsValue> {
    let utilization: metrics::MarginUtilization = from_js_json(&utilization, "utilization")?;
    Ok(utilization.ratio)
}

/// Whether posted margin covers the requirement (`ratio >= 1`).
/// @param utilization - `MarginUtilization` (object or JSON).
/// @returns `true` when the posted margin is at least the required margin.
///
/// # Errors
///
/// Throws if `utilization` is malformed, mixes currencies, or has a non-finite or negative amount.
#[wasm_bindgen(js_name = marginUtilizationIsAdequate)]
pub fn margin_utilization_is_adequate(
    #[wasm_bindgen(unchecked_param_type = "MarginUtilization | string")] utilization: JsValue,
) -> Result<bool, JsValue> {
    let utilization: metrics::MarginUtilization = from_js_json(&utilization, "utilization")?;
    Ok(utilization.is_adequate())
}

/// Margin shortfall: `max(required - posted, 0)`.
/// @param utilization - `MarginUtilization` (object or JSON).
/// @returns The shortfall in major units of the utilization's currency; zero when adequately margined.
///
/// # Errors
///
/// Throws if `utilization` is malformed, mixes currencies, or has a non-finite or negative amount.
#[wasm_bindgen(js_name = marginUtilizationShortfall)]
pub fn margin_utilization_shortfall(
    #[wasm_bindgen(unchecked_param_type = "MarginUtilization | string")] utilization: JsValue,
) -> Result<f64, JsValue> {
    let utilization: metrics::MarginUtilization = from_js_json(&utilization, "utilization")?;
    Ok(utilization.shortfall().map_err(to_js_err)?.amount())
}

// ---------------------------------------------------------------------------
// ExcessCollateral
// ---------------------------------------------------------------------------

/// Excess collateral: collateral value against the required value.
///
/// `excess = collateral_value - required_value`; negative means a shortfall.
/// @param collateralValue - Collateral market value, in major units of `currency`.
/// @param requiredValue - Required collateral value, in major units of `currency`.
/// @param currency - ISO-4217 currency of both amounts.
/// @returns The `ExcessCollateral` (`collateral_value`, `required_value`, `excess`) as a plain object.
///
/// # Errors
///
/// Throws for an unknown currency or a non-finite amount.
#[wasm_bindgen(js_name = excessCollateral, unchecked_return_type = "ExcessCollateral")]
pub fn excess_collateral(
    #[wasm_bindgen(js_name = "collateralValue", unchecked_param_type = "number")] collateral_value: JsValue,
    #[wasm_bindgen(js_name = "requiredValue", unchecked_param_type = "number")]
    required_value: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = js_currency(&currency, "currency")?;
    let collateral = js_money(&collateral_value, "collateralValue", currency)?;
    let required = js_money(&required_value, "requiredValue", currency)?;
    to_js_value(&metrics::ExcessCollateral::new(collateral, required).map_err(to_js_err)?)
}

/// Whether collateral exceeds the requirement.
/// @param excessCollateral - `ExcessCollateral` (object or JSON).
/// @returns `true` when the excess amount is positive.
///
/// # Errors
///
/// Throws if `excess_collateral` is malformed.
#[wasm_bindgen(js_name = excessCollateralHasExcess)]
pub fn excess_collateral_has_excess(
    #[wasm_bindgen(
        js_name = "excessCollateral",
        unchecked_param_type = "ExcessCollateral | string"
    )]
    excess_collateral: JsValue,
) -> Result<bool, JsValue> {
    let value: metrics::ExcessCollateral = from_js_json(&excess_collateral, "excessCollateral")?;
    Ok(value.has_excess())
}

/// Whether collateral falls short of the requirement.
/// @param excessCollateral - `ExcessCollateral` (object or JSON).
/// @returns `true` when the excess amount is negative.
///
/// # Errors
///
/// Throws if `excess_collateral` is malformed.
#[wasm_bindgen(js_name = excessCollateralHasShortfall)]
pub fn excess_collateral_has_shortfall(
    #[wasm_bindgen(
        js_name = "excessCollateral",
        unchecked_param_type = "ExcessCollateral | string"
    )]
    excess_collateral: JsValue,
) -> Result<bool, JsValue> {
    let value: metrics::ExcessCollateral = from_js_json(&excess_collateral, "excessCollateral")?;
    Ok(value.has_shortfall())
}

/// Excess as a fraction of the required value.
/// @param excessCollateral - `ExcessCollateral` (object or JSON).
/// @returns `excess / required_value` as a decimal (0.05 is 5% over-collateralized); zero when nothing is required.
///
/// # Errors
///
/// Throws if `excess_collateral` is malformed.
#[wasm_bindgen(js_name = excessCollateralExcessPercentage)]
pub fn excess_collateral_excess_percentage(
    #[wasm_bindgen(
        js_name = "excessCollateral",
        unchecked_param_type = "ExcessCollateral | string"
    )]
    excess_collateral: JsValue,
) -> Result<f64, JsValue> {
    let value: metrics::ExcessCollateral = from_js_json(&excess_collateral, "excessCollateral")?;
    Ok(value.excess_percentage())
}

// ---------------------------------------------------------------------------
// MarginFundingCost
// ---------------------------------------------------------------------------

/// Annualized cost of funding posted margin.
///
/// `annual_cost = margin_posted * (funding_rate - collateral_rate)`.
/// @param marginPosted - Margin posted, in major units of `currency`.
/// @param fundingRate - Unsecured funding rate as an annualized decimal (0.05 is 5%).
/// @param collateralRate - Rate earned on posted collateral as an annualized decimal.
/// @param currency - ISO-4217 currency of the posted margin.
/// @returns The `MarginFundingCost` (`margin_posted`, `funding_rate`, `collateral_rate`, `annual_cost`) as a plain object.
///
/// # Errors
///
/// Throws for an unknown currency, a non-finite margin amount or funding
/// spread, or an annual cost outside the Decimal range.
#[wasm_bindgen(js_name = marginFundingCost, unchecked_return_type = "MarginFundingCost")]
pub fn margin_funding_cost(
    #[wasm_bindgen(js_name = "marginPosted", unchecked_param_type = "number")]
    margin_posted: JsValue,
    #[wasm_bindgen(js_name = "fundingRate", unchecked_param_type = "number")] funding_rate: JsValue,
    #[wasm_bindgen(js_name = "collateralRate", unchecked_param_type = "number")]
    collateral_rate: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = js_currency(&currency, "currency")?;
    let margin_posted = js_money(&margin_posted, "marginPosted", currency)?;
    let funding_rate = js_f64(&funding_rate, "fundingRate")?;
    let collateral_rate = js_f64(&collateral_rate, "collateralRate")?;
    to_js_value(
        &metrics::MarginFundingCost::calculate(margin_posted, funding_rate, collateral_rate)
            .map_err(to_js_err)?,
    )
}

/// Funding spread: funding rate minus collateral rate.
/// @param fundingCost - `MarginFundingCost` (object or JSON).
/// @returns The spread as an annualized decimal.
///
/// # Errors
///
/// Throws if `funding_cost` is malformed.
#[wasm_bindgen(js_name = marginFundingCostSpread)]
pub fn margin_funding_cost_spread(
    #[wasm_bindgen(
        js_name = "fundingCost",
        unchecked_param_type = "MarginFundingCost | string"
    )]
    funding_cost: JsValue,
) -> Result<f64, JsValue> {
    let value: metrics::MarginFundingCost = from_js_json(&funding_cost, "fundingCost")?;
    Ok(value.spread())
}

/// Funding cost over a period: `annual_cost * year_fraction`.
/// @param fundingCost - `MarginFundingCost` (object or JSON).
/// @param yearFraction - Length of the period in years.
/// @returns The cost in major units of the posted margin's currency.
///
/// # Errors
///
/// Throws if `funding_cost` is malformed, `year_fraction` is non-finite, or
/// the period cost is outside the Decimal range.
#[wasm_bindgen(js_name = marginFundingCostCostForPeriod)]
pub fn margin_funding_cost_cost_for_period(
    #[wasm_bindgen(
        js_name = "fundingCost",
        unchecked_param_type = "MarginFundingCost | string"
    )]
    funding_cost: JsValue,
    #[wasm_bindgen(js_name = "yearFraction", unchecked_param_type = "number")]
    year_fraction: JsValue,
) -> Result<f64, JsValue> {
    let value: metrics::MarginFundingCost = from_js_json(&funding_cost, "fundingCost")?;
    let year_fraction = js_f64(&year_fraction, "yearFraction")?;
    Ok(value
        .cost_for_period(year_fraction)
        .map_err(to_js_err)?
        .amount())
}

// ---------------------------------------------------------------------------
// Haircut01
// ---------------------------------------------------------------------------

/// Haircut01: PV change for a one basis point increase in the haircut.
/// @param collateralValue - Collateral market value, in major units of `currency`.
/// @param currentHaircut - Current haircut as a decimal fraction (0.02 is 2%).
/// @param currency - ISO-4217 currency of the collateral value.
/// @returns The `Haircut01` (`collateral_value`, `current_haircut`, `pv_change`) as a plain object.
///
/// # Errors
///
/// Throws for an unknown currency, a non-finite collateral value, or a
/// one-basis-point PV change outside the monetary amount range.
#[wasm_bindgen(js_name = haircut01, unchecked_return_type = "Haircut01")]
pub fn haircut01(
    #[wasm_bindgen(js_name = "collateralValue", unchecked_param_type = "number")] collateral_value: JsValue,
    #[wasm_bindgen(js_name = "currentHaircut", unchecked_param_type = "number")]
    current_haircut: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = js_currency(&currency, "currency")?;
    let collateral_value = js_money(&collateral_value, "collateralValue", currency)?;
    let current_haircut = js_f64(&current_haircut, "currentHaircut")?;
    to_js_value(
        &metrics::Haircut01::calculate(collateral_value, current_haircut).map_err(to_js_err)?,
    )
}

/// Current haircut expressed in basis points.
/// @param haircut01 - `Haircut01` (object or JSON).
/// @returns `current_haircut * 10_000` (200 for a 2% haircut).
///
/// # Errors
///
/// Throws if `haircut01` is malformed.
#[wasm_bindgen(js_name = haircut01HaircutBp)]
pub fn haircut01_haircut_bp(
    #[wasm_bindgen(unchecked_param_type = "Haircut01 | string")] haircut01: JsValue,
) -> Result<f64, JsValue> {
    let value: metrics::Haircut01 = from_js_json(&haircut01, "haircut01")?;
    Ok(value.haircut_bp())
}
