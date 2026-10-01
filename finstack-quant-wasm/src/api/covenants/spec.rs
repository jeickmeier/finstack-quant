//! Covenant definitions: constructors and modifiers over wire values.
//!
//! `CovenantType`, `CovenantConsequence`, `Covenant`, `CovenantSpec` and
//! `ThresholdSchedule` cross the boundary as the plain objects their serde
//! contracts describe, typed by the generated `covenants` TypeScript
//! declarations. The Rust constructors and methods are free functions here.

use crate::utils::input::{js_f64, js_opt_int, js_string};
use crate::utils::wire::{js_date, js_wire};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::Tenor;
use finstack_quant_covenants::{
    Covenant, CovenantConsequence, CovenantScope, CovenantSpec, CovenantType, SpringingCondition,
    ThresholdSchedule, ThresholdTest,
};
use wasm_bindgen::prelude::*;

/// Maximum total debt / EBITDA covenant.
///
/// @param threshold - Maximum allowed ratio in turns (`4.5` means 4.5x).
/// @returns `CovenantType` wire value `{ max_debt_to_ebitda: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMaxDebtToEbitda)]
pub fn covenant_type_max_debt_to_ebitda(threshold: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MaxDebtToEbitda {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Minimum interest coverage (EBIT / interest) covenant.
///
/// @param threshold - Minimum required ratio in turns.
/// @returns `CovenantType` wire value `{ min_interest_coverage: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMinInterestCoverage)]
pub fn covenant_type_min_interest_coverage(threshold: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MinInterestCoverage {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Minimum fixed-charge coverage covenant.
///
/// @param threshold - Minimum required ratio in turns.
/// @returns `CovenantType` wire value `{ min_fixed_charge_coverage: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMinFixedChargeCoverage)]
pub fn covenant_type_min_fixed_charge_coverage(threshold: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MinFixedChargeCoverage {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Maximum total leverage covenant.
///
/// @param threshold - Maximum allowed total debt / EBITDA in turns.
/// @returns `CovenantType` wire value `{ max_total_leverage: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMaxTotalLeverage)]
pub fn covenant_type_max_total_leverage(threshold: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MaxTotalLeverage {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Maximum senior leverage covenant.
///
/// @param threshold - Maximum allowed senior debt / EBITDA in turns.
/// @returns `CovenantType` wire value `{ max_senior_leverage: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMaxSeniorLeverage)]
pub fn covenant_type_max_senior_leverage(threshold: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MaxSeniorLeverage {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Minimum asset coverage covenant.
///
/// @param threshold - Minimum required asset coverage ratio in turns.
/// @returns `CovenantType` wire value `{ min_asset_coverage: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMinAssetCoverage)]
pub fn covenant_type_min_asset_coverage(threshold: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MinAssetCoverage {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Minimum debt-service coverage ratio covenant.
///
/// @param threshold - Minimum required DSCR in turns.
/// @returns `CovenantType` wire value `{ min_dscr: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMinDscr)]
pub fn covenant_type_min_dscr(threshold: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MinDscr {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Maximum net debt / EBITDA covenant.
///
/// @param threshold - Maximum allowed net debt / EBITDA in turns; the specification needs an earnings denominator metric.
/// @returns `CovenantType` wire value `{ max_net_debt_to_ebitda: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMaxNetDebtToEbitda)]
pub fn covenant_type_max_net_debt_to_ebitda(threshold: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MaxNetDebtToEbitda {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Maximum capital expenditure covenant.
///
/// @param threshold - Maximum allowed capex amount in the reporting currency.
/// @returns `CovenantType` wire value `{ max_capex: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMaxCapex)]
pub fn covenant_type_max_capex(threshold: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MaxCapex {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Minimum liquidity covenant.
///
/// @param threshold - Minimum required liquidity amount in the reporting currency.
/// @returns `CovenantType` wire value `{ min_liquidity: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMinLiquidity)]
pub fn covenant_type_min_liquidity(threshold: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MinLiquidity {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Negative (restrictive) covenant described in words.
///
/// @param restriction - Description of the restricted action, for example `"no additional senior debt"`.
/// @returns `CovenantType` wire value `{ negative: { restriction } }`; it has no numeric test.
/// @throws If `restriction` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeNegative)]
pub fn covenant_type_negative(restriction: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::Negative {
        restriction: js_string(&restriction, "restriction")?,
    })
}

/// Affirmative covenant described in words.
///
/// @param requirement - Description of the required action, for example `"deliver audited financials within 90 days"`.
/// @returns `CovenantType` wire value `{ affirmative: { requirement } }`; it has no numeric test.
/// @throws If `requirement` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeAffirmative)]
pub fn covenant_type_affirmative(requirement: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::Affirmative {
        requirement: js_string(&requirement, "requirement")?,
    })
}

/// Read a `("maximum" | "minimum", value)` pair as a threshold test.
fn threshold_test(test: &JsValue, value: &JsValue) -> Result<ThresholdTest, JsValue> {
    let test = js_string(test, "test")?;
    let value = js_f64(value, "value")?;
    serde_json::from_value(serde_json::json!({ test: value })).map_err(|error| {
        to_js_err(finstack_quant_core::Error::Validation(format!(
            "test: {error}"
        )))
    })
}

/// Custom covenant on a caller-named metric.
///
/// @param metric - Metric identifier the test reads, for example `"net_working_capital"`.
/// @param test - `"maximum"` (the metric must not exceed `value`) or `"minimum"` (the metric must reach `value`).
/// @param value - Finite threshold in the metric's own units.
/// @returns `CovenantType` wire value `{ custom: { metric, test } }`.
/// @throws If `metric` or `test` is not a string or `value` is not a number (kind `invalid_type`), or `test` is not one of the listed strings or `value` is non-finite (kind `validation`).
#[wasm_bindgen(js_name = covenantTypeCustom)]
pub fn covenant_type_custom(
    metric: JsValue,
    test: JsValue,
    value: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::Custom {
        metric: js_string(&metric, "metric")?,
        test: threshold_test(&test, &value)?,
    })
}

/// Basket covenant: a capped amount for a named category.
///
/// @param name - Basket name, for example `"restricted_payments"`.
/// @param limit - Maximum basket usage in the reporting currency.
/// @returns `CovenantType` wire value `{ basket: { name, limit } }`.
/// @throws If `name` is not a string or `limit` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeBasket)]
pub fn covenant_type_basket(name: JsValue, limit: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::Basket {
        name: js_string(&name, "name")?,
        limit: js_f64(&limit, "limit")?,
    })
}

// --- CovenantConsequence ----------------------------------------------------

/// Breach consequence: event of default.
///
/// @returns `CovenantConsequence` wire value `"default"`.
/// @throws If the value cannot be serialized.
#[wasm_bindgen(js_name = covenantConsequenceDefault)]
pub fn covenant_consequence_default() -> Result<JsValue, JsValue> {
    to_js_value(&CovenantConsequence::Default)
}

/// Breach consequence: the borrowing rate steps up.
///
/// @param bp_increase - Rate increase in basis points per annum (`200` = +2.00%); must be finite and non-negative when the covenant is validated.
/// @returns `CovenantConsequence` wire value `{ rate_increase: { bp_increase } }`.
/// @throws If `bpIncrease` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantConsequenceRateIncrease)]
pub fn covenant_consequence_rate_increase(bp_increase: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantConsequence::RateIncrease {
        bp_increase: js_f64(&bp_increase, "bpIncrease")?,
    })
}

/// Breach consequence: excess cash is swept to repay debt.
///
/// @param sweep_percentage - Share of excess cash swept, as a decimal in `[0, 1]` (`0.5` = 50%).
/// @returns `CovenantConsequence` wire value `{ cash_sweep: { sweep_percentage } }`.
/// @throws If `sweepPercentage` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantConsequenceCashSweep)]
pub fn covenant_consequence_cash_sweep(sweep_percentage: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantConsequence::CashSweep {
        sweep_percentage: js_f64(&sweep_percentage, "sweepPercentage")?,
    })
}

/// Breach consequence: distributions to equity are blocked.
///
/// @returns `CovenantConsequence` wire value `"block_distributions"`.
/// @throws If the value cannot be serialized.
#[wasm_bindgen(js_name = covenantConsequenceBlockDistributions)]
pub fn covenant_consequence_block_distributions() -> Result<JsValue, JsValue> {
    to_js_value(&CovenantConsequence::BlockDistributions)
}

/// Breach consequence: additional collateral must be posted.
///
/// @param description - Non-empty description of the required collateral.
/// @returns `CovenantConsequence` wire value `{ require_collateral: { description } }`.
/// @throws If `description` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantConsequenceRequireCollateral)]
pub fn covenant_consequence_require_collateral(description: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantConsequence::RequireCollateral {
        description: js_string(&description, "description")?,
    })
}

/// Breach consequence: the maturity is accelerated to a new date.
///
/// @param new_maturity - ISO-8601 accelerated maturity date.
/// @returns `CovenantConsequence` wire value `{ accelerate_maturity: { new_maturity } }`.
/// @throws If `newMaturity` is not an ISO-8601 string (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = covenantConsequenceAccelerateMaturity)]
pub fn covenant_consequence_accelerate_maturity(new_maturity: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantConsequence::AccelerateMaturity {
        new_maturity: js_date(&new_maturity, "newMaturity")?,
    })
}

// --- Covenant ----------------------------------------------------------------

/// Create a covenant with the Rust defaults.
///
/// Mirrors Rust `Covenant::new` (the Python `Covenant(...)` constructor): an
/// active maintenance covenant with no cure period, consequences or
/// springing condition.
///
/// @param covenant_type - `CovenantType` wire value, for example `{ max_debt_to_ebitda: { threshold: 4.5 } }`.
/// @param test_frequency - `Tenor` wire object (`{ count, unit }`) recording how often the covenant is tested; it is metadata, the caller chooses each test date.
/// @param label - Instance label; it is the report key and must be unique within an engine on a test date.
/// @returns `Covenant` wire object.
/// @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = covenantNew)]
pub fn covenant_new(
    covenant_type: JsValue,
    test_frequency: JsValue,
    label: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&Covenant::new(
        js_wire::<CovenantType>(&covenant_type, "covenantType")?,
        js_wire::<Tenor>(&test_frequency, "testFrequency")?,
        js_string(&label, "label")?,
    ))
}

/// Copy of a covenant with a cure period.
///
/// @param covenant - `Covenant` wire object.
/// @param days - Calendar days the borrower has to cure a breach (integer); omitted or `null` means no cure period.
/// @returns The updated `Covenant`.
/// @throws If `covenant` is not a `Covenant` (kind `validation`) or `days` is not an integer (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantWithCurePeriod)]
pub fn covenant_with_cure_period(
    covenant: JsValue,
    days: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let covenant = js_wire::<Covenant>(&covenant, "covenant")?;
    to_js_value(&covenant.with_cure_period(js_opt_int(days.as_ref(), "days")?))
}

/// Copy of a covenant with one more breach consequence.
///
/// @param covenant - `Covenant` wire object.
/// @param consequence - `CovenantConsequence` wire value appended to the covenant's consequences.
/// @returns The updated `Covenant`.
/// @throws If either argument does not match its wire type (kind `validation`).
#[wasm_bindgen(js_name = covenantWithConsequence)]
pub fn covenant_with_consequence(
    covenant: JsValue,
    consequence: JsValue,
) -> Result<JsValue, JsValue> {
    let covenant = js_wire::<Covenant>(&covenant, "covenant")?;
    to_js_value(
        &covenant.with_consequence(js_wire::<CovenantConsequence>(&consequence, "consequence")?),
    )
}

/// Copy of a covenant with a different scope.
///
/// @param covenant - `Covenant` wire object.
/// @param scope - `"maintenance"` (tested on a schedule) or `"incurrence"` (tested only when an action is taken).
/// @returns The updated `Covenant`.
/// @throws If `covenant` is not a `Covenant` or `scope` is not one of the listed strings (kind `validation`).
#[wasm_bindgen(js_name = covenantWithScope)]
pub fn covenant_with_scope(covenant: JsValue, scope: JsValue) -> Result<JsValue, JsValue> {
    let covenant = js_wire::<Covenant>(&covenant, "covenant")?;
    to_js_value(&covenant.with_scope(js_wire::<CovenantScope>(&scope, "scope")?))
}

/// Copy of a covenant that is tested only while a springing condition holds.
///
/// @param covenant - `Covenant` wire object.
/// @param condition - `SpringingCondition` wire object (`{ metric_id, test }`), for example revolver utilization above a level; the covenant is inactive while the condition is unmet.
/// @returns The updated `Covenant`.
/// @throws If either argument does not match its wire type (kind `validation`).
#[wasm_bindgen(js_name = covenantWithSpringingCondition)]
pub fn covenant_with_springing_condition(
    covenant: JsValue,
    condition: JsValue,
) -> Result<JsValue, JsValue> {
    let covenant = js_wire::<Covenant>(&covenant, "covenant")?;
    to_js_value(
        &covenant.with_springing_condition(js_wire::<SpringingCondition>(&condition, "condition")?),
    )
}

// --- CovenantSpec / ThresholdSchedule ----------------------------------------------

/// Covenant specification that reads its test value from a named metric.
///
/// Mirrors Rust `CovenantSpec::with_metric` (the Python `CovenantSpec(...)`
/// constructor with a metric id): a net debt / EBITDA covenant also gets the
/// default `"ebitda"` earnings denominator.
///
/// @param covenant - `Covenant` wire object.
/// @param metric_id - Identifier of the metric the covenant tests, as supplied in the metric map, for example `"debt_to_ebitda"`.
/// @returns `CovenantSpec` wire object.
/// @throws If `covenant` is not a `Covenant` (kind `validation`) or `metricId` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantSpecWithMetric)]
pub fn covenant_spec_with_metric(
    covenant: JsValue,
    metric_id: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantSpec::with_metric(
        js_wire::<Covenant>(&covenant, "covenant")?,
        js_string(&metric_id, "metricId")?,
    ))
}

/// Copy of a specification with an explicit earnings denominator metric.
///
/// @param spec - `CovenantSpec` wire object of a built-in leverage covenant.
/// @param metric_id - Identifier of the earnings metric the leverage ratio divides by, for example `"adjusted_ebitda"`.
/// @returns The updated `CovenantSpec`.
/// @throws If `spec` is not a `CovenantSpec` (kind `validation`) or `metricId` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantSpecWithDenominatorMetric)]
pub fn covenant_spec_with_denominator_metric(
    spec: JsValue,
    metric_id: JsValue,
) -> Result<JsValue, JsValue> {
    let spec = js_wire::<CovenantSpec>(&spec, "spec")?;
    to_js_value(&spec.with_denominator_metric(js_string(&metric_id, "metricId")?))
}

/// Copy of a specification with step-down (or step-up) thresholds.
///
/// @param spec - `CovenantSpec` wire object.
/// @param schedule - `ThresholdSchedule` wire value: `[isoDate, threshold]` pairs; from each date that threshold replaces the covenant's static one.
/// @returns The updated `CovenantSpec`.
/// @throws If `spec` is not a `CovenantSpec`, or `schedule` has a non-finite threshold or repeats a date (kind `validation`).
#[wasm_bindgen(js_name = covenantSpecWithThresholdSchedule)]
pub fn covenant_spec_with_threshold_schedule(
    spec: JsValue,
    schedule: JsValue,
) -> Result<JsValue, JsValue> {
    let spec = js_wire::<CovenantSpec>(&spec, "spec")?;
    to_js_value(&spec.with_threshold_schedule(js_wire::<ThresholdSchedule>(&schedule, "schedule")?))
}

/// Threshold in force on a test date.
///
/// @param schedule - `ThresholdSchedule` wire value: `[isoDate, threshold]` pairs.
/// @param test_date - ISO-8601 covenant test date.
/// @returns The threshold of the latest entry effective on or before `testDate`, or `undefined` when the schedule starts later.
/// @throws If `schedule` has a non-finite threshold or repeats a date (kind `validation`), or `testDate` is not an ISO-8601 string (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = thresholdScheduleThresholdFor)]
pub fn threshold_schedule_threshold_for(
    schedule: JsValue,
    test_date: JsValue,
) -> Result<Option<f64>, JsValue> {
    Ok(js_wire::<ThresholdSchedule>(&schedule, "schedule")?
        .threshold_for(js_date(&test_date, "testDate")?))
}
