//! Cashflow builder specifications: constructors, presets and model
//! evaluations over wire values.
//!
//! Every spec crosses the boundary as the plain object (or wire string) its
//! serde contract describes, typed by the generated `cashflows` TypeScript
//! declarations. The Rust associated functions and methods are free functions
//! here: constructors return the canonical wire value, and methods take it as
//! their first argument.

use crate::utils::input::{js_currency, js_f64, js_uint};
use crate::utils::wire::{js_decimal, js_wire};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_cashflows::builder::{
    DefaultModelSpec, FloatingLegCompounding, FloatingRateSpec, Notional, PrepaymentModelSpec,
    RecoveryModelSpec, ScheduleParams,
};
use wasm_bindgen::prelude::*;

// --- Floating-rate conventions ----------------------------------------------

/// USD SOFR FRN compounding convention: ISDA 2021 observation shift of 2 business days (Rust `FloatingLegCompounding::sofr_observation_shift`).
///
/// @returns `FloatingLegCompounding` wire value `{ compounded_with_observation_shift: { shift_days: 2 } }`.
#[wasm_bindgen(js_name = floatingLegCompoundingSofrObservationShift)]
pub fn floating_leg_compounding_sofr_observation_shift() -> Result<JsValue, JsValue> {
    to_js_value(&FloatingLegCompounding::sofr_observation_shift())
}

/// GBP SONIA FRN compounding convention: ISDA 2021 observation shift of 5 business days (Rust `FloatingLegCompounding::sonia_observation_shift`).
///
/// @returns `FloatingLegCompounding` wire value `{ compounded_with_observation_shift: { shift_days: 5 } }`.
#[wasm_bindgen(js_name = floatingLegCompoundingSoniaObservationShift)]
pub fn floating_leg_compounding_sonia_observation_shift() -> Result<JsValue, JsValue> {
    to_js_value(&FloatingLegCompounding::sonia_observation_shift())
}

/// Whether a compounding convention builds the period rate from daily overnight
/// fixings (Rust `FloatingLegCompounding::is_overnight`).
///
/// @param value - `FloatingLegCompounding` wire value (for example `"simple"` or `{ compounded_in_arrears: { lookback_days: 0 } }`).
/// @returns `false` for `"simple"`, `true` for every other convention.
/// @throws If `value` is not a `FloatingLegCompounding` wire value (kind `validation`).
#[wasm_bindgen(js_name = floatingLegCompoundingIsOvernight)]
pub fn floating_leg_compounding_is_overnight(value: JsValue) -> Result<bool, JsValue> {
    Ok(js_wire::<FloatingLegCompounding>(&value, "value")?.is_overnight())
}

/// Check a floating-rate specification's reset lag, caps and floors.
///
/// @param spec - `FloatingRateSpec` wire object.
/// @throws If `spec` is not a `FloatingRateSpec`, its reset lag is negative, its index floor exceeds its index cap, or its all-in floor exceeds its all-in cap (kind `validation`).
#[wasm_bindgen(js_name = floatingRateSpecValidate)]
pub fn floating_rate_spec_validate(spec: JsValue) -> Result<(), JsValue> {
    js_wire::<FloatingRateSpec>(&spec, "spec")?
        .validate()
        .map_err(to_js_err)
}

/// USD SOFR floating-rate specification (curve `USD-SOFR`, compounded in arrears).
///
/// @param spread_bp - Spread over the index in basis points, as an exact decimal string (`"150"` = 1.50%).
/// @returns `FloatingRateSpec` wire object with the Rust SOFR conventions.
/// @throws If `spreadBp` is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
#[wasm_bindgen(js_name = floatingRateSpecSofr)]
pub fn floating_rate_spec_sofr(spread_bp: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&FloatingRateSpec::sofr(
        js_decimal(&spread_bp, "spreadBp")?.0,
    ))
}

/// GBP SONIA floating-rate specification (curve `GBP-SONIA`, compounded in arrears).
///
/// @param spread_bp - Spread over the index in basis points, as an exact decimal string (`"150"` = 1.50%).
/// @returns `FloatingRateSpec` wire object with the Rust SONIA conventions.
/// @throws If `spreadBp` is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
#[wasm_bindgen(js_name = floatingRateSpecSonia)]
pub fn floating_rate_spec_sonia(spread_bp: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&FloatingRateSpec::sonia(
        js_decimal(&spread_bp, "spreadBp")?.0,
    ))
}

/// EUR 3-month EURIBOR floating-rate specification (term rate set in advance).
///
/// @param spread_bp - Spread over the index in basis points, as an exact decimal string (`"150"` = 1.50%).
/// @returns `FloatingRateSpec` wire object with the Rust EURIBOR 3M conventions.
/// @throws If `spreadBp` is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
#[wasm_bindgen(js_name = floatingRateSpecEuribor3m)]
pub fn floating_rate_spec_euribor_3m(spread_bp: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&FloatingRateSpec::euribor_3m(
        js_decimal(&spread_bp, "spreadBp")?.0,
    ))
}

// --- Notional ------------------------------------------------------------------

/// Bullet notional of a given amount: no amortization.
///
/// @param amount - Finite initial outstanding balance in major currency units.
/// @param currency - ISO-4217 currency code such as `"USD"`.
/// @returns `Notional` wire object `{ initial, amort: "none" }`.
/// @throws If `amount` is not a finite number or `currency` is not an ISO-4217 code (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = notionalPar)]
pub fn notional_par(amount: JsValue, currency: JsValue) -> Result<JsValue, JsValue> {
    let currency = js_currency(&currency, "currency")?;
    let notional = Notional::par(js_f64(&amount, "amount")?, currency).map_err(to_js_err)?;
    to_js_value(&notional)
}

/// Check a notional and its amortization rule for consistency.
///
/// @param notional - `Notional` wire object.
/// @throws If `notional` is not a `Notional`, its initial balance is negative or non-finite, or its amortization rule mixes currencies, exceeds the initial balance, has a percentage outside `[0, 1]` or has dates out of order (kind `validation`).
#[wasm_bindgen(js_name = notionalValidate)]
pub fn notional_validate(notional: JsValue) -> Result<(), JsValue> {
    js_wire::<Notional>(&notional, "notional")?
        .validate()
        .map_err(to_js_err)
}

// --- DefaultModelSpec ------------------------------------------------------------

/// Check a default model's curve parameters.
///
/// @param spec - `DefaultModelSpec` wire object.
/// @throws If `spec` is not a `DefaultModelSpec`, or its CDR, SDA multiplier, vector, cumulative-loss or timing parameters are out of range (kind `validation`).
#[wasm_bindgen(js_name = defaultModelSpecValidate)]
pub fn default_model_spec_validate(spec: JsValue) -> Result<(), JsValue> {
    js_wire::<DefaultModelSpec>(&spec, "spec")?
        .validate()
        .map_err(to_js_err)
}

/// Monthly default rate (MDR) of a default model at a seasoning month.
///
/// @param spec - `DefaultModelSpec` wire object.
/// @param seasoning_months - Months since origination or pool start (non-negative integer).
/// @returns Monthly default rate as a decimal; a constant CDR converts as `1 - (1 - CDR)^(1/12)`.
/// @throws If `spec` is not a `DefaultModelSpec` or its curve parameters are invalid (kind `validation`), or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = defaultModelSpecMdr)]
pub fn default_model_spec_mdr(spec: JsValue, seasoning_months: JsValue) -> Result<f64, JsValue> {
    js_wire::<DefaultModelSpec>(&spec, "spec")?
        .mdr(js_uint(&seasoning_months, "seasoningMonths")?)
        .map_err(to_js_err)
}

/// Survival-adjusted monthly default rate (Rust `DefaultModelSpec::mdr_with_survival`).
///
/// Identical to `defaultModelSpecMdr` for constant, SDA and vector curves. For
/// cumulative-loss and timing curves the month's default share of the original
/// balance is divided by `survivingBalanceFraction`, so applying the rate to the
/// current balance reproduces the curve's defaults.
///
/// @param spec - `DefaultModelSpec` wire object.
/// @param seasoning_months - Months since origination or pool start (non-negative integer).
/// @param surviving_balance_fraction - Current pool balance divided by the balance the curve is expressed against, as a finite non-negative decimal (may exceed 1.0 after par build).
/// @returns Monthly default rate as a decimal in `[0, 1]`; zero once the surviving balance is exhausted.
/// @throws If `spec` is not a `DefaultModelSpec`, its curve parameters are invalid, or `survivingBalanceFraction` is negative or non-finite (kind `validation`); if `seasoningMonths` is not a non-negative integer or `survivingBalanceFraction` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = defaultModelSpecMdrWithSurvival)]
pub fn default_model_spec_mdr_with_survival(
    spec: JsValue,
    seasoning_months: JsValue,
    surviving_balance_fraction: JsValue,
) -> Result<f64, JsValue> {
    js_wire::<DefaultModelSpec>(&spec, "spec")?
        .mdr_with_survival(
            js_uint(&seasoning_months, "seasoningMonths")?,
            js_f64(&surviving_balance_fraction, "survivingBalanceFraction")?,
        )
        .map_err(to_js_err)
}

/// Cumulative defaults through a seasoning month as a fraction of the original
/// balance (Rust `DefaultModelSpec::cumulative_default_fraction`).
///
/// @param spec - `DefaultModelSpec` wire object.
/// @param seasoning_months - Months since origination (non-negative integer); month 0 is before any default.
/// @returns The fraction for cumulative-loss and timing curves (may exceed 1.0 with replenishment or par build); `undefined` for rate-based curves.
/// @throws If `spec` is not a `DefaultModelSpec` or its curve parameters are invalid (kind `validation`), or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = defaultModelSpecCumulativeDefaultFraction)]
pub fn default_model_spec_cumulative_default_fraction(
    spec: JsValue,
    seasoning_months: JsValue,
) -> Result<Option<f64>, JsValue> {
    js_wire::<DefaultModelSpec>(&spec, "spec")?
        .cumulative_default_fraction(js_uint(&seasoning_months, "seasoningMonths")?)
        .map_err(to_js_err)
}

// --- PrepaymentModelSpec ---------------------------------------------------------

/// Check a prepayment model's curve parameters.
///
/// @param spec - `PrepaymentModelSpec` wire object.
/// @throws If `spec` is not a `PrepaymentModelSpec`, or its CPR, PSA multiplier, ABS speed or vector is out of range (kind `validation`).
#[wasm_bindgen(js_name = prepaymentModelSpecValidate)]
pub fn prepayment_model_spec_validate(spec: JsValue) -> Result<(), JsValue> {
    js_wire::<PrepaymentModelSpec>(&spec, "spec")?
        .validate()
        .map_err(to_js_err)
}

/// Single-month mortality (SMM) of a prepayment model at a seasoning month.
///
/// @param spec - `PrepaymentModelSpec` wire object.
/// @param seasoning_months - Months since origination or pool start (non-negative integer).
/// @returns Monthly prepayment rate as a decimal; a constant CPR converts as `1 - (1 - CPR)^(1/12)`.
/// @throws If `spec` is not a `PrepaymentModelSpec` or its curve parameters are invalid (kind `validation`), or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = prepaymentModelSpecSmm)]
pub fn prepayment_model_spec_smm(spec: JsValue, seasoning_months: JsValue) -> Result<f64, JsValue> {
    js_wire::<PrepaymentModelSpec>(&spec, "spec")?
        .smm(js_uint(&seasoning_months, "seasoningMonths")?)
        .map_err(to_js_err)
}

// --- RecoveryModelSpec -----------------------------------------------------------

/// Recovery rate of a recovery model for a default in a seasoning month.
///
/// @param spec - `RecoveryModelSpec` wire object.
/// @param seasoning_months - Months since origination of the defaulting balance (non-negative integer).
/// @returns Recovery as a decimal fraction of defaulted par: `1 − severity` for the month when a severity vector is set, otherwise the flat `rate`.
/// @throws If `spec` is not a `RecoveryModelSpec`, its `rate` is outside `[0, 1]` or its severity vector is empty or out of range (kind `validation`; the spec is validated before the query, as Python validates at construction), or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = recoveryModelSpecRecoveryRate)]
pub fn recovery_model_spec_recovery_rate(
    spec: JsValue,
    seasoning_months: JsValue,
) -> Result<f64, JsValue> {
    let spec = js_wire::<RecoveryModelSpec>(&spec, "spec")?;
    spec.validate().map_err(to_js_err)?;
    Ok(spec.recovery_rate(js_uint(&seasoning_months, "seasoningMonths")?))
}

/// Check a recovery model's rate and severity vector.
///
/// @param spec - `RecoveryModelSpec` wire object.
/// @throws If `spec` is not a `RecoveryModelSpec`, its rate is outside `[0, 1]`, or its severity vector is empty or holds a value outside `[0, 1]` (kind `validation`).
#[wasm_bindgen(js_name = recoveryModelSpecValidate)]
pub fn recovery_model_spec_validate(spec: JsValue) -> Result<(), JsValue> {
    js_wire::<RecoveryModelSpec>(&spec, "spec")?
        .validate()
        .map_err(to_js_err)
}

// --- ScheduleParams ----------------------------------------------------------------

/// Quarterly Act/360 schedule with Modified Following on a weekends-only calendar.
///
/// @returns `ScheduleParams` wire object: short-front stubs, no end-of-month roll, no payment lag.
/// @throws If the parameters cannot be serialized.
#[wasm_bindgen(js_name = scheduleParamsQuarterlyAct360)]
pub fn schedule_params_quarterly_act360() -> Result<JsValue, JsValue> {
    to_js_value(&ScheduleParams::quarterly_act360())
}

/// Semi-annual 30/360 schedule with Modified Following on a weekends-only calendar.
///
/// @returns `ScheduleParams` wire object: short-front stubs, no end-of-month roll, no payment lag.
/// @throws If the parameters cannot be serialized.
#[wasm_bindgen(js_name = scheduleParamsSemiannual30360)]
pub fn schedule_params_semiannual_30360() -> Result<JsValue, JsValue> {
    to_js_value(&ScheduleParams::semiannual_30360())
}

/// Annual ISDA Act/Act schedule with Following on a weekends-only calendar.
///
/// This is ISDA Actual/Actual, not ICMA; government bonds use
/// `scheduleParamsEurGovBond` or `scheduleParamsUsdTreasury`.
///
/// @returns `ScheduleParams` wire object: short-front stubs, no end-of-month roll, no payment lag.
/// @throws If the parameters cannot be serialized.
#[wasm_bindgen(js_name = scheduleParamsAnnualActact)]
pub fn schedule_params_annual_actact() -> Result<JsValue, JsValue> {
    to_js_value(&ScheduleParams::annual_actact())
}

/// USD SOFR swap leg: quarterly, Act/360, Modified Following, USNY, T+2 payment lag.
///
/// @returns `ScheduleParams` wire object following ARRC SOFR conventions.
/// @throws If the parameters cannot be serialized.
#[wasm_bindgen(js_name = scheduleParamsUsdSofrSwap)]
pub fn schedule_params_usd_sofr_swap() -> Result<JsValue, JsValue> {
    to_js_value(&ScheduleParams::usd_sofr_swap())
}

/// USD corporate bond: semi-annual, 30/360, Following, USNY.
///
/// @returns `ScheduleParams` wire object for a plain USD corporate coupon schedule.
/// @throws If the parameters cannot be serialized.
#[wasm_bindgen(js_name = scheduleParamsUsdCorporateBond)]
pub fn schedule_params_usd_corporate_bond() -> Result<JsValue, JsValue> {
    to_js_value(&ScheduleParams::usd_corporate_bond())
}

/// USD Treasury bond: semi-annual, Act/Act, Following, USNY.
///
/// @returns `ScheduleParams` wire object for a Treasury-style coupon schedule.
/// @throws If the parameters cannot be serialized.
#[wasm_bindgen(js_name = scheduleParamsUsdTreasury)]
pub fn schedule_params_usd_treasury() -> Result<JsValue, JsValue> {
    to_js_value(&ScheduleParams::usd_treasury())
}

/// EUR €STR swap leg: annual, Act/360, Modified Following, TARGET2, T+2 payment lag (LCH template).
///
/// @returns `ScheduleParams` wire object for a €STR-style floating leg.
/// @throws If the parameters cannot be serialized.
#[wasm_bindgen(js_name = scheduleParamsEurEstrSwap)]
pub fn schedule_params_eur_estr_swap() -> Result<JsValue, JsValue> {
    to_js_value(&ScheduleParams::eur_estr_swap())
}

/// EUR government bond: annual, Act/Act, Following, TARGET2.
///
/// @returns `ScheduleParams` wire object for an annual EUR government coupon schedule.
/// @throws If the parameters cannot be serialized.
#[wasm_bindgen(js_name = scheduleParamsEurGovBond)]
pub fn schedule_params_eur_gov_bond() -> Result<JsValue, JsValue> {
    to_js_value(&ScheduleParams::eur_gov_bond())
}

/// GBP SONIA swap leg: annual, Act/365F, Modified Following, GBLO, no payment lag.
///
/// @returns `ScheduleParams` wire object for a SONIA-style floating leg.
/// @throws If the parameters cannot be serialized.
#[wasm_bindgen(js_name = scheduleParamsGbpSoniaSwap)]
pub fn schedule_params_gbp_sonia_swap() -> Result<JsValue, JsValue> {
    to_js_value(&ScheduleParams::gbp_sonia_swap())
}

/// JPY TONA swap leg: annual, Act/365F, Modified Following, JPTO, T+2 payment lag.
///
/// @returns `ScheduleParams` wire object for a TONA-style floating leg.
/// @throws If the parameters cannot be serialized.
#[wasm_bindgen(js_name = scheduleParamsJpyTonaSwap)]
pub fn schedule_params_jpy_tona_swap() -> Result<JsValue, JsValue> {
    to_js_value(&ScheduleParams::jpy_tona_swap())
}

/// Check schedule conventions before building a schedule.
///
/// @param params - `ScheduleParams` wire object.
/// @throws If `params` is not a `ScheduleParams`, its `calendar_id` names no registered holiday calendar (other than `"weekends_only"`), or its payment lag is negative (kind `validation`).
#[wasm_bindgen(js_name = scheduleParamsValidate)]
pub fn schedule_params_validate(params: JsValue) -> Result<(), JsValue> {
    js_wire::<ScheduleParams>(&params, "params")?
        .validate()
        .map_err(to_js_err)
}
