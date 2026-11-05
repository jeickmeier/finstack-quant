//! Free-function twins of the Python `CalibrationStep` constructors.
//!
//! Each function marshals its arguments into the wire fields of one
//! `StepParams` variant and calls Rust `CalibrationStep::from_wire_fields`,
//! which owns the omitted-argument defaults (quote set and produced-object
//! identifiers fall back to the step id) and the strict deserialization. The
//! result is the plain `CalibrationStep` object placed in `plan.steps`.
//!
//! The Python constructors additionally accept `quotes=[...]`, a side-car
//! consumed by the Python `CalibrationPlan` class. A plain `CalibrationStep`
//! object has no such slot: JavaScript callers list quotes in the envelope's
//! `market_data` and name their ids in `plan.quote_sets`.

use super::fields::Fields;
use crate::utils::input::{js_opt_string, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_calibration::api::schema::CalibrationStep;
use wasm_bindgen::prelude::*;

fn step(
    kind: &str,
    id: &JsValue,
    quote_set: Option<&JsValue>,
    fields: Fields,
) -> Result<JsValue, JsValue> {
    let id = js_string(id, "id")?;
    let quote_set = js_opt_string(quote_set, "quoteSet")?;
    let step = CalibrationStep::from_wire_fields(kind, &id, quote_set.as_deref(), fields.0)
        .map_err(to_js_err)?;
    to_js_value(&step)
}

/// Build a discount-curve bootstrap step.
///
/// Free-function twin of Python `CalibrationStep.discount` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name and the default identifier of the produced object.
/// @param currency - ISO-4217 currency code of the curve (for example `"USD"`).
/// @param baseDate - ISO-8601 base (valuation) date.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curveId - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `method` (`"bootstrap"` or `"global"`), `interpolation` (default `"log_linear"`), `extrapolation`, `pricing_discount_id`, `pricing_forward_id`, `conventions`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "discount"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepDiscount, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_discount(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(js_name = "curveId", unchecked_optional_param_type = "string | null")] curve_id: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .string("currency", &currency, "currency")?
        .date("base_date", &base_date, "baseDate")?
        .opt_string("curve_id", curve_id.as_ref(), "curveId")?;
    step("discount", &id, quote_set.as_ref(), fields)
}

/// Build a forward (index projection) curve step discounted on an existing curve.
///
/// Free-function twin of Python `CalibrationStep.forward` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name and the default identifier of the produced object.
/// @param currency - ISO-4217 currency code of the curve (for example `"USD"`).
/// @param baseDate - ISO-8601 base (valuation) date.
/// @param tenorYears - Index accrual tenor in years (`0.25` for 3M).
/// @param discountCurveId - Identifier of the discount curve used to price the quotes.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curveId - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `method`, `interpolation` (default `"cubic_hermite"`), `conventions`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "forward"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepForward, unchecked_return_type = "generated.calibration.CalibrationStep")]
#[allow(clippy::too_many_arguments)]
pub fn calibration_step_forward(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(js_name = "tenorYears", unchecked_param_type = "number")] tenor_years: JsValue,
    #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")] discount_curve_id: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(js_name = "curveId", unchecked_optional_param_type = "string | null")] curve_id: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .string("currency", &currency, "currency")?
        .date("base_date", &base_date, "baseDate")?
        .number("tenor_years", &tenor_years, "tenorYears")?
        .string("discount_curve_id", &discount_curve_id, "discountCurveId")?
        .opt_string("curve_id", curve_id.as_ref(), "curveId")?;
    step("forward", &id, quote_set.as_ref(), fields)
}

/// Build a hazard-curve bootstrap step from CDS quotes.
///
/// Free-function twin of Python `CalibrationStep.hazard` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name and the default identifier of the produced object.
/// @param entity - Reference entity name.
/// @param currency - ISO-4217 currency code of the curve (for example `"USD"`).
/// @param baseDate - ISO-8601 base (valuation) date.
/// @param discountCurveId - Discount curve used for CDS present values.
/// @param recoveryRate - Assumed recovery rate as a decimal.
/// @param seniority - Debt seniority label (`"senior_secured"`, `"senior"`, `"subordinated"`, `"junior"`); defaults to `"senior"`.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curveId - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `notional`, `method`, `interpolation`, `par_interp`, `doc_clause`, `cds_valuation_convention`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "hazard"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepHazard, unchecked_return_type = "generated.calibration.CalibrationStep")]
#[allow(clippy::too_many_arguments)]
pub fn calibration_step_hazard(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] entity: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")] discount_curve_id: JsValue,
    #[wasm_bindgen(js_name = "recoveryRate", unchecked_param_type = "number")]
    recovery_rate: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "string | null")] seniority: Option<JsValue>,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(js_name = "curveId", unchecked_optional_param_type = "string | null")] curve_id: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .string("entity", &entity, "entity")?
        .string("currency", &currency, "currency")?
        .date("base_date", &base_date, "baseDate")?
        .string("discount_curve_id", &discount_curve_id, "discountCurveId")?
        .number("recovery_rate", &recovery_rate, "recoveryRate")?
        .opt_string("seniority", seniority.as_ref(), "seniority")?
        .opt_string("curve_id", curve_id.as_ref(), "curveId")?;
    step("hazard", &id, quote_set.as_ref(), fields)
}

/// Build an inflation (CPI projection) curve step from inflation-swap quotes.
///
/// Free-function twin of Python `CalibrationStep.inflation` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name and the default identifier of the produced object.
/// @param currency - ISO-4217 currency code of the curve (for example `"USD"`).
/// @param baseDate - ISO-8601 base (valuation) date.
/// @param discountCurveId - Discount curve used for swap present values.
/// @param index - Inflation index identifier (for example `"USA-CPI-U"`).
/// @param observationLag - Observation lag tenor (for example `"3M"`).
/// @param baseCpi - CPI index level at the base date.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curveId - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `notional`, `method`, `interpolation`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "inflation"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepInflation, unchecked_return_type = "generated.calibration.CalibrationStep")]
#[allow(clippy::too_many_arguments)]
pub fn calibration_step_inflation(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")] discount_curve_id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] index: JsValue,
    #[wasm_bindgen(js_name = "observationLag", unchecked_param_type = "string")]
    observation_lag: JsValue,
    #[wasm_bindgen(js_name = "baseCpi", unchecked_param_type = "number")] base_cpi: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(js_name = "curveId", unchecked_optional_param_type = "string | null")] curve_id: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .string("currency", &currency, "currency")?
        .date("base_date", &base_date, "baseDate")?
        .string("discount_curve_id", &discount_curve_id, "discountCurveId")?
        .string("index", &index, "index")?
        .string("observation_lag", &observation_lag, "observationLag")?
        .number("base_cpi", &base_cpi, "baseCpi")?
        .opt_string("curve_id", curve_id.as_ref(), "curveId")?;
    step("inflation", &id, quote_set.as_ref(), fields)
}

/// Build an equity or FX volatility-surface (SABR) step from option vol quotes.
///
/// Free-function twin of Python `CalibrationStep.vol_surface` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name and the default identifier of the produced object.
/// @param baseDate - ISO-8601 surface base date.
/// @param underlyingTicker - Underlying identifier the quotes reference.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param volSurfaceId - Identifier of the produced surface; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `discount_curve_id`, `beta`, `target_expiries`, `target_strikes`, `spot_override`, `dividend_yield_override`, `expiry_extrapolation`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "vol_surface"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepVolSurface, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_vol_surface(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(js_name = "underlyingTicker", unchecked_param_type = "string")]
    underlying_ticker: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "volSurfaceId",
        unchecked_optional_param_type = "string | null"
    )]
    vol_surface_id: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .date("base_date", &base_date, "baseDate")?
        .string("underlying_ticker", &underlying_ticker, "underlyingTicker")?
        .opt_string("vol_surface_id", vol_surface_id.as_ref(), "volSurfaceId")?;
    step("vol_surface", &id, quote_set.as_ref(), fields)
}

/// Build a swaption volatility cube step from swaption vol quotes.
///
/// Free-function twin of Python `CalibrationStep.swaption_vol` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name and the default identifier of the produced object.
/// @param baseDate - ISO-8601 surface base date.
/// @param discountCurveId - Discount curve for forward-swap-rate construction.
/// @param currency - ISO-4217 currency code of the surface.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param volSurfaceId - Identifier of the produced surface; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `forward_id`, `vol_convention`, `sabr_beta`, `target_expiries`, `target_tenors`, `calendar_id`, `fixed_day_count`, `swap_index`, `vol_tolerance`, `sabr_extrapolation`, `allow_sabr_missing_bucket_fallback`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "swaption_vol"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepSwaptionVol, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_swaption_vol(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")] discount_curve_id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "volSurfaceId",
        unchecked_optional_param_type = "string | null"
    )]
    vol_surface_id: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .date("base_date", &base_date, "baseDate")?
        .string("discount_curve_id", &discount_curve_id, "discountCurveId")?
        .string("currency", &currency, "currency")?
        .opt_string("vol_surface_id", vol_surface_id.as_ref(), "volSurfaceId")?;
    step("swaption_vol", &id, quote_set.as_ref(), fields)
}

/// Build an index-tranche base-correlation step.
///
/// Free-function twin of Python `CalibrationStep.base_correlation` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name.
/// @param indexId - Credit index identifier (the curve is written as `"{index_id}_CORR"`).
/// @param series - Index series number (non-negative integer).
/// @param maturityYears - Tranche maturity in years.
/// @param baseDate - ISO-8601 valuation date.
/// @param discountCurveId - Discount curve for tranche present values.
/// @param currency - ISO-4217 currency code of the index.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `notional`, `frequency`, `day_count`, `business_day_convention`, `calendar_id`, `detachment_points`, `roll_rule`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "base_correlation"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepBaseCorrelation, unchecked_return_type = "generated.calibration.CalibrationStep")]
#[allow(clippy::too_many_arguments)]
pub fn calibration_step_base_correlation(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(js_name = "indexId", unchecked_param_type = "string")] index_id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] series: JsValue,
    #[wasm_bindgen(js_name = "maturityYears", unchecked_param_type = "number")]
    maturity_years: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")] discount_curve_id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .string("index_id", &index_id, "indexId")?
        .series(&series)?
        .number("maturity_years", &maturity_years, "maturityYears")?
        .date("base_date", &base_date, "baseDate")?
        .string("discount_curve_id", &discount_curve_id, "discountCurveId")?
        .string("currency", &currency, "currency")?;
    step("base_correlation", &id, quote_set.as_ref(), fields)
}

/// Build a Student-t copula degrees-of-freedom step for one tranche.
///
/// Free-function twin of Python `CalibrationStep.student_t` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name.
/// @param trancheInstrumentId - Tranche instrument whose `"{id}_STUDENT_T_DF"` scalar is written.
/// @param baseCorrelationCurveId - Base-correlation curve the tranche is priced on.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `discount_curve_id`, `initial_df`, `df_bounds`, `correlation`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "student_t"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepStudentT, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_student_t(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(js_name = "trancheInstrumentId", unchecked_param_type = "string")]
    tranche_instrument_id: JsValue,
    #[wasm_bindgen(js_name = "baseCorrelationCurveId", unchecked_param_type = "string")]
    base_correlation_curve_id: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .string(
            "tranche_instrument_id",
            &tranche_instrument_id,
            "trancheInstrumentId",
        )?
        .string(
            "base_correlation_curve_id",
            &base_correlation_curve_id,
            "baseCorrelationCurveId",
        )?;
    step("student_t", &id, quote_set.as_ref(), fields)
}

/// Build a Hull-White one-factor calibration step on ATM swaption quotes.
///
/// Free-function twin of Python `CalibrationStep.hull_white` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name.
/// @param curveId - Discount curve the model is calibrated on (scalars are written as `"{curve_id}_HW1F_KAPPA"` and `"{curve_id}_HW1F_SIGMA"`).
/// @param currency - ISO-4217 currency code of the model.
/// @param baseDate - ISO-8601 valuation date.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `initial_kappa`, `initial_sigma`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "hull_white"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepHullWhite, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_hull_white(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(js_name = "curveId", unchecked_param_type = "string")] curve_id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .string("curve_id", &curve_id, "curveId")?
        .string("currency", &currency, "currency")?
        .date("base_date", &base_date, "baseDate")?;
    step("hull_white", &id, quote_set.as_ref(), fields)
}

/// Build a Hull-White one-factor calibration step on cap/floor quotes.
///
/// Free-function twin of Python `CalibrationStep.cap_floor_hull_white` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name.
/// @param discountCurveId - Discounting curve and output-key prefix: scalar fits write `"{discount_curve_id}_CAPFLOOR_HW1F_KAPPA"` and `"{discount_curve_id}_CAPFLOOR_HW1F_SIGMA"`; piecewise fits write the same kappa scalar and the series `"{discount_curve_id}_CAPFLOOR_HW1F_SIGMA_SCHEDULE"`.
/// @param forwardCurveId - Curve projecting the caplet forwards.
/// @param indexId - Term-rate index identifier (for example `"EUR-EURIBOR-3M"`) whose conventions set caplet settlement, fixing and payment dates, calendars and accrual fractions; its currency must match `currency` and overnight indices are rejected at calibration.
/// @param currency - ISO-4217 currency code of the model.
/// @param baseDate - ISO-8601 valuation date.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: the required `fit_tolerance` (positive maximum implied-quote error in quoted volatility units), `fixed_kappa`, `initial_kappa`, `initial_sigma`, `volatility_mode`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "cap_floor_hull_white"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = calibrationStepCapFloorHullWhite, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_cap_floor_hull_white(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")] discount_curve_id: JsValue,
    #[wasm_bindgen(js_name = "forwardCurveId", unchecked_param_type = "string")]
    forward_curve_id: JsValue,
    #[wasm_bindgen(js_name = "indexId", unchecked_param_type = "string")] index_id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .string("discount_curve_id", &discount_curve_id, "discountCurveId")?
        .string("forward_curve_id", &forward_curve_id, "forwardCurveId")?
        .string("index_id", &index_id, "indexId")?
        .string("currency", &currency, "currency")?
        .date("base_date", &base_date, "baseDate")?;
    step("cap_floor_hull_white", &id, quote_set.as_ref(), fields)
}

/// Build an SVI volatility-surface step from option vol quotes.
///
/// Free-function twin of Python `CalibrationStep.svi_surface` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name and the default identifier of the produced object.
/// @param baseDate - ISO-8601 surface base date.
/// @param underlyingTicker - Underlying identifier the quotes reference.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param volSurfaceId - Identifier of the produced surface; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `discount_curve_id`, `target_expiries`, `target_strikes`, `spot_override`, `dividend_yield_override`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "svi_surface"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepSviSurface, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_svi_surface(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(js_name = "underlyingTicker", unchecked_param_type = "string")]
    underlying_ticker: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "volSurfaceId",
        unchecked_optional_param_type = "string | null"
    )]
    vol_surface_id: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .date("base_date", &base_date, "baseDate")?
        .string("underlying_ticker", &underlying_ticker, "underlyingTicker")?
        .opt_string("vol_surface_id", vol_surface_id.as_ref(), "volSurfaceId")?;
    step("svi_surface", &id, quote_set.as_ref(), fields)
}

/// Build a cross-currency basis curve step.
///
/// Free-function twin of Python `CalibrationStep.xccy_basis` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name and the default identifier of the produced object.
/// @param currency - ISO-4217 foreign (collateral) currency of the produced discount curve.
/// @param baseDate - ISO-8601 base (valuation) date.
/// @param fxSpot - Spot FX rate used to translate the basis quotes.
/// @param domesticDiscountId - Domestic discount curve the basis is measured against.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curveId - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `method`, `interpolation`, `extrapolation`, `conventions`, `basis_spread_curve_id`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "xccy_basis"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepXccyBasis, unchecked_return_type = "generated.calibration.CalibrationStep")]
#[allow(clippy::too_many_arguments)]
pub fn calibration_step_xccy_basis(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(js_name = "fxSpot", unchecked_param_type = "number")] fx_spot: JsValue,
    #[wasm_bindgen(js_name = "domesticDiscountId", unchecked_param_type = "string")]
    domestic_discount_id: JsValue,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(js_name = "curveId", unchecked_optional_param_type = "string | null")] curve_id: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .string("currency", &currency, "currency")?
        .date("base_date", &base_date, "baseDate")?
        .number("fx_spot", &fx_spot, "fxSpot")?
        .string(
            "domestic_discount_id",
            &domestic_discount_id,
            "domesticDiscountId",
        )?
        .opt_string("curve_id", curve_id.as_ref(), "curveId")?;
    step("xccy_basis", &id, quote_set.as_ref(), fields)
}

/// Build a parametric (Nelson-Siegel or Svensson) curve fit step.
///
/// Free-function twin of Python `CalibrationStep.parametric` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name and the default identifier of the produced object.
/// @param baseDate - ISO-8601 base (valuation) date.
/// @param model - Parametric family: `"ns"` (Nelson-Siegel) or `"nss"` (Svensson); defaults to `"ns"`.
/// @param quoteSet - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curveId - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `initial_params`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "parametric"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepParametric, unchecked_return_type = "generated.calibration.CalibrationStep")]
pub fn calibration_step_parametric(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "string | null")] model: Option<JsValue>,
    #[wasm_bindgen(js_name = "quoteSet", unchecked_optional_param_type = "string | null")]
    quote_set: Option<JsValue>,
    #[wasm_bindgen(js_name = "curveId", unchecked_optional_param_type = "string | null")] curve_id: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "Record<string, unknown> | string | null")]
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .date("base_date", &base_date, "baseDate")?
        .opt_string("model", model.as_ref(), "model")?
        .opt_string("curve_id", curve_id.as_ref(), "curveId")?;
    step("parametric", &id, quote_set.as_ref(), fields)
}
