//! SA-CCR Exposure at Default: netting-set configuration, engine and charge.
//!
//! `SaCcrEngine` is a WASM class mirroring the Python class.
//! `SaCcrNettingSetConfig`, `SaCcrTrade` and `EadResult` are plain JSON values
//! typed by the generated TypeScript.

use super::js_date;
use crate::utils::input::{from_js_json, js_f64, js_opt_f64, js_uint};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_margin::regulatory::sa_ccr::{
    saccr_ead as saccr_ead_rs, SaCcrEngine, SaCcrNettingSetConfig, SaCcrTrade,
};
use finstack_quant_margin::NettingSetId;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// SaCcrNettingSetConfig
// ---------------------------------------------------------------------------

/// Unmargined SA-CCR netting-set configuration.
///
/// Threshold, MTA and NICA are zero and `mpor_days` is the 10-business-day
/// bilateral default (used only for the reporting maturity factor).
/// @param netting_set_id - `NettingSetId` (object or JSON), bilateral or cleared.
/// @param collateral - Net collateral held, in the reporting currency; positive means the bank holds collateral.
/// @param as_of - ISO-8601 valuation date for forward-start and remaining-maturity calculations.
/// @returns The `SaCcrNettingSetConfig` as a plain object.
///
/// # Errors
///
/// Throws if `netting_set_id` is malformed, `collateral` is non-finite, or
/// the date is not ISO 8601.
#[wasm_bindgen(js_name = saCcrNettingSetConfigUnmargined)]
pub fn sa_ccr_netting_set_config_unmargined(
    netting_set_id: JsValue,
    collateral: JsValue,
    as_of: JsValue,
) -> Result<JsValue, JsValue> {
    let netting_set_id: NettingSetId = from_js_json(&netting_set_id, "nettingSetId")?;
    let config = SaCcrNettingSetConfig::unmargined(
        netting_set_id,
        js_f64(&collateral, "collateral")?,
        js_date(&as_of, "asOf")?,
    );
    config.validate().map_err(to_js_err)?;
    to_js_value(&config)
}

/// Margined SA-CCR netting-set configuration.
/// @param netting_set_id - `NettingSetId` (object or JSON), bilateral or cleared.
/// @param collateral - Net collateral held, in the reporting currency; positive means the bank holds collateral.
/// @param threshold - CSA threshold (TH), non-negative.
/// @param mta - Minimum transfer amount, non-negative.
/// @param nica - Net independent collateral amount, signed.
/// @param mpor_days - Margin period of risk in business days; at least 10 bilateral or 5 cleared.
/// @param as_of - ISO-8601 valuation date for forward-start and remaining-maturity calculations.
/// @returns The `SaCcrNettingSetConfig` as a plain object.
///
/// # Errors
///
/// Throws if `netting_set_id` is malformed, an amount is non-finite, the
/// threshold or MTA is negative, `mpor_days` is below the applicable floor,
/// or the date is not ISO 8601.
#[wasm_bindgen(js_name = saCcrNettingSetConfigMargined)]
pub fn sa_ccr_netting_set_config_margined(
    netting_set_id: JsValue,
    collateral: JsValue,
    threshold: JsValue,
    mta: JsValue,
    nica: JsValue,
    mpor_days: JsValue,
    as_of: JsValue,
) -> Result<JsValue, JsValue> {
    let netting_set_id: NettingSetId = from_js_json(&netting_set_id, "nettingSetId")?;
    let config = SaCcrNettingSetConfig::margined(
        netting_set_id,
        js_f64(&collateral, "collateral")?,
        js_f64(&threshold, "threshold")?,
        js_f64(&mta, "mta")?,
        js_f64(&nica, "nica")?,
        js_uint(&mpor_days, "mporDays")?,
        js_date(&as_of, "asOf")?,
    );
    config.validate().map_err(to_js_err)?;
    to_js_value(&config)
}

/// Validate an SA-CCR netting-set configuration.
/// @param config - `SaCcrNettingSetConfig` (object or JSON).
///
/// # Errors
///
/// Throws if `config` is malformed, an amount is non-finite, the threshold
/// or MTA is negative, or a margined `mpor_days` is below the applicable
/// floor.
#[wasm_bindgen(js_name = saCcrNettingSetConfigValidate)]
pub fn sa_ccr_netting_set_config_validate(config: JsValue) -> Result<(), JsValue> {
    let config: SaCcrNettingSetConfig = from_js_json(&config, "config")?;
    config.validate().map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// SaCcrEngine
// ---------------------------------------------------------------------------

/// SA-CCR Exposure at Default engine (BCBS 279): `EAD = alpha * (RC + PFE)`.
///
/// Monetary values in one calculation must already use one consistent
/// currency; the engine performs no currency conversion.
///
/// @example
/// ```javascript
/// import init, { margin } from "finstack-quant-wasm";
/// await init();
/// const config = margin.saCcrNettingSetConfigUnmargined(
///   margin.nettingSetIdBilateral("CPTY", "CSA"), 0, "2025-01-15");
/// new margin.SaCcrEngine().calculateEad(config, [{
///   trade_id: "t1", asset_class: "interest_rate", notional: 1_000_000,
///   start_date: "2025-01-15", end_date: "2030-01-15", underlier: "USD",
///   hedging_set: "USD", direction: 1, supervisory_delta: 1, mtm: 0, is_option: false,
/// }]).ead;
/// ```
#[wasm_bindgen(js_name = SaCcrEngine)]
pub struct JsSaCcrEngine {
    inner: SaCcrEngine,
}

#[wasm_bindgen(js_class = SaCcrEngine)]
impl JsSaCcrEngine {
    /// Configure the supervisory multiplier for SA-CCR.
    /// @param alpha - Supervisory alpha multiplier, finite and at least `1.0`; omitted uses the regulatory `1.4`.
    ///
    /// # Errors
    ///
    /// Throws if a supplied `alpha` is non-finite or below `1.0`.
    #[wasm_bindgen(constructor)]
    pub fn new(alpha: Option<JsValue>) -> Result<JsSaCcrEngine, JsValue> {
        let inner = match js_opt_f64(alpha.as_ref(), "alpha")? {
            Some(alpha) => SaCcrEngine::with_alpha(alpha).map_err(to_js_err)?,
            None => SaCcrEngine::default(),
        };
        Ok(Self { inner })
    }

    /// Alpha multiplier applied to `RC + PFE` (1.4 unless overridden).
    #[wasm_bindgen(getter)]
    pub fn alpha(&self) -> f64 {
        self.inner.alpha()
    }

    /// Calculate SA-CCR EAD for a netting set and its trades.
    /// @param config - `SaCcrNettingSetConfig` (object or JSON) with the valuation date and collateral terms.
    /// @param trades - Array (or JSON) of `SaCcrTrade` objects in the netting set; empty gives zero EAD.
    /// @returns The `EadResult` as a plain object: `ead`, `rc`, `pfe`, `multiplier`, the aggregate and per-asset-class add-ons, `alpha` and `maturity_factor`.
    ///
    /// # Errors
    ///
    /// Throws if `config` or a trade is malformed or fails validation: a
    /// non-finite amount, a negative threshold or MTA, or inconsistent
    /// direction, supervisory delta and option type.
    #[wasm_bindgen(js_name = calculateEad)]
    pub fn calculate_ead(&self, config: JsValue, trades: JsValue) -> Result<JsValue, JsValue> {
        let config: SaCcrNettingSetConfig = from_js_json(&config, "config")?;
        let trades: Vec<SaCcrTrade> = from_js_json(&trades, "trades")?;
        to_js_value(
            &self
                .inner
                .calculate_ead(&config, &trades)
                .map_err(to_js_err)?,
        )
    }
}

/// SA-CCR Exposure at Default for one netting set (BCBS 279).
/// @param trades - Array (or JSON) of `SaCcrTrade` objects in the netting set; empty gives zero EAD.
/// @param config - `SaCcrNettingSetConfig` (object or JSON): collateral, threshold, MTA, NICA, MPOR and valuation date.
/// @param alpha - Supervisory alpha override, finite and at least `1.0`; omitted uses the regulatory `1.4`.
/// @returns The `EadResult` as a plain object, after the unmargined cap for margined sets.
///
/// # Errors
///
/// Throws if `alpha` is invalid, or `config` or a trade is malformed or
/// fails validation.
#[wasm_bindgen(js_name = saccrEad)]
pub fn saccr_ead(
    trades: JsValue,
    config: JsValue,
    alpha: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let trades: Vec<SaCcrTrade> = from_js_json(&trades, "trades")?;
    let config: SaCcrNettingSetConfig = from_js_json(&config, "config")?;
    let alpha = js_opt_f64(alpha.as_ref(), "alpha")?;
    to_js_value(&saccr_ead_rs(&trades, &config, alpha).map_err(to_js_err)?)
}
