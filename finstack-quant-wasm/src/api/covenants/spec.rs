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
#[wasm_bindgen(js_name = covenantTypeMaxDebtToEbitda, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_max_debt_to_ebitda(
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MaxDebtToEbitda {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Minimum interest coverage (EBIT / interest) covenant.
///
/// @param threshold - Minimum required ratio in turns.
/// @returns `CovenantType` wire value `{ min_interest_coverage: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMinInterestCoverage, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_min_interest_coverage(
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MinInterestCoverage {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Minimum fixed-charge coverage covenant.
///
/// @param threshold - Minimum required ratio in turns.
/// @returns `CovenantType` wire value `{ min_fixed_charge_coverage: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMinFixedChargeCoverage, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_min_fixed_charge_coverage(
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MinFixedChargeCoverage {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Maximum total leverage covenant.
///
/// @param threshold - Maximum allowed total debt / EBITDA in turns.
/// @returns `CovenantType` wire value `{ max_total_leverage: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMaxTotalLeverage, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_max_total_leverage(
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MaxTotalLeverage {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Maximum senior leverage covenant.
///
/// @param threshold - Maximum allowed senior debt / EBITDA in turns.
/// @returns `CovenantType` wire value `{ max_senior_leverage: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMaxSeniorLeverage, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_max_senior_leverage(
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MaxSeniorLeverage {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Minimum asset coverage covenant.
///
/// @param threshold - Minimum required asset coverage ratio in turns.
/// @returns `CovenantType` wire value `{ min_asset_coverage: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMinAssetCoverage, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_min_asset_coverage(
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MinAssetCoverage {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Minimum debt-service coverage ratio covenant.
///
/// @param threshold - Minimum required DSCR in turns.
/// @returns `CovenantType` wire value `{ min_dscr: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMinDscr, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_min_dscr(
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MinDscr {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Maximum net debt / EBITDA covenant.
///
/// @param threshold - Maximum allowed net debt / EBITDA in turns; the specification needs an earnings denominator metric.
/// @returns `CovenantType` wire value `{ max_net_debt_to_ebitda: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMaxNetDebtToEbitda, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_max_net_debt_to_ebitda(
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MaxNetDebtToEbitda {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Maximum capital expenditure covenant.
///
/// @param threshold - Maximum allowed capex amount in the reporting currency.
/// @returns `CovenantType` wire value `{ max_capex: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMaxCapex, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_max_capex(
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MaxCapex {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Minimum liquidity covenant.
///
/// @param threshold - Minimum required liquidity amount in the reporting currency.
/// @returns `CovenantType` wire value `{ min_liquidity: { threshold } }`.
/// @throws If `threshold` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeMinLiquidity, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_min_liquidity(
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::MinLiquidity {
        threshold: js_f64(&threshold, "threshold")?,
    })
}

/// Negative (restrictive) covenant described in words.
///
/// @param restriction - Description of the restricted action, for example `"no additional senior debt"`.
/// @returns `CovenantType` wire value `{ negative: { restriction } }`; it has no numeric test.
/// @throws If `restriction` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeNegative, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_negative(
    #[wasm_bindgen(unchecked_param_type = "string")] restriction: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::Negative {
        restriction: js_string(&restriction, "restriction")?,
    })
}

/// Affirmative covenant described in words.
///
/// @param requirement - Description of the required action, for example `"deliver audited financials within 90 days"`.
/// @returns `CovenantType` wire value `{ affirmative: { requirement } }`; it has no numeric test.
/// @throws If `requirement` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantTypeAffirmative, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_affirmative(
    #[wasm_bindgen(unchecked_param_type = "string")] requirement: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantType::Affirmative {
        requirement: js_string(&requirement, "requirement")?,
    })
}

/// Read a `("maximum" | "minimum", value)` pair as a threshold test.
fn threshold_test(test: &JsValue, value: &JsValue) -> Result<ThresholdTest, JsValue> {
    ThresholdTest::new(&js_string(test, "test")?, js_f64(value, "value")?).map_err(to_js_err)
}

/// Custom covenant on a caller-named metric.
///
/// @param metric - Metric identifier the test reads, for example `"net_working_capital"`.
/// @param test - `"maximum"` (the metric must not exceed `value`) or `"minimum"` (the metric must reach `value`).
/// @param value - Finite threshold in the metric's own units.
/// @returns `CovenantType` wire value `{ custom: { metric, test } }`.
/// @throws If `metric` or `test` is not a string or `value` is not a number (kind `invalid_type`), or `test` is not one of the listed strings or `value` is non-finite (kind `validation`).
#[wasm_bindgen(js_name = covenantTypeCustom, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_custom(
    #[wasm_bindgen(unchecked_param_type = "string")] metric: JsValue,
    #[wasm_bindgen(unchecked_param_type = "'maximum' | 'minimum'")] test: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] value: JsValue,
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
#[wasm_bindgen(js_name = covenantTypeBasket, unchecked_return_type = "generated.covenants.CovenantType")]
pub fn covenant_type_basket(
    #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] limit: JsValue,
) -> Result<JsValue, JsValue> {
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
#[wasm_bindgen(js_name = covenantConsequenceDefault, unchecked_return_type = "generated.covenants.CovenantConsequence")]
pub fn covenant_consequence_default() -> Result<JsValue, JsValue> {
    to_js_value(&CovenantConsequence::Default)
}

/// Breach consequence: the borrowing rate steps up.
///
/// @param bpIncrease - Rate increase in basis points per annum (`200` = +2.00%); must be finite and non-negative when the covenant is validated.
/// @returns `CovenantConsequence` wire value `{ rate_increase: { bp_increase } }`.
/// @throws If `bpIncrease` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantConsequenceRateIncrease, unchecked_return_type = "generated.covenants.CovenantConsequence")]
pub fn covenant_consequence_rate_increase(
    #[wasm_bindgen(js_name = "bpIncrease", unchecked_param_type = "number")] bp_increase: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantConsequence::RateIncrease {
        bp_increase: js_f64(&bp_increase, "bpIncrease")?,
    })
}

/// Breach consequence: excess cash is swept to repay debt.
///
/// @param sweepPercentage - Share of excess cash swept, as a decimal in `[0, 1]` (`0.5` = 50%).
/// @returns `CovenantConsequence` wire value `{ cash_sweep: { sweep_percentage } }`.
/// @throws If `sweepPercentage` is not a number (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantConsequenceCashSweep, unchecked_return_type = "generated.covenants.CovenantConsequence")]
pub fn covenant_consequence_cash_sweep(
    #[wasm_bindgen(js_name = "sweepPercentage", unchecked_param_type = "number")] sweep_percentage: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantConsequence::CashSweep {
        sweep_percentage: js_f64(&sweep_percentage, "sweepPercentage")?,
    })
}

/// Breach consequence: distributions to equity are blocked.
///
/// @returns `CovenantConsequence` wire value `"block_distributions"`.
/// @throws If the value cannot be serialized.
#[wasm_bindgen(js_name = covenantConsequenceBlockDistributions, unchecked_return_type = "generated.covenants.CovenantConsequence")]
pub fn covenant_consequence_block_distributions() -> Result<JsValue, JsValue> {
    to_js_value(&CovenantConsequence::BlockDistributions)
}

/// Breach consequence: additional collateral must be posted.
///
/// @param description - Non-empty description of the required collateral.
/// @returns `CovenantConsequence` wire value `{ require_collateral: { description } }`.
/// @throws If `description` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantConsequenceRequireCollateral, unchecked_return_type = "generated.covenants.CovenantConsequence")]
pub fn covenant_consequence_require_collateral(
    #[wasm_bindgen(unchecked_param_type = "string")] description: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantConsequence::RequireCollateral {
        description: js_string(&description, "description")?,
    })
}

/// Breach consequence: the maturity is accelerated to a new date.
///
/// @param newMaturity - ISO-8601 accelerated maturity date.
/// @returns `CovenantConsequence` wire value `{ accelerate_maturity: { new_maturity } }`.
/// @throws If `newMaturity` is not an ISO-8601 string (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = covenantConsequenceAccelerateMaturity, unchecked_return_type = "generated.covenants.CovenantConsequence")]
pub fn covenant_consequence_accelerate_maturity(
    #[wasm_bindgen(js_name = "newMaturity", unchecked_param_type = "string")] new_maturity: JsValue,
) -> Result<JsValue, JsValue> {
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
/// @param covenantType - `CovenantType` wire value, for example `{ max_debt_to_ebitda: { threshold: 4.5 } }`.
/// @param testFrequency - `Tenor` wire object (`{ count, unit }`) recording how often the covenant is tested; it is metadata, the caller chooses each test date.
/// @param label - Instance label; it is the report key and must be unique within an engine on a test date.
/// @returns `Covenant` wire object.
/// @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = covenantNew, unchecked_return_type = "generated.covenants.Covenant")]
pub fn covenant_new(
    #[wasm_bindgen(
        js_name = "covenantType",
        unchecked_param_type = "generated.covenants.CovenantType"
    )]
    covenant_type: JsValue,
    #[wasm_bindgen(
        js_name = "testFrequency",
        unchecked_param_type = "generated.covenants.Tenor"
    )]
    test_frequency: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] label: JsValue,
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
#[wasm_bindgen(js_name = covenantWithCurePeriod, unchecked_return_type = "generated.covenants.Covenant")]
pub fn covenant_with_cure_period(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.Covenant")] covenant: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "number | null")] days: Option<JsValue>,
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
#[wasm_bindgen(js_name = covenantWithConsequence, unchecked_return_type = "generated.covenants.Covenant")]
pub fn covenant_with_consequence(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.Covenant")] covenant: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.CovenantConsequence")]
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
#[wasm_bindgen(js_name = covenantWithScope, unchecked_return_type = "generated.covenants.Covenant")]
pub fn covenant_with_scope(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.Covenant")] covenant: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.CovenantScope")] scope: JsValue,
) -> Result<JsValue, JsValue> {
    let covenant = js_wire::<Covenant>(&covenant, "covenant")?;
    to_js_value(&covenant.with_scope(js_wire::<CovenantScope>(&scope, "scope")?))
}

/// Copy of a covenant that is tested only while a springing condition holds.
///
/// @param covenant - `Covenant` wire object.
/// @param condition - `SpringingCondition` wire object (`{ metric_id, test }`), for example revolver utilization above a level; the covenant is inactive while the condition is unmet.
/// @returns The updated `Covenant`.
/// @throws If either argument does not match its wire type (kind `validation`).
#[wasm_bindgen(js_name = covenantWithSpringingCondition, unchecked_return_type = "generated.covenants.Covenant")]
pub fn covenant_with_springing_condition(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.Covenant")] covenant: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.SpringingCondition")]
    condition: JsValue,
) -> Result<JsValue, JsValue> {
    let covenant = js_wire::<Covenant>(&covenant, "covenant")?;
    to_js_value(
        &covenant.with_springing_condition(js_wire::<SpringingCondition>(&condition, "condition")?),
    )
}

// --- CovenantSpec / ThresholdSchedule ----------------------------------------------

/// Covenant specification that reads the covenant type's default metric.
///
/// Mirrors Rust `CovenantSpec::new` (the Python `CovenantSpec(covenant)`
/// constructor without a metric id): a net debt / EBITDA covenant also gets
/// the default `"ebitda"` earnings denominator.
///
/// @param covenant - `Covenant` wire object.
/// @returns `CovenantSpec` wire object with no `metric_id`; evaluation reads the covenant type's conventional metric (for example `"debt_to_ebitda"`) or the `metric` / `name` of a custom or basket covenant.
/// @throws If `covenant` is not a `Covenant` (kind `validation`).
#[wasm_bindgen(js_name = covenantSpecNew, unchecked_return_type = "generated.covenants.CovenantSpec")]
pub fn covenant_spec_new(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.Covenant")] covenant: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantSpec::new(js_wire::<Covenant>(
        &covenant, "covenant",
    )?))
}

/// Covenant specification that reads its test value from a named metric.
///
/// Mirrors Rust `CovenantSpec::with_metric` (the Python `CovenantSpec(...)`
/// constructor with a metric id): a net debt / EBITDA covenant also gets the
/// default `"ebitda"` earnings denominator.
///
/// @param covenant - `Covenant` wire object.
/// @param metricId - Identifier of the metric the covenant tests, as supplied in the metric map, for example `"debt_to_ebitda"`.
/// @returns `CovenantSpec` wire object.
/// @throws If `covenant` is not a `Covenant` (kind `validation`) or `metricId` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantSpecWithMetric, unchecked_return_type = "generated.covenants.CovenantSpec")]
pub fn covenant_spec_with_metric(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.Covenant")] covenant: JsValue,
    #[wasm_bindgen(js_name = "metricId", unchecked_param_type = "string")] metric_id: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantSpec::with_metric(
        js_wire::<Covenant>(&covenant, "covenant")?,
        js_string(&metric_id, "metricId")?,
    ))
}

/// Copy of a specification with an explicit earnings denominator metric.
///
/// @param spec - `CovenantSpec` wire object of a built-in leverage covenant.
/// @param metricId - Identifier of the earnings metric the leverage ratio divides by, for example `"adjusted_ebitda"`.
/// @returns The updated `CovenantSpec`.
/// @throws If `spec` is not a `CovenantSpec` (kind `validation`) or `metricId` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantSpecWithDenominatorMetric, unchecked_return_type = "generated.covenants.CovenantSpec")]
pub fn covenant_spec_with_denominator_metric(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.CovenantSpec")] spec: JsValue,
    #[wasm_bindgen(js_name = "metricId", unchecked_param_type = "string")] metric_id: JsValue,
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
#[wasm_bindgen(js_name = covenantSpecWithThresholdSchedule, unchecked_return_type = "generated.covenants.CovenantSpec")]
pub fn covenant_spec_with_threshold_schedule(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.CovenantSpec")] spec: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.ThresholdSchedule")]
    schedule: JsValue,
) -> Result<JsValue, JsValue> {
    let spec = js_wire::<CovenantSpec>(&spec, "spec")?;
    to_js_value(&spec.with_threshold_schedule(js_wire::<ThresholdSchedule>(&schedule, "schedule")?))
}

/// Threshold in force on a test date.
///
/// @param schedule - `ThresholdSchedule` wire value: `[isoDate, threshold]` pairs.
/// @param testDate - ISO-8601 covenant test date.
/// @returns The threshold of the latest entry effective on or before `testDate`, or `undefined` when the schedule starts later.
/// @throws If `schedule` has a non-finite threshold or repeats a date (kind `validation`), or `testDate` is not an ISO-8601 string (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = thresholdScheduleThresholdFor)]
pub fn threshold_schedule_threshold_for(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.ThresholdSchedule")]
    schedule: JsValue,
    #[wasm_bindgen(js_name = "testDate", unchecked_param_type = "string")] test_date: JsValue,
) -> Result<Option<f64>, JsValue> {
    Ok(js_wire::<ThresholdSchedule>(&schedule, "schedule")?
        .threshold_for(js_date(&test_date, "testDate")?))
}
