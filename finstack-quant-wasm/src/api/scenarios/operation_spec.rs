//! Free-function twins of the Python `OperationSpec` classmethod constructors
//! and predicates.
//!
//! WASM operations are plain objects (the generated `OperationSpec` type), so
//! each Python `OperationSpec.<variant>(...)` constructor is a function
//! `operationSpec<Variant>(...)` returning that object. Every function builds
//! the Rust enum variant and serializes it through serde, so the wire shape is
//! the one `ScenarioSpec` deserializes.

use std::str::FromStr;

use crate::utils::input::{
    from_js_json, js_f64, js_opt_bool, js_opt_f64_seq, js_opt_string, js_opt_string_seq, js_string,
    js_string_seq,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
use finstack_quant_core::types::CurveId;
use finstack_quant_scenarios::{
    CurveKind, InstrumentType, NodeId, OperationSpec, RateBindingSpec, ScenarioSpec,
    TenorMatchMode, TimeRollMode,
};
use indexmap::IndexMap;
use wasm_bindgen::prelude::*;

/// Attribute filter input: a `{key: value}` object or `[key, value]` pairs.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Attrs {
    Map(IndexMap<String, String>),
    Pairs(Vec<(String, String)>),
}

impl Attrs {
    fn into_map(self) -> IndexMap<String, String> {
        match self {
            Self::Map(map) => map,
            Self::Pairs(pairs) => pairs.into_iter().collect(),
        }
    }
}

/// Parse a serde string-enum argument from its wire label.
fn label<T: serde::de::DeserializeOwned>(value: &JsValue, name: &str) -> Result<T, JsValue> {
    finstack_quant_core::wire::serde_parse(&js_string(value, name)?).map_err(to_js_err)
}

/// Parse an optional serde string-enum argument from its wire label.
fn opt_label<T: serde::de::DeserializeOwned>(
    value: Option<&JsValue>,
    name: &str,
) -> Result<Option<T>, JsValue> {
    js_opt_string(value, name)?
        .as_deref()
        .map(finstack_quant_core::wire::serde_parse)
        .transpose()
        .map_err(to_js_err)
}

fn curve_id(value: &JsValue, name: &str) -> Result<CurveId, JsValue> {
    Ok(CurveId::from(js_string(value, name)?.as_str()))
}

fn opt_curve_id(value: Option<&JsValue>, name: &str) -> Result<Option<CurveId>, JsValue> {
    Ok(js_opt_string(value, name)?.map(|id| CurveId::from(id.as_str())))
}

fn currency(value: &JsValue, name: &str) -> Result<Currency, JsValue> {
    Currency::from_str(&js_string(value, name)?).map_err(to_js_err)
}

fn instrument_types(value: &JsValue) -> Result<Vec<InstrumentType>, JsValue> {
    js_string_seq(value, "instrumentTypes")?
        .iter()
        .map(|name| InstrumentType::from_str(name).map_err(to_js_err))
        .collect()
}

/// Instrument attribute filter: attribute name -> required value.
type AttributeFilter = IndexMap<String, String>;

fn parse_attrs(value: &JsValue) -> Result<AttributeFilter, JsValue> {
    Ok(from_js_json::<Attrs>(value, "attrs")?.into_map())
}

/// Parse an optional structured argument; `null` / `undefined` mean omitted.
fn opt_json<T: serde::de::DeserializeOwned>(
    value: Option<&JsValue>,
    name: &str,
) -> Result<Option<T>, JsValue> {
    match value {
        Some(v) if !(v.is_null() || v.is_undefined()) => from_js_json(v, name).map(Some),
        _ => Ok(None),
    }
}

fn operation(value: &JsValue) -> Result<OperationSpec, JsValue> {
    from_js_json(value, "operation")
}

/// Build an FX spot percent shock operation.
///
/// Free-function twin of Python `OperationSpec.market_fx_pct` (Rust
/// `OperationSpec::MarketFxPct`).
/// @param base - ISO-4217 code of the base currency being strengthened or weakened.
/// @param quote - ISO-4217 code of the quote currency; must differ from `base` to validate.
/// @param pct - Shock in percentage points (`5.0` = base +5% against quote); validation rejects `pct <= -100`.
/// @returns The `market_fx_pct` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for a non-string code or non-number `pct`, and a
/// `validation` error for an unknown currency code.
#[wasm_bindgen(js_name = operationSpecMarketFxPct)]
pub fn operation_spec_market_fx_pct(
    base: JsValue,
    quote: JsValue,
    pct: JsValue,
) -> Result<JsValue, JsValue> {
    let base = currency(&base, "base")?;
    let quote = currency(&quote, "quote")?;
    let pct = js_f64(&pct, "pct")?;
    to_js_value(&OperationSpec::MarketFxPct { base, quote, pct })
}

/// Build an equity price percent shock for a list of identifiers.
///
/// Free-function twin of Python `OperationSpec.equity_price_pct` (Rust
/// `OperationSpec::EquityPricePct`).
/// @param ids - Equity price identifiers in the market context; every one receives the shock.
/// @param pct - Shock in percentage points (`-10.0` = -10%); validation rejects `pct < -100`.
/// @returns The `equity_price_pct` operation object.
///
/// # Errors
///
/// Throws a `TypeError` when `ids` is not an array of strings or `pct` is not
/// a number.
#[wasm_bindgen(js_name = operationSpecEquityPricePct)]
pub fn operation_spec_equity_price_pct(ids: JsValue, pct: JsValue) -> Result<JsValue, JsValue> {
    let ids = js_string_seq(&ids, "ids")?;
    let pct = js_f64(&pct, "pct")?;
    to_js_value(&OperationSpec::EquityPricePct { ids, pct })
}

/// Build an instrument price percent shock selected by exact attribute match.
///
/// Free-function twin of Python `OperationSpec.instrument_price_pct_by_attr`
/// (Rust `OperationSpec::InstrumentPricePctByAttr`). Applying it requires an
/// instrument inventory.
/// @param attrs - `{key: value}` object or array of `[key, value]` pairs; order is preserved and every pair must match the instrument's attributes.
/// @param pct - Price shock in percentage points (`-5.0` = -5%).
/// @returns The `instrument_price_pct_by_attr` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for a non-object `attrs` or non-number `pct`, and a
/// `validation` error when `attrs` is not a string-to-string mapping or pair
/// list.
#[wasm_bindgen(js_name = operationSpecInstrumentPricePctByAttr)]
pub fn operation_spec_instrument_price_pct_by_attr(
    attrs: JsValue,
    pct: JsValue,
) -> Result<JsValue, JsValue> {
    let attrs = parse_attrs(&attrs)?;
    let pct = js_f64(&pct, "pct")?;
    to_js_value(&OperationSpec::InstrumentPricePctByAttr { attrs, pct })
}

/// Build a parallel basis-point curve shift for one curve or several.
///
/// Free-function twin of Python `OperationSpec.curve_parallel_bp`. A single
/// identifier builds Rust `OperationSpec::CurveParallelBp`; an array expands
/// through Rust `ScenarioSpec::parallel_bp_many` to one operation per
/// identifier, in the given order.
/// @param curve_kind - Curve family label: `"discount"`, `"forward"`, `"par_cds"`, `"inflation"` or `"commodity"`.
/// @param curve_id - One curve identifier, or an array of identifiers to shift by the same amount.
/// @param bp - Additive shift in basis points (1 bp = 1e-4); for `"commodity"` curves, percent of the forward.
/// @param discount_curve_id - Optional discount curve used when re-bootstrapping shocked ParCDS quotes.
/// @returns One `curve_parallel_bp` operation object for a string `curveId`, an array of them for an array.
///
/// # Errors
///
/// Throws a `TypeError` when `curveId` is neither a string nor an array of
/// strings or `bp` is not a number, and a `validation` error for an unknown
/// `curveKind` label.
#[wasm_bindgen(js_name = operationSpecCurveParallelBp)]
pub fn operation_spec_curve_parallel_bp(
    curve_kind: JsValue,
    curve_id: JsValue,
    bp: JsValue,
    discount_curve_id: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let curve_kind: CurveKind = label(&curve_kind, "curveKind")?;
    let bp = js_f64(&bp, "bp")?;
    let discount_curve_id = opt_curve_id(discount_curve_id.as_ref(), "discountCurveId")?;
    if curve_id.is_string() {
        return to_js_value(&OperationSpec::CurveParallelBp {
            curve_kind,
            curve_id: self::curve_id(&curve_id, "curveId")?,
            discount_curve_id,
            bp,
        });
    }
    let ids = js_string_seq(&curve_id, "curveId")?;
    to_js_value(&ScenarioSpec::parallel_bp_many(
        curve_kind,
        ids.iter().map(String::as_str),
        bp,
        discount_curve_id,
    ))
}

/// Build node-level basis-point shifts on a curve.
///
/// Free-function twin of Python `OperationSpec.curve_node_bp` (Rust
/// `OperationSpec::CurveNodeBp`).
/// @param curve_kind - Curve family label: `"discount"`, `"forward"`, `"par_cds"`, `"inflation"` or `"commodity"`.
/// @param curve_id - Identifier of the curve whose pillars are shifted.
/// @param nodes - Array of `[tenor, bp]` pairs, e.g. `[["2Y", 10], ["10Y", -5]]`; bp is additive basis points (percent of forward for commodity curves).
/// @param match_mode - Optional pillar alignment label, `"exact"` or `"interpolate"`; omit for the Rust default (`TenorMatchMode::default()`, `"interpolate"`).
/// @param discount_curve_id - Optional discount curve used when re-bootstrapping shocked ParCDS quotes.
/// @returns The `curve_node_bp` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments and a `validation` error
/// for an unknown `curveKind` / `matchMode` label or `nodes` that are not
/// `[string, number]` pairs.
#[wasm_bindgen(js_name = operationSpecCurveNodeBp)]
pub fn operation_spec_curve_node_bp(
    curve_kind: JsValue,
    curve_id: JsValue,
    nodes: JsValue,
    match_mode: Option<JsValue>,
    discount_curve_id: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let curve_kind: CurveKind = label(&curve_kind, "curveKind")?;
    let curve_id = self::curve_id(&curve_id, "curveId")?;
    let nodes: Vec<(String, f64)> = from_js_json(&nodes, "nodes")?;
    let match_mode: Option<TenorMatchMode> = opt_label(match_mode.as_ref(), "matchMode")?;
    let discount_curve_id = opt_curve_id(discount_curve_id.as_ref(), "discountCurveId")?;
    to_js_value(&OperationSpec::CurveNodeBp {
        curve_kind,
        curve_id,
        discount_curve_id,
        nodes,
        match_mode: match_mode.unwrap_or_default(),
    })
}

/// Build a parallel shock to a volatility-index curve.
///
/// Free-function twin of Python `OperationSpec.vol_index_parallel_pts` (Rust
/// `OperationSpec::VolIndexParallelPts`).
/// @param curve_id - Identifier of the volatility-index curve.
/// @param points - Additive shift in absolute index points (`1.0` moves 18.5 to 19.5).
/// @returns The `vol_index_parallel_pts` operation object.
///
/// # Errors
///
/// Throws a `TypeError` when `curveId` is not a string or `points` is not a
/// number.
#[wasm_bindgen(js_name = operationSpecVolIndexParallelPts)]
pub fn operation_spec_vol_index_parallel_pts(
    curve_id: JsValue,
    points: JsValue,
) -> Result<JsValue, JsValue> {
    let curve_id = self::curve_id(&curve_id, "curveId")?;
    let points = js_f64(&points, "points")?;
    to_js_value(&OperationSpec::VolIndexParallelPts { curve_id, points })
}

/// Build node-level shocks to a volatility-index curve.
///
/// Free-function twin of Python `OperationSpec.vol_index_node_pts` (Rust
/// `OperationSpec::VolIndexNodePts`).
/// @param curve_id - Identifier of the volatility-index curve.
/// @param nodes - Array of `[tenor, points]` pairs; points are additive absolute index points.
/// @param match_mode - Optional pillar alignment label, `"exact"` or `"interpolate"`; omit for the Rust default (`TenorMatchMode::default()`, `"interpolate"`).
/// @returns The `vol_index_node_pts` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments and a `validation` error
/// for an unknown `matchMode` label or `nodes` that are not `[string, number]`
/// pairs.
#[wasm_bindgen(js_name = operationSpecVolIndexNodePts)]
pub fn operation_spec_vol_index_node_pts(
    curve_id: JsValue,
    nodes: JsValue,
    match_mode: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let curve_id = self::curve_id(&curve_id, "curveId")?;
    let nodes: Vec<(String, f64)> = from_js_json(&nodes, "nodes")?;
    let match_mode: Option<TenorMatchMode> = opt_label(match_mode.as_ref(), "matchMode")?;
    to_js_value(&OperationSpec::VolIndexNodePts {
        curve_id,
        nodes,
        match_mode: match_mode.unwrap_or_default(),
    })
}

/// Build a parallel shift to a base-correlation surface.
///
/// Free-function twin of Python `OperationSpec.base_corr_parallel_pts` (Rust
/// `OperationSpec::BaseCorrParallelPts`).
/// @param surface_id - Identifier of the base-correlation surface.
/// @param points - Additive shift in decimal correlation (`0.02` = +0.02, not percentage points).
/// @returns The `base_corr_parallel_pts` operation object.
///
/// # Errors
///
/// Throws a `TypeError` when `surfaceId` is not a string or `points` is not a
/// number.
#[wasm_bindgen(js_name = operationSpecBaseCorrParallelPts)]
pub fn operation_spec_base_corr_parallel_pts(
    surface_id: JsValue,
    points: JsValue,
) -> Result<JsValue, JsValue> {
    let surface_id = self::curve_id(&surface_id, "surfaceId")?;
    let points = js_f64(&points, "points")?;
    to_js_value(&OperationSpec::BaseCorrParallelPts { surface_id, points })
}

/// Build a bucketed shift to a base-correlation surface.
///
/// Free-function twin of Python `OperationSpec.base_corr_bucket_pts` (Rust
/// `OperationSpec::BaseCorrBucketPts`).
/// @param surface_id - Identifier of the base-correlation surface.
/// @param points - Additive shift in decimal correlation (`0.02` = +0.02).
/// @param detachment_bp - Optional detachment points to shift, as integer basis points of the capital structure (`300` = 3%); omit to shift every bucket.
/// @returns The `base_corr_bucket_pts` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments and a `validation` error
/// when `detachmentBp` is not an array of 32-bit integers.
#[wasm_bindgen(js_name = operationSpecBaseCorrBucketPts)]
pub fn operation_spec_base_corr_bucket_pts(
    surface_id: JsValue,
    points: JsValue,
    detachment_bp: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let surface_id = self::curve_id(&surface_id, "surfaceId")?;
    let points = js_f64(&points, "points")?;
    let detachment_bp: Option<Vec<i32>> = opt_json(detachment_bp.as_ref(), "detachmentBp")?;
    to_js_value(&OperationSpec::BaseCorrBucketPts {
        surface_id,
        detachment_bp,
        points,
    })
}

/// Build a parallel percent shift to a volatility surface.
///
/// Free-function twin of Python `OperationSpec.vol_surface_parallel_pct` (Rust
/// `OperationSpec::VolSurfaceParallelPct`).
/// @param vol_surface_id - Identifier of the volatility surface.
/// @param pct - Relative shift in percentage points (`10.0` scales every vol by 1.10).
/// @returns The `vol_surface_parallel_pct` operation object.
///
/// # Errors
///
/// Throws a `TypeError` when `volSurfaceId` is not a string or `pct` is not a
/// number.
#[wasm_bindgen(js_name = operationSpecVolSurfaceParallelPct)]
pub fn operation_spec_vol_surface_parallel_pct(
    vol_surface_id: JsValue,
    pct: JsValue,
) -> Result<JsValue, JsValue> {
    let vol_surface_id = self::curve_id(&vol_surface_id, "volSurfaceId")?;
    let pct = js_f64(&pct, "pct")?;
    to_js_value(&OperationSpec::VolSurfaceParallelPct {
        vol_surface_id,
        pct,
    })
}

/// Build a bucketed percent shock to a volatility surface.
///
/// Free-function twin of Python `OperationSpec.vol_surface_bucket_pct` (Rust
/// `OperationSpec::VolSurfaceBucketPct`).
/// @param vol_surface_id - Identifier of the volatility surface.
/// @param pct - Relative shift in percentage points applied to the selected buckets.
/// @param tenors - Optional expiry tenors (e.g. `["1M", "1Y"]`) restricting the shock; omit for every expiry.
/// @param strikes - Optional strikes, in the surface's own strike units, restricting the shock; omit for every strike.
/// @returns The `vol_surface_bucket_pct` operation object.
///
/// # Errors
///
/// Throws a `TypeError` when an argument has the wrong JavaScript type
/// (`tenors` must be an array of strings, `strikes` an array of numbers).
#[wasm_bindgen(js_name = operationSpecVolSurfaceBucketPct)]
pub fn operation_spec_vol_surface_bucket_pct(
    vol_surface_id: JsValue,
    pct: JsValue,
    tenors: Option<JsValue>,
    strikes: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let vol_surface_id = self::curve_id(&vol_surface_id, "volSurfaceId")?;
    let pct = js_f64(&pct, "pct")?;
    let tenors = js_opt_string_seq(tenors.as_ref(), "tenors")?;
    let strikes = js_opt_f64_seq(strikes.as_ref(), "strikes")?;
    to_js_value(&OperationSpec::VolSurfaceBucketPct {
        vol_surface_id,
        tenors,
        strikes,
        pct,
    })
}

/// Build a statement forecast percent change on one node.
///
/// Free-function twin of Python `OperationSpec.stmt_forecast_percent` (Rust
/// `OperationSpec::StmtForecastPercent`).
/// @param node_id - Statement node identifier whose forecast values are scaled.
/// @param pct - Change in percentage points (`-5.0` = -5%).
/// @returns The `stmt_forecast_percent` operation object.
///
/// # Errors
///
/// Throws a `TypeError` when `nodeId` is not a string or `pct` is not a
/// number.
#[wasm_bindgen(js_name = operationSpecStmtForecastPercent)]
pub fn operation_spec_stmt_forecast_percent(
    node_id: JsValue,
    pct: JsValue,
) -> Result<JsValue, JsValue> {
    let node_id = NodeId::from(js_string(&node_id, "nodeId")?.as_str());
    let pct = js_f64(&pct, "pct")?;
    to_js_value(&OperationSpec::StmtForecastPercent { node_id, pct })
}

/// Build a statement forecast value assignment on one node.
///
/// Free-function twin of Python `OperationSpec.stmt_forecast_assign` (Rust
/// `OperationSpec::StmtForecastAssign`).
/// @param node_id - Statement node identifier whose forecast values are replaced.
/// @param value - Replacement forecast value, in the node's own units.
/// @returns The `stmt_forecast_assign` operation object.
///
/// # Errors
///
/// Throws a `TypeError` when `nodeId` is not a string or `value` is not a
/// number.
#[wasm_bindgen(js_name = operationSpecStmtForecastAssign)]
pub fn operation_spec_stmt_forecast_assign(
    node_id: JsValue,
    value: JsValue,
) -> Result<JsValue, JsValue> {
    let node_id = NodeId::from(js_string(&node_id, "nodeId")?.as_str());
    let value = js_f64(&value, "value")?;
    to_js_value(&OperationSpec::StmtForecastAssign { node_id, value })
}

/// Build an operation binding a statement rate node to a market curve.
///
/// Free-function twin of Python `OperationSpec.rate_binding` (Rust
/// `OperationSpec::RateBinding`).
/// @param binding - `RateBindingSpec` object or JSON: `node_id`, `curve_id`, `tenor`, optional `compounding` (default `"continuous"`) and `day_count`.
/// @returns The `rate_binding` operation object.
///
/// # Errors
///
/// Throws a `TypeError` when `binding` is not an object or JSON string and a
/// `validation` error when it does not match the `RateBindingSpec` contract.
#[wasm_bindgen(js_name = operationSpecRateBinding)]
pub fn operation_spec_rate_binding(binding: JsValue) -> Result<JsValue, JsValue> {
    let binding: RateBindingSpec = from_js_json(&binding, "binding")?;
    to_js_value(&OperationSpec::RateBinding { binding })
}

/// Build an instrument spread shock selected by exact attribute match.
///
/// Free-function twin of Python `OperationSpec.instrument_spread_bp_by_attr`
/// (Rust `OperationSpec::InstrumentSpreadBpByAttr`). Applying it requires an
/// instrument inventory.
/// @param attrs - `{key: value}` object or array of `[key, value]` pairs; order is preserved and every pair must match the instrument's attributes.
/// @param bp - Additive spread shock in basis points (1 bp = 1e-4).
/// @returns The `instrument_spread_bp_by_attr` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for a non-object `attrs` or non-number `bp`, and a
/// `validation` error when `attrs` is not a string-to-string mapping or pair
/// list.
#[wasm_bindgen(js_name = operationSpecInstrumentSpreadBpByAttr)]
pub fn operation_spec_instrument_spread_bp_by_attr(
    attrs: JsValue,
    bp: JsValue,
) -> Result<JsValue, JsValue> {
    let attrs = parse_attrs(&attrs)?;
    let bp = js_f64(&bp, "bp")?;
    to_js_value(&OperationSpec::InstrumentSpreadBpByAttr { attrs, bp })
}

/// Build an instrument price percent shock selected by instrument type.
///
/// Free-function twin of Python `OperationSpec.instrument_price_pct_by_type`
/// (Rust `OperationSpec::InstrumentPricePctByType`). Applying it requires an
/// instrument inventory.
/// @param instrument_types - Snake_case instrument type identifiers, e.g. `["bond", "cds_index"]`.
/// @param pct - Price shock in percentage points (`-5.0` = -5%).
/// @returns The `instrument_price_pct_by_type` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments and a `validation` error
/// for an unknown instrument type.
#[wasm_bindgen(js_name = operationSpecInstrumentPricePctByType)]
pub fn operation_spec_instrument_price_pct_by_type(
    instrument_types: JsValue,
    pct: JsValue,
) -> Result<JsValue, JsValue> {
    let instrument_types = self::instrument_types(&instrument_types)?;
    let pct = js_f64(&pct, "pct")?;
    to_js_value(&OperationSpec::InstrumentPricePctByType {
        instrument_types,
        pct,
    })
}

/// Build an instrument spread shock selected by instrument type.
///
/// Free-function twin of Python `OperationSpec.instrument_spread_bp_by_type`
/// (Rust `OperationSpec::InstrumentSpreadBpByType`). Applying it requires an
/// instrument inventory.
/// @param instrument_types - Snake_case instrument type identifiers, e.g. `["bond", "cds_index"]`.
/// @param bp - Additive spread shock in basis points (1 bp = 1e-4).
/// @returns The `instrument_spread_bp_by_type` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments and a `validation` error
/// for an unknown instrument type.
#[wasm_bindgen(js_name = operationSpecInstrumentSpreadBpByType)]
pub fn operation_spec_instrument_spread_bp_by_type(
    instrument_types: JsValue,
    bp: JsValue,
) -> Result<JsValue, JsValue> {
    let instrument_types = self::instrument_types(&instrument_types)?;
    let bp = js_f64(&bp, "bp")?;
    to_js_value(&OperationSpec::InstrumentSpreadBpByType {
        instrument_types,
        bp,
    })
}

/// Build a structured-credit asset-correlation shock.
///
/// Free-function twin of Python `OperationSpec.asset_correlation_pts` (Rust
/// `OperationSpec::AssetCorrelationPts`).
/// @param delta_pts - Additive shift in decimal correlation (`0.05` adds 0.05 to the asset correlation).
/// @returns The `asset_correlation_pts` operation object.
///
/// # Errors
///
/// Throws a `TypeError` when `deltaPts` is not a number.
#[wasm_bindgen(js_name = operationSpecAssetCorrelationPts)]
pub fn operation_spec_asset_correlation_pts(delta_pts: JsValue) -> Result<JsValue, JsValue> {
    let delta_pts = js_f64(&delta_pts, "deltaPts")?;
    to_js_value(&OperationSpec::AssetCorrelationPts { delta_pts })
}

/// Build a structured-credit prepay/default correlation shock.
///
/// Free-function twin of Python `OperationSpec.prepay_default_correlation_pts`
/// (Rust `OperationSpec::PrepayDefaultCorrelationPts`).
/// @param delta_pts - Additive shift in decimal correlation between prepayment and default.
/// @returns The `prepay_default_correlation_pts` operation object.
///
/// # Errors
///
/// Throws a `TypeError` when `deltaPts` is not a number.
#[wasm_bindgen(js_name = operationSpecPrepayDefaultCorrelationPts)]
pub fn operation_spec_prepay_default_correlation_pts(
    delta_pts: JsValue,
) -> Result<JsValue, JsValue> {
    let delta_pts = js_f64(&delta_pts, "deltaPts")?;
    to_js_value(&OperationSpec::PrepayDefaultCorrelationPts { delta_pts })
}

/// Build a hierarchy-targeted parallel curve shift.
///
/// Free-function twin of Python `OperationSpec.hierarchy_curve_parallel_bp`
/// (Rust `OperationSpec::HierarchyCurveParallelBp`).
/// @param curve_kind - Curve family label: `"discount"`, `"forward"`, `"par_cds"`, `"inflation"` or `"commodity"`.
/// @param target - `HierarchyTarget` object or JSON (`{path, tag_filter?}`) selecting the market-data subtree.
/// @param bp - Additive shift in basis points (percent of forward for commodity curves).
/// @param discount_curve_id - Optional discount curve used when re-bootstrapping shocked ParCDS quotes.
/// @returns The `hierarchy_curve_parallel_bp` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments and a `validation` error
/// for an unknown `curveKind` label or a `target` that is not a
/// `HierarchyTarget`.
#[wasm_bindgen(js_name = operationSpecHierarchyCurveParallelBp)]
pub fn operation_spec_hierarchy_curve_parallel_bp(
    curve_kind: JsValue,
    target: JsValue,
    bp: JsValue,
    discount_curve_id: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let curve_kind: CurveKind = label(&curve_kind, "curveKind")?;
    let target: HierarchyTarget = from_js_json(&target, "target")?;
    let bp = js_f64(&bp, "bp")?;
    let discount_curve_id = opt_curve_id(discount_curve_id.as_ref(), "discountCurveId")?;
    to_js_value(&OperationSpec::HierarchyCurveParallelBp {
        curve_kind,
        target,
        bp,
        discount_curve_id,
    })
}

/// Build a hierarchy-targeted volatility-surface percent shift.
///
/// Free-function twin of Python
/// `OperationSpec.hierarchy_vol_surface_parallel_pct` (Rust
/// `OperationSpec::HierarchyVolSurfaceParallelPct`).
/// @param target - `HierarchyTarget` object or JSON (`{path, tag_filter?}`) selecting the market-data subtree.
/// @param pct - Relative shift in percentage points applied to every targeted surface.
/// @returns The `hierarchy_vol_surface_parallel_pct` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments and a `validation` error
/// when `target` is not a `HierarchyTarget`.
#[wasm_bindgen(js_name = operationSpecHierarchyVolSurfaceParallelPct)]
pub fn operation_spec_hierarchy_vol_surface_parallel_pct(
    target: JsValue,
    pct: JsValue,
) -> Result<JsValue, JsValue> {
    let target: HierarchyTarget = from_js_json(&target, "target")?;
    let pct = js_f64(&pct, "pct")?;
    to_js_value(&OperationSpec::HierarchyVolSurfaceParallelPct { target, pct })
}

/// Build a hierarchy-targeted equity price percent shift.
///
/// Free-function twin of Python `OperationSpec.hierarchy_equity_price_pct`
/// (Rust `OperationSpec::HierarchyEquityPricePct`).
/// @param target - `HierarchyTarget` object or JSON (`{path, tag_filter?}`) selecting the market-data subtree.
/// @param pct - Price shock in percentage points applied to every targeted equity.
/// @returns The `hierarchy_equity_price_pct` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments and a `validation` error
/// when `target` is not a `HierarchyTarget`.
#[wasm_bindgen(js_name = operationSpecHierarchyEquityPricePct)]
pub fn operation_spec_hierarchy_equity_price_pct(
    target: JsValue,
    pct: JsValue,
) -> Result<JsValue, JsValue> {
    let target: HierarchyTarget = from_js_json(&target, "target")?;
    let pct = js_f64(&pct, "pct")?;
    to_js_value(&OperationSpec::HierarchyEquityPricePct { target, pct })
}

/// Build a hierarchy-targeted base-correlation parallel shift.
///
/// Free-function twin of Python
/// `OperationSpec.hierarchy_base_corr_parallel_pts` (Rust
/// `OperationSpec::HierarchyBaseCorrParallelPts`).
/// @param target - `HierarchyTarget` object or JSON (`{path, tag_filter?}`) selecting the market-data subtree.
/// @param points - Additive shift in decimal correlation (`0.02` = +0.02).
/// @returns The `hierarchy_base_corr_parallel_pts` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments and a `validation` error
/// when `target` is not a `HierarchyTarget`.
#[wasm_bindgen(js_name = operationSpecHierarchyBaseCorrParallelPts)]
pub fn operation_spec_hierarchy_base_corr_parallel_pts(
    target: JsValue,
    points: JsValue,
) -> Result<JsValue, JsValue> {
    let target: HierarchyTarget = from_js_json(&target, "target")?;
    let points = js_f64(&points, "points")?;
    to_js_value(&OperationSpec::HierarchyBaseCorrParallelPts { target, points })
}

/// Build a time-roll operation moving the valuation horizon forward.
///
/// Free-function twin of Python `OperationSpec.time_roll_forward` (Rust
/// `OperationSpec::time_roll_forward`).
/// @param period - Tenor-style roll period such as `"1D"`, `"1W"`, `"1M"` or `"1Y"`; parsed when the operation is validated.
/// @param apply_shocks - Whether the remaining operations run after the roll; omit for the Rust default, `true`.
/// @param roll_mode - Optional roll semantics label: `"business_days"`, `"calendar_days"` or `"approximate"`; omit for the Rust default, `"business_days"`.
/// @returns The `time_roll_forward` operation object.
///
/// # Errors
///
/// Throws a `TypeError` for wrongly typed arguments and a `validation` error
/// for an unknown `rollMode` label.
#[wasm_bindgen(js_name = operationSpecTimeRollForward)]
pub fn operation_spec_time_roll_forward(
    period: JsValue,
    apply_shocks: Option<JsValue>,
    roll_mode: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let period = js_string(&period, "period")?;
    let apply_shocks = js_opt_bool(apply_shocks.as_ref(), "applyShocks")?;
    let roll_mode: Option<TimeRollMode> = opt_label(roll_mode.as_ref(), "rollMode")?;
    to_js_value(&OperationSpec::time_roll_forward(
        period,
        apply_shocks,
        roll_mode,
    ))
}

/// Validate one scenario operation with the canonical Rust rules.
///
/// Free-function twin of Python `OperationSpec.validate` (Rust
/// `OperationSpec::validate`): identifiers, finite numbers, variant-specific
/// floors and tenor parsing. Returns `undefined` when valid.
/// @param operation - `OperationSpec` object or JSON to check.
///
/// # Errors
///
/// Throws a `TypeError` when `operation` is not an object or JSON string, and
/// a `validation` error when it matches no `OperationSpec` variant or fails
/// the variant's rules (blank identifier, non-finite number, FX `pct <= -100`,
/// unparseable tenor).
#[wasm_bindgen(js_name = operationSpecValidate)]
pub fn operation_spec_validate(operation: JsValue) -> Result<(), JsValue> {
    self::operation(&operation)?.validate().map_err(to_js_err)
}

/// Report whether an operation needs instruments in the execution context.
///
/// Free-function twin of Python `OperationSpec.requires_instruments` (Rust
/// `OperationSpec::requires_instruments`).
/// @param operation - `OperationSpec` object or JSON to inspect.
/// @returns `true` for instrument-scoped shocks and time rolls.
///
/// # Errors
///
/// Throws a `TypeError` when `operation` is not an object or JSON string, and
/// a `validation` error when it matches no `OperationSpec` variant.
#[wasm_bindgen(js_name = operationSpecRequiresInstruments)]
pub fn operation_spec_requires_instruments(operation: JsValue) -> Result<bool, JsValue> {
    Ok(self::operation(&operation)?.requires_instruments())
}

/// Report whether an operation can replace or mutate instruments.
///
/// Free-function twin of Python `OperationSpec.mutates_instruments` (Rust
/// `OperationSpec::mutates_instruments`).
/// @param operation - `OperationSpec` object or JSON to inspect.
/// @returns `true` for price, spread and structured-credit correlation shocks; `false` for a time roll.
///
/// # Errors
///
/// Throws a `TypeError` when `operation` is not an object or JSON string, and
/// a `validation` error when it matches no `OperationSpec` variant.
#[wasm_bindgen(js_name = operationSpecMutatesInstruments)]
pub fn operation_spec_mutates_instruments(operation: JsValue) -> Result<bool, JsValue> {
    Ok(self::operation(&operation)?.mutates_instruments())
}

#[cfg(test)]
mod tests {
    use super::Attrs;

    #[test]
    fn attrs_accept_a_mapping_or_pairs_and_keep_order() {
        let from_map: Attrs =
            serde_json::from_str(r#"{"sector":"energy","rating":"BB"}"#).expect("mapping attrs");
        let from_pairs: Attrs =
            serde_json::from_str(r#"[["sector","energy"],["rating","BB"]]"#).expect("pair attrs");
        let expected = vec![
            ("sector".to_string(), "energy".to_string()),
            ("rating".to_string(), "BB".to_string()),
        ];
        assert_eq!(
            from_map.into_map().into_iter().collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            from_pairs.into_map().into_iter().collect::<Vec<_>>(),
            expected
        );
    }
}
