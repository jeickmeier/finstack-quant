//! Cashflow builder specifications: constructors, presets and model
//! evaluations over wire values.
//!
//! Every spec crosses the boundary as the plain object (or wire string) its
//! serde contract describes, typed by the generated `cashflows` TypeScript
//! declarations. The Rust associated functions and methods are free functions
//! here: constructors return the canonical wire value, and methods take it as
//! their first argument.

use crate::utils::input::{js_f64, js_f64_seq, js_uint};
use crate::utils::wire::{js_date, js_decimal, js_wire};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_cashflows::builder::{
    AmortizationSpec, CouponType, DefaultModelSpec, FeeBase, FeeSpec, FloatingLegCompounding,
    FloatingRateFallback, FloatingRateSpec, Notional, PrepaymentModelSpec, RecoveryModelSpec,
    ScheduleParams,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use wasm_bindgen::prelude::*;

/// `[date, Money]` pairs in the wire form `AmortizationSpec` uses.
#[derive(serde::Deserialize)]
#[serde(transparent)]
struct DatedMoneyValues(
    #[serde(with = "finstack_quant_core::wire::dated_money_values")] Vec<(Date, Money)>,
);

// --- AmortizationSpec --------------------------------------------------------

/// Straight-line amortization from the initial notional down to a final balance.
///
/// @param final_notional - `Money` wire object: outstanding balance left at maturity, in the notional currency.
/// @returns `AmortizationSpec` wire value `{ linear_to: { final_notional } }`.
/// @throws If `finalNotional` is not a `Money` wire object (kind `validation`).
#[wasm_bindgen(js_name = amortizationSpecLinearTo)]
pub fn amortization_spec_linear_to(final_notional: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&AmortizationSpec::LinearTo {
        final_notional: js_wire(&final_notional, "finalNotional")?,
    })
}

/// Amortization to an explicit remaining balance on each listed date.
///
/// @param schedule - `[isoDate, Money]` pairs: the outstanding balance after each date, in the notional currency.
/// @returns `AmortizationSpec` wire value `{ step_remaining: { schedule } }`.
/// @throws If `schedule` is not an array of `[isoDate, Money]` pairs (kind `validation`).
#[wasm_bindgen(js_name = amortizationSpecStepRemaining)]
pub fn amortization_spec_step_remaining(schedule: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&AmortizationSpec::StepRemaining {
        schedule: js_wire::<DatedMoneyValues>(&schedule, "schedule")?.0,
    })
}

/// Amortization of a fixed share of the original notional each period.
///
/// @param pct - Share of the original notional repaid per period as a decimal (`0.05` = 5%).
/// @returns `AmortizationSpec` wire value `{ percent_of_original_per_period: { pct } }`.
/// @throws If `pct` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = amortizationSpecPercentOfOriginalPerPeriod)]
pub fn amortization_spec_percent_of_original_per_period(pct: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&AmortizationSpec::PercentOfOriginalPerPeriod {
        pct: js_f64(&pct, "pct")?,
    })
}

/// Amortization of a fixed share of the remaining balance each period.
///
/// @param pct - Share of the then-outstanding balance repaid per period as a decimal (`0.05` = 5%).
/// @returns `AmortizationSpec` wire value `{ percent_of_remaining_per_period: { pct } }`.
/// @throws If `pct` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = amortizationSpecPercentOfRemainingPerPeriod)]
pub fn amortization_spec_percent_of_remaining_per_period(pct: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&AmortizationSpec::PercentOfRemainingPerPeriod {
        pct: js_f64(&pct, "pct")?,
    })
}

/// Straight-line amortization to zero between two dates.
///
/// @param start - ISO-8601 date on which amortization begins; the balance is flat before it.
/// @param end - ISO-8601 date on which the balance reaches zero.
/// @returns `AmortizationSpec` wire value `{ linear_between: { start, end } }`.
/// @throws If either date is not an ISO-8601 string (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = amortizationSpecLinearBetween)]
pub fn amortization_spec_linear_between(start: JsValue, end: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&AmortizationSpec::LinearBetween {
        start: js_date(&start, "start")?,
        end: js_date(&end, "end")?,
    })
}

/// Amortization by explicit principal payments on listed dates.
///
/// @param items - `[isoDate, Money]` pairs: the principal repaid on each date, in the notional currency.
/// @returns `AmortizationSpec` wire value `{ custom_principal: { items } }`.
/// @throws If `items` is not an array of `[isoDate, Money]` pairs (kind `validation`).
#[wasm_bindgen(js_name = amortizationSpecCustomPrincipal)]
pub fn amortization_spec_custom_principal(items: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&AmortizationSpec::CustomPrincipal {
        items: js_wire::<DatedMoneyValues>(&items, "items")?.0,
    })
}

// --- CouponType / FeeBase / FeeSpec -----------------------------------------

/// Coupon paid partly in cash and partly in kind.
///
/// @param cash_fraction - Share of each coupon paid in cash, as an exact decimal string (`"0.6"`).
/// @param pik_fraction - Share of each coupon capitalized into principal, as an exact decimal string (`"0.4"`); the two shares must sum to one when the schedule is built.
/// @returns `CouponType` wire value `{ split: { cash_fraction, pik_fraction } }`.
/// @throws If a fraction is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
#[wasm_bindgen(js_name = couponTypeSplit)]
pub fn coupon_type_split(
    cash_fraction: JsValue,
    pik_fraction: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CouponType::Split {
        cash_fraction: js_decimal(&cash_fraction, "cashFraction")?.0,
        pik_fraction: js_decimal(&pik_fraction, "pikFraction")?.0,
    })
}

/// Fee base equal to the undrawn part of a commitment.
///
/// @param commitment - `Money` wire object: total facility commitment; the fee accrues on commitment minus drawn balance.
/// @returns `FeeBase` wire value `{ undrawn: { commitment } }`.
/// @throws If `commitment` is not a `Money` wire object (kind `validation`).
#[wasm_bindgen(js_name = feeBaseUndrawn)]
pub fn fee_base_undrawn(commitment: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&FeeBase::Undrawn {
        commitment: js_wire(&commitment, "commitment")?,
    })
}

/// One-off fee of a fixed amount on a date.
///
/// @param date - ISO-8601 payment date of the fee.
/// @param amount - `Money` wire object: fee amount in its own currency.
/// @returns `FeeSpec` wire value `{ fixed: { date, amount } }`.
/// @throws If `date` is not an ISO-8601 string or `amount` is not a `Money` wire object (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = feeSpecFixed)]
pub fn fee_spec_fixed(date: JsValue, amount: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&FeeSpec::Fixed {
        date: js_date(&date, "date")?,
        amount: js_wire(&amount, "amount")?,
    })
}

/// Recurring fee quoted in basis points of a drawn or undrawn balance.
///
/// The argument is the `periodic_bp` payload of `FeeSpec`, parsed by the Rust
/// serde contract: `stub` defaults to `"short_front"` and `accrual_basis` to
/// `"point_in_time"`; every other field is required. (Python's
/// `FeeSpec.periodic_bp` takes the same fields positionally.)
///
/// @param fields - `{ base, bp, frequency, day_count, business_day_convention, calendar_id, stub?, accrual_basis? }`: `base` is a `FeeBase`, `bp` an exact decimal string in basis points per annum (`"25"` = 0.25%), `frequency` a `Tenor`.
/// @returns Canonical `FeeSpec` wire value `{ periodic_bp: { ... } }` with defaults filled in.
/// @throws If `fields` is missing a required field, has an unknown field or a field of the wrong shape (kind `validation`).
#[wasm_bindgen(js_name = feeSpecPeriodicBp)]
pub fn fee_spec_periodic_bp(fields: JsValue) -> Result<JsValue, JsValue> {
    let fields: serde_json::Value = js_wire(&fields, "fields")?;
    let spec: FeeSpec = serde_json::from_value(serde_json::json!({ "periodic_bp": fields }))
        .map_err(|error| {
            to_js_err(finstack_quant_core::Error::Validation(format!(
                "fields: {error}"
            )))
        })?;
    to_js_value(&spec)
}

// --- Floating-rate conventions ----------------------------------------------

/// Fallback that uses a fixed index rate when a projection is unavailable.
///
/// @param rate - Index rate used in place of the missing projection, as an exact decimal string (`"0.03"` = 3%), before spread and gearing.
/// @returns `FloatingRateFallback` wire value `{ fixed_rate: rate }`.
/// @throws If `rate` is not a string (kind `invalid_type`) or not a decimal number (kind `validation`).
#[wasm_bindgen(js_name = floatingRateFallbackFixedRate)]
pub fn floating_rate_fallback_fixed_rate(rate: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&FloatingRateFallback::FixedRate(
        js_decimal(&rate, "rate")?.0,
    ))
}

/// Overnight compounding in arrears with a lookback.
///
/// @param lookback_days - Business days each daily observation is shifted back (non-negative integer; `0` for none).
/// @returns `FloatingLegCompounding` wire value `{ compounded_in_arrears: { lookback_days } }`.
/// @throws If `lookbackDays` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = floatingLegCompoundingCompoundedInArrears)]
pub fn floating_leg_compounding_compounded_in_arrears(
    lookback_days: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&FloatingLegCompounding::CompoundedInArrears {
        lookback_days: js_uint(&lookback_days, "lookbackDays")?,
    })
}

/// Overnight compounding with an observation-period shift.
///
/// @param shift_days - Business days the whole observation period (rates and weights) is shifted back (non-negative integer).
/// @returns `FloatingLegCompounding` wire value `{ compounded_with_observation_shift: { shift_days } }`.
/// @throws If `shiftDays` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = floatingLegCompoundingCompoundedWithObservationShift)]
pub fn floating_leg_compounding_compounded_with_observation_shift(
    shift_days: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&FloatingLegCompounding::CompoundedWithObservationShift {
        shift_days: js_uint(&shift_days, "shiftDays")?,
    })
}

/// Overnight compounding with a rate cutoff before period end.
///
/// @param cutoff_days - Business days before period end from which the last observed rate is repeated (non-negative integer).
/// @returns `FloatingLegCompounding` wire value `{ compounded_with_rate_cutoff: { cutoff_days } }`.
/// @throws If `cutoffDays` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = floatingLegCompoundingCompoundedWithRateCutoff)]
pub fn floating_leg_compounding_compounded_with_rate_cutoff(
    cutoff_days: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&FloatingLegCompounding::CompoundedWithRateCutoff {
        cutoff_days: js_uint(&cutoff_days, "cutoffDays")?,
    })
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
    let currency: Currency = js_wire(&currency, "currency")?;
    let notional = Notional::par(js_f64(&amount, "amount")?, currency).map_err(to_js_err)?;
    to_js_value(&notional)
}

/// Currency of a notional's initial balance.
///
/// @param notional - `Notional` wire object.
/// @returns ISO-4217 currency code.
/// @throws If `notional` is not a `Notional` (kind `validation`).
#[wasm_bindgen(js_name = notionalCurrency)]
pub fn notional_currency(notional: JsValue) -> Result<String, JsValue> {
    Ok(js_wire::<Notional>(&notional, "notional")?
        .currency()
        .to_string())
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

/// Constant annual default rate.
///
/// @param cdr - Annual constant default rate as a decimal (`0.02` = 2% CDR).
/// @returns `DefaultModelSpec` wire object with no seasoning curve.
/// @throws If `cdr` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = defaultModelSpecConstantCdr)]
pub fn default_model_spec_constant_cdr(cdr: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&DefaultModelSpec::constant_cdr(js_f64(&cdr, "cdr")?))
}

/// PSA/BMA Standard Default Assumption curve.
///
/// Annual CDR ramps 0.02% per month to a 0.60% peak at month 30, is flat to
/// month 60, declines to 0.03% at month 120 and stays there.
///
/// @param speed_multiplier - SDA speed, where `1.0` means 100% SDA and `2.0` means 200% SDA.
/// @returns `DefaultModelSpec` wire object using the SDA curve.
/// @throws If `speedMultiplier` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = defaultModelSpecSda)]
pub fn default_model_spec_sda(speed_multiplier: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&DefaultModelSpec::sda(js_f64(
        &speed_multiplier,
        "speedMultiplier",
    )?))
}

/// Constant 2% CDR baseline.
///
/// @returns `DefaultModelSpec` wire object equal to `defaultModelSpecConstantCdr(0.02)`.
/// @throws If the model cannot be serialized.
#[wasm_bindgen(js_name = defaultModelSpecCdr2pct)]
pub fn default_model_spec_cdr_2pct() -> Result<JsValue, JsValue> {
    to_js_value(&DefaultModelSpec::cdr_2pct())
}

/// Explicit annual CDR for each month of seasoning.
///
/// @param monthly_cdr - Annual CDR per seasoning month as decimals in `[0, 1]`, month 1 first; the last value is held. Must be non-empty.
/// @returns `DefaultModelSpec` wire object using the vector curve.
/// @throws If `monthlyCdr` is not an array of numbers (kind `invalid_type`).
#[wasm_bindgen(js_name = defaultModelSpecVector)]
pub fn default_model_spec_vector(monthly_cdr: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&DefaultModelSpec::vector(js_f64_seq(
        &monthly_cdr,
        "monthlyCdr",
    )?))
}

/// Cumulative net-loss curve with a constant severity.
///
/// @param cumulative_net_loss_pct - Cumulative net loss in percent of the original balance per seasoning month (`1.5` = 1.5%), non-decreasing, month 1 first; the last value is held.
/// @param severity - Loss severity as a decimal fraction of defaulted par in `(0, 1]`; defaults are loss divided by severity.
/// @returns `DefaultModelSpec` wire object using the cumulative-loss curve.
/// @throws If an argument is not a number or array of numbers (kind `invalid_type`).
#[wasm_bindgen(js_name = defaultModelSpecCumulativeLoss)]
pub fn default_model_spec_cumulative_loss(
    cumulative_net_loss_pct: JsValue,
    severity: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&DefaultModelSpec::cumulative_loss(
        js_f64_seq(&cumulative_net_loss_pct, "cumulativeNetLossPct")?,
        js_f64(&severity, "severity")?,
    ))
}

/// Rating-agency default timing over a lifetime cumulative default rate.
///
/// @param cumulative_default_rate - Lifetime defaults as a decimal fraction of the original balance in `[0, 1]`.
/// @param annual_pct - Share of lifetime defaults in each year of seasoning, in percent (for example `[15, 30, 30, 15, 10]`), summing to 100.
/// @returns `DefaultModelSpec` wire object using the timing curve.
/// @throws If an argument is not a number or array of numbers (kind `invalid_type`).
#[wasm_bindgen(js_name = defaultModelSpecTiming)]
pub fn default_model_spec_timing(
    cumulative_default_rate: JsValue,
    annual_pct: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&DefaultModelSpec::timing(
        js_f64(&cumulative_default_rate, "cumulativeDefaultRate")?,
        js_f64_seq(&annual_pct, "annualPct")?,
    ))
}

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

// --- PrepaymentModelSpec ---------------------------------------------------------

/// Constant annual prepayment rate.
///
/// @param cpr - Annual constant prepayment rate as a decimal (`0.06` = 6% CPR).
/// @returns `PrepaymentModelSpec` wire object with no seasoning curve.
/// @throws If `cpr` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = prepaymentModelSpecConstantCpr)]
pub fn prepayment_model_spec_constant_cpr(cpr: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&PrepaymentModelSpec::constant_cpr(js_f64(&cpr, "cpr")?))
}

/// PSA prepayment curve: a 30-month ramp to a 6% annual CPR, then flat.
///
/// @param speed_multiplier - PSA speed, where `1.0` means 100% PSA and `1.5` means 150% PSA.
/// @returns `PrepaymentModelSpec` wire object using the PSA curve.
/// @throws If `speedMultiplier` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = prepaymentModelSpecPsa)]
pub fn prepayment_model_spec_psa(speed_multiplier: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&PrepaymentModelSpec::psa(js_f64(
        &speed_multiplier,
        "speedMultiplier",
    )?))
}

/// 100% PSA, the standard prepayment assumption.
///
/// @returns `PrepaymentModelSpec` wire object equal to `prepaymentModelSpecPsa(1.0)`.
/// @throws If the model cannot be serialized.
#[wasm_bindgen(js_name = prepaymentModelSpecPsa100)]
pub fn prepayment_model_spec_psa_100() -> Result<JsValue, JsValue> {
    to_js_value(&PrepaymentModelSpec::psa_100())
}

/// CMBS-style lockout: no prepayment during the lockout, then a constant CPR.
///
/// @param lockout_months - Months with zero prepayment (non-negative integer; for example `60` for five years).
/// @param post_lockout_cpr - Annual CPR after the lockout as a decimal (`0.10` = 10%).
/// @returns `PrepaymentModelSpec` wire object using the lockout curve.
/// @throws If `lockoutMonths` is not a non-negative integer or `postLockoutCpr` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = prepaymentModelSpecCmbsWithLockout)]
pub fn prepayment_model_spec_cmbs_with_lockout(
    lockout_months: JsValue,
    post_lockout_cpr: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&PrepaymentModelSpec::cmbs_with_lockout(
        js_uint(&lockout_months, "lockoutMonths")?,
        js_f64(&post_lockout_cpr, "postLockoutCpr")?,
    ))
}

/// ABS speed curve (auto-loan and consumer ABS convention).
///
/// Each month the same share of the original balance prepays, so
/// `SMM_t = speed / (1 − speed·(t − 1))`.
///
/// @param speed - Monthly prepayment as a decimal fraction of the original balance (`0.015` = 1.5% ABS), in `[0, 1]`.
/// @returns `PrepaymentModelSpec` wire object using the ABS curve.
/// @throws If `speed` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = prepaymentModelSpecAbs)]
pub fn prepayment_model_spec_abs(speed: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&PrepaymentModelSpec::abs(js_f64(&speed, "speed")?))
}

/// Explicit annual CPR for each month of seasoning.
///
/// @param monthly_cpr - Annual CPR per seasoning month as decimals in `[0, 1]`, month 1 first; the last value is held. Must be non-empty.
/// @returns `PrepaymentModelSpec` wire object using the vector curve.
/// @throws If `monthlyCpr` is not an array of numbers (kind `invalid_type`).
#[wasm_bindgen(js_name = prepaymentModelSpecVector)]
pub fn prepayment_model_spec_vector(monthly_cpr: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&PrepaymentModelSpec::vector(js_f64_seq(
        &monthly_cpr,
        "monthlyCpr",
    )?))
}

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

/// Copy of a recovery model with a loss-severity vector by month of default.
///
/// @param spec - `RecoveryModelSpec` wire object.
/// @param severity_vector - Loss severity (`1 − recovery`) per seasoning month of the default as decimals in `[0, 1]`, month 1 first; the last value is held. Must be non-empty.
/// @returns The `RecoveryModelSpec` with the vector attached; `rate` stays as the flat fallback but no longer drives recoveries.
/// @throws If `spec` is not a `RecoveryModelSpec` (kind `validation`) or `severityVector` is not an array of numbers (kind `invalid_type`).
#[wasm_bindgen(js_name = recoveryModelSpecWithSeverityVector)]
pub fn recovery_model_spec_with_severity_vector(
    spec: JsValue,
    severity_vector: JsValue,
) -> Result<JsValue, JsValue> {
    let spec = js_wire::<RecoveryModelSpec>(&spec, "spec")?;
    to_js_value(&spec.with_severity_vector(js_f64_seq(&severity_vector, "severityVector")?))
}

/// Recovery rate of a recovery model for a default in a seasoning month.
///
/// @param spec - `RecoveryModelSpec` wire object.
/// @param seasoning_months - Months since origination of the defaulting balance (non-negative integer).
/// @returns Recovery as a decimal fraction of defaulted par: `1 − severity` for the month when a severity vector is set, otherwise the flat `rate`.
/// @throws If `spec` is not a `RecoveryModelSpec` (kind `validation`) or `seasoningMonths` is not a non-negative integer (kind `invalid_type`).
#[wasm_bindgen(js_name = recoveryModelSpecRecoveryRate)]
pub fn recovery_model_spec_recovery_rate(
    spec: JsValue,
    seasoning_months: JsValue,
) -> Result<f64, JsValue> {
    Ok(js_wire::<RecoveryModelSpec>(&spec, "spec")?
        .recovery_rate(js_uint(&seasoning_months, "seasoningMonths")?))
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
