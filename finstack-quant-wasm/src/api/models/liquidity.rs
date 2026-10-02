//! WASM bindings for product-independent liquidity models.

use crate::utils::input::{from_js_json, js_f64, js_f64_seq, js_opt_f64, js_uint};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::liquidity::{
    self, AlmgrenChrissModel, KyleLambdaModel, LiquidityProfile, TradeParams,
};
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
pub fn days_to_liquidate(
    position_quantity: JsValue,
    adv: JsValue,
    participation_rate: JsValue,
) -> Result<f64, JsValue> {
    let position_quantity = js_f64(&position_quantity, "positionQuantity")?;
    let adv = js_f64(&adv, "adv")?;
    let participation_rate = js_f64(&participation_rate, "participationRate")?;
    Ok(liquidity::days_to_liquidate(
        position_quantity,
        adv,
        participation_rate,
    ))
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
    days_to_liquidate: JsValue,
    thresholds: Option<JsValue>,
) -> Result<String, JsValue> {
    let days_to_liquidate = js_f64(&days_to_liquidate, "daysToLiquidate")?;
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
    var: JsValue,
    spread_mean: JsValue,
    spread_vol: JsValue,
    confidence: JsValue,
    position_value: JsValue,
) -> Result<JsValue, JsValue> {
    let var = js_f64(&var, "varValue")?;
    let spread_mean = js_f64(&spread_mean, "spreadMean")?;
    let spread_vol = js_f64(&spread_vol, "spreadVol")?;
    let confidence = js_f64(&confidence, "confidence")?;
    let position_value = js_f64(&position_value, "positionValue")?;
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
    position_size: JsValue,
    avg_daily_volume: JsValue,
    volatility: JsValue,
    execution_horizon_days: JsValue,
    permanent_impact_coef: JsValue,
    temporary_impact_coef: JsValue,
    reference_price: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let position_size = js_f64(&position_size, "positionSize")?;
    let avg_daily_volume = js_f64(&avg_daily_volume, "avgDailyVolume")?;
    let volatility = js_f64(&volatility, "volatility")?;
    let execution_horizon_days = js_f64(&execution_horizon_days, "executionHorizonDays")?;
    let permanent_impact_coef = js_f64(&permanent_impact_coef, "permanentImpactCoef")?;
    let temporary_impact_coef = js_f64(&temporary_impact_coef, "temporaryImpactCoef")?;
    let reference_price = js_opt_f64(reference_price.as_ref(), "referencePrice")?;
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
    reference_price: JsValue,
) -> Result<Option<f64>, JsValue> {
    let reference_price = js_f64(&reference_price, "referencePrice")?;
    let returns = js_f64_seq(&returns, "returns")?;
    let volumes = js_f64_seq(&volumes, "volumes")?;
    Ok(KyleLambdaModel::lambda_from_series(
        &returns,
        &volumes,
        reference_price,
    ))
}

/// Almgren-Chriss market-impact model with linear permanent impact and
/// power-law temporary impact.
#[wasm_bindgen(js_name = AlmgrenChrissModel)]
pub struct JsAlmgrenChrissModel {
    pub(crate) inner: AlmgrenChrissModel,
}

json_round_trip!(JsAlmgrenChrissModel, AlmgrenChrissModel);

#[wasm_bindgen(js_class = AlmgrenChrissModel)]
impl JsAlmgrenChrissModel {
    /// Model from explicit impact coefficients.
    /// @param gamma - Permanent-impact coefficient: price move per unit traded; non-negative.
    /// @param eta - Temporary-impact coefficient: price concession per unit of trading rate; non-negative.
    /// @param delta - Temporary-impact exponent on the trading rate, in `(0, 1]`; 1 is the linear model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if a coefficient is negative, non-finite, or
    /// `delta` is outside `(0, 1]`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        gamma: JsValue,
        eta: JsValue,
        delta: JsValue,
    ) -> Result<JsAlmgrenChrissModel, JsValue> {
        AlmgrenChrissModel::new(
            js_f64(&gamma, "gamma")?,
            js_f64(&eta, "eta")?,
            js_f64(&delta, "delta")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Calibrate the impact coefficients from a liquidity profile.
    /// @param profile - `LiquidityProfile` object or JSON (`instrument_id`, `mid`, `bid`, `ask`, `avg_daily_volume`, `avg_trade_size`, `spread_volatility`, `spread_volatility_kind`, `observation_days`).
    /// @param daily_volatility - Daily return volatility as a decimal; positive.
    /// @returns The calibrated model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the profile is malformed or the
    /// volatility or volume is not positive.
    #[wasm_bindgen(js_name = fromProfile)]
    pub fn from_profile(
        profile: JsValue,
        daily_volatility: JsValue,
    ) -> Result<JsAlmgrenChrissModel, JsValue> {
        let profile: LiquidityProfile = from_js_json(&profile, "profile")?;
        AlmgrenChrissModel::from_profile(&profile, js_f64(&daily_volatility, "dailyVolatility")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Permanent-impact coefficient.
    #[wasm_bindgen(getter)]
    pub fn gamma(&self) -> f64 {
        self.inner.gamma()
    }

    /// Temporary-impact coefficient.
    #[wasm_bindgen(getter)]
    pub fn eta(&self) -> f64 {
        self.inner.eta()
    }

    /// Temporary-impact exponent on the trading rate.
    #[wasm_bindgen(getter)]
    pub fn delta(&self) -> f64 {
        self.inner.delta()
    }

    /// Model name for diagnostics.
    #[wasm_bindgen(getter, js_name = modelName)]
    pub fn model_name(&self) -> String {
        self.inner.model_name().to_string()
    }

    /// Expected cost and risk of executing a trade at a uniform rate.
    /// @param params - `TradeParams` object or JSON: `quantity` (signed, in units), `horizon_days`, `daily_volatility`, `profile` (a `LiquidityProfile`), and optional `risk_aversion` and `reference_price`.
    /// @returns The `ImpactEstimate` object (`permanent_impact`, `temporary_impact`, `total_cost`, `cost_bp`, `execution_risk`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the parameters are malformed or out of range.
    #[wasm_bindgen(js_name = estimateCost)]
    pub fn estimate_cost(&self, params: JsValue) -> Result<JsValue, JsValue> {
        let params: TradeParams = from_js_json(&params, "params")?;
        to_js_value(&self.inner.estimate_cost(&params).map_err(to_js_err)?)
    }

    /// Risk-averse optimal execution schedule (Almgren-Chriss 2000).
    /// @param params - `TradeParams` object or JSON; `risk_aversion` selects the urgency of the schedule.
    /// @param num_buckets - Number of equal time buckets over the horizon; a positive safe integer.
    /// @returns The `ExecutionTrajectory` object (`quantities`, `remaining`, `time_points`, `expected_cost`, `cost_variance`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the parameters are malformed or
    /// `numBuckets` is zero.
    #[wasm_bindgen(js_name = optimalTrajectory)]
    pub fn optimal_trajectory(
        &self,
        params: JsValue,
        num_buckets: JsValue,
    ) -> Result<JsValue, JsValue> {
        let params: TradeParams = from_js_json(&params, "params")?;
        let trajectory = self
            .inner
            .optimal_trajectory(&params, js_uint(&num_buckets, "numBuckets")?)
            .map_err(to_js_err)?;
        to_js_value(&trajectory)
    }
}

/// Kyle (1985) linear price-impact model.
#[wasm_bindgen(js_name = KyleLambdaModel)]
pub struct JsKyleLambdaModel {
    pub(crate) inner: KyleLambdaModel,
}

json_round_trip!(JsKyleLambdaModel, KyleLambdaModel);

#[wasm_bindgen(js_class = KyleLambdaModel)]
impl JsKyleLambdaModel {
    /// Model from an explicit price-impact slope.
    /// @param lambda - Kyle's lambda: price change per unit of signed order flow; non-negative.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `lambda` is negative or non-finite.
    #[wasm_bindgen(constructor)]
    pub fn new(lambda: JsValue) -> Result<JsKyleLambdaModel, JsValue> {
        KyleLambdaModel::new(js_f64(&lambda, "lambda")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Model whose lambda is implied by an Amihud illiquidity ratio.
    /// @param amihud_ratio - Amihud ratio: average absolute return per unit of traded value; non-negative.
    /// @param reference_price - Reference price used to turn the return impact into a price impact; positive.
    /// @returns The Kyle model with `lambda = amihudRatio * referencePrice`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is negative or non-finite.
    #[wasm_bindgen(js_name = fromAmihud)]
    pub fn from_amihud(
        amihud_ratio: JsValue,
        reference_price: JsValue,
    ) -> Result<JsKyleLambdaModel, JsValue> {
        KyleLambdaModel::from_amihud(
            js_f64(&amihud_ratio, "amihudRatio")?,
            js_f64(&reference_price, "referencePrice")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Kyle's lambda: price change per unit of signed order flow (Python `lambda_`).
    #[wasm_bindgen(getter)]
    pub fn lambda(&self) -> f64 {
        self.inner.lambda()
    }

    /// Model name for diagnostics.
    #[wasm_bindgen(getter, js_name = modelName)]
    pub fn model_name(&self) -> String {
        self.inner.model_name().to_string()
    }

    /// Expected cost and risk of executing a trade at a uniform rate.
    /// @param params - `TradeParams` object or JSON: `quantity` (signed, in units), `horizon_days`, `daily_volatility`, `profile` (a `LiquidityProfile`), and optional `risk_aversion` and `reference_price`.
    /// @returns The `ImpactEstimate` object (`permanent_impact`, `temporary_impact`, `total_cost`, `cost_bp`, `execution_risk`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the parameters are malformed or out of range.
    #[wasm_bindgen(js_name = estimateCost)]
    pub fn estimate_cost(&self, params: JsValue) -> Result<JsValue, JsValue> {
        let params: TradeParams = from_js_json(&params, "params")?;
        to_js_value(&self.inner.estimate_cost(&params).map_err(to_js_err)?)
    }

    /// Execution schedule under linear impact.
    /// @param params - `TradeParams` object or JSON describing the trade.
    /// @param num_buckets - Number of equal time buckets over the horizon; a positive safe integer.
    /// @returns The `ExecutionTrajectory` object (`quantities`, `remaining`, `time_points`, `expected_cost`, `cost_variance`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the parameters are malformed or
    /// `numBuckets` is zero.
    #[wasm_bindgen(js_name = optimalTrajectory)]
    pub fn optimal_trajectory(
        &self,
        params: JsValue,
        num_buckets: JsValue,
    ) -> Result<JsValue, JsValue> {
        let params: TradeParams = from_js_json(&params, "params")?;
        let trajectory = self
            .inner
            .optimal_trajectory(&params, js_uint(&num_buckets, "numBuckets")?)
            .map_err(to_js_err)?;
        to_js_value(&trajectory)
    }
}
