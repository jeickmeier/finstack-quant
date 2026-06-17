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

/// Apply a JSON panel transform pipeline.
#[wasm_bindgen(js_name = transformPanel)]
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
