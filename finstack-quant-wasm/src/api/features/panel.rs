//! Typed panel pipeline and operation-selector twins.
//!
//! WASM panel specs, results and operation selectors are plain values (the
//! `PanelTransformSpec`, `PanelTransformResult`, `TimeSeriesOp`,
//! `CrossSectionalOp` and `PairwiseOp` types of `index.d.ts`), so the Python
//! class members are functions taking that value.

use crate::utils::input::{from_js_json, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_features::{
    CrossSectionalOp, PairwiseOp, PanelTransformResult, PanelTransformSpec, TimeSeriesOp,
};
use wasm_bindgen::prelude::*;

/// Apply a named panel transform pipeline and return its columns as an object.
///
/// Typed twin of `transformPanelJson` (Rust `transform_panel`). Operations run
/// sequentially; each reads the previous column unless `input` selects
/// `"values"` or an earlier operation name.
/// @param spec - `PanelTransformSpec` object or JSON: `values` (`null` marks a missing row), `operations`, and the `entity` / `order` / `time_key` columns the operations need.
/// @returns `PanelTransformResult` with one `{name, values}` column per operation, in operation order, row-aligned to `spec.values`.
///
/// # Errors
///
/// Throws a `TypeError` when `spec` is not an object or JSON string, and a
/// `validation` error for a malformed specification, blank, reserved
/// (`values`) or duplicate operation names, an unknown `input` column, a
/// missing partition column, unequal row counts, malformed operation
/// parameters or an operation that cannot be evaluated.
#[wasm_bindgen(js_name = transformPanel)]
pub fn transform_panel(spec: JsValue) -> Result<JsValue, JsValue> {
    let spec: PanelTransformSpec = from_js_json(&spec, "spec")?;
    let result = finstack_quant_features::transform_panel(&spec).map_err(to_js_err)?;
    to_js_value(&result)
}

/// Look up one output column of a panel transform result by name.
///
/// Free-function twin of Python `PanelTransformResult.get_column` (Rust
/// `PanelTransformResult::get_column`).
/// @param result - `PanelTransformResult` object or JSON returned by `transformPanel` / `transformPanelJson`.
/// @param name - Operation output name; the lookup is case-sensitive.
/// @returns The column's values, row-aligned to the input rows, with `null` for missing rows.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments, a `validation` error
/// when `result` is not a `PanelTransformResult`, and a `not_found` error
/// when no column is named `name`.
#[wasm_bindgen(js_name = panelTransformResultGetColumn)]
pub fn panel_transform_result_get_column(
    result: JsValue,
    name: JsValue,
) -> Result<JsValue, JsValue> {
    let result: PanelTransformResult = from_js_json(&result, "result")?;
    let name = js_string(&name, "name")?;
    let column = result.get_column(&name).ok_or_else(|| {
        to_js_err(finstack_quant_core::Error::Input(
            finstack_quant_core::error::InputError::NotFound {
                id: format!("panel transform column '{name}'"),
            },
        ))
    })?;
    to_js_value(&column)
}

/// List every accepted time-series operation name.
///
/// Free-function twin of Python `TimeSeriesOp.values` (Rust `TimeSeriesOp::names`).
/// @returns Snake_case operation names accepted by `transformTimeseries`, in declaration order.
///
/// # Errors
///
/// Throws only if the names cannot be converted to a JavaScript array.
#[wasm_bindgen(js_name = timeSeriesOpValues)]
pub fn time_series_op_values() -> Result<JsValue, JsValue> {
    to_js_value(&TimeSeriesOp::names())
}

/// List the `params` keys one time-series operation reads.
///
/// Free-function twin of the Python `TimeSeriesOp.param_keys` property (Rust
/// `TimeSeriesOp::param_keys`). Any other key in `params` is rejected.
/// @param op - Snake_case operation name, e.g. `"rolling_mean"`; see `timeSeriesOpValues`.
/// @returns Parameter keys the operation reads; empty when it takes none.
///
/// # Errors
///
/// Throws a `TypeError` when `op` is not a string and a `validation` error,
/// listing the accepted names, when it is not a time-series operation.
#[wasm_bindgen(js_name = timeSeriesOpParamKeys)]
pub fn time_series_op_param_keys(op: JsValue) -> Result<JsValue, JsValue> {
    let op: TimeSeriesOp = js_string(&op, "op")?.parse().map_err(to_js_err)?;
    to_js_value(&op.param_keys())
}

/// List every accepted cross-sectional operation name.
///
/// Free-function twin of Python `CrossSectionalOp.values` (Rust `CrossSectionalOp::names`).
/// @returns Snake_case operation names accepted by `transformCrossSectional`, in declaration order.
///
/// # Errors
///
/// Throws only if the names cannot be converted to a JavaScript array.
#[wasm_bindgen(js_name = crossSectionalOpValues)]
pub fn cross_sectional_op_values() -> Result<JsValue, JsValue> {
    to_js_value(&CrossSectionalOp::names())
}

/// List the `params` keys one cross-sectional operation reads.
///
/// Free-function twin of the Python `CrossSectionalOp.param_keys` property (Rust
/// `CrossSectionalOp::param_keys`). Any other key in `params` is rejected.
/// @param op - Snake_case operation name, e.g. `"winsorize"`; see `crossSectionalOpValues`.
/// @returns Parameter keys the operation reads; empty when it takes none.
///
/// # Errors
///
/// Throws a `TypeError` when `op` is not a string and a `validation` error,
/// listing the accepted names, when it is not a cross-sectional operation.
#[wasm_bindgen(js_name = crossSectionalOpParamKeys)]
pub fn cross_sectional_op_param_keys(op: JsValue) -> Result<JsValue, JsValue> {
    let op: CrossSectionalOp = js_string(&op, "op")?.parse().map_err(to_js_err)?;
    to_js_value(&op.param_keys())
}

/// List every accepted pairwise rolling operation name.
///
/// Free-function twin of Python `PairwiseOp.values` (Rust `PairwiseOp::names`).
/// @returns Snake_case operation names accepted by `transformTimeseriesPairwise`, in declaration order.
///
/// # Errors
///
/// Throws only if the names cannot be converted to a JavaScript array.
#[wasm_bindgen(js_name = pairwiseOpValues)]
pub fn pairwise_op_values() -> Result<JsValue, JsValue> {
    to_js_value(&PairwiseOp::names())
}

/// List the `params` keys one pairwise rolling operation reads.
///
/// Free-function twin of the Python `PairwiseOp.param_keys` property (Rust
/// `PairwiseOp::param_keys`). Any other key in `params` is rejected.
/// @param op - Snake_case operation name, e.g. `"rolling_beta"`; see `pairwiseOpValues`.
/// @returns Parameter keys the operation reads; empty when it takes none.
///
/// # Errors
///
/// Throws a `TypeError` when `op` is not a string and a `validation` error,
/// listing the accepted names, when it is not a pairwise rolling operation.
#[wasm_bindgen(js_name = pairwiseOpParamKeys)]
pub fn pairwise_op_param_keys(op: JsValue) -> Result<JsValue, JsValue> {
    let op: PairwiseOp = js_string(&op, "op")?.parse().map_err(to_js_err)?;
    to_js_value(&op.param_keys())
}
