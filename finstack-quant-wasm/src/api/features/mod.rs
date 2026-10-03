//! WASM bindings for vectorized panel feature transforms.
//!
//! The binding accepts JavaScript arrays/objects, converts them into the Rust
//! crate's canonical inputs, and delegates all transform behavior to
//! `finstack-quant-features`.

use crate::utils::input::{
    from_js_json, js_nullable_f64_matrix, js_nullable_f64_seq, js_string, js_string_seq, json_text,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_features::{CrossSectionalOp, PairwiseOp, TimeSeriesOp};
use serde_json::Value;
use wasm_bindgen::prelude::*;

pub mod panel;

/// Transform a time-series panel column per entity.
///
/// `order` is lexicographic; temporal strings need a common timezone and fixed
/// precision. `window` spans rows including gaps; `min_periods <= window` counts
/// finite observations within it. `periods`, `half_life`, and EWMA `span` use
/// finite observation time. Aggregates can emit at missing current rows.
/// EWMA requires `span >= 1`; centered biased variance is used, with missing
/// initial volatility and zero volatility for constant series after two finite rows.
/// Rolling slope uses row positions and preserves missing-row gaps.
/// `drawdown` takes a level series. `rolling_sharpe` is a period feature
/// `(mean - risk_free) / sample_std` on returns, not the annualized
/// `analytics` Sharpe. Optional JSON `risk_free` defaults to `0.0` in the same
/// units as the return series.
///
/// # Arguments
///
/// * `values` - Numeric observations aligned with the key columns; null and non-finite inputs are missing.
/// * `entity` - String entity identifiers grouping observations into independent time series.
/// * `order` - Lexicographic order keys within each entity; temporal keys require a common timezone and fixed precision.
/// * `op` - Snake_case TimeSeriesOp selector accepted by timeSeriesOpValues; parsed by the canonical Rust enum.
/// * `params` - Optional operation parameters; omitted or null uses Rust defaults and unknown keys fail validation.
///   `rolling_sharpe` accepts optional `risk_free` (default `0.0`) in the same units as the input return series, with no annualization.
///
/// # Errors
///
/// Rejects values that cannot be decoded into the declared arrays or JSON
/// parameters, unequal row counts, an unsupported `op`, malformed operation
/// parameters, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
#[wasm_bindgen(js_name = transformTimeseries)]
pub fn transform_timeseries(
    values: JsValue,
    entity: JsValue,
    order: JsValue,
    op: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let op: TimeSeriesOp = js_string(&op, "op")?.parse().map_err(to_js_err)?;
    let values = js_nullable_f64_seq(&values, "values")?;
    let entity = js_string_seq(&entity, "entity")?;
    let order = js_string_seq(&order, "order")?;
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
///
/// `cap_weights` enforces final `0 < max_abs <= 1` with zero net and unit gross.
/// Each demeaned-signal side must support gross 0.5 at the cap; otherwise it
/// fails. Constant signals produce zero weights. Signed zeros always tie.
///
/// # Arguments
///
/// * `values` - Numeric observations aligned with time_key; null and non-finite inputs are missing.
/// * `time_key` - String partition keys; equal keys select the same cross-section.
/// * `op` - Snake_case CrossSectionalOp selector accepted by crossSectionalOpValues; parsed by the canonical Rust enum.
/// * `params` - Optional operation parameters; omitted or null uses Rust defaults and unknown keys fail validation.
///
/// # Errors
///
/// Rejects values that cannot be decoded into the declared arrays or JSON
/// parameters, unequal `values` and `time_key` lengths, an unsupported `op`,
/// malformed operation parameters, non-finite arithmetic, or a result that cannot be serialized to
/// JavaScript.
#[wasm_bindgen(js_name = transformCrossSectional)]
pub fn transform_cross_sectional(
    values: JsValue,
    time_key: JsValue,
    op: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let op: CrossSectionalOp = js_string(&op, "op")?.parse().map_err(to_js_err)?;
    let values = js_nullable_f64_seq(&values, "values")?;
    let time_key = js_string_seq(&time_key, "timeKey")?;
    let params = parse_params(params)?;
    let result =
        finstack_quant_features::transform_cross_sectional(&values, &time_key, op, params.as_ref())
            .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Transform a cross-section within each time/group sub-partition.
///
/// # Arguments
///
/// * `values` - Numeric observations aligned with both key columns; null and non-finite inputs are missing.
/// * `time_key` - String timestamp partition keys; equal keys select the same cross-section.
/// * `groups` - String subgroup labels aligned with values; transforms run independently for each time_key and group pair.
/// * `op` - Snake_case CrossSectionalOp selector accepted by crossSectionalOpValues; parsed by the canonical Rust enum.
/// * `params` - Optional operation parameters; omitted or null uses Rust defaults and unknown keys fail validation.
///
/// # Errors
///
/// Rejects values that cannot be decoded into the declared arrays or JSON
/// parameters, unequal `values`, `time_key`, and `groups` lengths, an
/// unsupported `op`, malformed operation parameters, or a result that cannot
/// be serialized to JavaScript.
#[wasm_bindgen(js_name = transformCrossSectionalGrouped)]
pub fn transform_cross_sectional_grouped(
    values: JsValue,
    time_key: JsValue,
    groups: JsValue,
    op: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let op: CrossSectionalOp = js_string(&op, "op")?.parse().map_err(to_js_err)?;
    let values = js_nullable_f64_seq(&values, "values")?;
    let time_key = js_string_seq(&time_key, "timeKey")?;
    let groups = js_string_seq(&groups, "groups")?;
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
///
/// Equal-weighted OLS. A singular or underdetermined design in any time
/// partition fails the call and names that `timeKey`.
///
/// # Errors
///
/// Rejects values that cannot be decoded into the declared arrays or JSON
/// parameters, unequal row counts, exposure columns whose lengths differ from
/// `values`, a non-boolean `fit_intercept`, a singular or underdetermined
/// cross-section, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param time_key - Cross-sectional time key shared by values evaluated in the same slice.
/// @param exposures - Factor-exposure matrix aligned with the supplied observations.
/// @param params - Operation-specific parameter object defining transformation settings.
#[wasm_bindgen(js_name = neutralize)]
pub fn neutralize(
    values: JsValue,
    time_key: JsValue,
    exposures: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values = js_nullable_f64_seq(&values, "values")?;
    let time_key = js_string_seq(&time_key, "timeKey")?;
    let exposures = js_nullable_f64_matrix(&exposures, "exposures")?;
    let params = parse_params(params)?;
    let result =
        finstack_quant_features::neutralize(&values, &time_key, &exposures, params.as_ref())
            .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Transform two time-series panel columns per entity.
///
/// `window` spans entity rows including gaps; `min_periods <= window` counts
/// complete pairs within it. Results can emit at a missing current row.
/// `order` is lexicographic; temporal strings need a common timezone and precision.
///
/// # Arguments
///
/// * `values` - Primary numeric observations aligned with other and both key columns; null and non-finite inputs are missing.
/// * `other` - Second numeric observation column; rolling windows retain only complete finite pairs.
/// * `entity` - String entity identifiers grouping observations into independent time series.
/// * `order` - Lexicographic order keys within each entity; temporal keys require a common timezone and fixed precision.
/// * `op` - Snake_case PairwiseOp selector accepted by pairwiseOpValues; parsed by the canonical Rust enum.
/// * `params` - Optional rolling parameters; window counts entity rows, min_periods counts complete pairs and cannot exceed window.
///
/// # Errors
///
/// Rejects values that cannot be decoded into the declared arrays or JSON
/// parameters, unequal row counts, an unsupported `op`, non-positive or
/// non-integer `window` or `min_periods` parameters, or a result that cannot be
/// serialized to JavaScript.
#[wasm_bindgen(js_name = transformTimeseriesPairwise)]
pub fn transform_timeseries_pairwise(
    values: JsValue,
    other: JsValue,
    entity: JsValue,
    order: JsValue,
    op: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let op: PairwiseOp = js_string(&op, "op")?.parse().map_err(to_js_err)?;
    let values = js_nullable_f64_seq(&values, "values")?;
    let other = js_nullable_f64_seq(&other, "other")?;
    let entity = js_string_seq(&entity, "entity")?;
    let order = js_string_seq(&order, "order")?;
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
///
/// `window` spans rows including missing rows; `min_periods <= window` counts
/// complete rows within it. Missing current responses or exposures yield null.
/// The scaled SVD fit is independent of exposure units.
///
/// Rank-deficient windows emit `null` for that row. That is intentional and
/// unlike `neutralize`, which fails the call.
///
/// # Errors
///
/// Rejects values that cannot be decoded into the declared arrays or JSON
/// parameters, unequal row counts, exposure columns whose lengths differ from
/// `values`, malformed `window`, `min_periods`, or `fit_intercept` parameters,
/// non-finite arithmetic, or a result that cannot be serialized to JavaScript.
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param exposures - Factor-exposure matrix aligned with the supplied observations.
/// @param entity - Entity identifier used to group ordered time-series observations.
/// @param order - Observation-order key used to sort each entity time series.
/// @param params - Operation-specific parameter object defining transformation settings.
#[wasm_bindgen(js_name = rollingRegressionResidual)]
pub fn rolling_regression_residual(
    values: JsValue,
    exposures: JsValue,
    entity: JsValue,
    order: JsValue,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let values = js_nullable_f64_seq(&values, "values")?;
    let exposures = js_nullable_f64_matrix(&exposures, "exposures")?;
    let entity = js_string_seq(&entity, "entity")?;
    let order = js_string_seq(&order, "order")?;
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

/// Convert a signal to dollar-neutral inverse-risk-scaled weights per timestamp.
///
/// Finite rows become `raw = signal / vol`, then `centered = raw - mean(raw)`,
/// then `weight = centered / sum(|centered|)`. A zero centered gross
/// emits `0.0` for those finite rows.
///
/// # Errors
///
/// Rejects inputs that cannot be decoded into the declared arrays, unequal
/// `values`, `time_key`, and `volatility` lengths, negative volatility, or a result that cannot be
/// serialized to JavaScript.
/// @param values - Numeric signal observations aligned with `timeKey` and `volatility`.
/// @param time_key - Cross-sectional time key shared by values evaluated in the same slice.
/// @param volatility - Row-aligned nonnegative risk estimates in a common horizon and units, used as `signal / volatility`; negative estimates fail, while zero, missing, or non-finite values yield missing weights.
#[wasm_bindgen(js_name = riskScaledWeights)]
pub fn risk_scaled_weights(
    values: JsValue,
    time_key: JsValue,
    volatility: JsValue,
) -> Result<JsValue, JsValue> {
    let values = js_nullable_f64_seq(&values, "values")?;
    let time_key = js_string_seq(&time_key, "timeKey")?;
    let volatility = js_nullable_f64_seq(&volatility, "volatility")?;
    let result = finstack_quant_features::risk_scaled_weights(&values, &time_key, &volatility)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Convert ranks into long/short weights.
///
/// # Errors
///
/// Rejects inputs that cannot be decoded into the declared arrays, unequal
/// `values` and `time_key` lengths, non-finite arithmetic, or a result that cannot be serialized to
/// JavaScript.
/// @param values - Numeric observations in the shape and order required by the selected transformation.
/// @param time_key - Cross-sectional time key shared by values evaluated in the same slice.
#[wasm_bindgen(js_name = rankToWeights)]
pub fn rank_to_weights(values: JsValue, time_key: JsValue) -> Result<JsValue, JsValue> {
    let values = js_nullable_f64_seq(&values, "values")?;
    let time_key = js_string_seq(&time_key, "timeKey")?;
    let result = finstack_quant_features::rank_to_weights(&values, &time_key).map_err(to_js_err)?;
    to_js_value(&result)
}

/// Neutralize a signal and z-score residuals.
///
/// Equal-weighted OLS always includes an intercept so z-scoring preserves
/// neutrality to the supplied exposures.
///
/// # Arguments
///
/// * `values` - Numeric signal observations, with null or non-finite inputs
///   treated as missing, aligned with the partition and exposure columns.
/// * `time_key` - String partition keys grouping observations into independent
///   cross-sections; exact equal strings select the same partition.
/// * `exposures` - Column-major numeric factor exposures, each column aligned
///   with `values`; rows missing any exposure are excluded from the fit.
///
/// # Errors
///
/// Rejects inputs that cannot be decoded into the declared arrays, unequal row
/// counts, exposure columns whose lengths differ from `values`, a singular or
/// underdetermined cross-section, non-finite arithmetic, or a result that cannot
/// be serialized to JavaScript.
#[wasm_bindgen(js_name = neutralizeAndZscore)]
pub fn neutralize_and_zscore(
    values: JsValue,
    time_key: JsValue,
    exposures: JsValue,
) -> Result<JsValue, JsValue> {
    let values = js_nullable_f64_seq(&values, "values")?;
    let time_key = js_string_seq(&time_key, "timeKey")?;
    let exposures = js_nullable_f64_matrix(&exposures, "exposures")?;
    let result = finstack_quant_features::neutralize_and_zscore(&values, &time_key, &exposures)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Apply a JSON panel transform pipeline.
///
/// Operations run sequentially. Each op reads the previous column by default;
/// set `input` to `"values"` or an earlier operation name to select a source.
///
/// # Errors
///
/// Rejects malformed JSON or panel specifications, blank, reserved (`values`),
/// or duplicate operation names, unknown `input` columns, missing partition
/// columns, unequal row counts, malformed operation parameters, operations
/// that cannot be evaluated, non-finite arithmetic, or a result that cannot be serialized to JSON.
/// @param spec_json - Canonical panel-transformation JSON. Each operation may set optional `input` (`undefined` default: previous column, or raw `values` for the first op).
#[wasm_bindgen(js_name = transformPanelJson)]
pub fn transform_panel_json(spec_json: JsValue) -> Result<String, JsValue> {
    let spec_json: &str = &json_text(&spec_json, "specJson")?;
    finstack_quant_features::transform_panel_json(spec_json).map_err(to_js_err)
}

fn parse_params(params: Option<JsValue>) -> Result<Option<Value>, JsValue> {
    params
        .filter(|value| !value.is_null() && !value.is_undefined())
        .map(|value| from_js_json(&value, "params"))
        .transpose()
}
