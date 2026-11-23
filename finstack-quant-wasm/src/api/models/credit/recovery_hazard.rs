//! Dynamic recovery and endogenous hazard WASM specifications.

use crate::utils::input::{js_f64, js_f64_seq};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::wire::serde_tag;
use finstack_quant_models::credit::{DynamicRecoverySpec, EndogenousHazardSpec};
use wasm_bindgen::prelude::*;

/// Notional-dependent recovery specification for PIK-accreting instruments.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const spec = models.credit.DynamicRecoverySpec.flooredInverse(0.4, 100, 0.1);
/// console.log(spec.kind, spec.recoveryAtNotional(150));
/// spec.free();
/// ```
#[wasm_bindgen(js_name = DynamicRecoverySpec)]
pub struct JsDynamicRecoverySpec {
    pub(crate) inner: DynamicRecoverySpec,
}

json_round_trip!(JsDynamicRecoverySpec, DynamicRecoverySpec);

wasm_getters!(JsDynamicRecoverySpec, DynamicRecoverySpec |s| {
    /// Canonical name of the active recovery model: `"constant"`,
    /// `"inverse_linear"`, `"inverse_power"`, `"floored_inverse"` or
    /// `"linear_decline"`.
    kind -> Result<String, JsValue> = serde_tag(s.inner.model()).map_err(to_js_err);
    /// Base (reference) recovery rate `R0`, as a fraction from 0 through 1.
    base_recovery as baseRecovery -> f64 = s.inner.base_recovery();
    /// Base (reference) notional `N0` the recovery mapping is anchored to.
    base_notional as baseNotional -> f64 = s.inner.base_notional();
    /// Notional-to-recovery mapping in canonical JSON form: a tag string for
    /// the parameterless models, or a single-key object carrying the parameters.
    model returns "generated.models.DynamicRecoveryModel" -> Result<JsValue, JsValue> = to_js_value(s.inner.model());
});

#[wasm_bindgen(js_class = DynamicRecoverySpec)]
impl JsDynamicRecoverySpec {
    /// Recovery that does not depend on notional.
    /// @param recovery - Recovery rate at default as a fraction of par from 0 through 1.
    /// @returns The constant recovery specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `recovery` is outside `[0, 1]`.
    pub fn constant(
        #[wasm_bindgen(unchecked_param_type = "number")] recovery: JsValue,
    ) -> Result<JsDynamicRecoverySpec, JsValue> {
        DynamicRecoverySpec::constant(js_f64(&recovery, "recovery")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Recovery `R0 * N0 / N`: fixed recoverable value spread over a larger claim.
    /// @param baseRecovery - Recovery rate `R0` at the base notional, as a fraction from 0 through 1.
    /// @param baseNotional - Positive reference notional `N0` in the instrument's currency.
    /// @returns The inverse-linear recovery specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    #[wasm_bindgen(js_name = inverseLinear)]
    pub fn inverse_linear(
        #[wasm_bindgen(js_name = "baseRecovery", unchecked_param_type = "number")]
        base_recovery: JsValue,
        #[wasm_bindgen(js_name = "baseNotional", unchecked_param_type = "number")]
        base_notional: JsValue,
    ) -> Result<JsDynamicRecoverySpec, JsValue> {
        DynamicRecoverySpec::inverse_linear(
            js_f64(&base_recovery, "baseRecovery")?,
            js_f64(&base_notional, "baseNotional")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Recovery `R0 * (N0 / N)^exponent`.
    /// @param baseRecovery - Recovery rate `R0` at the base notional, as a fraction from 0 through 1.
    /// @param baseNotional - Positive reference notional `N0` in the instrument's currency.
    /// @param exponent - Non-negative power applied to the notional ratio.
    /// @returns The inverse-power recovery specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    #[wasm_bindgen(js_name = inversePower)]
    pub fn inverse_power(
        #[wasm_bindgen(js_name = "baseRecovery", unchecked_param_type = "number")]
        base_recovery: JsValue,
        #[wasm_bindgen(js_name = "baseNotional", unchecked_param_type = "number")]
        base_notional: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] exponent: JsValue,
    ) -> Result<JsDynamicRecoverySpec, JsValue> {
        DynamicRecoverySpec::inverse_power(
            js_f64(&base_recovery, "baseRecovery")?,
            js_f64(&base_notional, "baseNotional")?,
            js_f64(&exponent, "exponent")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Inverse-linear recovery that never falls below a floor.
    /// @param baseRecovery - Recovery rate `R0` at the base notional, as a fraction from 0 through 1.
    /// @param baseNotional - Positive reference notional `N0` in the instrument's currency.
    /// @param floor - Minimum recovery as a fraction in `[0, baseRecovery]`.
    /// @returns The floored-inverse recovery specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    #[wasm_bindgen(js_name = flooredInverse)]
    pub fn floored_inverse(
        #[wasm_bindgen(js_name = "baseRecovery", unchecked_param_type = "number")]
        base_recovery: JsValue,
        #[wasm_bindgen(js_name = "baseNotional", unchecked_param_type = "number")]
        base_notional: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] floor: JsValue,
    ) -> Result<JsDynamicRecoverySpec, JsValue> {
        DynamicRecoverySpec::floored_inverse(
            js_f64(&base_recovery, "baseRecovery")?,
            js_f64(&base_notional, "baseNotional")?,
            js_f64(&floor, "floor")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Recovery `clamp(R0 * (1 - sensitivity * (N / N0 - 1)), floor, R0)`.
    /// @param baseRecovery - Recovery rate `R0` at the base notional, as a fraction from 0 through 1.
    /// @param baseNotional - Positive reference notional `N0` in the instrument's currency.
    /// @param sensitivity - Non-negative dimensionless relative recovery sensitivity per unit increase of `N / N0 - 1`.
    /// @param floor - Minimum recovery as a fraction in `[0, baseRecovery]`.
    /// @returns The linear-decline recovery specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    #[wasm_bindgen(js_name = linearDecline)]
    pub fn linear_decline(
        #[wasm_bindgen(js_name = "baseRecovery", unchecked_param_type = "number")]
        base_recovery: JsValue,
        #[wasm_bindgen(js_name = "baseNotional", unchecked_param_type = "number")]
        base_notional: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] sensitivity: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] floor: JsValue,
    ) -> Result<JsDynamicRecoverySpec, JsValue> {
        DynamicRecoverySpec::linear_decline(
            js_f64(&base_recovery, "baseRecovery")?,
            js_f64(&base_notional, "baseNotional")?,
            js_f64(&sensitivity, "sensitivity")?,
            js_f64(&floor, "floor")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Recovery rate at a given current notional, clamped to `[0, baseRecovery]`.
    /// @param notional - Current (accreted) notional in the instrument's currency.
    /// @returns Recovery rate as a fraction of par.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `notional` is not a number.
    #[wasm_bindgen(js_name = recoveryAtNotional)]
    pub fn recovery_at_notional(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] notional: JsValue,
    ) -> Result<f64, JsValue> {
        Ok(self
            .inner
            .recovery_at_notional(js_f64(&notional, "notional")?))
    }
}

/// Leverage-dependent hazard-rate feedback for PIK-accreting instruments.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const spec = models.credit.EndogenousHazardSpec.powerLaw(0.05, 0.6, 2.0);
/// console.log(spec.kind, spec.hazardAtLeverage(0.8));
/// spec.free();
/// ```
#[wasm_bindgen(js_name = EndogenousHazardSpec)]
pub struct JsEndogenousHazardSpec {
    pub(crate) inner: EndogenousHazardSpec,
}

json_round_trip!(JsEndogenousHazardSpec, EndogenousHazardSpec);

wasm_getters!(JsEndogenousHazardSpec, EndogenousHazardSpec |s| {
    /// Canonical name of the active mapping: `"power_law"`, `"exponential"` or
    /// `"tabular"`.
    kind -> Result<String, JsValue> = serde_tag(s.inner.leverage_hazard_map()).map_err(to_js_err);
    /// Base (reference) hazard rate, annualized, as a decimal.
    base_hazard_rate as baseHazardRate -> f64 = s.inner.base_hazard_rate();
    /// Base (reference) leverage the hazard mapping is anchored to.
    base_leverage as baseLeverage -> f64 = s.inner.base_leverage();
    /// Leverage-to-hazard mapping in canonical JSON form: a single-key object
    /// (`power_law`, `exponential`, `tabular`) carrying that model's parameters.
    leverage_hazard_map as leverageHazardMap returns "generated.models.LeverageHazardMap" -> Result<JsValue, JsValue> = to_js_value(s.inner.leverage_hazard_map());
});

#[wasm_bindgen(js_class = EndogenousHazardSpec)]
impl JsEndogenousHazardSpec {
    /// Power-law mapping `lambda(L) = baseHazard * (L / baseLeverage)^exponent`.
    /// @param baseHazard - Annualized hazard rate at the base leverage, as a decimal; non-negative.
    /// @param baseLeverage - Positive reference debt-to-assets leverage ratio.
    /// @param exponent - Power-law exponent applied to the leverage ratio.
    /// @returns The power-law hazard specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    #[wasm_bindgen(js_name = powerLaw)]
    pub fn power_law(
        #[wasm_bindgen(js_name = "baseHazard", unchecked_param_type = "number")]
        base_hazard: JsValue,
        #[wasm_bindgen(js_name = "baseLeverage", unchecked_param_type = "number")]
        base_leverage: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] exponent: JsValue,
    ) -> Result<JsEndogenousHazardSpec, JsValue> {
        EndogenousHazardSpec::power_law(
            js_f64(&base_hazard, "baseHazard")?,
            js_f64(&base_leverage, "baseLeverage")?,
            js_f64(&exponent, "exponent")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Exponential mapping `lambda(L) = baseHazard * exp(sensitivity * (L - baseLeverage))`.
    /// @param baseHazard - Annualized hazard rate at the base leverage, as a decimal; non-negative.
    /// @param baseLeverage - Positive reference debt-to-assets leverage ratio.
    /// @param sensitivity - Log-hazard change per unit of leverage above the base.
    /// @returns The exponential hazard specification.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error on non-finite or out-of-range inputs.
    pub fn exponential(
        #[wasm_bindgen(js_name = "baseHazard", unchecked_param_type = "number")]
        base_hazard: JsValue,
        #[wasm_bindgen(js_name = "baseLeverage", unchecked_param_type = "number")]
        base_leverage: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] sensitivity: JsValue,
    ) -> Result<JsEndogenousHazardSpec, JsValue> {
        EndogenousHazardSpec::exponential(
            js_f64(&base_hazard, "baseHazard")?,
            js_f64(&base_leverage, "baseLeverage")?,
            js_f64(&sensitivity, "sensitivity")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Piecewise-linear leverage-to-hazard table.
    /// @param leveragePoints - One or more finite non-negative leverage ratios, strictly increasing when multiple points are supplied; a single point gives constant hazard.
    /// @param hazardPoints - Annualized hazard rates at those leverage points; same length, non-negative.
    /// @returns The tabular hazard specification, with base leverage and hazard taken from the first pair.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the arrays differ in length, are empty,
    /// are not strictly increasing in leverage, or contain
    /// non-finite or negative values.
    pub fn tabular(
        #[wasm_bindgen(js_name = "leveragePoints", unchecked_param_type = "NumericArray")]
        leverage_points: JsValue,
        #[wasm_bindgen(js_name = "hazardPoints", unchecked_param_type = "NumericArray")]
        hazard_points: JsValue,
    ) -> Result<JsEndogenousHazardSpec, JsValue> {
        EndogenousHazardSpec::tabular(
            js_f64_seq(&leverage_points, "leveragePoints")?,
            js_f64_seq(&hazard_points, "hazardPoints")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Annualized hazard rate at a leverage ratio, floored at zero.
    /// @param leverage - Debt-to-assets leverage ratio; non-negative.
    /// @returns Hazard rate per year, as a decimal.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `leverage` is not a number.
    #[wasm_bindgen(js_name = hazardAtLeverage)]
    pub fn hazard_at_leverage(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] leverage: JsValue,
    ) -> Result<f64, JsValue> {
        Ok(self
            .inner
            .hazard_at_leverage(js_f64(&leverage, "leverage")?))
    }

    /// Hazard rate after PIK accretion, with leverage `accretedNotional / assetValue`.
    /// @param accretedNotional - Notional outstanding after PIK accrual, in the instrument's currency.
    /// @param assetValue - Firm asset value in the same currency; strictly positive.
    /// @returns Hazard rate per year, as a decimal.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number.
    #[wasm_bindgen(js_name = hazardAfterPikAccrual)]
    pub fn hazard_after_pik_accrual(
        &self,
        #[wasm_bindgen(js_name = "accretedNotional", unchecked_param_type = "number")]
        accreted_notional: JsValue,
        #[wasm_bindgen(js_name = "assetValue", unchecked_param_type = "number")]
        asset_value: JsValue,
    ) -> Result<f64, JsValue> {
        Ok(self.inner.hazard_after_pik_accrual(
            js_f64(&accreted_notional, "accretedNotional")?,
            js_f64(&asset_value, "assetValue")?,
        ))
    }
}
