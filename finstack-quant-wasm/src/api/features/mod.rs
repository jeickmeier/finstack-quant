//! WASM bindings for vectorized panel feature transforms.
//!
//! The binding accepts JavaScript arrays/objects, converts them into the Rust
//! crate's canonical inputs, and delegates all transform behavior to
//! `finstack-quant-features`.

use crate::utils::{to_js_err, to_js_value};
use serde_json::Value;
use wasm_bindgen::prelude::*;

/// Transform a time-series panel column per entity.
#[wasm_bindgen(js_name = transformTimeseries)]
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param entity - Entity identifier used to group ordered time-series observations.
/// @param order - Observation-order key used to sort each entity time series.
/// @param op - Transformation operation identifier supported by the feature-engineering API.
/// @param params - Operation-specific parameter object defining transformation settings.
pub fn transform_timeseries(
    values: JsValue,
    entity: JsValue,
    order: JsValue,
    op: &str,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values: Vec<Option<f64>> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    let entity: Vec<String> = serde_wasm_bindgen::from_value(entity).map_err(to_js_err)?;
    let order: Vec<String> = serde_wasm_bindgen::from_value(order).map_err(to_js_err)?;
    let params = parse_params(params)?;
    let result = finstack_quant_features::transform_timeseries(
        &values,
        &entity,
        &order,
        op,
        params.as_ref(),
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Transform a cross-section per timestamp.
#[wasm_bindgen(js_name = transformCrossSectional)]
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param time_key - Cross-sectional time key shared by values evaluated in the same slice.
/// @param op - Transformation operation identifier supported by the feature-engineering API.
/// @param params - Operation-specific parameter object defining transformation settings.
pub fn transform_cross_sectional(
    values: JsValue,
    time_key: JsValue,
    op: &str,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values: Vec<Option<f64>> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    let time_key: Vec<String> = serde_wasm_bindgen::from_value(time_key).map_err(to_js_err)?;
    let params = parse_params(params)?;
    let result =
        finstack_quant_features::transform_cross_sectional(&values, &time_key, op, params.as_ref())
            .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Transform a cross-section within each time/group sub-partition.
#[wasm_bindgen(js_name = transformCrossSectionalGrouped)]
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param time_key - Cross-sectional time key shared by values evaluated in the same slice.
/// @param groups - Group labels aligned with values for within-group cross-sectional operations.
/// @param op - Transformation operation identifier supported by the feature-engineering API.
/// @param params - Operation-specific parameter object defining transformation settings.
pub fn transform_cross_sectional_grouped(
    values: JsValue,
    time_key: JsValue,
    groups: JsValue,
    op: &str,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values: Vec<Option<f64>> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    let time_key: Vec<String> = serde_wasm_bindgen::from_value(time_key).map_err(to_js_err)?;
    let groups: Vec<String> = serde_wasm_bindgen::from_value(groups).map_err(to_js_err)?;
    let params = parse_params(params)?;
    let result = finstack_quant_features::transform_cross_sectional_grouped(
        &values,
        &time_key,
        &groups,
        op,
        params.as_ref(),
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Remove cross-sectional exposure effects by OLS residualization.
#[wasm_bindgen(js_name = neutralize)]
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param time_key - Cross-sectional time key shared by values evaluated in the same slice.
/// @param exposures - Factor-exposure matrix aligned with the supplied observations.
/// @param params - Operation-specific parameter object defining transformation settings.
pub fn neutralize(
    values: JsValue,
    time_key: JsValue,
    exposures: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values: Vec<Option<f64>> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    let time_key: Vec<String> = serde_wasm_bindgen::from_value(time_key).map_err(to_js_err)?;
    let exposures: Vec<Vec<Option<f64>>> =
        serde_wasm_bindgen::from_value(exposures).map_err(to_js_err)?;
    let params = parse_params(params)?;
    let result =
        finstack_quant_features::neutralize(&values, &time_key, &exposures, params.as_ref())
            .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Transform two time-series panel columns per entity.
#[wasm_bindgen(js_name = transformTimeseriesPairwise)]
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param other - Second value series aligned with the primary series for a pairwise transformation.
/// @param entity - Entity identifier used to group ordered time-series observations.
/// @param order - Observation-order key used to sort each entity time series.
/// @param op - Transformation operation identifier supported by the feature-engineering API.
/// @param params - Operation-specific parameter object defining transformation settings.
pub fn transform_timeseries_pairwise(
    values: JsValue,
    other: JsValue,
    entity: JsValue,
    order: JsValue,
    op: &str,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values: Vec<Option<f64>> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    let other: Vec<Option<f64>> = serde_wasm_bindgen::from_value(other).map_err(to_js_err)?;
    let entity: Vec<String> = serde_wasm_bindgen::from_value(entity).map_err(to_js_err)?;
    let order: Vec<String> = serde_wasm_bindgen::from_value(order).map_err(to_js_err)?;
    let params = parse_params(params)?;
    let result = finstack_quant_features::transform_timeseries_pairwise(
        &values,
        &other,
        &entity,
        &order,
        op,
        params.as_ref(),
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Return rolling OLS residuals per entity.
#[wasm_bindgen(js_name = rollingRegressionResidual)]
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param exposures - Factor-exposure matrix aligned with the supplied observations.
/// @param entity - Entity identifier used to group ordered time-series observations.
/// @param order - Observation-order key used to sort each entity time series.
/// @param params - Operation-specific parameter object defining transformation settings.
pub fn rolling_regression_residual(
    values: JsValue,
    exposures: JsValue,
    entity: JsValue,
    order: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values: Vec<Option<f64>> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    let exposures: Vec<Vec<Option<f64>>> =
        serde_wasm_bindgen::from_value(exposures).map_err(to_js_err)?;
    let entity: Vec<String> = serde_wasm_bindgen::from_value(entity).map_err(to_js_err)?;
    let order: Vec<String> = serde_wasm_bindgen::from_value(order).map_err(to_js_err)?;
    let params = parse_params(params)?;
    let result = finstack_quant_features::rolling_regression_residual(
        &values,
        &exposures,
        &entity,
        &order,
        params.as_ref(),
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Convert a signal to inverse-risk-scaled weights per timestamp.
#[wasm_bindgen(js_name = riskScaledWeights)]
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param time_key - Cross-sectional time key shared by values evaluated in the same slice.
/// @param volatility - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param params - Operation-specific parameter object defining transformation settings.
pub fn risk_scaled_weights(
    values: JsValue,
    time_key: JsValue,
    volatility: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values: Vec<Option<f64>> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    let time_key: Vec<String> = serde_wasm_bindgen::from_value(time_key).map_err(to_js_err)?;
    let volatility: Vec<Option<f64>> =
        serde_wasm_bindgen::from_value(volatility).map_err(to_js_err)?;
    let params = parse_params(params)?;
    let result = finstack_quant_features::risk_scaled_weights(
        &values,
        &time_key,
        &volatility,
        params.as_ref(),
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Apply the default signal cleaning pass.
#[wasm_bindgen(js_name = cleanSignal)]
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param time_key - Cross-sectional time key shared by values evaluated in the same slice.
/// @param params - Operation-specific parameter object defining transformation settings.
pub fn clean_signal(
    values: JsValue,
    time_key: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values: Vec<Option<f64>> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    let time_key: Vec<String> = serde_wasm_bindgen::from_value(time_key).map_err(to_js_err)?;
    let params = parse_params(params)?;
    let result = finstack_quant_features::clean_signal(&values, &time_key, params.as_ref())
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Normalize a signal cross-sectionally.
#[wasm_bindgen(js_name = normalizeSignal)]
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param time_key - Cross-sectional time key shared by values evaluated in the same slice.
/// @param params - Operation-specific parameter object defining transformation settings.
pub fn normalize_signal(
    values: JsValue,
    time_key: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values: Vec<Option<f64>> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    let time_key: Vec<String> = serde_wasm_bindgen::from_value(time_key).map_err(to_js_err)?;
    let params = parse_params(params)?;
    let result = finstack_quant_features::normalize_signal(&values, &time_key, params.as_ref())
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Convert ranks into long/short weights.
#[wasm_bindgen(js_name = rankToWeights)]
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param time_key - Cross-sectional time key shared by values evaluated in the same slice.
/// @param params - Operation-specific parameter object defining transformation settings.
pub fn rank_to_weights(
    values: JsValue,
    time_key: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values: Vec<Option<f64>> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    let time_key: Vec<String> = serde_wasm_bindgen::from_value(time_key).map_err(to_js_err)?;
    let params = parse_params(params)?;
    let result = finstack_quant_features::rank_to_weights(&values, &time_key, params.as_ref())
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Neutralize a signal and z-score residuals.
#[wasm_bindgen(js_name = neutralizeAndZscore)]
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param time_key - Cross-sectional time key shared by values evaluated in the same slice.
/// @param exposures - Factor-exposure matrix aligned with the supplied observations.
/// @param params - Operation-specific parameter object defining transformation settings.
pub fn neutralize_and_zscore(
    values: JsValue,
    time_key: JsValue,
    exposures: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values: Vec<Option<f64>> = serde_wasm_bindgen::from_value(values).map_err(to_js_err)?;
    let time_key: Vec<String> = serde_wasm_bindgen::from_value(time_key).map_err(to_js_err)?;
    let exposures: Vec<Vec<Option<f64>>> =
        serde_wasm_bindgen::from_value(exposures).map_err(to_js_err)?;
    let params = parse_params(params)?;
    let result = finstack_quant_features::neutralize_and_zscore(
        &values,
        &time_key,
        &exposures,
        params.as_ref(),
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Apply a JSON panel transform pipeline.
#[wasm_bindgen(js_name = transformPanel)]
/// @param spec_json - Canonical panel-transformation JSON specifying input columns, operations, and parameters.
pub fn transform_panel(spec_json: &str) -> Result<String, JsValue> {
    finstack_quant_features::transform_panel(spec_json).map_err(to_js_err)
}

fn parse_params(params: Option<JsValue>) -> Result<Option<Value>, JsValue> {
    params
        .filter(|value| !value.is_null() && !value.is_undefined())
        .map(serde_wasm_bindgen::from_value)
        .transpose()
        .map_err(to_js_err)
}
