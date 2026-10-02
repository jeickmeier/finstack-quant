//! Constructors and methods of the instrument data types.
//!
//! Instrument sub-specs (pool assets, tranches, penalties, PIK schedules,
//! index presets, ...) cross the boundary as plain objects typed by the
//! schema-generated TypeScript types. The Rust constructors and methods of
//! those types are bound here as free functions that take and return the
//! plain object: `PoolAsset::fixed_rate_bond` is
//! `valuations.instruments.poolAssetFixedRateBond`, and so on. Monetary
//! amounts, tenors and day counts are their serde shapes here
//! (`{ amount, currency }`, `"act_360"`), not WASM handles.

use super::arg;
use super::structured::JsTrancheBuilder;
use crate::utils::input::{js_opt_f64, js_opt_string, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::Date;
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndexParams;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTrancheParams;
use finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::AmortizationEvent;
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::merton_mc::{
    BarrierCrossing, MertonMcConfig, PikMode, PikSchedule,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, CoverageRules, CoverageTestSpec, PenaltyStep, PoolAsset, PrepaymentPenalty,
    ScenarioTable, Tranche, TrancheStructure, Waterfall,
};
use wasm_bindgen::prelude::*;

/// An optional ISO-8601 date argument; `null` and `undefined` mean absent.
fn opt_date(value: Option<&JsValue>, label: &str) -> Result<Option<Date>, JsValue> {
    js_opt_string(value, label)?
        .map(|text| crate::utils::parse_iso_date(&text))
        .transpose()
}

// ---------------------------------------------------------------------------
// Merton Monte Carlo configuration
// ---------------------------------------------------------------------------

/// Configuration of the Merton structural Monte Carlo bond engine.
///
/// Wraps the Rust `MertonMcConfig`: the constructor takes the asset-value
/// model and a recovery rate, and each fluent method returns a new
/// configuration with one field replaced. Pass it to `Bond.priceMertonMc`.
#[wasm_bindgen(js_name = MertonMcConfig)]
#[derive(Clone)]
pub struct JsMertonMcConfig {
    pub(crate) inner: MertonMcConfig,
}

#[wasm_bindgen(js_class = MertonMcConfig)]
impl JsMertonMcConfig {
    /// Create a configuration with the Rust defaults (cash coupons, discrete barrier monitoring).
    /// @param merton - `MertonModel` plain object or JSON string: asset value, asset volatility, debt barrier and risk-free rate.
    /// @param recovery_rate - Recovery on default as a decimal fraction of the accreted notional, in `[0, 1]`.
    /// @returns The configuration with Rust defaults for every other field.
    /// @throws Error - Throws with kind `validation` if `merton` does not match the `MertonModel` schema or `recoveryRate` is outside `[0, 1]`, and kind `invalid_type` for a wrong argument type.
    #[wasm_bindgen(constructor)]
    pub fn new(merton: JsValue, recovery_rate: JsValue) -> Result<JsMertonMcConfig, JsValue> {
        MertonMcConfig::new(
            arg::json(&merton, "merton")?,
            arg::num(&recovery_rate, "recoveryRate")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Return a copy with a different PIK schedule.
    /// @param s - `PikSchedule` plain object or JSON string, e.g. from `valuations.instruments.pikScheduleUniform`.
    /// @returns A new configuration; the receiver is not modified.
    /// @throws Error - Throws with kind `validation` if `s` does not match the `PikSchedule` schema.
    #[wasm_bindgen(js_name = pikSchedule)]
    pub fn pik_schedule(&self, s: JsValue) -> Result<JsMertonMcConfig, JsValue> {
        let schedule = arg::json(&s, "s")?;
        Ok(Self {
            inner: self.inner.clone().pik_schedule(schedule),
        })
    }

    /// Return a copy with a different number of simulation steps per year.
    /// @param n - Time steps per year; more steps monitor the default barrier more finely.
    /// @returns A new configuration; the receiver is not modified.
    /// @throws Error - Throws with kind `invalid_type` if `n` is not a non-negative whole number.
    #[wasm_bindgen(js_name = stepsPerYear)]
    pub fn steps_per_year(&self, n: JsValue) -> Result<JsMertonMcConfig, JsValue> {
        let steps = arg::uint(&n, "n")?;
        Ok(Self {
            inner: self.inner.clone().steps_per_year(steps),
        })
    }

    /// Return a copy with a different barrier-crossing treatment.
    /// @param p - `BarrierCrossing` serde string: `"discrete"` or `"brownian_bridge"`.
    /// @returns A new configuration; the receiver is not modified.
    /// @throws Error - Throws with kind `validation` if `p` is not a known barrier-crossing name.
    #[wasm_bindgen(js_name = barrierCrossing)]
    pub fn barrier_crossing(&self, p: JsValue) -> Result<JsMertonMcConfig, JsValue> {
        let crossing = arg::en(&p, "p")?;
        Ok(Self {
            inner: self.inner.clone().barrier_crossing(crossing),
        })
    }

    /// Return a copy with a different recovery rate.
    /// @param r - Recovery on default as a decimal fraction of the accreted notional, in `[0, 1]`.
    /// @returns A new configuration; the receiver is not modified.
    /// @throws Error - Throws with kind `validation` if `r` is outside `[0, 1]`, and kind `invalid_type` if it is not a number.
    #[wasm_bindgen(js_name = recoveryRate)]
    pub fn recovery_rate(&self, r: JsValue) -> Result<JsMertonMcConfig, JsValue> {
        self.inner
            .clone()
            .recovery_rate(arg::num(&r, "r")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Return a copy with an endogenous (leverage-dependent) hazard specification.
    /// @param h - `EndogenousHazardSpec` plain object or JSON string.
    /// @returns A new configuration; the receiver is not modified.
    /// @throws Error - Throws with kind `validation` if `h` does not match the `EndogenousHazardSpec` schema.
    #[wasm_bindgen(js_name = endogenousHazard)]
    pub fn endogenous_hazard(&self, h: JsValue) -> Result<JsMertonMcConfig, JsValue> {
        let hazard = arg::json(&h, "h")?;
        Ok(Self {
            inner: self.inner.clone().endogenous_hazard(hazard),
        })
    }

    /// Return a copy with a dynamic (notional-dependent) recovery specification.
    /// @param r - `DynamicRecoverySpec` plain object or JSON string.
    /// @returns A new configuration; the receiver is not modified.
    /// @throws Error - Throws with kind `validation` if `r` does not match the `DynamicRecoverySpec` schema.
    #[wasm_bindgen(js_name = dynamicRecovery)]
    pub fn dynamic_recovery(&self, r: JsValue) -> Result<JsMertonMcConfig, JsValue> {
        let recovery = arg::json(&r, "r")?;
        Ok(Self {
            inner: self.inner.clone().dynamic_recovery(recovery),
        })
    }

    /// Return a copy with a PIK-toggle exercise model.
    /// @param t - `ToggleExerciseModel` plain object or JSON string.
    /// @returns A new configuration; the receiver is not modified.
    /// @throws Error - Throws with kind `validation` if `t` does not match the `ToggleExerciseModel` schema.
    #[wasm_bindgen(js_name = toggleModel)]
    pub fn toggle_model(&self, t: JsValue) -> Result<JsMertonMcConfig, JsValue> {
        let toggle = arg::json(&t, "t")?;
        Ok(Self {
            inner: self.inner.clone().toggle_model(toggle),
        })
    }

    /// Deserialize a configuration from its JSON form.
    /// @param json - `MertonMcConfig` JSON string or plain object.
    /// @returns The configuration.
    /// @throws Error - Throws with kind `validation` if `json` does not match the `MertonMcConfig` schema.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsMertonMcConfig, JsValue> {
        Ok(Self {
            inner: arg::json(&json, "json")?,
        })
    }

    /// Serialize the configuration to JSON.
    /// @returns Compact `MertonMcConfig` JSON accepted by `fromJson`.
    /// @throws Error - Throws if the configuration cannot be serialized.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }
}

/// `BarrierCrossing::Discrete`: default is checked only on the simulation grid.
/// @returns The `BarrierCrossing` value `"discrete"`.
/// @throws Error - Throws if the value cannot be converted to JavaScript.
#[wasm_bindgen(js_name = barrierCrossingDiscrete)]
pub fn barrier_crossing_discrete() -> Result<JsValue, JsValue> {
    to_js_value(&BarrierCrossing::Discrete)
}

/// `BarrierCrossing::BrownianBridge`: corrects for crossings between grid points.
/// @returns The `BarrierCrossing` value `"brownian_bridge"`.
/// @throws Error - Throws if the value cannot be converted to JavaScript.
#[wasm_bindgen(js_name = barrierCrossingBrownianBridge)]
pub fn barrier_crossing_brownian_bridge() -> Result<JsValue, JsValue> {
    to_js_value(&BarrierCrossing::BrownianBridge)
}

/// `PikMode::Cash`: the coupon is paid in cash.
/// @returns The `PikMode` value.
/// @throws Error - Throws if the value cannot be converted to JavaScript.
#[wasm_bindgen(js_name = pikModeCash)]
pub fn pik_mode_cash() -> Result<JsValue, JsValue> {
    to_js_value(&PikMode::Cash)
}

/// `PikMode::Pik`: the coupon accretes to the notional.
/// @returns The `PikMode` value.
/// @throws Error - Throws if the value cannot be converted to JavaScript.
#[wasm_bindgen(js_name = pikModePik)]
pub fn pik_mode_pik() -> Result<JsValue, JsValue> {
    to_js_value(&PikMode::Pik)
}

/// `PikMode::Split`: part of the coupon is paid in cash and part accretes.
/// @param cash_fraction - Fraction of the coupon paid in cash, in `[0, 1]`.
/// @param pik_fraction - Fraction of the coupon accreted, in `[0, 1]`.
/// @returns The `PikMode` value.
/// @throws Error - Throws with kind `invalid_type` if a fraction is not a number.
#[wasm_bindgen(js_name = pikModeSplit)]
pub fn pik_mode_split(cash_fraction: JsValue, pik_fraction: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&PikMode::Split {
        cash_fraction: arg::num(&cash_fraction, "cashFraction")?,
        pik_fraction: arg::num(&pik_fraction, "pikFraction")?,
    })
}

/// `PikMode::Toggle`: the issuer elects cash or PIK each period under the toggle model.
/// @returns The `PikMode` value.
/// @throws Error - Throws if the value cannot be converted to JavaScript.
#[wasm_bindgen(js_name = pikModeToggle)]
pub fn pik_mode_toggle() -> Result<JsValue, JsValue> {
    to_js_value(&PikMode::Toggle)
}

/// `PikSchedule::Uniform`: one PIK mode for the life of the bond.
/// @param mode - `PikMode` plain value or JSON string.
/// @returns The `PikSchedule` plain object.
/// @throws Error - Throws with kind `validation` if `mode` does not match the `PikMode` schema.
#[wasm_bindgen(js_name = pikScheduleUniform)]
pub fn pik_schedule_uniform(mode: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&PikSchedule::Uniform(arg::json_or_name(&mode, "mode")?))
}

/// `PikSchedule::Stepped`: the PIK mode changes at given times.
/// @param steps - `[timeInYears, PikMode]` pairs in increasing time order; each mode applies from its time on.
/// @returns The `PikSchedule` plain object.
/// @throws Error - Throws with kind `validation` if `steps` is not an array of `[number, PikMode]` pairs.
#[wasm_bindgen(js_name = pikScheduleStepped)]
pub fn pik_schedule_stepped(steps: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&PikSchedule::Stepped(arg::json(&steps, "steps")?))
}

/// The PIK mode in force at a time (mirrors Rust `PikSchedule::mode_at`).
/// @param schedule - `PikSchedule` plain object or JSON string.
/// @param t - Time from issue in years.
/// @returns The `PikMode` applying at `t`.
/// @throws Error - Throws with kind `validation` if `schedule` does not match the `PikSchedule` schema, and kind `invalid_type` if `t` is not a number.
#[wasm_bindgen(js_name = pikScheduleModeAt)]
pub fn pik_schedule_mode_at(schedule: JsValue, t: JsValue) -> Result<JsValue, JsValue> {
    let schedule: PikSchedule = arg::json(&schedule, "schedule")?;
    to_js_value(&schedule.mode_at(arg::num(&t, "t")?))
}

// ---------------------------------------------------------------------------
// Structured-credit collateral and liabilities
// ---------------------------------------------------------------------------

/// Create a fluent `TrancheBuilder` (mirrors Rust `Tranche::builder`).
/// @returns An empty tranche builder.
#[wasm_bindgen(js_name = trancheBuilder)]
pub fn tranche_builder() -> JsTrancheBuilder {
    JsTrancheBuilder::new()
}

/// Build a tranche structure whose attachment points come from the balances.
///
/// Mirrors Rust `TrancheStructure::from_balances`: tranches are ordered by
/// seniority and each one attaches where the more junior ones detach.
/// @param tranches - `Tranche` plain objects (or a JSON array string).
/// @returns The validated `TrancheStructure` plain object.
/// @throws Error - Throws with kind `validation` if a tranche does not match the `Tranche` schema or the structure is invalid (empty, mixed currencies, duplicate ids).
#[wasm_bindgen(js_name = trancheStructureFromBalances)]
pub fn tranche_structure_from_balances(tranches: JsValue) -> Result<JsValue, JsValue> {
    let tranches: Vec<Tranche> = arg::json(&tranches, "tranches")?;
    to_js_value(&TrancheStructure::from_balances(tranches).map_err(to_js_err)?)
}

/// Create a fixed-rate bond pool asset (mirrors Rust `PoolAsset::fixed_rate_bond`).
/// @param id - Asset identifier, unique within the pool.
/// @param balance - Current balance as a `Money` plain object.
/// @param rate - Annual coupon as a decimal (`0.07` = 7%).
/// @param maturity - Maturity date as an ISO-8601 string.
/// @param day_count - Day-count serde name, e.g. `"30_360"`.
/// @returns The `PoolAsset` plain object.
/// @throws Error - Throws with kind `validation` if `balance`, `maturity` or `dayCount` is malformed, and kind `invalid_type` for a wrong argument type.
#[wasm_bindgen(js_name = poolAssetFixedRateBond)]
pub fn pool_asset_fixed_rate_bond(
    id: JsValue,
    balance: JsValue,
    rate: JsValue,
    maturity: JsValue,
    day_count: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&PoolAsset::fixed_rate_bond(
        js_string(&id, "id")?,
        arg::json(&balance, "balance")?,
        arg::num(&rate, "rate")?,
        arg::date(&maturity, "maturity")?,
        arg::en(&day_count, "dayCount")?,
    ))
}

/// Create a floating-rate loan pool asset (mirrors Rust `PoolAsset::floating_rate_loan`).
/// @param id - Asset identifier, unique within the pool.
/// @param balance - Current balance as a `Money` plain object.
/// @param forward_curve_id - Forward curve projecting the loan's index, e.g. `"USD-SOFR-3M"`.
/// @param spread_bp - Margin over the index in basis points.
/// @param maturity - Maturity date as an ISO-8601 string.
/// @param day_count - Day-count serde name, e.g. `"act_360"`.
/// @returns The `PoolAsset` plain object.
/// @throws Error - Throws with kind `validation` if `balance`, `maturity` or `dayCount` is malformed, and kind `invalid_type` for a wrong argument type.
#[wasm_bindgen(js_name = poolAssetFloatingRateLoan)]
pub fn pool_asset_floating_rate_loan(
    id: JsValue,
    balance: JsValue,
    forward_curve_id: JsValue,
    spread_bp: JsValue,
    maturity: JsValue,
    day_count: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&PoolAsset::floating_rate_loan(
        js_string(&id, "id")?,
        arg::json(&balance, "balance")?,
        js_string(&forward_curve_id, "forwardCurveId")?,
        arg::num(&spread_bp, "spreadBp")?,
        arg::date(&maturity, "maturity")?,
        arg::en(&day_count, "dayCount")?,
    ))
}

/// Return a copy of a pool with its representative lines replaced.
/// @param pool - `AssetPool` plain object or JSON string.
/// @param rep_lines - `RepLine` plain objects (or a JSON array string).
/// @returns A new `AssetPool` plain object carrying `rep_lines`.
/// @throws Error - Throws with kind `validation` if `pool` or `repLines` does not match its schema.
#[wasm_bindgen(js_name = assetPoolWithRepLines)]
pub fn asset_pool_with_rep_lines(pool: JsValue, rep_lines: JsValue) -> Result<JsValue, JsValue> {
    let mut pool: AssetPool = arg::json(&pool, "pool")?;
    pool.rep_lines = Some(arg::json(&rep_lines, "repLines")?);
    to_js_value(&pool)
}

/// Return a copy of a pool with its loan-level assets replaced.
/// @param pool - `AssetPool` plain object or JSON string.
/// @param value - `PoolAsset` plain objects (or a JSON array string).
/// @returns A new `AssetPool` plain object carrying `assets`.
/// @throws Error - Throws with kind `validation` if `pool` or `value` does not match its schema.
#[wasm_bindgen(js_name = assetPoolWithAssets)]
pub fn asset_pool_with_assets(pool: JsValue, value: JsValue) -> Result<JsValue, JsValue> {
    let mut pool: AssetPool = arg::json(&pool, "pool")?;
    pool.assets = arg::json(&value, "value")?;
    to_js_value(&pool)
}

/// Return a copy of a pool collateralized by typed instruments.
/// @param pool - `AssetPool` plain object or JSON string.
/// @param collateral - `InstrumentCollateral` plain object: `bonds`, `term_loans` and `revolvers` instrument specs plus the optional `call_exercise`, `put_exercise` and `overrides` exercise policies.
/// @returns A new `AssetPool` plain object carrying `instruments`.
/// @throws Error - Throws with kind `validation` if `pool` or `collateral` does not match its schema.
#[wasm_bindgen(js_name = assetPoolWithInstruments)]
pub fn asset_pool_with_instruments(pool: JsValue, collateral: JsValue) -> Result<JsValue, JsValue> {
    let mut pool: AssetPool = arg::json(&pool, "pool")?;
    pool.instruments = Some(arg::json(&collateral, "collateral")?);
    to_js_value(&pool)
}

/// Return a copy of a pool with its reserve account configured.
/// @param pool - `AssetPool` plain object or JSON string.
/// @param reserve_account - Opening reserve balance as a `Money` plain object.
/// @param reserve_account_rate - Annual interest rate earned on the reserve, as a decimal.
/// @param reserve_target - Optional reserve target as a `Money` plain object; omit for no target.
/// @param reserve_interest_destination - Optional `ReserveInterestDestination` plain object; omit for the Rust default (interest stays in the reserve).
/// @returns A new `AssetPool` plain object with the reserve fields set.
/// @throws Error - Throws with kind `validation` if an argument does not match its schema, and kind `invalid_type` if `reserveAccountRate` is not a number.
#[wasm_bindgen(js_name = assetPoolWithReserve)]
pub fn asset_pool_with_reserve(
    pool: JsValue,
    reserve_account: JsValue,
    reserve_account_rate: JsValue,
    reserve_target: Option<JsValue>,
    reserve_interest_destination: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let mut pool: AssetPool = arg::json(&pool, "pool")?;
    pool.reserve_account = arg::json(&reserve_account, "reserveAccount")?;
    pool.reserve_account_rate = arg::num(&reserve_account_rate, "reserveAccountRate")?;
    pool.reserve_target = present(reserve_target.as_ref())
        .map(|value| arg::json(value, "reserveTarget"))
        .transpose()?;
    pool.reserve_interest_destination = present(reserve_interest_destination.as_ref())
        .map(|value| arg::json(value, "reserveInterestDestination"))
        .transpose()?
        .unwrap_or_default();
    to_js_value(&pool)
}

/// Return a copy of a pool with a reinvestment period.
/// @param pool - `AssetPool` plain object or JSON string.
/// @param value - `ReinvestmentPeriod` plain object or JSON string: end date, active flag and reinvestment criteria.
/// @returns A new `AssetPool` plain object carrying `reinvestment_period`.
/// @throws Error - Throws with kind `validation` if `pool` or `value` does not match its schema.
#[wasm_bindgen(js_name = assetPoolWithReinvestmentPeriod)]
pub fn asset_pool_with_reinvestment_period(
    pool: JsValue,
    value: JsValue,
) -> Result<JsValue, JsValue> {
    let mut pool: AssetPool = arg::json(&pool, "pool")?;
    pool.reinvestment_period = Some(arg::json(&value, "value")?);
    to_js_value(&pool)
}

/// Account balances accepted by `assetPoolWithAccounts`; absent fields are left unchanged.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolAccounts {
    #[serde(default)]
    cumulative_defaults: Option<finstack_quant_core::money::Money>,
    #[serde(default)]
    cumulative_recoveries: Option<finstack_quant_core::money::Money>,
    #[serde(default)]
    cumulative_prepayments: Option<finstack_quant_core::money::Money>,
    #[serde(default)]
    cumulative_scheduled_amortization: Option<finstack_quant_core::money::Money>,
    #[serde(default)]
    collection_account: Option<finstack_quant_core::money::Money>,
    #[serde(default)]
    excess_spread_account: Option<finstack_quant_core::money::Money>,
    #[serde(default)]
    original_balance: Option<finstack_quant_core::money::Money>,
}

/// Return a copy of a pool with seasoned account balances.
/// @param pool - `AssetPool` plain object or JSON string.
/// @param accounts - Plain object with any of `cumulative_defaults`, `cumulative_recoveries`, `cumulative_prepayments`, `cumulative_scheduled_amortization`, `collection_account`, `excess_spread_account` and `original_balance`, each a `Money` plain object; absent fields keep the pool's value.
/// @returns A new `AssetPool` plain object with the given balances.
/// @throws Error - Throws with kind `validation` if `pool` does not match the `AssetPool` schema or `accounts` has an unknown field or a malformed amount.
#[wasm_bindgen(js_name = assetPoolWithAccounts)]
pub fn asset_pool_with_accounts(pool: JsValue, accounts: JsValue) -> Result<JsValue, JsValue> {
    let mut pool: AssetPool = arg::json(&pool, "pool")?;
    let accounts: PoolAccounts = arg::json(&accounts, "accounts")?;
    if let Some(value) = accounts.original_balance {
        pool.original_balance = Some(value);
    }
    if let Some(value) = accounts.cumulative_defaults {
        pool.cumulative_defaults = value;
    }
    if let Some(value) = accounts.cumulative_recoveries {
        pool.cumulative_recoveries = value;
    }
    if let Some(value) = accounts.cumulative_prepayments {
        pool.cumulative_prepayments = value;
    }
    if let Some(value) = accounts.cumulative_scheduled_amortization {
        pool.cumulative_scheduled_amortization = value;
    }
    if let Some(value) = accounts.collection_account {
        pool.collection_account = value;
    }
    if let Some(value) = accounts.excess_spread_account {
        pool.excess_spread_account = value;
    }
    to_js_value(&pool)
}

/// A present optional argument (`null` and `undefined` mean absent).
fn present(value: Option<&JsValue>) -> Option<&JsValue> {
    value.filter(|value| !value.is_null() && !value.is_undefined())
}

/// Validate a prepayment penalty and return its plain object.
fn penalty(value: PrepaymentPenalty) -> Result<JsValue, JsValue> {
    value.validate().map_err(to_js_err)?;
    to_js_value(&value)
}

/// `PrepaymentPenalty::Lockout`: voluntary prepayment is not allowed.
/// @param through - Optional last date of the lockout as an ISO-8601 string; omit for a lockout to maturity.
/// @returns The `PrepaymentPenalty` plain object.
/// @throws Error - Throws with kind `validation` if `through` is malformed, and kind `invalid_type` if it is not a string.
#[wasm_bindgen(js_name = prepaymentPenaltyLockout)]
pub fn prepayment_penalty_lockout(through: Option<JsValue>) -> Result<JsValue, JsValue> {
    penalty(PrepaymentPenalty::Lockout {
        through: opt_date(through.as_ref(), "through")?,
    })
}

/// `PrepaymentPenalty::Fixed`: a fixed percentage of the prepaid balance.
/// @param pct - Penalty in percent of the prepaid balance (`3` = 3%).
/// @param through - Optional last date the penalty applies, as an ISO-8601 string; omit for the life of the loan.
/// @returns The `PrepaymentPenalty` plain object.
/// @throws Error - Throws with kind `validation` if `pct` is negative or not finite or `through` is malformed, and kind `invalid_type` for a wrong argument type.
#[wasm_bindgen(js_name = prepaymentPenaltyFixed)]
pub fn prepayment_penalty_fixed(
    pct: JsValue,
    through: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    penalty(PrepaymentPenalty::Fixed {
        pct: arg::num(&pct, "pct")?,
        through: opt_date(through.as_ref(), "through")?,
    })
}

/// `PrepaymentPenalty::StepDown`: a declining schedule of percentages.
/// @param schedule - `[throughDate, pct]` pairs in increasing date order: `pct` percent of the prepaid balance applies through each ISO-8601 date.
/// @returns The `PrepaymentPenalty` plain object.
/// @throws Error - Throws with kind `validation` if `schedule` is not an array of `[string, number]` pairs, a date is malformed, or the schedule is empty, unordered or has a negative percentage.
#[wasm_bindgen(js_name = prepaymentPenaltyStepDown)]
pub fn prepayment_penalty_step_down(schedule: JsValue) -> Result<JsValue, JsValue> {
    let rows: Vec<(String, f64)> = arg::json(&schedule, "schedule")?;
    let schedule = rows
        .into_iter()
        .map(|(through, pct)| {
            Ok(PenaltyStep {
                through: crate::utils::parse_iso_date(&through)?,
                pct,
            })
        })
        .collect::<Result<Vec<_>, JsValue>>()?;
    penalty(PrepaymentPenalty::StepDown { schedule })
}

/// `PrepaymentPenalty::YieldMaintenance`: the lender is made whole on lost interest.
/// @param reinvestment_rate - Optional flat reinvestment rate as a decimal; omit to discount on `discountCurveId`.
/// @param discount_curve_id - Optional discount curve used for the reinvestment yield; omit to use `reinvestmentRate`.
/// @param floor_pct - Optional minimum penalty in percent of the prepaid balance.
/// @param through - Optional last date the penalty applies, as an ISO-8601 string; omit for the life of the loan.
/// @returns The `PrepaymentPenalty` plain object.
/// @throws Error - Throws with kind `validation` if neither or both of `reinvestmentRate` and `discountCurveId` are usable, a value is negative or not finite, or `through` is malformed; and kind `invalid_type` for a wrong argument type.
#[wasm_bindgen(js_name = prepaymentPenaltyYieldMaintenance)]
pub fn prepayment_penalty_yield_maintenance(
    reinvestment_rate: Option<JsValue>,
    discount_curve_id: Option<JsValue>,
    floor_pct: Option<JsValue>,
    through: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    penalty(PrepaymentPenalty::YieldMaintenance {
        discount_curve_id: js_opt_string(discount_curve_id.as_ref(), "discountCurveId")?
            .map(Into::into),
        reinvestment_rate: js_opt_f64(reinvestment_rate.as_ref(), "reinvestmentRate")?,
        floor_pct: js_opt_f64(floor_pct.as_ref(), "floorPct")?,
        through: opt_date(through.as_ref(), "through")?,
    })
}

/// Standard CLO par-value test rules (mirrors Rust `CoverageRules::clo_standard`).
///
/// Performing collateral at par, defaulted collateral at recovery, a 7.5%
/// CCC bucket carried at market value and an 80% discount-obligation threshold.
/// @returns The `CoverageRules` plain object.
/// @throws Error - Throws if the value cannot be converted to JavaScript.
#[wasm_bindgen(js_name = coverageRulesCloStandard)]
pub fn coverage_rules_clo_standard() -> Result<JsValue, JsValue> {
    to_js_value(&CoverageRules::clo_standard())
}

/// Validate coverage rules (mirrors Rust `CoverageRules::validate`).
/// @param rules - `CoverageRules` plain object or JSON string.
/// @throws Error - Throws with kind `validation` if `rules` does not match the `CoverageRules` schema or a haircut, threshold or carry value is out of range.
#[wasm_bindgen(js_name = coverageRulesValidate)]
pub fn coverage_rules_validate(rules: JsValue) -> Result<(), JsValue> {
    let rules: CoverageRules = arg::json(&rules, "rules")?;
    rules.validate().map_err(to_js_err)
}

/// Coverage tests a waterfall evaluates, in tier order (mirrors Rust `Waterfall::coverage_tests`).
/// @param waterfall - `Waterfall` plain object or JSON string.
/// @returns `CoverageTestSpec` plain objects in the order the waterfall runs them.
/// @throws Error - Throws with kind `validation` if `waterfall` does not match the `Waterfall` schema.
#[wasm_bindgen(js_name = waterfallCoverageTests)]
pub fn waterfall_coverage_tests(waterfall: JsValue) -> Result<JsValue, JsValue> {
    let waterfall: Waterfall = arg::json(&waterfall, "waterfall")?;
    let tests: Vec<&CoverageTestSpec> = waterfall.coverage_tests().collect();
    to_js_value(&tests)
}

/// The grid cells of a structured-credit scenario table.
/// @param table - `ScenarioTable` plain object or JSON string, as returned by `structuredCreditTrancheScenarioTable`.
/// @returns `ScenarioCell` plain objects: `cpr`, `cdr`, `severity`, `price`, `wal` and `writedown` per grid point.
/// @throws Error - Throws with kind `validation` if `table` does not match the `ScenarioTable` schema.
#[wasm_bindgen(js_name = scenarioTableCells)]
pub fn scenario_table_cells(table: JsValue) -> Result<JsValue, JsValue> {
    let table: ScenarioTable = arg::json(&table, "table")?;
    to_js_value(&table.cells)
}

// ---------------------------------------------------------------------------
// Asset-backed facility terms
// ---------------------------------------------------------------------------

/// Validate an amortization event and return its plain object.
fn amortization_event(event: AmortizationEvent) -> Result<JsValue, JsValue> {
    event.validate().map_err(to_js_err)?;
    to_js_value(&event)
}

/// `AmortizationEvent::Date`: revolving stops on a scheduled date.
/// @param date - Date revolving stops, as an ISO-8601 string.
/// @returns The `AmortizationEvent` plain object.
/// @throws Error - Throws with kind `validation` if `date` is malformed, and kind `invalid_type` if it is not a string.
#[wasm_bindgen(js_name = amortizationEventDate)]
pub fn amortization_event_date(date: JsValue) -> Result<JsValue, JsValue> {
    amortization_event(AmortizationEvent::Date {
        date: arg::date(&date, "date")?,
    })
}

/// `AmortizationEvent::CumulativeLoss`: revolving stops when losses pass a limit.
/// @param max_cumulative_loss - Cumulative net loss limit as a decimal fraction of the original pool balance (`0.05` = 5%).
/// @returns The `AmortizationEvent` plain object.
/// @throws Error - Throws with kind `validation` if the limit is outside `(0, 1]`, and kind `invalid_type` if it is not a number.
#[wasm_bindgen(js_name = amortizationEventCumulativeLoss)]
pub fn amortization_event_cumulative_loss(
    max_cumulative_loss: JsValue,
) -> Result<JsValue, JsValue> {
    amortization_event(AmortizationEvent::CumulativeLoss {
        max_cumulative_loss: arg::num(&max_cumulative_loss, "maxCumulativeLoss")?,
    })
}

/// `AmortizationEvent::ExcessSpread`: revolving stops when excess spread falls below a floor.
/// @param min_excess_spread_3m - Minimum three-month average annualized excess spread as a decimal (`0.01` = 1%).
/// @returns The `AmortizationEvent` plain object.
/// @throws Error - Throws with kind `validation` if the floor is not finite, and kind `invalid_type` if it is not a number.
#[wasm_bindgen(js_name = amortizationEventExcessSpread)]
pub fn amortization_event_excess_spread(min_excess_spread_3m: JsValue) -> Result<JsValue, JsValue> {
    amortization_event(AmortizationEvent::ExcessSpread {
        min_excess_spread_3m: arg::num(&min_excess_spread_3m, "minExcessSpread3m")?,
    })
}

// ---------------------------------------------------------------------------
// CDS index and tranche presets
// ---------------------------------------------------------------------------

/// One `CdsIndexParams` preset factory `(series, version, couponBp)`.
macro_rules! index_preset {
    ($(#[$doc:meta])* $name:ident as $js_name:ident => $preset:ident) => {
        $(#[$doc])*
        /// @param series - Published series number of the index.
        /// @param version - Index version within the series after credit events (1 at launch).
        /// @param coupon_bp - Running fixed coupon in basis points.
        /// @returns The `CdsIndexParams` plain object for `CdsIndex.fromPreset`.
        /// @throws Error - Throws with kind `invalid_type` if `series` or `version` is not a whole number in `0..=65535` or `couponBp` is not a number.
        #[wasm_bindgen(js_name = $js_name)]
        pub fn $name(series: JsValue, version: JsValue, coupon_bp: JsValue) -> Result<JsValue, JsValue> {
            to_js_value(&CdsIndexParams::$preset(
                arg::uint(&series, "series")?,
                arg::uint(&version, "version")?,
                arg::num(&coupon_bp, "couponBp")?,
            ))
        }
    };
}

index_preset!(
    /// CDX North America Investment Grade preset (mirrors Rust `CdsIndexParams::cdx_na_ig`).
    cds_index_params_cdx_na_ig as cdsIndexParamsCdxNaIg => cdx_na_ig
);
index_preset!(
    /// CDX North America High Yield preset (mirrors Rust `CdsIndexParams::cdx_na_hy`).
    cds_index_params_cdx_na_hy as cdsIndexParamsCdxNaHy => cdx_na_hy
);
index_preset!(
    /// iTraxx Europe preset (mirrors Rust `CdsIndexParams::itraxx_europe`).
    cds_index_params_itraxx_europe as cdsIndexParamsItraxxEurope => itraxx_europe
);

/// One `CdsTrancheParams` factory `(indexName, series, notional, maturity, couponBp)`.
macro_rules! tranche_preset {
    ($(#[$doc:meta])* $name:ident as $js_name:ident => $preset:ident) => {
        $(#[$doc])*
        /// @param index_name - Index family name, e.g. `"CDX.NA.IG"`.
        /// @param series - Published series number of the index.
        /// @param notional - Tranche notional as a `Money` plain object.
        /// @param maturity - Maturity date as an ISO-8601 string.
        /// @param coupon_bp - Running coupon in basis points.
        /// @returns The `CdsTrancheParams` plain object for `CdsTranche.standard`.
        /// @throws Error - Throws with kind `validation` if `notional` or `maturity` is malformed, and kind `invalid_type` for a wrong argument type.
        #[wasm_bindgen(js_name = $js_name)]
        pub fn $name(
            index_name: JsValue,
            series: JsValue,
            notional: JsValue,
            maturity: JsValue,
            coupon_bp: JsValue,
        ) -> Result<JsValue, JsValue> {
            to_js_value(&CdsTrancheParams::$preset(
                js_string(&index_name, "indexName")?,
                arg::uint(&series, "series")?,
                arg::json(&notional, "notional")?,
                arg::date(&maturity, "maturity")?,
                arg::num(&coupon_bp, "couponBp")?,
            ))
        }
    };
}

tranche_preset!(
    /// Equity tranche, 0% to 3% (mirrors Rust `CdsTrancheParams::equity_tranche`).
    cds_tranche_params_equity_tranche as cdsTrancheParamsEquityTranche => equity_tranche
);
tranche_preset!(
    /// Mezzanine tranche, 3% to 7% (mirrors Rust `CdsTrancheParams::mezzanine_tranche`).
    cds_tranche_params_mezzanine_tranche as cdsTrancheParamsMezzanineTranche => mezzanine_tranche
);

// ---------------------------------------------------------------------------
// Instrument JSON helpers
// ---------------------------------------------------------------------------

/// Validate a canonical envelope for one exact instrument type.
///
/// Mirrors Rust `pricer::validate_typed_instrument_json`.
/// @param type_tag - Canonical instrument discriminator expected by the caller, e.g. `"bond"`.
/// @param json - Canonical v1 instrument envelope (JSON string or plain object).
/// @returns The canonical envelope JSON.
/// @throws Error - Throws with kind `validation` if `json` is malformed, fails instrument validation, or carries another instrument type.
#[wasm_bindgen(js_name = validateTypedInstrumentJson)]
pub fn validate_typed_instrument_json(type_tag: JsValue, json: JsValue) -> Result<String, JsValue> {
    finstack_quant_valuations::pricer::validate_typed_instrument_json(
        &js_string(&type_tag, "typeTag")?,
        &crate::utils::input::json_text(&json, "json")?,
    )
    .map_err(to_js_err)
}

/// Pretty-print a canonical instrument envelope.
///
/// Mirrors Rust `pricer::pretty_instrument_json`: validates the envelope and
/// re-serializes it with indentation.
/// @param json - Canonical v1 instrument envelope (JSON string or plain object).
/// @returns Indented canonical envelope JSON.
/// @throws Error - Throws with kind `validation` if `json` is malformed or fails instrument validation.
#[wasm_bindgen(js_name = prettyInstrumentJson)]
pub fn pretty_instrument_json(json: JsValue) -> Result<String, JsValue> {
    finstack_quant_valuations::pricer::pretty_instrument_json(&crate::utils::input::json_text(
        &json, "json",
    )?)
    .map_err(to_js_err)
}
