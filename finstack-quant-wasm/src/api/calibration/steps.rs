//! Free-function twins of the Python `CalibrationStep` constructors.
//!
//! Each function takes the step id, one object (or JSON text) holding the
//! wire fields of one `StepParams` variant, and an optional quote-set name,
//! and calls Rust `CalibrationStep::from_wire_fields`, which owns the
//! omitted-field defaults (quote set and produced-object identifiers fall back
//! to the step id) and the strict deserialization. The result is the plain
//! `CalibrationStep` object placed in `plan.steps`; quotes are listed in the
//! envelope's `market_data` and named in `plan.quote_sets`.

use crate::utils::input::{from_js_json, js_opt_string, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_calibration::api::schema::CalibrationStep;
use serde_json::{Map, Value};
use wasm_bindgen::prelude::*;

fn step(
    kind: &str,
    id: &JsValue,
    params: &JsValue,
    quote_set: Option<&JsValue>,
) -> Result<JsValue, JsValue> {
    let id = js_string(id, "id")?;
    let fields: Map<String, Value> = from_js_json(params, "params")?;
    let quote_set = js_opt_string(quote_set, "quoteSet")?;
    let step = CalibrationStep::from_wire_fields(kind, &id, quote_set.as_deref(), fields)
        .map_err(to_js_err)?;
    to_js_value(&step)
}

/// Build a discount-curve bootstrap step.
///
/// Free-function twin of Python `CalibrationStep.discount` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name and `curve_id`.
/// @param params - Object (or JSON text) holding every wire field of the `discount` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `currency` (string, required): Upper-case ISO 4217 code of the curve currency.
///   - `base_date` (string, required): Curve base (spot) date as an ISO-8601 date string (`"YYYY-MM-DD"`).
///   - `curve_id` (string, optional, default `id`): Identifier of the produced curve.
///   - `method` (string | object, optional, default `"bootstrap"`): Calibration method: `"bootstrap"` or `{"global_solve": {"use_analytical_jacobian": bool}}`.
///   - `interpolation` (string, optional, default `"log_linear"`): Discount-factor interpolation: `"linear"`, `"log_linear"`, `"monotone_convex"`, `"cubic_hermite"` or `"piecewise_quadratic_forward"`.
///   - `extrapolation` (string, optional, default `"flat_forward"`): Extrapolation policy: `"flat_zero"`, `"flat_forward"` or `"none"`.
///   - `pricing_discount_id` (string | null, optional, default `null`): Separate discount curve used to price the quotes; `null` uses `curve_id`.
///   - `pricing_forward_id` (string | null, optional, default `null`): Forward curve used to price the quotes, if any.
///   - `conventions` (object, optional, default both `null`): Step-level rates conventions `{"curve_day_count": ..., "ois_compounding": ...}`; both entries default to `null` (the quote conventions apply).
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "discount"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepDiscount, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_discount(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("discount", &id, &params, quote_set.as_ref())
}

/// Build a projection (forward) curve calibration step.
///
/// Free-function twin of Python `CalibrationStep.forward` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name and `curve_id`.
/// @param params - Object (or JSON text) holding every wire field of the `forward` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `currency` (string, required): Upper-case ISO 4217 code of the curve currency.
///   - `base_date` (string, required): Curve base date as an ISO-8601 date string (`"YYYY-MM-DD"`).
///   - `tenor_years` (number, required): Index accrual tenor in years (`0.25` for a 3M index).
///   - `discount_curve_id` (string, required): Identifier of the discount curve solved in an earlier step.
///   - `curve_id` (string, optional, default `id`): Identifier of the produced curve.
///   - `method` (object, optional, default `global_solve` with a finite-difference Jacobian): Calibration method; forward curves require `global_solve` (`"bootstrap"` is rejected).
///   - `interpolation` (string, optional, default `"cubic_hermite"`): Forward-rate interpolation: `"linear"`, `"log_linear"`, `"monotone_convex"`, `"cubic_hermite"` or `"piecewise_quadratic_forward"`; `"monotone_convex"` is rejected.
///   - `conventions` (object, optional, default both `null`): Step-level rates conventions `{"curve_day_count": ..., "ois_compounding": ...}`; both entries default to `null` (the quote conventions apply).
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "forward"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepForward, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_forward(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("forward", &id, &params, quote_set.as_ref())
}

/// Build a credit hazard-curve bootstrap step.
///
/// Free-function twin of Python `CalibrationStep.hazard` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name and `curve_id`.
/// @param params - Object (or JSON text) holding every wire field of the `hazard` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `entity` (string, required): Reference entity identifier.
///   - `currency` (string, required): Upper-case ISO 4217 code of the contract currency.
///   - `base_date` (string, required): Curve base date as an ISO-8601 date string (`"YYYY-MM-DD"`).
///   - `discount_curve_id` (string, required): Identifier of the discount curve solved in an earlier step.
///   - `recovery_rate` (number, required): Assumed recovery as a decimal fraction of notional in `[0, 1)`.
///   - `seniority` (string | null, optional, default `"senior"`): Debt seniority recorded on the curve: `"senior_secured"`, `"senior"`, `"subordinated"` or `"junior"`. Omitted or `null` uses the Rust authoring default `CalibrationStep::DEFAULT_HAZARD_SENIORITY`.
///   - `curve_id` (string, optional, default `id`): Identifier of the produced curve.
///   - `notional` (number, optional, default `1.0`): Notional of the synthetic CDS repriced during calibration.
///   - `method` (string | object, optional, default `"bootstrap"`): Calibration method: `"bootstrap"` or `{"global_solve": {"use_analytical_jacobian": bool}}`.
///   - `par_interp` (string, optional, default `"linear"`): Interpolation of reported par spreads: `"linear"` or `"log_linear"`.
///   - `doc_clause` (string | null, optional, default `null`): CDS documentation-clause assertion; when set it must match the quote conventions.
///   - `cds_valuation_convention` (string | null, optional, default `null`): Synthetic-CDS valuation policy: `"isda_dirty"`, `"bloomberg_cdsw_clean"`, `"bloomberg_cdsw_clean_full_premium"` or `"quant_lib_isda_parity"`.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "hazard"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepHazard, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_hazard(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("hazard", &id, &params, quote_set.as_ref())
}

/// Build an inflation-curve bootstrap step.
///
/// Free-function twin of Python `CalibrationStep.inflation` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name and `curve_id`.
/// @param params - Object (or JSON text) holding every wire field of the `inflation` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `currency` (string, required): Upper-case ISO 4217 code of the curve currency.
///   - `base_date` (string, required): Valuation and calibration-instrument start date as an ISO-8601 date string (`"YYYY-MM-DD"`). The curve's reference CPI date is this date minus `observation_lag`.
///   - `discount_curve_id` (string, required): Identifier of the discount curve solved in an earlier step.
///   - `index` (string, required): Inflation index identifier, for example `"USA-CPI-U"`.
///   - `observation_lag` (string, required): Whole-month observation lag, for example `"3M"`, up to 255 months. No lag accepts `""`, `"none"`, `"0"`, `"0M"` or `"0D"` (case-insensitive); nonzero day lags are rejected.
///   - `base_cpi` (number, required): Contractual reference CPI at the start date in index points, after observation lag and monthly interpolation; supplied index fixings must reproduce it, including seasonality.
///   - `curve_id` (string, optional, default `id`): Identifier of the produced curve.
///   - `notional` (number, optional, default `1.0`): Notional of the synthetic inflation swaps repriced during calibration.
///   - `method` (string | object, optional, default `"bootstrap"`): Calibration method: `"bootstrap"` or `{"global_solve": {"use_analytical_jacobian": bool}}`.
///   - `interpolation` (string, optional, default `"linear"`): CPI interpolation: `"linear"`, `"log_linear"`, `"monotone_convex"`, `"cubic_hermite"` or `"piecewise_quadratic_forward"`.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "inflation"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepInflation, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_inflation(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("inflation", &id, &params, quote_set.as_ref())
}

/// Build an equity/FX SABR volatility-surface fit step from option vol quotes.
///
/// Free-function twin of Python `CalibrationStep.vol_surface` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name and `vol_surface_id`.
/// @param params - Object (or JSON text) holding every wire field of the `vol_surface` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `base_date` (string, required): Surface base date as an ISO-8601 date string (`"YYYY-MM-DD"`).
///   - `underlying_ticker` (string, required): Underlying identifier the quotes reference.
///   - `vol_surface_id` (string, optional, default `id`): Identifier of the produced surface.
///   - `discount_curve_id` (string | null, optional, default `null`): Discount curve used for forwards.
///   - `beta` (number, optional, default `0.5`): SABR beta.
///   - `target_expiries` (number[], optional, default `[]`): Expiry grid of the published surface in years.
///   - `target_strikes` (number[], optional, default `[]`): Strike grid of the published surface in price units.
///   - `spot_override` (number | null, optional, default `null`): Spot price used instead of the market scalar.
///   - `dividend_yield_override` (number | null, optional, default `null`): Continuous dividend yield as a decimal.
///   - `expiry_extrapolation` (string, optional, default `"error"`): Total-variance fill outside the calibrated expiries: `"error"` or `"clamp"`.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "vol_surface"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepVolSurface, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_vol_surface(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("vol_surface", &id, &params, quote_set.as_ref())
}

/// Build a swaption volatility-cube (SABR) fit step.
///
/// Free-function twin of Python `CalibrationStep.swaption_vol` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name and `vol_surface_id`.
/// @param params - Object (or JSON text) holding every wire field of the `swaption_vol` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `base_date` (string, required): Surface base date as an ISO-8601 date string (`"YYYY-MM-DD"`).
///   - `discount_curve_id` (string, required): Identifier of the discount curve solved in an earlier step.
///   - `currency` (string, required): Upper-case ISO 4217 code of the swaption currency.
///   - `vol_surface_id` (string, optional, default `id`): Identifier of the produced surface.
///   - `forward_id` (string | null, optional, default `null`): Forward curve, if different from the discount curve.
///   - `swap_index` (string | null, optional, default `null`): Floating index whose conventions define the underlying swaps; when omitted every quote must carry `float_leg_conventions.index`.
///   - `vol_convention` (string | object, optional, default `"lognormal"`): Quoting convention: `"normal"`, `"lognormal"` or `{"shifted_lognormal": ...}`.
///   - `sabr_beta` (number, optional, default `0.5`): SABR beta (`0.0` normal-like, `1.0` lognormal).
///   - `sabr_extrapolation` (string, optional, default `"error"`): SABR-parameter extrapolation across the expiry-tenor grid: `"error"` or `"clamp"`.
///   - `allow_sabr_missing_bucket_fallback` (boolean, optional, default `false`): Allow a nearby bucket to stand in for a missing SABR grid corner.
///   - `target_expiries` (number[], optional, default `[]`): Option expiries in ACT/365F years from `base_date`.
///   - `target_tenors` (number[], optional, default `[]`): Underlying swap tenors in years.
///   - `fixed_day_count` (string | null, optional, default `null`): Fixed-leg coupon accrual day count only; option expiry and variance always use ACT/365F.
///   - `calendar_id` (string | null, optional, default `null`): Holiday calendar for date adjustments.
///   - `vol_tolerance` (number | null, optional, default `null` (0.0015)): Maximum absolute error of any fitted volatility, in the quote's volatility units.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "swaption_vol"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepSwaptionVol, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_swaption_vol(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("swaption_vol", &id, &params, quote_set.as_ref())
}

/// Build an index-tranche base-correlation bootstrap step.
///
/// Free-function twin of Python `CalibrationStep.base_correlation` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name.
/// @param params - Object (or JSON text) holding every wire field of the `base_correlation` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `index_id` (string, required): Credit index identifier, for example `"CDX.NA.IG"`.
///   - `series` (integer, required): Index series number.
///   - `maturity_years` (number, required): Finite positive CDS tenor in years, a whole number of months; quotes must resolve to the same convention maturity.
///   - `base_date` (string, required): Calibration base date as an ISO-8601 date string (`"YYYY-MM-DD"`).
///   - `discount_curve_id` (string, required): Identifier of the discount curve solved in an earlier step.
///   - `currency` (string, required): Upper-case ISO 4217 code of the tranche currency.
///   - `detachment_points` (number[], optional, default `[]`): Expected detachment points as decimals in `(0, 1]`; when non-empty every quote must match one point and every point must be quoted.
///   - `notional` (number, optional, default `1.0`): Notional of the synthetic tranches.
///   - `frequency` (object | null, optional, default `null`): Premium frequency as a tenor object (convention default).
///   - `day_count` (string | null, optional, default `null`): Premium accrual day count (convention default).
///   - `business_day_convention` (string | null, optional, default `null`): Schedule business-day convention.
///   - `calendar_id` (string | null, optional, default `null`): Holiday calendar for schedule generation.
///   - `roll_rule` (string, optional, default `"none"`): Coupon roll grid: `"cds_imm"` (20 Mar/Jun/Sep/Dec) or `"none"`; `"imm"` is rejected.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "base_correlation"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepBaseCorrelation, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_base_correlation(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("base_correlation", &id, &params, quote_set.as_ref())
}

/// Build a Student-t copula degrees-of-freedom fit step for one tranche.
///
/// Free-function twin of Python `CalibrationStep.student_t` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name.
/// @param params - Object (or JSON text) holding every wire field of the `student_t` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `tranche_instrument_id` (string, required): Tranche instrument repriced during the fit; the result is written to the scalar `"{tranche_instrument_id}_STUDENT_T_DF"`.
///   - `base_correlation_curve_id` (string, required): Identifier of the base-correlation curve solved in an earlier step.
///   - `discount_curve_id` (string | null, optional, default `null`): Discount curve pricing the tranche; `null` uses the only discount curve in the market.
///   - `initial_df` (number, optional, default `5.0`): Starting degrees of freedom.
///   - `df_bounds` (number[], optional, default `[2.1, 50.0]`): Feasible `[lower, upper]` degrees of freedom (> 2).
///   - `correlation` (number, optional, default `0.3`): Flat correlation of the tranche.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "student_t"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepStudentT, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_student_t(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("student_t", &id, &params, quote_set.as_ref())
}

/// Build a Hull-White one-factor swaption calibration step.
///
/// Free-function twin of Python `CalibrationStep.hull_white` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name.
/// @param params - Object (or JSON text) holding every wire field of the `hull_white` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `curve_id` (string, required): Discount curve the model is fitted against (must already exist).
///   - `currency` (string, required): Upper-case ISO 4217 code of the model currency.
///   - `base_date` (string, required): Model base date as an ISO-8601 date string (`"YYYY-MM-DD"`).
///   - `fit_tolerance` (number, required): Finite positive maximum implied-quote error in the supplied volatility units (decimal rate volatility for normal quotes, relative volatility for Black quotes), separate from the solver tolerance.
///   - `initial_kappa` (number | null, optional, default `null`): Initial mean-reversion guess.
///   - `initial_sigma` (number | null, optional, default `null`): Initial short-rate volatility guess.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "hull_white"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepHullWhite, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_hull_white(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("hull_white", &id, &params, quote_set.as_ref())
}

/// Build a Hull-White one-factor cap/floor calibration step.
///
/// Free-function twin of Python `CalibrationStep.cap_floor_hull_white` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name.
/// @param params - Object (or JSON text) holding every wire field of the `cap_floor_hull_white` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `discount_curve_id` (string, required): Discounting curve and output-key prefix. Scalar fits write `"{discount_curve_id}_CAPFLOOR_HW1F_KAPPA"` and `"{discount_curve_id}_CAPFLOOR_HW1F_SIGMA"`; piecewise fits write the same kappa scalar and the series `"{discount_curve_id}_CAPFLOOR_HW1F_SIGMA_SCHEDULE"`.
///   - `forward_curve_id` (string, required): Curve projecting the caplet forwards; equal to `discount_curve_id` for single-curve.
///   - `index_id` (string, required): Term-rate index, for example `"EUR-EURIBOR-3M"`, whose conventions set the caplet dates and accruals; overnight indices are rejected.
///   - `currency` (string, required): Upper-case ISO 4217 code of the model currency; must match the index.
///   - `base_date` (string, required): Model base date as an ISO-8601 date string (`"YYYY-MM-DD"`).
///   - `fit_tolerance` (number, required): Finite positive maximum implied-quote error in the supplied volatility units, separate from the solver tolerance.
///   - `volatility_mode` (string, optional, default `"scalar"`): `"scalar"` or `"piecewise"` sigma.
///   - `fixed_kappa` (number | null, optional, default `null`): Fixed mean reversion; required for a one-quote fit.
///   - `initial_kappa` (number | null, optional, default `null`): Initial mean-reversion guess.
///   - `initial_sigma` (number | null, optional, default `null`): Initial short-rate volatility guess.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "cap_floor_hull_white"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepCapFloorHullWhite, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_cap_floor_hull_white(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("cap_floor_hull_white", &id, &params, quote_set.as_ref())
}

/// Build an SVI volatility-surface fit step from option vol quotes.
///
/// Free-function twin of Python `CalibrationStep.svi_surface` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name and `vol_surface_id`.
/// @param params - Object (or JSON text) holding every wire field of the `svi_surface` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `base_date` (string, required): Surface base date as an ISO-8601 date string (`"YYYY-MM-DD"`).
///   - `underlying_ticker` (string, required): Underlying identifier the quotes reference.
///   - `vol_surface_id` (string, optional, default `id`): Identifier of the produced surface.
///   - `discount_curve_id` (string | null, optional, default `null`): Discount curve used for forwards.
///   - `target_expiries` (number[], optional, default `[]`): Expiry grid of the published surface in years.
///   - `target_strikes` (number[], optional, default `[]`): Strike grid of the published surface in price units.
///   - `spot_override` (number | null, optional, default `null`): Spot price used instead of the market scalar.
///   - `dividend_yield_override` (number | null, optional, default `null`): Continuous dividend yield as a decimal; `null` reads the scalar `"{underlying_ticker}-DIVYIELD"` (or a cash-dividend schedule when a discount curve is set).
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "svi_surface"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepSviSurface, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_svi_surface(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("svi_surface", &id, &params, quote_set.as_ref())
}

/// Build a cross-currency basis (foreign discount curve) bootstrap step.
///
/// Free-function twin of Python `CalibrationStep.xccy_basis` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name and `curve_id`.
/// @param params - Object (or JSON text) holding every wire field of the `xccy_basis` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `currency` (string, required): Upper-case ISO 4217 code of the foreign (solved) currency.
///   - `base_date` (string, required): Curve base date as an ISO-8601 date string (`"YYYY-MM-DD"`).
///   - `fx_spot` (number, required): T+0 cash FX rate (domestic per foreign) used when a quote omits `spot_fx`.
///   - `domestic_discount_id` (string, required): Domestic discount curve solved in an earlier step.
///   - `curve_id` (string, optional, default `id`): Identifier of the produced foreign curve.
///   - `basis_spread_curve_id` (string | null, optional, default `null`): Identifier of the byproduct basis-spread curve.
///   - `method` (string, optional, default `"bootstrap"`): Sequential `"bootstrap"`; `global_solve` is rejected.
///   - `interpolation` (string, optional, default `"log_linear"`): Discount-factor interpolation: `"linear"`, `"log_linear"`, `"monotone_convex"`, `"cubic_hermite"` or `"piecewise_quadratic_forward"`.
///   - `extrapolation` (string, optional, default `"flat_forward"`): Extrapolation policy: `"flat_zero"`, `"flat_forward"` or `"none"`.
///   - `conventions` (object, optional, default both `null`): Step-level rates conventions `{"curve_day_count": ..., "ois_compounding": ...}`; both entries default to `null` (the quote conventions apply).
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "xccy_basis"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepXccyBasis, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_xccy_basis(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("xccy_basis", &id, &params, quote_set.as_ref())
}

/// Build a parametric (Nelson-Siegel / Svensson) single-curve fit step.
///
/// Free-function twin of Python `CalibrationStep.parametric` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the residual key in calibration reports and the default quote-set name and `curve_id`.
/// @param params - Object (or JSON text) holding every wire field of the `parametric` step, keyed by its snake_case serde name as in the calibration JSON schema:
///   - `base_date` (string, required): Curve base date as an ISO-8601 date string (`"YYYY-MM-DD"`).
///   - `curve_id` (string, optional, default `id`): Identifier of the single discount curve fitted; every instrument discounts and projects on it.
///   - `model` (string | null, optional, default `"ns"`): `"ns"` (Nelson-Siegel) or `"nss"` (Svensson). Omitted or `null` uses the Rust authoring default `CalibrationStep::DEFAULT_PARAMETRIC_MODEL`.
///   - `initial_params` (object | null, optional, default `null`): Initial guesses as a `NelsonSiegelModel` object (`beta0`, `beta1`, `beta2`, `tau` for NS; plus `beta3`, `tau1`, `tau2` for NSS). Fit acceptance uses the discount-curve tolerance in per-notional PV units without a floor; `discount_curve_id` is not accepted.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @returns A `CalibrationStep` object with `kind: "parametric"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `id` or `quoteSet` is not a string or
/// `params` is neither an object nor JSON text, and a `FinstackError` (`kind: "validation"`)
/// if a required field is missing or a field is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepParametric, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_parametric(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Record<string, unknown> | string")] params: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    step("parametric", &id, &params, quote_set.as_ref())
}
