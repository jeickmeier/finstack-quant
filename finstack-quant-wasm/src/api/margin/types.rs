//! Margin data types: enum labels, netting-set ids, CSA specifications and
//! eligible-collateral schedules.
//!
//! These Rust types cross the boundary as plain JSON values (the generated
//! `ImMethodology`, `MarginTenor`, `CsaSpec`, … TypeScript types). The
//! functions here are the twins of the Python class factories and methods
//! that validate, default or compute in Rust: each takes and returns the
//! plain value. A payload-free enum variant (`"simm"`, `"daily"`, `"cash"`)
//! has no function; its twin is the TypeScript string literal.

use super::parse::{base_money, parse_label};
use crate::api::core::config::JsFinstackConfig;
use crate::utils::input::js_currency;
use crate::utils::input::{from_js_json, js_f64, js_opt_bool, js_opt_f64, js_string, js_uint};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_margin as fm;
use wasm_bindgen::prelude::*;
/// BCBS-IOSCO standard haircut for a collateral asset class.
/// @param assetClass - `CollateralAssetClass` wire label such as `"government_bonds"`.
/// @returns The haircut as a decimal fraction of collateral value (0.02 is 2%).
///
/// # Errors
///
/// Throws a validation error for an unknown label, or if the embedded margin
/// registry cannot be loaded.
#[wasm_bindgen(js_name = collateralAssetClassStandardHaircut)]
pub fn collateral_asset_class_standard_haircut(
    #[wasm_bindgen(js_name = "assetClass", unchecked_param_type = "CollateralAssetClass")]
    asset_class: JsValue,
) -> Result<f64, JsValue> {
    parse_label::<fm::types::CollateralAssetClass>(&asset_class, "assetClass")?
        .standard_haircut()
        .map_err(to_js_err)
}

/// Additional haircut applied when collateral and exposure currencies differ.
/// @param assetClass - `CollateralAssetClass` wire label such as `"government_bonds"`.
/// @returns The FX add-on as a decimal fraction of collateral value (0.08 is 8%).
///
/// # Errors
///
/// Throws a validation error for an unknown label, or if the embedded margin
/// registry cannot be loaded.
#[wasm_bindgen(js_name = collateralAssetClassFxAddon)]
pub fn collateral_asset_class_fx_addon(
    #[wasm_bindgen(js_name = "assetClass", unchecked_param_type = "CollateralAssetClass")]
    asset_class: JsValue,
) -> Result<f64, JsValue> {
    parse_label::<fm::types::CollateralAssetClass>(&asset_class, "assetClass")?
        .fx_addon()
        .map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// CsaSpec
// ---------------------------------------------------------------------------

/// Deserialize and validate a CSA specification argument.
pub(super) fn parse_csa(csa: &JsValue, label: &str) -> Result<fm::types::CsaSpec, JsValue> {
    let csa: fm::types::CsaSpec = from_js_json(csa, label)?;
    csa.validate().map_err(to_js_err)?;
    Ok(csa)
}

/// Standard USD regulatory CSA specification (BCBS-IOSCO terms, SIMM initial margin).
/// @returns The canonical `CsaSpec` as a plain object.
///
/// # Errors
///
/// Throws if the embedded margin registry cannot be loaded.
#[wasm_bindgen(js_name = csaSpecUsdRegulatory, unchecked_return_type = "CsaSpec")]
pub fn csa_spec_usd_regulatory() -> Result<JsValue, JsValue> {
    to_js_value(&fm::types::CsaSpec::usd_regulatory().map_err(to_js_err)?)
}

/// Standard EUR regulatory CSA specification (BCBS-IOSCO terms, SIMM initial margin).
/// @returns The canonical `CsaSpec` as a plain object.
///
/// # Errors
///
/// Throws if the embedded margin registry cannot be loaded.
#[wasm_bindgen(js_name = csaSpecEurRegulatory, unchecked_return_type = "CsaSpec")]
pub fn csa_spec_eur_regulatory() -> Result<JsValue, JsValue> {
    to_js_value(&fm::types::CsaSpec::eur_regulatory().map_err(to_js_err)?)
}

/// Builder for explicit variation-margin, collateral eligibility and settlement terms.
///
/// @example
/// ```typescript
/// import { margin } from "finstack-quant-wasm";
/// const original = margin.csaSpecUsdRegulatory();
/// const agreement = margin.csaSpecToBuilder(original).calendarId("usny").build();
/// console.log(agreement.base_currency);
/// ```
#[wasm_bindgen(js_name = CsaSpecBuilder)]
pub struct JsCsaSpecBuilder {
    inner: fm::types::CsaSpecBuilder,
}

/// Start authoring an agreement with an explicit currency and market reference.
/// @param id - Stable agreement identifier for trade and collateral account joins.
/// @param baseCurrency - ISO currency for transfers, thresholds and collateral amounts.
/// @param collateralCurveId - Exact curve identifier used to discount collateral cash.
/// @returns Builder requiring the agreement's terms, timing and calendar.
/// @throws FinstackError for an unknown currency or incompatible input type.
#[wasm_bindgen(js_name = csaSpecBuilder)]
pub fn csa_spec_builder(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(js_name = "baseCurrency", unchecked_param_type = "string")]
    base_currency: JsValue,
    #[wasm_bindgen(js_name = "collateralCurveId", unchecked_param_type = "string")]
    collateral_curve_id: JsValue,
) -> Result<JsCsaSpecBuilder, JsValue> {
    Ok(JsCsaSpecBuilder {
        inner: fm::types::CsaSpec::builder(
            js_string(&id, "id")?,
            js_currency(&base_currency, "baseCurrency")?,
            js_string(&collateral_curve_id, "collateralCurveId")?,
        ),
    })
}

/// Copy a complete CSA value into an independent builder.
/// @param value - The CSA terms to edit.
/// @returns Builder carrying the original CSA's agreement terms.
/// @throws FinstackError when the provided value has invalid contract fields.
#[wasm_bindgen(js_name = csaSpecToBuilder)]
pub fn csa_spec_to_builder(
    #[wasm_bindgen(unchecked_param_type = "CsaSpec")] value: JsValue,
) -> Result<JsCsaSpecBuilder, JsValue> {
    let spec: fm::types::CsaSpec = from_js_json(&value, "csa")?;
    spec.validate().map_err(to_js_err)?;
    Ok(JsCsaSpecBuilder {
        inner: spec.to_builder(),
    })
}

#[wasm_bindgen(js_class = CsaSpecBuilder)]
impl JsCsaSpecBuilder {
    /// Validate and assemble the elected terms.
    /// @returns Plain, complete CSA agreement which can be used by margin engines.
    /// @throws FinstackError if required terms are absent or the agreement fails validation.
    #[wasm_bindgen(unchecked_return_type = "CsaSpec")]
    pub fn build(self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.build().map_err(to_js_err)?)
    }

    /// Elect SIMM initial-margin terms in the agreement currency.
    /// @returns Builder with the SIMM terms selected from the embedded registry.
    /// @throws FinstackError if registry terms cannot be resolved.
    #[wasm_bindgen(js_name = imSimm)]
    pub fn im_simm(self) -> Result<JsCsaSpecBuilder, JsValue> {
        Ok(Self {
            inner: self.inner.im_simm().map_err(to_js_err)?,
        })
    }

    /// Set the uncollateralized allowance and minimum variation-margin transfer.
    /// @param threshold - Exact money in the agreement currency.
    /// @param mta - Exact minimum transfer in the same currency.
    /// @returns Builder with the elected terms.
    #[wasm_bindgen(js_name = vmThreshold)]
    pub fn vm_threshold(
        self,
        threshold: &crate::api::core::money::JsMoney,
        mta: &crate::api::core::money::JsMoney,
    ) -> JsCsaSpecBuilder {
        Self {
            inner: self.inner.vm_threshold(threshold.inner, mta.inner),
        }
    }

    /// Set the agreement's calendar id.
    /// @param value - Registered business-day calendar for the agreement.
    /// @returns Builder carrying the supplied agreement term.
    /// @throws FinstackError when the value does not match the required type.
    #[wasm_bindgen(js_name = calendarId)]
    pub fn calendar_id(
        self,
        #[wasm_bindgen(unchecked_param_type = "string")] value: JsValue,
    ) -> Result<JsCsaSpecBuilder, JsValue> {
        Ok(Self {
            inner: self.inner.calendar_id(js_string(&value, "value")?),
        })
    }

    /// Set the agreement's vm params.
    /// @param value - Canonical variation-margin terms and settlement frequency.
    /// @returns Builder carrying the supplied agreement term.
    /// @throws FinstackError when the value does not match the required type.
    #[wasm_bindgen(js_name = vmParams)]
    pub fn vm_params(
        self,
        #[wasm_bindgen(unchecked_param_type = "generated.margin.VmParameters")] value: JsValue,
    ) -> Result<JsCsaSpecBuilder, JsValue> {
        Ok(Self {
            inner: self.inner.vm_params(from_js_json(&value, "value")?),
        })
    }

    /// Set the agreement's im params.
    /// @param value - Initial-margin method, MPOR, minimum transfer, threshold and segregation. Pass null to clear.
    /// @returns Builder carrying the supplied agreement term.
    /// @throws FinstackError when the value does not match the required type.
    #[wasm_bindgen(js_name = imParams)]
    pub fn im_params(
        self,
        #[wasm_bindgen(unchecked_param_type = "generated.margin.ImParameters | null")]
        value: JsValue,
    ) -> Result<JsCsaSpecBuilder, JsValue> {
        let params = if value.is_null() {
            None
        } else {
            Some(from_js_json(&value, "value")?)
        };
        Ok(Self {
            inner: self.inner.im_params(params),
        })
    }

    /// Set the agreement's eligible collateral.
    /// @param value - Contractual list of eligible instruments and valuation haircuts.
    /// @returns Builder carrying the supplied agreement term.
    /// @throws FinstackError when the value does not match the required type.
    #[wasm_bindgen(js_name = eligibleCollateral)]
    pub fn eligible_collateral(
        self,
        #[wasm_bindgen(unchecked_param_type = "generated.margin.EligibleCollateralSchedule")] value: JsValue,
    ) -> Result<JsCsaSpecBuilder, JsValue> {
        Ok(Self {
            inner: self
                .inner
                .eligible_collateral(from_js_json(&value, "value")?),
        })
    }

    /// Set the agreement's call timing.
    /// @param value - Notification deadline, delivery grace and dispute timing.
    /// @returns Builder carrying the supplied agreement term.
    /// @throws FinstackError when the value does not match the required type.
    #[wasm_bindgen(js_name = callTiming)]
    pub fn call_timing(
        self,
        #[wasm_bindgen(unchecked_param_type = "generated.margin.MarginCallTiming")] value: JsValue,
    ) -> Result<JsCsaSpecBuilder, JsValue> {
        Ok(Self {
            inner: self.inner.call_timing(from_js_json(&value, "value")?),
        })
    }
}

/// Regulatory CSA specification for any supported base currency.
/// @param currency - ISO-4217 base currency of the agreement; every CSA amount is in it.
/// @param id - Identifier stamped on the specification; must be non-empty (an empty or whitespace-only id is rejected).
/// @param collateralCurve - Curve id used to discount and accrue interest on collateral, for example `"USD-OIS"`.
/// @returns The `CsaSpec` as a plain object.
///
/// # Errors
///
/// Throws a `validation` error for an empty or whitespace-only id or an
/// empty collateral curve; throws for an unknown currency, or if the embedded
/// margin registry has no regulatory terms for it or cannot be loaded.
#[wasm_bindgen(js_name = csaSpecRegulatory, unchecked_return_type = "CsaSpec")]
pub fn csa_spec_regulatory(
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(js_name = "collateralCurve", unchecked_param_type = "string")] collateral_curve: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = js_currency(&currency, "currency")?;
    let id = js_string(&id, "id")?;
    let collateral_curve = js_string(&collateral_curve, "collateralCurve")?;
    let csa = fm::types::CsaSpec::regulatory_for_currency(currency, &id, &collateral_curve)
        .map_err(to_js_err)?;
    to_js_value(&csa)
}

/// Regulatory CSA specification built from a config's margin-registry overlay.
///
/// Same terms as `csaSpecRegulatory`, except that a `"margin.registry.v1"`
/// extension in `config` takes precedence over the embedded margin registry.
/// @param config - `FinstackConfig` handle whose `"margin.registry.v1"` extension (if any) overlays the embedded margin registry.
/// @param currency - ISO-4217 base currency of the agreement; every CSA amount is in it.
/// @param id - Identifier stamped on the specification; must be non-empty.
/// @param collateralCurve - Curve id used to discount and accrue interest on collateral, for example `"USD-OIS"`.
/// @returns The `CsaSpec` as a plain object.
///
/// # Errors
///
/// Throws for an unknown currency or empty id, or if the overlay is
/// malformed or fails registry validation.
#[wasm_bindgen(js_name = csaSpecRegulatoryFromConfig, unchecked_return_type = "CsaSpec")]
pub fn csa_spec_regulatory_from_config(
    config: &JsFinstackConfig,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(js_name = "collateralCurve", unchecked_param_type = "string")] collateral_curve: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = js_currency(&currency, "currency")?;
    let id = js_string(&id, "id")?;
    let collateral_curve = js_string(&collateral_curve, "collateralCurve")?;
    let csa =
        fm::types::CsaSpec::regulatory_from_config(&config.inner, currency, &id, &collateral_curve)
            .map_err(to_js_err)?;
    to_js_value(&csa)
}

/// Copy of a CSA with new variation margin threshold terms.
/// @param csa - `CsaSpec` (object or JSON) to copy.
/// @param threshold - VM threshold in major units of the CSA base currency; exposure below it is uncollateralized.
/// @param mta - Minimum transfer amount in major units of the CSA base currency.
/// @param rounding - Optional rounding increment in major units of the base currency; omitted keeps the current one.
/// @param independentAmount - Optional independent amount in major units of the base currency; omitted keeps the current one.
/// @returns The updated `CsaSpec` as a plain object.
///
/// # Errors
///
/// Throws if `csa` is malformed or invalid, or an amount is non-finite,
/// negative, or otherwise rejected by the CSA validation.
#[wasm_bindgen(js_name = csaSpecWithVmThreshold, unchecked_return_type = "CsaSpec")]
pub fn csa_spec_with_vm_threshold(
    #[wasm_bindgen(unchecked_param_type = "CsaSpec | string")] csa: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] mta: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "number | null")] rounding: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "independentAmount",
        unchecked_optional_param_type = "number | null"
    )]
    independent_amount: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let csa = parse_csa(&csa, "csa")?;
    let threshold = base_money(&csa, js_f64(&threshold, "threshold")?)?;
    let mta = base_money(&csa, js_f64(&mta, "mta")?)?;
    let rounding = js_opt_f64(rounding.as_ref(), "rounding")?
        .map(|amount| base_money(&csa, amount))
        .transpose()?;
    let independent_amount = js_opt_f64(independent_amount.as_ref(), "independentAmount")?
        .map(|amount| base_money(&csa, amount))
        .transpose()?;
    let csa = csa
        .with_vm_threshold(threshold, mta, rounding, independent_amount)
        .map_err(to_js_err)?;
    to_js_value(&csa)
}

/// Copy of a CSA with initial margin terms.
/// @param csa - `CsaSpec` (object or JSON) to copy.
/// @param methodology - `ImMethodology` wire label such as `"simm"` or `"schedule"`.
/// @param mporDays - Margin period of risk in business days.
/// @param threshold - IM threshold in major units of the CSA base currency.
/// @param mta - IM minimum transfer amount in major units of the CSA base currency.
/// @param segregated - Whether posted IM is segregated and unavailable to meet VM; omitted keeps the margin-registry default for `methodology` (`true` for SIMM and schedule IM, `false` for clearing-house and haircut IM).
/// @returns The updated `CsaSpec` as a plain object.
///
/// # Errors
///
/// Throws if `csa` is malformed or invalid, the methodology label is
/// unknown, `mpor_days` is not a non-negative integer, or an amount is
/// rejected by the CSA validation.
#[wasm_bindgen(js_name = csaSpecWithIm, unchecked_return_type = "CsaSpec")]
pub fn csa_spec_with_im(
    #[wasm_bindgen(unchecked_param_type = "CsaSpec | string")] csa: JsValue,
    #[wasm_bindgen(unchecked_param_type = "ImMethodology")] methodology: JsValue,
    #[wasm_bindgen(js_name = "mporDays", unchecked_param_type = "number")] mpor_days: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] mta: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "boolean | null")] segregated: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let csa = parse_csa(&csa, "csa")?;
    let methodology = parse_label::<fm::types::ImMethodology>(&methodology, "methodology")?;
    let mpor_days: u32 = js_uint(&mpor_days, "mporDays")?;
    let threshold = base_money(&csa, js_f64(&threshold, "threshold")?)?;
    let mta = base_money(&csa, js_f64(&mta, "mta")?)?;
    let segregated = js_opt_bool(segregated.as_ref(), "segregated")?;
    let csa = csa
        .with_im(methodology, mpor_days, threshold, mta, segregated)
        .map_err(to_js_err)?;
    to_js_value(&csa)
}

/// Apply a CSA's initial margin terms to one gross IM figure.
///
/// Turns gross model IM into the collateral target after the CSA threshold
/// and the signed, MTA-filtered transfer against the current balance.
/// @param csa - `CsaSpec` (object or JSON) carrying initial margin terms.
/// @param grossInitialMargin - Gross model IM before contractual terms, in major units of the CSA base currency.
/// @param currentCollateral - Existing balance of this one-way IM account, in major units of the CSA base currency.
/// @returns The `ImCollateralResult` as a plain object (Money amounts as `{amount, currency}`).
///
/// # Errors
///
/// Throws if `csa` is malformed, invalid or has no initial margin terms, or an
/// amount is non-finite or negative.
#[wasm_bindgen(js_name = csaSpecApplyImTerms, unchecked_return_type = "ImCollateralResult")]
pub fn csa_spec_apply_im_terms(
    #[wasm_bindgen(unchecked_param_type = "CsaSpec | string")] csa: JsValue,
    #[wasm_bindgen(js_name = "grossInitialMargin", unchecked_param_type = "number")]
    gross_initial_margin: JsValue,
    #[wasm_bindgen(js_name = "currentCollateral", unchecked_param_type = "number")]
    current_collateral: JsValue,
) -> Result<JsValue, JsValue> {
    let csa = parse_csa(&csa, "csa")?;
    let gross = base_money(&csa, js_f64(&gross_initial_margin, "grossInitialMargin")?)?;
    let current = base_money(&csa, js_f64(&current_collateral, "currentCollateral")?)?;
    to_js_value(&csa.apply_im_terms(gross, current).map_err(to_js_err)?)
}

/// Validate a CSA specification: currencies, monetary bounds and calendar lookup.
/// @param csa - `CsaSpec` (object or JSON) to check.
///
/// # Errors
///
/// Throws if `csa` is malformed, has unknown fields, mixes currencies, has a
/// negative or non-finite amount, or names an unregistered calendar.
#[wasm_bindgen(js_name = csaSpecValidate)]
pub fn csa_spec_validate(
    #[wasm_bindgen(unchecked_param_type = "CsaSpec | string")] csa: JsValue,
) -> Result<(), JsValue> {
    parse_csa(&csa, "csa").map(|_| ())
}

// ---------------------------------------------------------------------------
// EligibleCollateralSchedule
// ---------------------------------------------------------------------------

/// Eligible-collateral schedule that accepts cash only.
/// @returns The `EligibleCollateralSchedule` as a plain object.
///
/// # Errors
///
/// Throws if the embedded margin registry cannot be loaded.
#[wasm_bindgen(js_name = eligibleCollateralScheduleCashOnly, unchecked_return_type = "EligibleCollateralSchedule")]
pub fn eligible_collateral_schedule_cash_only() -> Result<JsValue, JsValue> {
    to_js_value(&fm::types::EligibleCollateralSchedule::cash_only().map_err(to_js_err)?)
}

/// BCBS-IOSCO standard eligible-collateral schedule with regulatory haircuts.
/// @returns The `EligibleCollateralSchedule` as a plain object.
///
/// # Errors
///
/// Throws if the embedded margin registry cannot be loaded.
#[wasm_bindgen(js_name = eligibleCollateralScheduleBcbsStandard, unchecked_return_type = "EligibleCollateralSchedule")]
pub fn eligible_collateral_schedule_bcbs_standard() -> Result<JsValue, JsValue> {
    to_js_value(&fm::types::EligibleCollateralSchedule::bcbs_standard().map_err(to_js_err)?)
}

/// Eligible-collateral schedule for US Treasury repo collateral.
/// @returns The `EligibleCollateralSchedule` as a plain object.
///
/// # Errors
///
/// Throws if the embedded margin registry cannot be loaded.
#[wasm_bindgen(js_name = eligibleCollateralScheduleUsTreasuries, unchecked_return_type = "EligibleCollateralSchedule")]
pub fn eligible_collateral_schedule_us_treasuries() -> Result<JsValue, JsValue> {
    to_js_value(&fm::types::EligibleCollateralSchedule::us_treasuries().map_err(to_js_err)?)
}

/// Named eligible-collateral schedule from a config's margin-registry overlay.
/// @param config - `FinstackConfig` handle whose `"margin.registry.v1"` extension (if any) overlays the embedded margin registry; configured schedules take precedence.
/// @param scheduleId - Registry id of the schedule, e.g. `"bcbs_standard"`, `"cash_only"`, `"us_treasuries"` or an id the overlay adds.
/// @returns A copy of the merged registry's `EligibleCollateralSchedule` as a plain object.
///
/// # Errors
///
/// Throws if the overlay is malformed or `schedule_id` names no schedule.
#[wasm_bindgen(js_name = eligibleCollateralScheduleFromFinstackConfig, unchecked_return_type = "EligibleCollateralSchedule")]
pub fn eligible_collateral_schedule_from_finstack_config(
    config: &JsFinstackConfig,
    #[wasm_bindgen(js_name = "scheduleId", unchecked_param_type = "string")] schedule_id: JsValue,
) -> Result<JsValue, JsValue> {
    let schedule_id = js_string(&schedule_id, "scheduleId")?;
    let schedule =
        fm::types::EligibleCollateralSchedule::from_finstack_config(&config.inner, &schedule_id)
            .map_err(to_js_err)?;
    to_js_value(&schedule)
}

/// Whether a schedule accepts an asset class, by entry or through its default haircut.
/// @param schedule - `EligibleCollateralSchedule` (object or JSON).
/// @param assetClass - `CollateralAssetClass` wire label such as `"government_bonds"`.
/// @returns `true` when the asset class is listed or the schedule has a default haircut.
///
/// # Errors
///
/// Throws if `schedule` is malformed or the asset class label is unknown.
#[wasm_bindgen(js_name = eligibleCollateralScheduleIsEligible)]
pub fn eligible_collateral_schedule_is_eligible(
    #[wasm_bindgen(unchecked_param_type = "EligibleCollateralSchedule | string")] schedule: JsValue,
    #[wasm_bindgen(js_name = "assetClass", unchecked_param_type = "CollateralAssetClass")]
    asset_class: JsValue,
) -> Result<bool, JsValue> {
    let schedule: fm::types::EligibleCollateralSchedule = from_js_json(&schedule, "schedule")?;
    let asset_class = parse_label::<fm::types::CollateralAssetClass>(&asset_class, "assetClass")?;
    Ok(schedule.is_eligible(&asset_class))
}

/// Haircut a schedule applies to an asset class.
/// @param schedule - `EligibleCollateralSchedule` (object or JSON).
/// @param assetClass - `CollateralAssetClass` wire label such as `"government_bonds"`.
/// @returns The haircut as a decimal fraction, or `undefined` when the asset class is not eligible.
///
/// # Errors
///
/// Throws if `schedule` is malformed or the asset class label is unknown.
#[wasm_bindgen(js_name = eligibleCollateralScheduleHaircutFor)]
pub fn eligible_collateral_schedule_haircut_for(
    #[wasm_bindgen(unchecked_param_type = "EligibleCollateralSchedule | string")] schedule: JsValue,
    #[wasm_bindgen(js_name = "assetClass", unchecked_param_type = "CollateralAssetClass")]
    asset_class: JsValue,
) -> Result<Option<f64>, JsValue> {
    let schedule: fm::types::EligibleCollateralSchedule = from_js_json(&schedule, "schedule")?;
    let asset_class = parse_label::<fm::types::CollateralAssetClass>(&asset_class, "assetClass")?;
    Ok(schedule.haircut_for(&asset_class))
}

/// Haircut a schedule applies to an asset class at a given remaining maturity.
/// @param schedule - `EligibleCollateralSchedule` (object or JSON).
/// @param assetClass - `CollateralAssetClass` wire label such as `"government_bonds"`.
/// @param remainingYears - Remaining maturity of the collateral in years.
/// @returns The haircut as a decimal fraction, or `undefined` when no entry admits that maturity.
///
/// # Errors
///
/// Throws if `schedule` is malformed or the asset class label is unknown.
#[wasm_bindgen(js_name = eligibleCollateralScheduleHaircutForMaturity)]
pub fn eligible_collateral_schedule_haircut_for_maturity(
    #[wasm_bindgen(unchecked_param_type = "EligibleCollateralSchedule | string")] schedule: JsValue,
    #[wasm_bindgen(js_name = "assetClass", unchecked_param_type = "CollateralAssetClass")]
    asset_class: JsValue,
    #[wasm_bindgen(js_name = "remainingYears", unchecked_param_type = "number")]
    remaining_years: JsValue,
) -> Result<Option<f64>, JsValue> {
    let schedule: fm::types::EligibleCollateralSchedule = from_js_json(&schedule, "schedule")?;
    let asset_class = parse_label::<fm::types::CollateralAssetClass>(&asset_class, "assetClass")?;
    let remaining_years = js_f64(&remaining_years, "remainingYears")?;
    Ok(schedule.haircut_for_maturity(&asset_class, remaining_years))
}

/// Check a proposed collateral portfolio against a schedule's concentration limits.
/// @param schedule - `EligibleCollateralSchedule` (object or JSON).
/// @param allocations - Array of `[assetClass, amount]` pairs: a `CollateralAssetClass` wire label and the proposed amount in one common currency.
/// @returns One `ConcentrationBreach` (`asset_class`, `fraction`, `limit`, `excess`) per asset class over its limit; empty when the total is not positive.
///
/// # Errors
///
/// Throws if `schedule` is malformed, `allocations` is not an array of
/// `[string, number]` pairs, or an asset class label is unknown.
#[wasm_bindgen(js_name = eligibleCollateralScheduleCheckConcentrationLimits, unchecked_return_type = "ConcentrationBreach[]")]
pub fn eligible_collateral_schedule_check_concentration_limits(
    #[wasm_bindgen(unchecked_param_type = "EligibleCollateralSchedule | string")] schedule: JsValue,
    #[wasm_bindgen(
        unchecked_param_type = "readonly (readonly [CollateralAssetClass, number])[] | string"
    )]
    allocations: JsValue,
) -> Result<JsValue, JsValue> {
    let schedule: fm::types::EligibleCollateralSchedule = from_js_json(&schedule, "schedule")?;
    let allocations = from_js_json::<Vec<(String, f64)>>(&allocations, "allocations")?
        .into_iter()
        .map(|(asset_class, amount)| {
            asset_class
                .parse::<fm::types::CollateralAssetClass>()
                .map(|asset_class| (asset_class, amount))
                .map_err(to_js_err)
        })
        .collect::<Result<Vec<_>, JsValue>>()?;
    to_js_value(&schedule.check_concentration_limits(&allocations))
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Margin constants needed to interpret inputs and results.
///
/// The Rust `MarginConstants::current()` value, published as
/// `margin.constants()` (the twin of Python `margin.CONSTANTS`): year basis, duration factor, one basis point, the SIMM
/// tenor bucket boundaries and labels, the BCBS-IOSCO schedule id, the
/// haircut margin period of risk and the SIMM commodity bucket count.
/// @returns The `MarginConstants` value as a plain object.
///
/// # Errors
///
/// Throws if the value cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = marginConstants, unchecked_return_type = "MarginConstants")]
pub fn margin_constants() -> Result<JsValue, JsValue> {
    to_js_value(&fm::constants::MarginConstants::current())
}

/// Set ISDA SIMM on a copy of the bilateral agreement.
/// @param csa - Authored CSA; collateral, variation margin and timing terms are preserved.
/// @returns OTC margin specification with the selected methodology if IM is elected.
/// @throws FinstackError if the CSA document cannot be decoded.
#[wasm_bindgen(js_name = otcMarginSpecBilateralSimm, unchecked_return_type = "generated.margin.OtcMarginSpec")]
pub fn otc_margin_spec_bilateral_simm(
    #[wasm_bindgen(unchecked_param_type = "generated.margin.CsaSpec")] csa: JsValue,
) -> Result<JsValue, JsValue> {
    let csa = from_js_json(&csa, "csa")?;
    to_js_value(&fm::types::OtcMarginSpec::bilateral_simm(csa))
}

/// Set schedule-based initial margin on a copy of the bilateral agreement.
/// @param csa - Authored CSA; collateral, variation margin and timing terms are preserved.
/// @returns OTC margin specification with the selected methodology if IM is elected.
/// @throws FinstackError if the CSA document cannot be decoded.
#[wasm_bindgen(js_name = otcMarginSpecBilateralSchedule, unchecked_return_type = "generated.margin.OtcMarginSpec")]
pub fn otc_margin_spec_bilateral_schedule(
    #[wasm_bindgen(unchecked_param_type = "generated.margin.CsaSpec")] csa: JsValue,
) -> Result<JsValue, JsValue> {
    let csa = from_js_json(&csa, "csa")?;
    to_js_value(&fm::types::OtcMarginSpec::bilateral_schedule(csa))
}

/// Construct margin terms from the embedded cleared-derivative defaults.
/// @param ccp - Identified clearinghouse, such as LCH.
/// @param currency - ISO-4217 settlement and collateral currency.
/// @returns Native cleared OTC margin specification.
/// @throws FinstackError if the currency or native margin registry is invalid.
#[wasm_bindgen(js_name = otcMarginSpecCleared, unchecked_return_type = "generated.margin.OtcMarginSpec")]
pub fn otc_margin_spec_cleared(
    #[wasm_bindgen(unchecked_param_type = "string")] ccp: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
    let ccp = js_string(&ccp, "ccp")?;
    let currency = js_currency(&currency, "currency")?;
    to_js_value(&fm::types::OtcMarginSpec::cleared(ccp, currency).map_err(to_js_err)?)
}
