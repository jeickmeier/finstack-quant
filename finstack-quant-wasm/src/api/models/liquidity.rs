//! WASM bindings for product-independent liquidity models.

use crate::utils::input::js_f64_seq;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::liquidity::{self, KyleLambdaModel};
use wasm_bindgen::prelude::*;

/// Estimate the effective bid-ask spread using Roll's serial-covariance model.
/// @param returns - Decimal returns in time order, as a `number[]` or `Float64Array`.
/// @returns Effective spread in return units, or `undefined` when it cannot be estimated.
///
/// # Errors
///
/// Throws a `TypeError` if `returns` is not an array of numbers. Invalid
/// estimator samples return `undefined`.
#[wasm_bindgen(js_name = rollEffectiveSpread)]
pub fn roll_effective_spread(returns: JsValue) -> Result<Option<f64>, JsValue> {
    let returns = js_f64_seq(&returns, "returns")?;
    Ok(liquidity::roll_effective_spread(&returns))
}

/// Compute the Amihud illiquidity ratio from aligned returns and volumes.
/// @param returns - Decimal returns in time order, as a `number[]` or `Float64Array`.
/// @param volumes - Positive traded volumes aligned with `returns`, as a `number[]` or `Float64Array`.
/// @returns Mean absolute return per unit volume, or `undefined` for an invalid sample.
///
/// # Errors
///
/// Throws a `TypeError` if either argument is not an array of numbers.
/// Invalid estimator samples return `undefined`.
#[wasm_bindgen(js_name = amihudIlliquidity)]
pub fn amihud_illiquidity(returns: JsValue, volumes: JsValue) -> Result<Option<f64>, JsValue> {
    let returns = js_f64_seq(&returns, "returns")?;
    let volumes = js_f64_seq(&volumes, "volumes")?;
    Ok(liquidity::amihud_illiquidity(&returns, &volumes))
}

/// Calculate the trading days required to liquidate a position.
/// @param positionQuantity - Shares or contracts to liquidate; the absolute value is used.
/// @param adv - Average daily volume in the same quantity units.
/// @param participationRate - Fraction of ADV available for execution each trading day.
/// @returns Liquidation horizon in trading days, or infinity for non-positive capacity.
#[wasm_bindgen(js_name = daysToLiquidate)]
pub fn days_to_liquidate(position_quantity: f64, adv: f64, participation_rate: f64) -> f64 {
    liquidity::days_to_liquidate(position_quantity, adv, participation_rate)
}

/// Classify a liquidation horizon into a liquidity tier.
/// @param daysToLiquidate - Estimated unwind horizon in trading days.
/// @param thresholds - Optional upper bounds of Tiers 1-4 in trading days, `[tier1Max, tier2Max, tier3Max, tier4Max]`; omitted or `null` uses the Rust `LiquidityConfig` default `[1, 5, 20, 60]`.
/// @returns One of `tier1` through `tier5`, with Tier 1 the most liquid.
///
/// # Errors
///
/// Throws a `TypeError` if `thresholds` is not an array of four numbers, and
/// a `validation` error if a threshold is non-finite or not positive, or the
/// thresholds are not strictly ascending.
#[wasm_bindgen(js_name = liquidityTier)]
pub fn liquidity_tier(
    days_to_liquidate: f64,
    thresholds: Option<JsValue>,
) -> Result<String, JsValue> {
    let thresholds = match thresholds {
        Some(value) if !(value.is_null() || value.is_undefined()) => {
            let values = js_f64_seq(&value, "thresholds")?;
            let count = values.len();
            Some(<[f64; 4]>::try_from(values).map_err(|_| {
                crate::utils::input::invalid_type(
                    "thresholds",
                    &format!("expected 4 tier thresholds, got {count}"),
                )
            })?)
        }
        _ => None,
    };
    liquidity::liquidity_tier(days_to_liquidate, thresholds)
        .map(|tier| tier.as_binding_str().to_string())
        .map_err(to_js_err)
}

/// Compute Bangia liquidity-adjusted VaR under the loss-sign convention.
/// @param var - Finite non-positive base VaR; a negative value denotes a loss.
/// @param spreadMean - Finite non-negative mean relative bid-ask spread as a decimal.
/// @param spreadVol - Finite non-negative volatility of the relative spread.
/// @param confidence - Confidence level strictly between 0.5 and 1.
/// @param positionValue - Finite current market value; only its magnitude is used.
/// @returns An object containing `var`, `spread_cost`, `lvar`, and `lvar_ratio`.
///
/// # Errors
///
/// Throws a JavaScript exception if an input violates the stated finiteness,
/// sign, or range contract, or if the result cannot be converted.
#[wasm_bindgen(js_name = lvarBangia)]
pub fn lvar_bangia(
    var: f64,
    spread_mean: f64,
    spread_vol: f64,
    confidence: f64,
    position_value: f64,
) -> Result<JsValue, JsValue> {
    let result =
        liquidity::lvar_bangia_scalar(var, spread_mean, spread_vol, confidence, position_value)
            .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Estimate uniform Almgren-Chriss execution-impact components.
/// @param positionSize - Finite signed quantity in shares or contracts.
/// @param avgDailyVolume - Positive finite ADV in matching quantity units.
/// @param volatility - Positive finite daily volatility as a decimal.
/// @param executionHorizonDays - Positive finite execution horizon in trading days.
/// @param permanentImpactCoef - Non-negative finite multiplier on permanent impact.
/// @param temporaryImpactCoef - Positive finite multiplier on temporary impact.
/// @param referencePrice - Optional positive finite price for notional and basis-point scaling.
/// @returns Permanent, temporary, total, basis-point, and execution-risk impact fields.
///
/// # Errors
///
/// Throws a JavaScript exception if an input violates the stated finiteness,
/// sign, or range contract, calculation fails, or conversion fails.
#[wasm_bindgen(js_name = almgrenChrissImpact)]
#[allow(clippy::too_many_arguments)]
pub fn almgren_chriss_impact(
    position_size: f64,
    avg_daily_volume: f64,
    volatility: f64,
    execution_horizon_days: f64,
    permanent_impact_coef: f64,
    temporary_impact_coef: f64,
    reference_price: Option<f64>,
) -> Result<JsValue, JsValue> {
    let estimate = liquidity::almgren_chriss_uniform_impact(
        position_size,
        avg_daily_volume,
        volatility,
        execution_horizon_days,
        permanent_impact_coef,
        temporary_impact_coef,
        reference_price,
    )
    .map_err(to_js_err)?;
    to_js_value(&estimate)
}

/// Estimate price-space Kyle lambda using an Amihud-ratio proxy.
///
/// Argument order matches `amihudIlliquidity`: returns first, then volumes.
/// @param returns - Decimal returns in time order, as a `number[]` or `Float64Array`.
/// @param volumes - Positive volume observations aligned with `returns`, as a `number[]` or `Float64Array`.
/// @param referencePrice - Positive price per share or contract.
/// @returns Estimated price-space impact coefficient, or `undefined` for invalid inputs.
///
/// # Errors
///
/// Throws a `TypeError` if `returns` or `volumes` is not an array of numbers.
/// Invalid estimator samples return `undefined`.
#[wasm_bindgen(js_name = kyleLambda)]
pub fn kyle_lambda(
    returns: JsValue,
    volumes: JsValue,
    reference_price: f64,
) -> Result<Option<f64>, JsValue> {
    let returns = js_f64_seq(&returns, "returns")?;
    let volumes = js_f64_seq(&volumes, "volumes")?;
    Ok(KyleLambdaModel::lambda_from_series(
        &returns,
        &volumes,
        reference_price,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_without_thresholds_uses_the_rust_default() {
        assert_eq!(liquidity_tier(3.0, None).expect("default tiers"), "tier2");
    }
}
