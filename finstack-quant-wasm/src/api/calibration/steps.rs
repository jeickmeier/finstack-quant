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
/// @param base_date - ISO-8601 base (valuation) date.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curve_id - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `method` (`"bootstrap"` or `"global"`), `interpolation` (default `"log_linear"`), `extrapolation`, `pricing_discount_id`, `pricing_forward_id`, `conventions`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "discount"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepDiscount)]
pub fn calibration_step_discount(
    id: JsValue,
    currency: JsValue,
    base_date: JsValue,
    quote_set: Option<JsValue>,
    curve_id: Option<JsValue>,
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
/// @param base_date - ISO-8601 base (valuation) date.
/// @param tenor_years - Index accrual tenor in years (`0.25` for 3M).
/// @param discount_curve_id - Identifier of the discount curve used to price the quotes.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curve_id - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `method`, `interpolation` (default `"monotone_convex"`), `conventions`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "forward"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepForward)]
#[allow(clippy::too_many_arguments)]
pub fn calibration_step_forward(
    id: JsValue,
    currency: JsValue,
    base_date: JsValue,
    tenor_years: JsValue,
    discount_curve_id: JsValue,
    quote_set: Option<JsValue>,
    curve_id: Option<JsValue>,
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
/// @param base_date - ISO-8601 base (valuation) date.
/// @param discount_curve_id - Discount curve used for CDS present values.
/// @param recovery_rate - Assumed recovery rate as a decimal.
/// @param seniority - Debt seniority label (`"senior_secured"`, `"senior"`, `"subordinated"`, `"junior"`); defaults to `"senior"`.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curve_id - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `notional`, `method`, `interpolation`, `par_interp`, `doc_clause`, `cds_valuation_convention`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "hazard"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepHazard)]
#[allow(clippy::too_many_arguments)]
pub fn calibration_step_hazard(
    id: JsValue,
    entity: JsValue,
    currency: JsValue,
    base_date: JsValue,
    discount_curve_id: JsValue,
    recovery_rate: JsValue,
    seniority: Option<JsValue>,
    quote_set: Option<JsValue>,
    curve_id: Option<JsValue>,
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
/// @param base_date - ISO-8601 base (valuation) date.
/// @param discount_curve_id - Discount curve used for swap present values.
/// @param index - Inflation index identifier (for example `"USA-CPI-U"`).
/// @param observation_lag - Observation lag tenor (for example `"3M"`).
/// @param base_cpi - CPI index level at the base date.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curve_id - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `notional`, `method`, `interpolation`, `seasonal_factors`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "inflation"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepInflation)]
#[allow(clippy::too_many_arguments)]
pub fn calibration_step_inflation(
    id: JsValue,
    currency: JsValue,
    base_date: JsValue,
    discount_curve_id: JsValue,
    index: JsValue,
    observation_lag: JsValue,
    base_cpi: JsValue,
    quote_set: Option<JsValue>,
    curve_id: Option<JsValue>,
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
/// @param base_date - ISO-8601 surface base date.
/// @param underlying_ticker - Underlying identifier the quotes reference.
/// @param model - Surface model label; defaults to `"sabr"`.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param vol_surface_id - Identifier of the produced surface; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `discount_curve_id`, `beta`, `target_expiries`, `target_strikes`, `spot_override`, `dividend_yield_override`, `expiry_extrapolation`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "vol_surface"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepVolSurface)]
pub fn calibration_step_vol_surface(
    id: JsValue,
    base_date: JsValue,
    underlying_ticker: JsValue,
    model: Option<JsValue>,
    quote_set: Option<JsValue>,
    vol_surface_id: Option<JsValue>,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .date("base_date", &base_date, "baseDate")?
        .string("underlying_ticker", &underlying_ticker, "underlyingTicker")?
        .opt_string("model", model.as_ref(), "model")?
        .opt_string("vol_surface_id", vol_surface_id.as_ref(), "volSurfaceId")?;
    step("vol_surface", &id, quote_set.as_ref(), fields)
}

/// Build a swaption volatility cube step from swaption vol quotes.
///
/// Free-function twin of Python `CalibrationStep.swaption_vol` (Rust
/// `CalibrationStep::from_wire_fields`). Quotes are listed in the envelope's
/// `market_data` and named in `plan.quote_sets` rather than attached to the step.
/// @param id - Step identifier; also the default quote-set name and the default identifier of the produced object.
/// @param base_date - ISO-8601 surface base date.
/// @param discount_curve_id - Discount curve for forward-swap-rate construction.
/// @param currency - ISO-4217 currency code of the surface.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param vol_surface_id - Identifier of the produced surface; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `forward_id`, `vol_convention`, `sabr_beta`, `target_expiries`, `target_tenors`, `sabr_interpolation`, `calendar_id`, `fixed_day_count`, `swap_index`, `vol_tolerance`, `sabr_extrapolation`, `allow_sabr_missing_bucket_fallback`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "swaption_vol"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepSwaptionVol)]
pub fn calibration_step_swaption_vol(
    id: JsValue,
    base_date: JsValue,
    discount_curve_id: JsValue,
    currency: JsValue,
    quote_set: Option<JsValue>,
    vol_surface_id: Option<JsValue>,
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
/// @param index_id - Credit index identifier (the curve is written as `"{index_id}_CORR"`).
/// @param series - Index series number (non-negative integer).
/// @param maturity_years - Tranche maturity in years.
/// @param base_date - ISO-8601 valuation date.
/// @param discount_curve_id - Discount curve for tranche present values.
/// @param currency - ISO-4217 currency code of the index.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `notional`, `frequency`, `day_count`, `business_day_convention`, `calendar_id`, `detachment_points`, `roll_rule`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "base_correlation"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepBaseCorrelation)]
#[allow(clippy::too_many_arguments)]
pub fn calibration_step_base_correlation(
    id: JsValue,
    index_id: JsValue,
    series: JsValue,
    maturity_years: JsValue,
    base_date: JsValue,
    discount_curve_id: JsValue,
    currency: JsValue,
    quote_set: Option<JsValue>,
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
/// @param tranche_instrument_id - Tranche instrument whose `"{id}_STUDENT_T_DF"` scalar is written.
/// @param base_correlation_curve_id - Base-correlation curve the tranche is priced on.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `discount_curve_id`, `initial_df`, `df_bounds`, `correlation`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "student_t"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepStudentT)]
pub fn calibration_step_student_t(
    id: JsValue,
    tranche_instrument_id: JsValue,
    base_correlation_curve_id: JsValue,
    quote_set: Option<JsValue>,
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
/// @param curve_id - Discount curve the model is calibrated on (scalars are written as `"{curve_id}_HW1F"`).
/// @param currency - ISO-4217 currency code of the model.
/// @param base_date - ISO-8601 valuation date.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `initial_kappa`, `initial_sigma`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "hull_white"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepHullWhite)]
pub fn calibration_step_hull_white(
    id: JsValue,
    curve_id: JsValue,
    currency: JsValue,
    base_date: JsValue,
    quote_set: Option<JsValue>,
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
/// @param discount_curve_id - Discounting curve (scalars are written as `"{discount_curve_id}_CAPFLOOR_HW1F"`).
/// @param forward_curve_id - Curve projecting the caplet forwards.
/// @param currency - ISO-4217 currency code of the model.
/// @param base_date - ISO-8601 valuation date.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `fixed_kappa`, `initial_kappa`, `initial_sigma`, `payment_frequency`, `volatility_mode`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "cap_floor_hull_white"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepCapFloorHullWhite)]
pub fn calibration_step_cap_floor_hull_white(
    id: JsValue,
    discount_curve_id: JsValue,
    forward_curve_id: JsValue,
    currency: JsValue,
    base_date: JsValue,
    quote_set: Option<JsValue>,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .string("discount_curve_id", &discount_curve_id, "discountCurveId")?
        .string("forward_curve_id", &forward_curve_id, "forwardCurveId")?
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
/// @param base_date - ISO-8601 surface base date.
/// @param underlying_ticker - Underlying identifier the quotes reference.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param vol_surface_id - Identifier of the produced surface; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `discount_curve_id`, `target_expiries`, `target_strikes`, `spot_override`, `dividend_yield_override`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "svi_surface"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepSviSurface)]
pub fn calibration_step_svi_surface(
    id: JsValue,
    base_date: JsValue,
    underlying_ticker: JsValue,
    quote_set: Option<JsValue>,
    vol_surface_id: Option<JsValue>,
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
/// @param base_date - ISO-8601 base (valuation) date.
/// @param fx_spot - Spot FX rate used to translate the basis quotes.
/// @param domestic_discount_id - Domestic discount curve the basis is measured against.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curve_id - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `method`, `interpolation`, `extrapolation`, `conventions`, `basis_spread_curve_id`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "xccy_basis"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepXccyBasis)]
#[allow(clippy::too_many_arguments)]
pub fn calibration_step_xccy_basis(
    id: JsValue,
    currency: JsValue,
    base_date: JsValue,
    fx_spot: JsValue,
    domestic_discount_id: JsValue,
    quote_set: Option<JsValue>,
    curve_id: Option<JsValue>,
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
/// @param base_date - ISO-8601 base (valuation) date.
/// @param model - Parametric family: `"ns"` (Nelson-Siegel) or `"nss"` (Svensson); defaults to `"ns"`.
/// @param quote_set - Name of the quote set in `plan.quote_sets`; defaults to `id`.
/// @param curve_id - Identifier of the produced curve; defaults to `id`.
/// @param params - Optional object (or JSON) of further wire fields: `initial_params`. An entry named like another argument is replaced by that argument.
/// @returns A `CalibrationStep` object with `kind: "parametric"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601 or a field
/// (including a `params` entry) is unknown or has the wrong shape.
#[wasm_bindgen(js_name = calibrationStepParametric)]
pub fn calibration_step_parametric(
    id: JsValue,
    base_date: JsValue,
    model: Option<JsValue>,
    quote_set: Option<JsValue>,
    curve_id: Option<JsValue>,
    params: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::from_overrides(params.as_ref())?
        .date("base_date", &base_date, "baseDate")?
        .opt_string("model", model.as_ref(), "model")?
        .opt_string("curve_id", curve_id.as_ref(), "curveId")?;
    step("parametric", &id, quote_set.as_ref(), fields)
}
