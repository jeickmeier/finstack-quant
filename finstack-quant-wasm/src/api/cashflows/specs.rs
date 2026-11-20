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

macro_rules! zero_arg_specs {
    ($ty:ident, $ret_ty:literal => {
        $(
            $(#[$doc:meta])*
            $method:ident as $fn_name:ident => $js_name:ident;
        )+
    }) => {
        $(
            #[finstack_quant_wasm_macros::plain_docs]
            $(#[$doc])*
            #[wasm_bindgen(js_name = $js_name, unchecked_return_type = $ret_ty)]
            pub fn $fn_name() -> Result<JsValue, JsValue> {
                to_js_value(&$ty::$method())
            }
        )+
    };
}

macro_rules! validate_specs {
    ($(
        $(#[$doc:meta])*
        fn $fn_name:ident($param:ident : $ty:ty => $param_ty:literal) as $js_name:ident;
    )+) => {
        $(
            #[finstack_quant_wasm_macros::plain_docs]
            $(#[$doc])*
            #[wasm_bindgen(js_name = $js_name)]
            pub fn $fn_name(
                #[wasm_bindgen(unchecked_param_type = $param_ty)] $param: JsValue,
            ) -> Result<(), JsValue> {
                js_wire::<$ty>(&$param, stringify!($param))?
                    .validate()
                    .map_err(to_js_err)
            }
        )+
    };
}

macro_rules! spread_float_specs {
    ($(
        $(#[$doc:meta])*
        $method:ident as $fn_name:ident => $js_name:ident;
    )+) => {
        $(
            #[finstack_quant_wasm_macros::plain_docs]
            $(#[$doc])*
            #[wasm_bindgen(js_name = $js_name, unchecked_return_type = "generated.cashflows.FloatingRateSpec")]
            pub fn $fn_name(
                #[wasm_bindgen(js_name = "spreadBp", unchecked_param_type = "string")]
                spread_bp: JsValue,
            ) -> Result<JsValue, JsValue> {
                to_js_value(&FloatingRateSpec::$method(
                    js_decimal(&spread_bp, "spreadBp")?.0,
                ))
            }
        )+
    };
}

// --- Validation ------------------------------------------------------------------

validate_specs! {
    /// Check a floating-rate specification's reset lag, caps and floors.
    ///
    /// @param spec - `FloatingRateSpec` wire object.
    /// @throws If `spec` is not a `FloatingRateSpec`, its reset lag is negative, its index floor exceeds its index cap, or its all-in floor exceeds its all-in cap (kind `validation`).
    fn floating_rate_spec_validate(spec: FloatingRateSpec => "generated.cashflows.FloatingRateSpec") as floatingRateSpecValidate;

    /// Check a notional and its amortization rule for consistency.
    ///
    /// @param notional - `Notional` wire object.
    /// @throws If `notional` is not a `Notional`, its initial balance is negative or non-finite, or its amortization rule mixes currencies, exceeds the initial balance, has a percentage outside `[0, 1]` or has dates out of order (kind `validation`).
    fn notional_validate(notional: Notional => "generated.cashflows.Notional") as notionalValidate;

    /// Check a default model's curve parameters.
    ///
    /// @param spec - `DefaultModelSpec` wire object.
    /// @throws If `spec` is not a `DefaultModelSpec`, or its CDR, SDA multiplier, vector, cumulative-loss or timing parameters are out of range (kind `validation`).
    fn default_model_spec_validate(spec: DefaultModelSpec => "generated.cashflows.DefaultModelSpec") as defaultModelSpecValidate;

    /// Check a prepayment model's curve parameters.
    ///
    /// @param spec - `PrepaymentModelSpec` wire object.
    /// @throws If `spec` is not a `PrepaymentModelSpec`, or its CPR, PSA multiplier, ABS speed or vector is out of range (kind `validation`).
    fn prepayment_model_spec_validate(spec: PrepaymentModelSpec => "generated.cashflows.PrepaymentModelSpec") as prepaymentModelSpecValidate;

    /// Check a recovery model's rate and severity vector.
    ///
    /// @param spec - `RecoveryModelSpec` wire object.
    /// @throws If `spec` is not a `RecoveryModelSpec`, its rate is outside `[0, 1]`, or its severity vector is empty or holds a value outside `[0, 1]` (kind `validation`).
    fn recovery_model_spec_validate(spec: RecoveryModelSpec => "generated.cashflows.RecoveryModelSpec") as recoveryModelSpecValidate;

    /// Check schedule conventions before building a schedule.
    ///
    /// @param params - `ScheduleParams` wire object.
    /// @throws If `params` is not a `ScheduleParams`, its `calendar_id` names no registered holiday calendar (other than `"weekends_only"`), or its payment lag is negative (kind `validation`).
    fn schedule_params_validate(params: ScheduleParams => "generated.cashflows.ScheduleParams") as scheduleParamsValidate;
}

// --- Floating-rate conventions ----------------------------------------------

zero_arg_specs! { FloatingLegCompounding, "generated.cashflows.FloatingLegCompounding" => {
    /// USD SOFR FRN compounding convention: ISDA 2021 observation shift of 2 business days (Rust `FloatingLegCompounding::sofr_observation_shift`).
    ///
    /// @returns `FloatingLegCompounding` wire value `{ compounded_with_observation_shift: { shift_days: 2 } }`.
    sofr_observation_shift as floating_leg_compounding_sofr_observation_shift => floatingLegCompoundingSofrObservationShift;

    /// GBP SONIA FRN compounding convention: ISDA 2021 observation shift of 5 business days (Rust `FloatingLegCompounding::sonia_observation_shift`).
    ///
    /// @returns `FloatingLegCompounding` wire value `{ compounded_with_observation_shift: { shift_days: 5 } }`.
    sonia_observation_shift as floating_leg_compounding_sonia_observation_shift => floatingLegCompoundingSoniaObservationShift;
}}

/// Whether a compounding convention builds the period rate from daily overnight
/// fixings (Rust `FloatingLegCompounding::is_overnight`).
///
/// @param value - `FloatingLegCompounding` wire value (for example `"simple"` or `{ compounded_in_arrears: { lookback_days: 0 } }`).
/// @returns `false` for `"simple"`, `true` for every other convention.
/// @throws If `value` is not a `FloatingLegCompounding` wire value (kind `validation`).
#[wasm_bindgen(js_name = floatingLegCompoundingIsOvernight)]
pub fn floating_leg_compounding_is_overnight(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.FloatingLegCompounding")]
    value: JsValue,
) -> Result<bool, JsValue> {
    Ok(js_wire::<FloatingLegCompounding>(&value, "value")?.is_overnight())
}

spread_float_specs! {
    /// USD SOFR floating-rate specification (curve `USD-SOFR`, compounded in arrears).
    ///
    /// @param spreadBp - Spread over the index in basis points, as an exact decimal string (`"150"` = 1.50%).
    /// @returns `FloatingRateSpec` wire object with the Rust SOFR conventions.
    /// @throws If `spreadBp` is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
    sofr as floating_rate_spec_sofr => floatingRateSpecSofr;

    /// GBP SONIA floating-rate specification (curve `GBP-SONIA`, compounded in arrears).
    ///
    /// @param spreadBp - Spread over the index in basis points, as an exact decimal string (`"150"` = 1.50%).
    /// @returns `FloatingRateSpec` wire object with the Rust SONIA conventions.
    /// @throws If `spreadBp` is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
    sonia as floating_rate_spec_sonia => floatingRateSpecSonia;

    /// EUR 3-month EURIBOR floating-rate specification (term rate set in advance).
    ///
    /// @param spreadBp - Spread over the index in basis points, as an exact decimal string (`"150"` = 1.50%).
    /// @returns `FloatingRateSpec` wire object with the Rust EURIBOR 3M conventions.
    /// @throws If `spreadBp` is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
    euribor_3m as floating_rate_spec_euribor_3m => floatingRateSpecEuribor3m;
}

// --- Notional ------------------------------------------------------------------

/// Bullet notional of a given amount: no amortization.
///
/// @param amount - Finite initial outstanding balance in major currency units.
/// @param currency - ISO-4217 currency code such as `"USD"`.
/// @returns `Notional` wire object `{ initial, amort: "none" }`.
/// @throws If `amount` is not a finite number or `currency` is not an ISO-4217 code (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = notionalPar, unchecked_return_type = "generated.cashflows.Notional")]
pub fn notional_par(
    #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
    #[wasm_bindgen(
        unchecked_param_type = "generated.core.Currency | Lowercase<generated.core.Currency>"
    )]
    currency: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = js_currency(&currency, "currency")?;
    let notional = Notional::par(js_f64(&amount, "amount")?, currency).map_err(to_js_err)?;
    to_js_value(&notional)
}

// --- DefaultModelSpec ------------------------------------------------------------

/// Monthly default rate (MDR) of a default model at a seasoning month.
///
/// @param spec - `DefaultModelSpec` wire object.
/// @param seasoningMonths - Months since origination or pool start (positive integer, 1-indexed).
/// @returns Monthly default rate as a decimal; a constant CDR converts as `1 - (1 - CDR)^(1/12)`.
/// @throws If `spec` is not a `DefaultModelSpec`, its curve parameters are invalid or `seasoningMonths` is `0` (kind `validation`), or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = defaultModelSpecMdr)]
pub fn default_model_spec_mdr(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.DefaultModelSpec")] spec: JsValue,
    #[wasm_bindgen(js_name = "seasoningMonths", unchecked_param_type = "number")] seasoning_months: JsValue,
) -> Result<f64, JsValue> {
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
/// @param seasoningMonths - Months since origination or pool start (positive integer, 1-indexed).
/// @param survivingBalanceFraction - Current pool balance divided by the balance the curve is expressed against, as a finite non-negative decimal (may exceed 1.0 after par build).
/// @returns Monthly default rate as a decimal in `[0, 1]`; zero once the surviving balance is exhausted.
/// @throws If `spec` is not a `DefaultModelSpec`, its curve parameters are invalid, `seasoningMonths` is `0`, or `survivingBalanceFraction` is negative or non-finite (kind `validation`); if `seasoningMonths` is not a non-negative integer or `survivingBalanceFraction` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = defaultModelSpecMdrWithSurvival)]
pub fn default_model_spec_mdr_with_survival(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.DefaultModelSpec")] spec: JsValue,
    #[wasm_bindgen(js_name = "seasoningMonths", unchecked_param_type = "number")] seasoning_months: JsValue,
    #[wasm_bindgen(js_name = "survivingBalanceFraction", unchecked_param_type = "number")]
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
/// @param seasoningMonths - Months since origination (non-negative integer); month 0 is before any default.
/// @returns The fraction for cumulative-loss and timing curves (may exceed 1.0 with replenishment or par build); `undefined` for rate-based curves.
/// @throws If `spec` is not a `DefaultModelSpec` or its curve parameters are invalid (kind `validation`), or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = defaultModelSpecCumulativeDefaultFraction)]
pub fn default_model_spec_cumulative_default_fraction(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.DefaultModelSpec")] spec: JsValue,
    #[wasm_bindgen(js_name = "seasoningMonths", unchecked_param_type = "number")] seasoning_months: JsValue,
) -> Result<Option<f64>, JsValue> {
    js_wire::<DefaultModelSpec>(&spec, "spec")?
        .cumulative_default_fraction(js_uint(&seasoning_months, "seasoningMonths")?)
        .map_err(to_js_err)
}

// --- PrepaymentModelSpec ---------------------------------------------------------

/// Single-month mortality (SMM) of a prepayment model at a seasoning month.
///
/// @param spec - `PrepaymentModelSpec` wire object.
/// @param seasoningMonths - Months since origination or pool start (positive integer, 1-indexed).
/// @returns Monthly prepayment rate as a decimal; a constant CPR converts as `1 - (1 - CPR)^(1/12)`.
/// @throws If `spec` is not a `PrepaymentModelSpec`, its curve parameters are invalid or `seasoningMonths` is `0` (kind `validation`), or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = prepaymentModelSpecSmm)]
pub fn prepayment_model_spec_smm(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.PrepaymentModelSpec")] spec: JsValue,
    #[wasm_bindgen(js_name = "seasoningMonths", unchecked_param_type = "number")] seasoning_months: JsValue,
) -> Result<f64, JsValue> {
    js_wire::<PrepaymentModelSpec>(&spec, "spec")?
        .smm(js_uint(&seasoning_months, "seasoningMonths")?)
        .map_err(to_js_err)
}

// --- RecoveryModelSpec -----------------------------------------------------------

/// Recovery rate of a recovery model for a default in a seasoning month.
///
/// @param spec - `RecoveryModelSpec` wire object.
/// @param seasoningMonths - Months since origination of the defaulting balance (positive integer, 1-indexed).
/// @returns Recovery as a decimal fraction of defaulted par: `1 − severity` for the month when a severity vector is set, otherwise the flat `rate`.
/// @throws If `spec` is not a `RecoveryModelSpec`, its `rate` is outside `[0, 1]`, its severity vector is empty or out of range, or `seasoningMonths` is `0` (kind `validation`; the spec is validated before the query, as Python validates at construction), or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = recoveryModelSpecRecoveryRate)]
pub fn recovery_model_spec_recovery_rate(
    #[wasm_bindgen(unchecked_param_type = "generated.cashflows.RecoveryModelSpec")] spec: JsValue,
    #[wasm_bindgen(js_name = "seasoningMonths", unchecked_param_type = "number")] seasoning_months: JsValue,
) -> Result<f64, JsValue> {
    js_wire::<RecoveryModelSpec>(&spec, "spec")?
        .recovery_rate(js_uint(&seasoning_months, "seasoningMonths")?)
        .map_err(to_js_err)
}

// --- ScheduleParams ----------------------------------------------------------------

zero_arg_specs! { ScheduleParams, "generated.cashflows.ScheduleParams" => {
    /// Quarterly Act/360 schedule with Modified Following on a weekends-only calendar.
    ///
    /// @returns `ScheduleParams` wire object: short-front stubs, no end-of-month roll, no payment lag.
    /// @throws If the parameters cannot be serialized.
    quarterly_act360 as schedule_params_quarterly_act360 => scheduleParamsQuarterlyAct360;

    /// Semi-annual 30/360 schedule with Modified Following on a weekends-only calendar.
    ///
    /// @returns `ScheduleParams` wire object: short-front stubs, no end-of-month roll, no payment lag.
    /// @throws If the parameters cannot be serialized.
    semiannual_30360 as schedule_params_semiannual_30360 => scheduleParamsSemiannual30360;

    /// Annual ISDA Act/Act schedule with Following on a weekends-only calendar.
    ///
    /// This is ISDA Actual/Actual, not ICMA; government bonds use
    /// `scheduleParamsEurGovBond` or `scheduleParamsUsdTreasury`.
    ///
    /// @returns `ScheduleParams` wire object: short-front stubs, no end-of-month roll, no payment lag.
    /// @throws If the parameters cannot be serialized.
    annual_actact as schedule_params_annual_actact => scheduleParamsAnnualActact;

    /// USD SOFR swap leg: quarterly, Act/360, Modified Following, USNY, T+2 payment lag.
    ///
    /// @returns `ScheduleParams` wire object following ARRC SOFR conventions.
    /// @throws If the parameters cannot be serialized.
    usd_sofr_swap as schedule_params_usd_sofr_swap => scheduleParamsUsdSofrSwap;

    /// USD corporate bond: semi-annual, 30/360, Following, USNY.
    ///
    /// @returns `ScheduleParams` wire object for a plain USD corporate coupon schedule.
    /// @throws If the parameters cannot be serialized.
    usd_corporate_bond as schedule_params_usd_corporate_bond => scheduleParamsUsdCorporateBond;

    /// USD Treasury bond: semi-annual, Act/Act, Following, USNY.
    ///
    /// @returns `ScheduleParams` wire object for a Treasury-style coupon schedule.
    /// @throws If the parameters cannot be serialized.
    usd_treasury as schedule_params_usd_treasury => scheduleParamsUsdTreasury;

    /// EUR €STR swap leg: annual, Act/360, Modified Following, TARGET2, T+2 payment lag (LCH template).
    ///
    /// @returns `ScheduleParams` wire object for a €STR-style floating leg.
    /// @throws If the parameters cannot be serialized.
    eur_estr_swap as schedule_params_eur_estr_swap => scheduleParamsEurEstrSwap;

    /// EUR government bond: annual, Act/Act, Following, TARGET2.
    ///
    /// @returns `ScheduleParams` wire object for an annual EUR government coupon schedule.
    /// @throws If the parameters cannot be serialized.
    eur_gov_bond as schedule_params_eur_gov_bond => scheduleParamsEurGovBond;

    /// GBP SONIA swap leg: annual, Act/365F, Modified Following, GBLO, no payment lag.
    ///
    /// @returns `ScheduleParams` wire object for a SONIA-style floating leg.
    /// @throws If the parameters cannot be serialized.
    gbp_sonia_swap as schedule_params_gbp_sonia_swap => scheduleParamsGbpSoniaSwap;

    /// JPY TONA swap leg: annual, Act/365F, Modified Following, JPTO, T+2 payment lag.
    ///
    /// @returns `ScheduleParams` wire object for a TONA-style floating leg.
    /// @throws If the parameters cannot be serialized.
    jpy_tona_swap as schedule_params_jpy_tona_swap => scheduleParamsJpyTonaSwap;
}}
