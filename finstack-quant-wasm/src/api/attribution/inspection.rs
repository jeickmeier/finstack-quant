//! Result explanation, bucket-share, tolerance, and validation WASM entry points.

//! inside JSON strings, where `JSON.parse` would silently round a larger
//! integer. Today every count in the attribution surface is bounded by a
//! handful of factors (≤ 12) and a handful of repricings (≤ ~30), well under
//! the safe-integer ceiling.

use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

fn parse_pnl(value: &JsValue) -> Result<finstack_quant_attribution::PnlAttribution, JsValue> {
    crate::utils::input::from_js_json(value, "pnl")
}

/// Human-readable tree explanation of an attribution's factor breakdown.
///
/// Free-function twin of Python `PnlAttribution.explain` (Rust
/// `PnlAttribution::explain`).
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
/// @param showZeros - Optional boolean; `true` lists every factor including
///   zero-valued ones, while omitted or `false` hides zero-valued factors.
/// @returns Multi-line tree text.
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution`, and
/// kind `invalid_type` if `showZeros` is not a boolean.
#[wasm_bindgen(js_name = pnlAttributionExplainText)]
pub fn pnl_attribution_explain(
    #[wasm_bindgen(unchecked_param_type = "PnlAttribution | string")] pnl: JsValue,
    #[wasm_bindgen(
        js_name = "showZeros",
        unchecked_optional_param_type = "boolean | null"
    )]
    show_zeros: Option<JsValue>,
) -> Result<String, JsValue> {
    let show_zeros =
        crate::utils::input::js_opt_bool(show_zeros.as_ref(), "showZeros")?.unwrap_or(false);
    Ok(parse_pnl(&pnl)?.explain(show_zeros))
}

/// Share of total P&L, in percent, that an amount represents.
///
/// Free-function twin of Python `PnlAttribution.pct_of_total` (Rust
/// `PnlAttribution::pct_of_total`): `amount / total_pnl × 100`, with the
/// zero test on `total_pnl` taken from the run's rounding context.
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
/// @param amount - P&L amount in `total_pnl` currency units, normally one factor field (`carry`, `rates_curves_pnl`, `residual`, ...) or `total_pnl` itself.
/// @returns Signed percentage points (`25` = 25% of total P&L), or `undefined` when `total_pnl` is effectively zero and the share is undefined.
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution`, and kind
/// `invalid_type` if `amount` is not a number.
#[wasm_bindgen(js_name = pnlAttributionPctOfTotal)]
pub fn pnl_attribution_pct_of_total(
    #[wasm_bindgen(unchecked_param_type = "PnlAttribution | string")] pnl: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
) -> Result<Option<f64>, JsValue> {
    let pnl = parse_pnl(&pnl)?;
    Ok(pnl.pct_of_total(crate::utils::input::js_f64(&amount, "amount")?))
}

/// Every money bucket with its share of total P&L, in the struct's field order.
///
/// Free-function twin of Python `PnlAttribution.pct_of_total_by_bucket` (Rust
/// `PnlAttribution::pct_of_total_by_bucket`). Lists the eleven factor buckets and
/// `residual`, so a report reads the buckets from Rust instead of naming them.
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
/// @returns One `PnlBucketShare` per bucket: the attribute's wire `field`, its `amount` in the `total_pnl` currency and `pct_of_total` in signed percentage points, `null` when `total_pnl` is effectively zero.
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution`.
#[wasm_bindgen(js_name = pnlAttributionPctOfTotalByBucket, unchecked_return_type = "PnlBucketShare[]")]
pub fn pnl_attribution_pct_of_total_by_bucket(
    #[wasm_bindgen(unchecked_param_type = "PnlAttribution | string")] pnl: JsValue,
) -> Result<JsValue, JsValue> {
    crate::utils::to_js_value(&parse_pnl(&pnl)?.pct_of_total_by_bucket())
}

/// Whether the attribution residual is within tolerance.
///
/// Free-function twin of Python `PnlAttribution.residual_within_tolerance`
/// (Rust `PnlAttribution::residual_within_tolerance`): the tolerance is the
/// larger of `pctTolerance`% of |total P&L| and `absTolerance`; an
/// attribution flagged `result_invalid` is never within tolerance.
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
/// @param pctTolerance - Optional percentage tolerance (`0.1` = 0.1%); omitted uses the run's `meta.tolerance_pct`.
/// @param absTolerance - Optional absolute tolerance in `total_pnl` currency units; omitted uses `meta.tolerance_abs`.
/// @returns `true` when the residual is within tolerance.
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution`, and kind
/// `invalid_type` if a tolerance is not a number.
#[wasm_bindgen(js_name = pnlAttributionResidualWithinTolerance)]
pub fn pnl_attribution_residual_within_tolerance(
    #[wasm_bindgen(unchecked_param_type = "PnlAttribution | string")] pnl: JsValue,
    #[wasm_bindgen(
        js_name = "pctTolerance",
        unchecked_optional_param_type = "number | null"
    )]
    pct_tolerance: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "absTolerance",
        unchecked_optional_param_type = "number | null"
    )]
    abs_tolerance: Option<JsValue>,
) -> Result<bool, JsValue> {
    let pnl = parse_pnl(&pnl)?;
    Ok(pnl.residual_within_tolerance(
        crate::utils::input::js_opt_f64(pct_tolerance.as_ref(), "pctTolerance")?,
        crate::utils::input::js_opt_f64(abs_tolerance.as_ref(), "absTolerance")?,
    ))
}

/// Check that every factor's currency matches the total P&L currency.
///
/// Free-function twin of Python `PnlAttribution.validate_currencies` (Rust
/// `PnlAttribution::validate_currencies`); run it before summing factors.
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution` or a
/// factor is denominated in another currency.
#[wasm_bindgen(js_name = pnlAttributionValidateCurrencies)]
pub fn pnl_attribution_validate_currencies(
    #[wasm_bindgen(unchecked_param_type = "PnlAttribution | string")] pnl: JsValue,
) -> Result<(), JsValue> {
    parse_pnl(&pnl)?.validate_currencies().map_err(to_js_err)
}

/// Metric identifiers the attribution's method needs pre-computed.
///
/// Free-function twin of Python `PnlAttribution.required_metrics` (Rust
/// `AttributionMethod::required_metrics` of `meta.method`): the metrics-based
/// method lists its sensitivities; repricing methods return an empty array.
/// @param pnl - `PnlAttribution` returned by `attributePnl` (object or JSON).
/// @returns Canonical metric identifiers.
///
/// # Errors
///
/// Throws with kind `validation` if `pnl` is not a `PnlAttribution`.
#[wasm_bindgen(js_name = pnlAttributionRequiredMetrics)]
pub fn pnl_attribution_required_metrics(
    #[wasm_bindgen(unchecked_param_type = "PnlAttribution | string")] pnl: JsValue,
) -> Result<Vec<String>, JsValue> {
    Ok(parse_pnl(&pnl)?
        .meta
        .method
        .required_metrics()
        .iter()
        .map(ToString::to_string)
        .collect())
}
