//! Initial margin: SIMM sensitivities and calculator, and the IM result twins.
//!
//! `SimmSensitivities` (a mutable builder) and `SimmCalculator` (an engine)
//! are WASM classes whose members mirror the Python classes. `ImResult` and
//! `SimmCurvatureSensitivity` are plain JSON values typed by the generated
//! TypeScript.

use super::parse::parse_label;
use crate::api::core::config::JsFinstackConfig;
use crate::utils::input::js_currency;
use crate::utils::input::{from_js_json, js_f64, js_opt_string, js_opt_uint, js_string};
use crate::utils::wire::js_date;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_margin as fm;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// ImResult
// ---------------------------------------------------------------------------

/// Component labels present in an initial margin result's breakdown.
///
/// SIMM publishes `IR_Delta`, `IR_Vega`, `FX_Delta`, `Curvature`, …; the
/// schedule calculator publishes the asset class (for example
/// `interest_rate`).
/// @param result - `ImResult` (object or JSON).
/// @returns The breakdown labels in sorted order.
///
/// # Errors
///
/// Throws if `result` is malformed.
#[wasm_bindgen(js_name = imResultBreakdownKeys)]
pub fn im_result_breakdown_keys(
    #[wasm_bindgen(unchecked_param_type = "ImResult | string")] result: JsValue,
) -> Result<Vec<String>, JsValue> {
    let result: fm::calculators::ImResult = from_js_json(&result, "result")?;
    Ok(result.breakdown.keys().cloned().collect())
}

/// Breakdown amount of an initial margin result for one component label.
/// @param result - `ImResult` (object or JSON).
/// @param key - Component label, for example `"IR_Delta"`.
/// @returns The component amount in major units of the result currency, or `undefined` when the label is absent.
///
/// # Errors
///
/// Throws if `result` is malformed or `key` is not a string.
#[wasm_bindgen(js_name = imResultBreakdownAmount)]
pub fn im_result_breakdown_amount(
    #[wasm_bindgen(unchecked_param_type = "ImResult | string")] result: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] key: JsValue,
) -> Result<Option<f64>, JsValue> {
    let result: fm::calculators::ImResult = from_js_json(&result, "result")?;
    let key = js_string(&key, "key")?;
    Ok(result.breakdown.get(&key).map(|money| money.amount()))
}

// ---------------------------------------------------------------------------
// SimmSensitivities
// ---------------------------------------------------------------------------

/// ISDA SIMM sensitivity portfolio.
///
/// Stores signed sensitivity amounts by SIMM risk class and bucket. Amounts
/// are currency amounts in the base currency, not percentages or spot
/// levels: rate and credit deltas are DV01/CS01-style amounts per 1bp move,
/// vegas are sigma times dPV/dsigma before VRW, HVR and concentration.
/// Tenor labels must be SIMM buckets (`margin.constants().SIMM_TENORS`);
/// `validate()` rejects anything else so a typo cannot price to zero margin.
///
/// @example
/// ```javascript
/// import init, { margin } from "finstack-quant-wasm";
/// await init();
/// const sens = new margin.SimmSensitivities("USD");
/// sens.addIrDelta("USD", "5Y", 25_000);
/// sens.totalIrDelta(); // 25000
/// const im = new margin.SimmCalculator().calculateFromSensitivities(sens, "USD", "2025-01-15");
/// im.amount.amount; // decimal string
/// ```
#[wasm_bindgen(js_name = SimmSensitivities)]
#[derive(Clone)]
pub struct JsSimmSensitivities {
    pub(crate) inner: fm::types::SimmSensitivities,
}

json_round_trip!(JsSimmSensitivities, SimmSensitivities, rust_json fm::types::SimmSensitivities);

macro_rules! simm_adders {
    (
        ccy_tenor: [$( $(#[$ccy_doc:meta])* ($ccy_method:ident, $ccy_js:ident) ),* $(,)?],
        cq: [$( $(#[$cq_doc:meta])* ($cq_method:ident, $cq_js:ident) ),* $(,)?],
        str_tenor: [$( $(#[$st_doc:meta])* ($st_method:ident, $st_js:ident) ),* $(,)?],
        str1: [$( $(#[$s1_doc:meta])* ($s1_method:ident, $s1_js:ident, $s1_param:ident) ),* $(,)?] $(,)?
    ) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = SimmSensitivities)]
        impl JsSimmSensitivities {
            $(
                $(#[$ccy_doc])*
                #[wasm_bindgen(js_name = $ccy_js)]
                pub fn $ccy_method(
                    &mut self,
                    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "string")] tenor: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
                ) -> Result<(), JsValue> {
                    let currency = js_currency(&currency, "currency")?;
                    let tenor = js_string(&tenor, "tenor")?;
                    self.inner.$ccy_method(currency, tenor, js_f64(&amount, "amount")?);
                    Ok(())
                }
            )*
            $(
                $(#[$cq_doc])*
                #[wasm_bindgen(js_name = $cq_js)]
                pub fn $cq_method(
                    &mut self,
                    #[wasm_bindgen(unchecked_param_type = "string")] sector: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "string")] tenor: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
                ) -> Result<(), JsValue> {
                    let sector = parse_label(&sector, "sector")?;
                    let name = js_string(&name, "name")?;
                    let tenor = js_string(&tenor, "tenor")?;
                    self.inner.$cq_method(sector, name, tenor, js_f64(&amount, "amount")?);
                    Ok(())
                }
            )*
            $(
                $(#[$st_doc])*
                #[wasm_bindgen(js_name = $st_js)]
                pub fn $st_method(
                    &mut self,
                    #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "string")] tenor: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
                ) -> Result<(), JsValue> {
                    let name = js_string(&name, "name")?;
                    let tenor = js_string(&tenor, "tenor")?;
                    self.inner.$st_method(name, tenor, js_f64(&amount, "amount")?);
                    Ok(())
                }
            )*
            $(
                $(#[$s1_doc])*
                #[wasm_bindgen(js_name = $s1_js)]
                pub fn $s1_method(
                    &mut self,
                    #[wasm_bindgen(unchecked_param_type = "string")] $s1_param: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
                ) -> Result<(), JsValue> {
                    let $s1_param = js_string(&$s1_param, stringify!($s1_param))?;
                    self.inner.$s1_method($s1_param, js_f64(&amount, "amount")?);
                    Ok(())
                }
            )*
        }
    };
}

simm_adders!(
    ccy_tenor: [
        /// Add an interest-rate delta bucket.
        /// @param currency - ISO-4217 currency risk factor, such as `"USD"`.
        /// @param tenor - SIMM tenor bucket, such as `"2W"`, `"1Y"`, `"5Y"` or `"30Y"`.
        /// @param amount - Signed DV01-style amount per 1bp move, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws if `currency` is not a known currency code.
        (add_ir_delta, addIrDelta),
        /// Add an interest-rate vega bucket.
        /// @param currency - ISO-4217 currency risk factor, such as `"USD"`.
        /// @param tenor - SIMM tenor bucket of the option expiry.
        /// @param amount - Signed sigma times dPV/dsigma, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws if `currency` is not a known currency code.
        (add_ir_vega, addIrVega),
    ],
    cq: [
        /// Add a qualifying-credit delta for one issuer and tenor.
        /// @param sector - SIMM credit-qualifying sector label, such as `"sovereign"` or `"financial"`.
        /// @param name - Issuer or index name.
        /// @param tenor - SIMM tenor bucket, such as `"5Y"`.
        /// @param amount - Signed CS01-style amount per 1bp spread move, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws if `sector` is not a known SIMM credit sector.
        (add_credit_qualifying_delta, addCreditQualifyingDelta),
        /// Add a qualifying-credit vega for one issuer and tenor.
        /// @param sector - SIMM credit-qualifying sector label, such as `"sovereign"` or `"financial"`.
        /// @param name - Issuer or index name.
        /// @param tenor - SIMM tenor bucket of the option expiry.
        /// @param amount - Signed sigma times dPV/dsigma, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws if `sector` is not a known SIMM credit sector.
        (add_credit_qualifying_vega, addCreditQualifyingVega),
    ],
    str_tenor: [
        /// Add a non-qualifying-credit delta for one name and tenor.
        /// @param name - Securitization or issuer name.
        /// @param tenor - SIMM tenor bucket, such as `"5Y"`.
        /// @param amount - Signed CS01-style amount per 1bp spread move, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type.
        (add_credit_non_qualifying_delta, addCreditNonQualifyingDelta),
        /// Add a non-qualifying-credit vega for one name and tenor.
        /// @param name - Securitization or issuer name.
        /// @param tenor - SIMM tenor bucket of the option expiry.
        /// @param amount - Signed sigma times dPV/dsigma, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type.
        (add_credit_non_qualifying_vega, addCreditNonQualifyingVega),
    ],
    str1: [
        /// Add an equity delta for one underlier.
        /// @param underlier - Equity or index identifier.
        /// @param amount - Signed value change for a 1% relative move, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type.
        (add_equity_delta, addEquityDelta, underlier),
        /// Add an equity vega for one underlier.
        /// @param underlier - Equity or index identifier.
        /// @param amount - Signed sigma times dPV/dsigma, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type.
        (add_equity_vega, addEquityVega, underlier),
        /// Add a commodity delta for one SIMM commodity bucket.
        /// @param bucket - SIMM commodity bucket label (one of the 17 ISDA buckets).
        /// @param amount - Signed value change for a 1% relative price move, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type.
        (add_commodity_delta, addCommodityDelta, bucket),
        /// Add a commodity vega for one SIMM commodity bucket.
        /// @param bucket - SIMM commodity bucket label (one of the 17 ISDA buckets).
        /// @param amount - Signed sigma times dPV/dsigma before HVR, VRW and concentration, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type.
        (add_commodity_vega, addCommodityVega, bucket),
    ],
);

#[wasm_bindgen(js_class = SimmSensitivities)]
impl JsSimmSensitivities {
    /// Create an empty SIMM sensitivity set.
    /// @param baseCurrency - ISO-4217 currency in which every sensitivity amount is expressed.
    ///
    /// # Errors
    ///
    /// Throws if `base_currency` is not a known currency code.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "baseCurrency", unchecked_param_type = "string")]
        base_currency: JsValue,
    ) -> Result<JsSimmSensitivities, JsValue> {
        Ok(Self {
            inner: fm::types::SimmSensitivities::new(js_currency(&base_currency, "baseCurrency")?),
        })
    }

    /// Add an FX delta for one currency against the base currency.
    /// @param currency - ISO-4217 currency risk factor.
    /// @param amount - Signed value change for a 1% relative move of the FX rate, in the base currency.
    ///
    /// # Errors
    ///
    /// Throws if `currency` is not a known currency code.
    #[wasm_bindgen(js_name = addFxDelta)]
    pub fn add_fx_delta(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
    ) -> Result<(), JsValue> {
        let currency = js_currency(&currency, "currency")?;
        self.inner
            .add_fx_delta(currency, js_f64(&amount, "amount")?);
        Ok(())
    }

    /// Add an FX vega for one currency pair.
    /// @param ccy1 - First ISO-4217 currency of the pair.
    /// @param ccy2 - Second ISO-4217 currency of the pair.
    /// @param amount - Signed sigma times dPV/dsigma, in the base currency.
    ///
    /// # Errors
    ///
    /// Throws if either currency code is unknown.
    #[wasm_bindgen(js_name = addFxVega)]
    pub fn add_fx_vega(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] ccy1: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] ccy2: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
    ) -> Result<(), JsValue> {
        let ccy1 = js_currency(&ccy1, "ccy1")?;
        let ccy2 = js_currency(&ccy2, "ccy2")?;
        self.inner
            .add_fx_vega(ccy1, ccy2, js_f64(&amount, "amount")?);
        Ok(())
    }

    /// Add an expiry-resolved curvature input, kept unscaled until netting.
    /// @param sensitivity - `SimmCurvatureSensitivity` (object or JSON): `risk_class`, `bucket`, `factor`, `expiry_tenor`, `volatility_weighted_vega` and optional `risk_tenor`.
    ///
    /// # Errors
    ///
    /// Throws if `sensitivity` is malformed or fails its validation (unknown
    /// risk class or tenor, non-finite vega).
    #[wasm_bindgen(js_name = addCurvature)]
    pub fn add_curvature(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "SimmCurvatureSensitivity | string")]
        sensitivity: JsValue,
    ) -> Result<(), JsValue> {
        let sensitivity: fm::types::SimmCurvatureSensitivity =
            from_js_json(&sensitivity, "sensitivity")?;
        sensitivity.validate().map_err(to_js_err)?;
        self.inner.add_curvature(sensitivity);
        Ok(())
    }

    /// Add every bucket of another set into this one, so offsetting risk nets.
    /// @param other - Sensitivities in the same base currency; use `scaledToCurrency` first otherwise.
    ///
    /// # Errors
    ///
    /// Throws a validation error, leaving this set unchanged, on a base
    /// currency mismatch.
    pub fn merge(&mut self, other: &JsSimmSensitivities) -> Result<(), JsValue> {
        self.inner.merge(&other.inner).map_err(to_js_err)
    }

    /// Copy with every amount multiplied by a signed factor.
    /// @param factor - Multiplier, for example the position quantity for unit-notional sensitivities; a negative value flips the position.
    /// @returns A new `SimmSensitivities` handle in the same base currency.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` when `factor` is not a number.
    pub fn scaled(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] factor: JsValue,
    ) -> Result<JsSimmSensitivities, JsValue> {
        Ok(Self {
            inner: self.inner.scaled(js_f64(&factor, "factor")?),
        })
    }

    /// Copy re-expressed in another currency at a spot FX rate.
    /// @param targetCurrency - ISO-4217 currency the amounts should be expressed in.
    /// @param fxRate - Value of one unit of the current base currency in `target_currency`; every amount is multiplied by it.
    /// @returns A new `SimmSensitivities` handle whose base currency is `target_currency`.
    ///
    /// # Errors
    ///
    /// Throws if `target_currency` is not a known currency code.
    #[wasm_bindgen(js_name = scaledToCurrency)]
    pub fn scaled_to_currency(
        &self,
        #[wasm_bindgen(js_name = "targetCurrency", unchecked_param_type = "string")]
        target_currency: JsValue,
        #[wasm_bindgen(js_name = "fxRate", unchecked_param_type = "number")] fx_rate: JsValue,
    ) -> Result<JsSimmSensitivities, JsValue> {
        let target_currency = js_currency(&target_currency, "targetCurrency")?;
        Ok(Self {
            inner: self
                .inner
                .scaled_to_currency(target_currency, js_f64(&fx_rate, "fxRate")?),
        })
    }

    /// Sum of all interest-rate deltas across currencies and tenors.
    /// @returns The net DV01-style amount in the base currency.
    #[wasm_bindgen(js_name = totalIrDelta)]
    pub fn total_ir_delta(&self) -> f64 {
        self.inner.total_ir_delta()
    }

    /// Sum of all equity deltas across underliers.
    /// @returns The net equity delta amount in the base currency.
    #[wasm_bindgen(js_name = totalEquityDelta)]
    pub fn total_equity_delta(&self) -> f64 {
        self.inner.total_equity_delta()
    }

    /// Check every tenor and bucket label and every amount.
    ///
    /// # Errors
    ///
    /// Throws a validation error for an unknown SIMM tenor or commodity
    /// bucket, a non-finite amount, or an invalid curvature input.
    pub fn validate(&self) -> Result<(), JsValue> {
        self.inner.validate().map_err(to_js_err)
    }

    /// Whether no sensitivity of any risk class has been added.
    /// @returns `true` for an empty set.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// ISO-4217 currency in which every sensitivity amount is expressed.
    #[wasm_bindgen(getter, js_name = baseCurrency)]
    pub fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }
}

// ---------------------------------------------------------------------------
// SimmCalculator
// ---------------------------------------------------------------------------

/// Indicative historical SIMM v2.6 calculator, with USD concentration thresholds.
///
/// Loads registry-backed SIMM parameters for the requested rule version and
/// calculates an approximation from explicit `SimmSensitivities`.
/// Product-class, subcurve and some non-IR dimensions are incomplete, so
/// every result has `approximation: true`; this is not a current regulatory
/// SIMM implementation.
///
/// @example
/// ```javascript
/// import init, { margin } from "finstack-quant-wasm";
/// await init();
/// const calculator = new margin.SimmCalculator("v2_6");
/// calculator.version;  // "v2_6"
/// calculator.mporDays; // 10
/// ```
#[wasm_bindgen(js_name = SimmCalculator)]
pub struct JsSimmCalculator {
    pub(crate) inner: fm::calculators::im::SimmCalculator,
}

#[wasm_bindgen(js_class = SimmCalculator)]
impl JsSimmCalculator {
    /// Create a SIMM calculator from the embedded margin registry.
    /// @param version - Canonical version label `"v2_6"`; omitted uses the default version.
    /// @param mporDays - Margin period of risk override in business days, stamped on results; omitted uses the registry default for the version (10).
    ///
    /// # Errors
    ///
    /// Throws if the version is unknown, `mpor_days` is not a non-negative
    /// integer, or the registry parameters cannot be loaded.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_optional_param_type = "string | null")] version: Option<JsValue>,
        #[wasm_bindgen(js_name = "mporDays", unchecked_optional_param_type = "number | null")]
        mpor_days: Option<JsValue>,
    ) -> Result<JsSimmCalculator, JsValue> {
        let version = js_opt_string(version.as_ref(), "version")?
            .map(|label| label.parse::<fm::SimmVersion>())
            .transpose()
            .map_err(to_js_err)?
            .unwrap_or_default();
        let mpor_days: Option<u32> = js_opt_uint(mpor_days.as_ref(), "mporDays")?;
        let mut inner = fm::calculators::im::SimmCalculator::new(version).map_err(to_js_err)?;
        if let Some(days) = mpor_days {
            inner = inner.with_mpor(days);
        }
        Ok(Self { inner })
    }

    /// Create a SIMM calculator from a config's margin-registry overlay.
    /// @param version - Canonical SIMM version label such as `"v2_6"`.
    /// @param config - `FinstackConfig` handle whose `"margin.registry.v1"` extension (if any) overlays the embedded margin registry; without one the result equals `new SimmCalculator(version)`.
    /// @returns A `SimmCalculator` handle using the merged registry's SIMM parameters and margin period of risk.
    ///
    /// # Errors
    ///
    /// Throws if the version is unknown, or the overlay is malformed or yields
    /// incomplete SIMM parameters.
    #[wasm_bindgen(js_name = fromFinstackConfig)]
    pub fn from_finstack_config(
        #[wasm_bindgen(unchecked_param_type = "string")] version: JsValue,
        config: &JsFinstackConfig,
    ) -> Result<JsSimmCalculator, JsValue> {
        let version = parse_label(&version, "version")?;
        let inner =
            fm::calculators::im::SimmCalculator::from_finstack_config(version, &config.inner)
                .map_err(to_js_err)?;
        Ok(Self { inner })
    }

    /// Supported SIMM version label, `"v2_6"`.
    #[wasm_bindgen(getter)]
    pub fn version(&self) -> String {
        self.inner.version().as_str().to_string()
    }

    /// Margin period of risk in business days stamped on every result.
    #[wasm_bindgen(getter, js_name = mporDays)]
    pub fn mpor_days(&self) -> u32 {
        self.inner.mpor_days()
    }

    /// Calculate SIMM initial margin from explicit sensitivities.
    /// @param sensitivities - Sensitivity set to aggregate; validated first, so an unknown tenor or commodity bucket throws instead of pricing to zero.
    /// @param currency - Reporting currency; must be `"USD"` and match the sensitivities' base currency (concentration thresholds are in USD).
    /// @param asOf - ISO-8601 calculation date stamped on the result.
    /// @returns The `ImResult` as a plain object: `amount` (Money), `methodology`, `mpor_days`, `as_of`, `approximation`, the SIMM component `breakdown`, and `simm_detail` (per-component buckets with weighted sensitivities, concentration factors and bucket `k`, plus `risk_class_margins` and `mpor_scale`).
    ///
    /// # Errors
    ///
    /// Throws if the sensitivities fail validation, the input or output
    /// currency is not USD, or the date is not ISO 8601.
    #[wasm_bindgen(js_name = calculateFromSensitivities, unchecked_return_type = "ImResult")]
    pub fn calculate_from_sensitivities(
        &self,
        sensitivities: &JsSimmSensitivities,
        #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
        #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let currency = js_currency(&currency, "currency")?;
        let as_of = js_date(&as_of, "asOf")?;
        let result = self
            .inner
            .calculate_from_sensitivities(&sensitivities.inner, currency, as_of)
            .map_err(to_js_err)?;
        to_js_value(&result)
    }
}
