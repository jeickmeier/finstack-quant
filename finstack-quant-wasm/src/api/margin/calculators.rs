//! Margin calculators: variation margin, schedule IM and haircut IM.
//!
//! `VmCalculator`, `ScheduleImCalculator` and `HaircutImCalculator` are WASM
//! classes whose members mirror the Python classes. Their results (`VmResult`,
//! `ImResult`, `MarginCall`) are plain JSON values typed by the generated
//! TypeScript.

use super::types::parse_csa;
use super::{js_currency, js_date, js_money};
use crate::utils::input::{from_js_json, js_bool, js_f64, js_opt_string, js_string};
use crate::utils::{date_to_iso, parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::money::Money;
use finstack_quant_margin as fm;
use wasm_bindgen::prelude::*;

fn collateral_asset_class(
    value: &JsValue,
    label: &str,
) -> Result<fm::CollateralAssetClass, JsValue> {
    js_string(value, label)?
        .parse::<fm::CollateralAssetClass>()
        .map_err(to_js_err)
}

fn schedule_asset_class(label: &str) -> Result<fm::ScheduleAssetClass, JsValue> {
    label.parse::<fm::ScheduleAssetClass>().map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// VmResult
// ---------------------------------------------------------------------------

/// Net desk cash outflow of a variation margin result: post minus collect.
/// @param result - `VmResult` (object or JSON).
/// @returns The net margin in major units of the CSA base currency; positive means the desk posts.
///
/// # Errors
///
/// Throws if `result` is malformed.
#[wasm_bindgen(js_name = vmResultNetMargin)]
pub fn vm_result_net_margin(result: JsValue) -> Result<f64, JsValue> {
    let result: fm::VmResult = from_js_json(&result, "result")?;
    Ok(result.net_margin().amount())
}

/// Whether a variation margin result requires a margin call.
/// @param result - `VmResult` (object or JSON).
/// @returns `true` when a post or collect amount is non-zero.
///
/// # Errors
///
/// Throws if `result` is malformed.
#[wasm_bindgen(js_name = vmResultRequiresCall)]
pub fn vm_result_requires_call(result: JsValue) -> Result<bool, JsValue> {
    let result: fm::VmResult = from_js_json(&result, "result")?;
    Ok(result.requires_call())
}

// ---------------------------------------------------------------------------
// VmCalculator
// ---------------------------------------------------------------------------

/// Variation margin calculator following ISDA CSA rules.
///
/// Applies the CSA threshold, independent amount, minimum transfer amount and
/// rounding to a signed exposure, dates the settlement on the CSA calendar,
/// and can run a whole MTM series into a margin-call schedule
/// (`generateMarginCalls`) or list the contractual call dates
/// (`marginCallDates`).
///
/// @example
/// ```javascript
/// import init, { margin } from "finstack-quant-wasm";
/// await init();
/// const calc = new margin.VmCalculator(margin.csaSpecUsdRegulatory());
/// const vm = calc.calculate(1_000_000, 0, "USD", "2024-06-17");
/// margin.vmResultRequiresCall(vm); // true
/// ```
#[wasm_bindgen(js_name = VmCalculator)]
pub struct JsVmCalculator {
    inner: fm::VmCalculator,
    csa: fm::CsaSpec,
}

#[wasm_bindgen(js_class = VmCalculator)]
impl JsVmCalculator {
    /// Bind a variation margin calculator to one CSA specification.
    /// @param csa - `CsaSpec` (object or JSON): thresholds, transfer minimums, rounding rules, base currency and calendar applied to each margin call.
    ///
    /// # Errors
    ///
    /// Throws if `csa` is malformed, has unknown fields, or fails CSA validation.
    #[wasm_bindgen(constructor)]
    pub fn new(csa: JsValue) -> Result<JsVmCalculator, JsValue> {
        let csa = parse_csa(&csa, "csa")?;
        Ok(Self {
            inner: fm::VmCalculator::new(csa.clone()),
            csa,
        })
    }

    /// CSA specification this calculator applies, as a plain `CsaSpec` object.
    ///
    /// # Errors
    ///
    /// Throws if the specification cannot be converted to a JavaScript value.
    #[wasm_bindgen(getter)]
    pub fn csa(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.csa)
    }

    /// Calculate variation margin for one date.
    /// @param exposure - Signed mark-to-market in `currency`: positive means the counterparty owes the desk.
    /// @param posted_collateral - Signed collateral balance in `currency`: positive held, negative posted, including pending agreed calls.
    /// @param currency - ISO-4217 code; must equal the CSA base currency.
    /// @param as_of - ISO-8601 calculation date; the settlement date is derived from it on the CSA calendar.
    /// @returns The `VmResult` as a plain object: `date`, `gross_exposure`, `net_exposure`, `post_amount`, `collect_amount` (Money) and `settlement_date`.
    ///
    /// # Errors
    ///
    /// Throws if the currency is unknown or differs from the CSA base
    /// currency, an amount is non-finite, the date is not ISO 8601, or the CSA
    /// calendar is not registered.
    pub fn calculate(
        &self,
        exposure: JsValue,
        posted_collateral: JsValue,
        currency: JsValue,
        as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let currency = js_currency(&currency, "currency")?;
        let exposure = js_money(&exposure, "exposure", currency)?;
        let posted = js_money(&posted_collateral, "postedCollateral", currency)?;
        let as_of = js_date(&as_of, "asOf")?;
        to_js_value(
            &self
                .inner
                .calculate(exposure, posted, as_of)
                .map_err(to_js_err)?,
        )
    }

    /// Run an exposure time series into a margin-call schedule.
    /// @param exposures - Array of `[date, exposure]` pairs: ISO-8601 date and signed exposure in the CSA base currency (positive means the counterparty owes the desk), processed in the order given.
    /// @param initial_collateral - Collateral balance before the first date, in major units of the CSA base currency.
    /// @returns One `MarginCall` plain object per call (`call_date`, `settlement_date`, `call_type`, `amount`, `mtm_trigger`, `threshold`, `mta_applied`); dates without a call produce no entry.
    ///
    /// # Errors
    ///
    /// Throws if `exposures` is not an array of `[string, number]` pairs, an
    /// amount is non-finite, a date is not ISO 8601, or the CSA calendar is
    /// not registered.
    #[wasm_bindgen(js_name = generateMarginCalls)]
    pub fn generate_margin_calls(
        &self,
        exposures: JsValue,
        initial_collateral: JsValue,
    ) -> Result<JsValue, JsValue> {
        let currency = self.csa.base_currency;
        let exposures = from_js_json::<Vec<(String, f64)>>(&exposures, "exposures")?
            .into_iter()
            .map(|(date, amount)| {
                Ok((
                    parse_iso_date(&date)?,
                    Money::new(amount, currency).map_err(to_js_err)?,
                ))
            })
            .collect::<Result<Vec<_>, JsValue>>()?;
        let initial = js_money(&initial_collateral, "initialCollateral", currency)?;
        to_js_value(
            &self
                .inner
                .generate_margin_calls(&exposures, initial)
                .map_err(to_js_err)?,
        )
    }

    /// Contractual margin-call dates between two dates, inclusive.
    ///
    /// Follows the CSA VM frequency on the CSA calendar: daily lists every
    /// business day, weekly and monthly roll from `start` with each date
    /// adjusted forward, on-demand returns just the adjusted endpoints.
    /// @param start - ISO-8601 first date of the window.
    /// @param end - ISO-8601 last date of the window.
    /// @returns The call dates as ISO-8601 strings, in order.
    ///
    /// # Errors
    ///
    /// Throws if a date is not ISO 8601 or the CSA calendar is not registered.
    #[wasm_bindgen(js_name = marginCallDates)]
    pub fn margin_call_dates(&self, start: JsValue, end: JsValue) -> Result<Vec<String>, JsValue> {
        let start = js_date(&start, "start")?;
        let end = js_date(&end, "end")?;
        Ok(self
            .inner
            .margin_call_dates(start, end)
            .map_err(to_js_err)?
            .into_iter()
            .map(date_to_iso)
            .collect())
    }
}

// ---------------------------------------------------------------------------
// ScheduleImCalculator
// ---------------------------------------------------------------------------

/// BCBS-IOSCO regulatory schedule initial margin calculator.
///
/// Applies registry-backed schedule rates to explicit notionals, or to a
/// heterogeneous netting set with trade-specific rates and the BCBS-IOSCO
/// net-to-gross ratio reduction.
///
/// @example
/// ```javascript
/// import init, { margin } from "finstack-quant-wasm";
/// await init();
/// const calc = margin.ScheduleImCalculator.bcbsStandard();
/// calc.rate("interest_rate", 5.0); // 0.04
/// calc.calculateForNotional(1_000_000, "USD", "interest_rate", 5.0, "2025-01-15").amount.amount;
/// ```
#[wasm_bindgen(js_name = ScheduleImCalculator)]
#[derive(Clone)]
pub struct JsScheduleImCalculator {
    inner: fm::ScheduleImCalculator,
}

#[wasm_bindgen(js_class = ScheduleImCalculator)]
impl JsScheduleImCalculator {
    /// Calculator on the BCBS-IOSCO standard regulatory schedule.
    /// @returns A `ScheduleImCalculator` handle.
    ///
    /// # Errors
    ///
    /// Throws if the embedded margin registry cannot be loaded.
    #[wasm_bindgen(js_name = bcbsStandard)]
    pub fn bcbs_standard() -> Result<JsScheduleImCalculator, JsValue> {
        Ok(Self {
            inner: fm::ScheduleImCalculator::bcbs_standard().map_err(to_js_err)?,
        })
    }

    /// Calculator on a schedule from the embedded margin registry.
    /// @param schedule_id - Schedule identifier in the registry, for example `"bcbs_iosco"` (`margin.constants().BCBS_IOSCO_SCHEDULE_ID`).
    /// @returns A `ScheduleImCalculator` handle.
    ///
    /// # Errors
    ///
    /// Throws if `schedule_id` is unknown or the registry data is invalid.
    #[wasm_bindgen(js_name = fromRegistryId)]
    pub fn from_registry_id(schedule_id: JsValue) -> Result<JsScheduleImCalculator, JsValue> {
        let schedule_id = js_string(&schedule_id, "scheduleId")?;
        Ok(Self {
            inner: fm::ScheduleImCalculator::from_registry_id(&schedule_id).map_err(to_js_err)?,
        })
    }

    /// Copy with a new default schedule asset class.
    /// @param asset_class - Lower-case schedule asset class label: `"interest_rate"`, `"credit"`, `"equity"`, `"commodity"`, `"fx"`, `"other"`, or `"custom_<name>"` for a registry-defined class.
    /// @returns A new `ScheduleImCalculator` handle.
    ///
    /// # Errors
    ///
    /// Throws if the asset class label is unknown.
    #[wasm_bindgen(js_name = withAssetClass)]
    pub fn with_asset_class(
        &self,
        asset_class: JsValue,
    ) -> Result<JsScheduleImCalculator, JsValue> {
        let asset_class = schedule_asset_class(&js_string(&asset_class, "assetClass")?)?;
        Ok(Self {
            inner: self.inner.clone().with_asset_class(asset_class),
        })
    }

    /// Copy with a new default maturity.
    /// @param years - Representative remaining maturity in years; finite and non-negative.
    /// @returns A new `ScheduleImCalculator` handle.
    ///
    /// # Errors
    ///
    /// Throws if `years` is negative or non-finite.
    #[wasm_bindgen(js_name = withMaturity)]
    pub fn with_maturity(&self, years: JsValue) -> Result<JsScheduleImCalculator, JsValue> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_maturity(js_f64(&years, "years")?)
                .map_err(to_js_err)?,
        })
    }

    /// Default schedule asset class label used when a trade does not name one.
    #[wasm_bindgen(getter, js_name = defaultAssetClass)]
    pub fn default_asset_class(&self) -> String {
        self.inner.default_asset_class.as_str().into_owned()
    }

    /// Default remaining maturity in years used for the schedule-rate lookup.
    #[wasm_bindgen(getter, js_name = defaultMaturityYears)]
    pub fn default_maturity_years(&self) -> f64 {
        self.inner.default_maturity_years
    }

    /// Margin period of risk in business days stamped on every result.
    #[wasm_bindgen(getter, js_name = mporDays)]
    pub fn mpor_days(&self) -> u32 {
        self.inner.mpor_days
    }

    /// Look up a schedule rate.
    /// @param asset_class - Lower-case schedule asset class label such as `"interest_rate"`.
    /// @param maturity_years - Remaining maturity in years; finite and non-negative.
    /// @returns The IM rate as a decimal fraction of notional (0.04 is 4%).
    ///
    /// # Errors
    ///
    /// Throws if the asset class is unknown, the maturity is negative or
    /// non-finite, or the selected rate is outside `[0, 1]`.
    pub fn rate(&self, asset_class: JsValue, maturity_years: JsValue) -> Result<f64, JsValue> {
        let asset_class = schedule_asset_class(&js_string(&asset_class, "assetClass")?)?;
        self.inner
            .rate(asset_class, js_f64(&maturity_years, "maturityYears")?)
            .map_err(to_js_err)
    }

    /// Gross schedule IM for an explicit notional: `abs(notional) * rate`.
    /// @param notional - Regulatory notional or exposure base, in major units of `currency`; its absolute value is used.
    /// @param currency - ISO-4217 currency of the notional and of the result.
    /// @param asset_class - Lower-case schedule asset class label such as `"interest_rate"`.
    /// @param maturity_years - Remaining maturity in years used for the rate lookup.
    /// @param as_of - ISO-8601 calculation date stamped on the result.
    /// @returns The `ImResult` as a plain object, with one breakdown entry keyed by the asset class.
    ///
    /// # Errors
    ///
    /// Throws if the currency, asset class, amount, maturity or date is invalid.
    #[wasm_bindgen(js_name = calculateForNotional)]
    pub fn calculate_for_notional(
        &self,
        notional: JsValue,
        currency: JsValue,
        asset_class: JsValue,
        maturity_years: JsValue,
        as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let currency = js_currency(&currency, "currency")?;
        let notional = js_money(&notional, "notional", currency)?;
        let asset_class = schedule_asset_class(&js_string(&asset_class, "assetClass")?)?;
        let maturity_years = js_f64(&maturity_years, "maturityYears")?;
        let as_of = js_date(&as_of, "asOf")?;
        to_js_value(
            &self
                .inner
                .calculate_for_notional(notional, asset_class, maturity_years, as_of)
                .map_err(to_js_err)?,
        )
    }

    /// Schedule IM for a netting set with the net-to-gross ratio reduction.
    ///
    /// Applies the BCBS-IOSCO reduction `0.4 + 0.6 * NGR` to the sum of
    /// trade-specific gross IM across the netting set.
    /// @param positions - Array of `[signedMtm, grossNotional, assetClass, maturityYears]` tuples in the common reporting currency: MTM signs determine NGR; each absolute notional receives its own class and maturity rate.
    /// @param currency - ISO-4217 reporting currency of every MTM, notional and of the result.
    /// @param as_of - ISO-8601 calculation date stamped on the result.
    /// @returns The NGR-adjusted `ImResult` as a plain object, or `undefined` for an empty position list or zero gross notional.
    ///
    /// # Errors
    ///
    /// Throws if `positions` is not an array of `[number, number, string,
    /// number]` tuples, or the currency, an asset class, an amount or the date
    /// is invalid.
    #[wasm_bindgen(js_name = calculateNettingSetWithNgr)]
    pub fn calculate_netting_set_with_ngr(
        &self,
        positions: JsValue,
        currency: JsValue,
        as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let currency = js_currency(&currency, "currency")?;
        let as_of = js_date(&as_of, "asOf")?;
        let positions = from_js_json::<Vec<(f64, f64, String, f64)>>(&positions, "positions")?
            .into_iter()
            .map(|(mtm, notional, asset_class, maturity)| {
                Ok((
                    Money::new(mtm, currency).map_err(to_js_err)?,
                    Money::new(notional, currency).map_err(to_js_err)?,
                    schedule_asset_class(&asset_class)?,
                    maturity,
                ))
            })
            .collect::<Result<Vec<_>, JsValue>>()?;
        match self
            .inner
            .calculate_netting_set_with_ngr(&positions, as_of)
            .map_err(to_js_err)?
        {
            Some(result) => to_js_value(&result),
            None => Ok(JsValue::UNDEFINED),
        }
    }
}

// ---------------------------------------------------------------------------
// HaircutImCalculator
// ---------------------------------------------------------------------------

/// Haircut-based initial margin calculator.
///
/// Applies eligible-collateral haircuts and optional FX add-ons to explicit
/// collateral values. This path is intended for repo and securities-financing
/// collateral IM rather than SIMM sensitivities.
///
/// @example
/// ```javascript
/// import init, { margin } from "finstack-quant-wasm";
/// await init();
/// const calc = margin.HaircutImCalculator.bcbsStandard();
/// calc.haircutFor("cash"); // 0
/// calc.calculateForCollateral(1e7, "USD", "cash", true, "2025-01-15").amount.amount;
/// ```
#[wasm_bindgen(js_name = HaircutImCalculator)]
#[derive(Clone)]
pub struct JsHaircutImCalculator {
    inner: fm::HaircutImCalculator,
}

#[wasm_bindgen(js_class = HaircutImCalculator)]
impl JsHaircutImCalculator {
    /// Calculator on the BCBS-IOSCO standard eligible-collateral schedule.
    /// @returns A `HaircutImCalculator` handle.
    ///
    /// # Errors
    ///
    /// Throws if the embedded margin registry cannot be loaded.
    #[wasm_bindgen(js_name = bcbsStandard)]
    pub fn bcbs_standard() -> Result<JsHaircutImCalculator, JsValue> {
        Ok(Self {
            inner: fm::HaircutImCalculator::bcbs_standard().map_err(to_js_err)?,
        })
    }

    /// Calculator on the US Treasury repo collateral schedule.
    /// @returns A `HaircutImCalculator` handle.
    ///
    /// # Errors
    ///
    /// Throws if the embedded margin registry cannot be loaded.
    #[wasm_bindgen(js_name = usTreasuries)]
    pub fn us_treasuries() -> Result<JsHaircutImCalculator, JsValue> {
        Ok(Self {
            inner: fm::HaircutImCalculator::us_treasuries().map_err(to_js_err)?,
        })
    }

    /// Calculator on a caller-supplied eligible-collateral schedule.
    /// @param schedule - `EligibleCollateralSchedule` (object or JSON) whose entries supply the haircuts.
    /// @returns A `HaircutImCalculator` handle.
    ///
    /// # Errors
    ///
    /// Throws if `schedule` is malformed or has unknown fields.
    #[wasm_bindgen(js_name = fromSchedule)]
    pub fn from_schedule(schedule: JsValue) -> Result<JsHaircutImCalculator, JsValue> {
        let schedule: fm::EligibleCollateralSchedule = from_js_json(&schedule, "schedule")?;
        Ok(Self {
            inner: fm::HaircutImCalculator::new(schedule),
        })
    }

    /// Copy with a default collateral asset class.
    /// @param asset_class - `CollateralAssetClass` wire label such as `"government_bonds"`.
    /// @returns A new `HaircutImCalculator` handle.
    ///
    /// # Errors
    ///
    /// Throws if the label is not a collateral asset class.
    #[wasm_bindgen(js_name = withDefaultAssetClass)]
    pub fn with_default_asset_class(
        &self,
        asset_class: JsValue,
    ) -> Result<JsHaircutImCalculator, JsValue> {
        let asset_class = collateral_asset_class(&asset_class, "assetClass")?;
        Ok(Self {
            inner: self.inner.clone().with_default_asset_class(asset_class),
        })
    }

    /// Copy with a posted-collateral currency, used to detect FX mismatch.
    /// @param currency - ISO-4217 currency of the posted collateral.
    /// @returns A new `HaircutImCalculator` handle.
    ///
    /// # Errors
    ///
    /// Throws if `currency` is not a known currency code.
    #[wasm_bindgen(js_name = withPostedCollateralCurrency)]
    pub fn with_posted_collateral_currency(
        &self,
        currency: JsValue,
    ) -> Result<JsHaircutImCalculator, JsValue> {
        let currency = js_currency(&currency, "currency")?;
        Ok(Self {
            inner: self.inner.clone().with_posted_collateral_currency(currency),
        })
    }

    /// Copy configured to select an eligible entry by collateral maturity and rating.
    /// @param remaining_years - Residual collateral maturity in years; finite and non-negative.
    /// @param rating - Credit rating such as `"AAA"` or `"A-"`; required when the schedule imposes a minimum rating (no rating is inferred).
    /// @returns A new `HaircutImCalculator` handle using these terms for eligibility, haircut and FX add-on lookup.
    ///
    /// # Errors
    ///
    /// Throws if the maturity is negative or non-finite, or the rating is unknown.
    #[wasm_bindgen(js_name = withCollateralTerms)]
    pub fn with_collateral_terms(
        &self,
        remaining_years: JsValue,
        rating: Option<JsValue>,
    ) -> Result<JsHaircutImCalculator, JsValue> {
        let remaining_years = js_f64(&remaining_years, "remainingYears")?;
        let rating = js_opt_string(rating.as_ref(), "rating")?
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(to_js_err)?;
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_collateral_terms(remaining_years, rating)
                .map_err(to_js_err)?,
        })
    }

    /// Eligible-collateral schedule supplying the haircuts, as a plain `EligibleCollateralSchedule` object.
    ///
    /// # Errors
    ///
    /// Throws if the schedule cannot be converted to a JavaScript value.
    #[wasm_bindgen(getter, js_name = eligibleCollateral)]
    pub fn eligible_collateral(&self) -> Result<JsValue, JsValue> {
        to_js_value(self.inner.eligible_collateral())
    }

    /// Default `CollateralAssetClass` wire label.
    #[wasm_bindgen(getter, js_name = defaultAssetClass)]
    pub fn default_asset_class(&self) -> String {
        self.inner.default_asset_class().to_string()
    }

    /// ISO-4217 posted-collateral currency, or `undefined` when none is configured.
    #[wasm_bindgen(getter, js_name = postedCollateralCurrency)]
    pub fn posted_collateral_currency(&self) -> Option<String> {
        self.inner
            .posted_collateral_currency()
            .map(|currency| currency.to_string())
    }

    /// Margin period of risk in business days stamped on every result (`HAIRCUT_MPOR_DAYS`).
    #[wasm_bindgen(getter, js_name = mporDays)]
    pub fn mpor_days(&self) -> u32 {
        self.inner.mpor_days()
    }

    /// Look up the haircut for a collateral asset class.
    /// @param asset_class - `CollateralAssetClass` wire label such as `"government_bonds"`.
    /// @returns The base haircut as a decimal fraction, without the FX add-on.
    ///
    /// # Errors
    ///
    /// Throws if the label is unknown or no schedule or standard haircut
    /// exists for the asset class.
    #[wasm_bindgen(js_name = haircutFor)]
    pub fn haircut_for(&self, asset_class: JsValue) -> Result<f64, JsValue> {
        let asset_class = collateral_asset_class(&asset_class, "assetClass")?;
        self.inner.haircut_for(&asset_class).map_err(to_js_err)
    }

    /// Haircut IM for an explicit collateral value and asset class.
    /// @param collateral_value - Collateral market value in major units of `currency`; finite and non-negative.
    /// @param currency - ISO-4217 currency of the collateral value and of the result.
    /// @param asset_class - `CollateralAssetClass` wire label used for the haircut lookup and as the breakdown key.
    /// @param currency_mismatch - Whether to add the asset class's FX mismatch add-on.
    /// @param as_of - ISO-8601 calculation date stamped on the result.
    /// @returns The `ImResult` as a plain object; its MPOR is the repo haircut horizon (`HAIRCUT_MPOR_DAYS`, 2 business days).
    ///
    /// # Errors
    ///
    /// Throws if the currency, amount or date is invalid, or the haircut or
    /// FX add-on cannot be resolved.
    #[wasm_bindgen(js_name = calculateForCollateral)]
    pub fn calculate_for_collateral(
        &self,
        collateral_value: JsValue,
        currency: JsValue,
        asset_class: JsValue,
        currency_mismatch: JsValue,
        as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let currency = js_currency(&currency, "currency")?;
        let collateral_value = js_money(&collateral_value, "collateralValue", currency)?;
        let asset_class = collateral_asset_class(&asset_class, "assetClass")?;
        let currency_mismatch = js_bool(&currency_mismatch, "currencyMismatch")?;
        let as_of = js_date(&as_of, "asOf")?;
        to_js_value(
            &self
                .inner
                .calculate_for_collateral(collateral_value, &asset_class, currency_mismatch, as_of)
                .map_err(to_js_err)?,
        )
    }
}
